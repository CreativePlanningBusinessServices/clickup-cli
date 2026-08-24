use std::time::Duration;

use serde_json::Value;

use crate::error::CliError;

const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF_MILLIS: u64 = 500;
/// Longest raw response body we'll echo back in an error message.
const MAX_BODY_ECHO_CHARS: usize = 500;

pub struct CuClient {
    http: reqwest::Client,
    base: String,
    token: String,
    workspace_id: Option<String>,
}

#[derive(Debug)]
pub struct CuResponse {
    pub status: u16,
    pub body: Option<Value>,
}

impl CuClient {
    pub fn new(base: String, token: String, workspace_id: Option<String>) -> CuClient {
        CuClient {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
            workspace_id,
        }
    }

    /// The configured workspace id, or a Usage error telling the caller how to set one.
    pub fn workspace_id(&self) -> Result<&str, CliError> {
        self.workspace_id.as_deref().ok_or_else(|| {
            CliError::Usage(
                "no workspace id: pass --workspace or re-run `clickup-cli account add`".into(),
            )
        })
    }

    pub async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(String, String)],
        body: Option<Value>,
    ) -> Result<CuResponse, CliError> {
        self.request_with_headers(method, path, query, &[], body)
            .await
    }

    pub async fn request_with_headers(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(String, String)],
        headers: &[(String, String)],
        body: Option<Value>,
    ) -> Result<CuResponse, CliError> {
        let url = resolve_url(&self.base, path);
        let build_request = || {
            let mut request = self.http.request(method.clone(), &url).query(query);
            for (name, value) in headers {
                request = request.header(name.as_str(), value.as_str());
            }
            // reqwest sets Content-Type: application/json for us when there's a body, and
            // omits it otherwise — no need to set it explicitly either way.
            if let Some(json_body) = &body {
                request = request.json(json_body);
            }
            request
        };
        let response = self.send_with_retry(&url, &build_request).await?;
        parse_json_response(&url, response).await
    }

    pub async fn request_multipart(
        &self,
        path: &str,
        query: &[(String, String)],
        form: reqwest::multipart::Form,
    ) -> Result<CuResponse, CliError> {
        let url = resolve_url(&self.base, path);
        // reqwest::multipart::Form is consumed on send and isn't Clone, so unlike `request`
        // this can't rebuild the request per attempt — it's sent once, with the same auth
        // header the retry loop would have applied.
        let response = self
            .http
            .post(&url)
            .query(query)
            .header("Authorization", self.token.as_str())
            .multipart(form)
            .send()
            .await
            .map_err(|send_error| {
                CliError::Network(format!("request to {url} failed: {send_error}"))
            })?;

        if response.status().as_u16() == 401 {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            return Err(auth_error(status, &raw));
        }
        parse_json_response(&url, response).await
    }

    /// Owns the shared per-attempt control flow: 429/5xx Retry-After/backoff, transport
    /// errors retried the same way. `build_request` produces a fresh, unauthenticated
    /// `RequestBuilder` for each attempt (method/url/query/body already applied); this
    /// helper adds the auth header before sending. 401 short-circuits with no retry —
    /// ClickUp personal tokens don't expire, so a retry cannot turn a rejected token into
    /// an accepted one.
    async fn send_with_retry(
        &self,
        url: &str,
        build_request: &(dyn Fn() -> reqwest::RequestBuilder + Send + Sync),
    ) -> Result<reqwest::Response, CliError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            // ClickUp personal API tokens go straight into the Authorization header with
            // no "Bearer " prefix — unlike the OAuth bearer tokens most other APIs use.
            let request = build_request().header("Authorization", self.token.as_str());

            let response = match request.send().await {
                Ok(response) => response,
                Err(send_error) => {
                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(backoff_delay(attempt)).await;
                        continue;
                    }
                    return Err(CliError::Network(format!(
                        "request to {url} failed: {send_error}"
                    )));
                }
            };
            let status = response.status();

            if status.as_u16() == 401 {
                let raw = response.text().await.unwrap_or_default();
                return Err(auth_error(status, &raw));
            }
            if (status.as_u16() == 429 || status.is_server_error()) && attempt < MAX_ATTEMPTS {
                let delay = retry_after_seconds(&response)
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| backoff_delay(attempt));
                tokio::time::sleep(delay).await;
                continue;
            }

            return Ok(response);
        }
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(BASE_BACKOFF_MILLIS * 2u64.pow(attempt - 1))
}

async fn parse_json_response(
    url: &str,
    response: reqwest::Response,
) -> Result<CuResponse, CliError> {
    let status = response.status();
    let raw = response.text().await.map_err(|read_error| {
        CliError::Network(format!("reading response from {url} failed: {read_error}"))
    })?;

    if status.is_success() {
        let body = if raw.trim().is_empty() {
            None
        } else {
            serde_json::from_str(&raw).ok()
        };
        return Ok(CuResponse {
            status: status.as_u16(),
            body,
        });
    }
    Err(api_error(status, &raw))
}

fn resolve_url(base: &str, path: &str) -> String {
    if path.starts_with("https://") || path.starts_with("http://") {
        path.to_string()
    } else if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

fn retry_after_seconds(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get("retry-after")?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn auth_error(status: reqwest::StatusCode, raw: &str) -> CliError {
    let (message, _) = describe_error(status, raw);
    CliError::Auth(format!(
        "ClickUp rejected the token ({message}) — check `clickup-cli account test` or re-run `account add`"
    ))
}

fn api_error(status: reqwest::StatusCode, raw: &str) -> CliError {
    let (message, ecode) = describe_error(status, raw);
    CliError::Api {
        status: status.as_u16(),
        message,
        ecode,
    }
}

/// ClickUp error bodies are `{"err": ..., "ECODE": ...}` or `{"error": ..., "ECODE": ...}`.
/// Message precedence: `err` key, else `error` key, else the raw body (truncated), else the
/// HTTP status's canonical reason phrase.
fn describe_error(status: reqwest::StatusCode, raw: &str) -> (String, Option<String>) {
    let parsed: Option<Value> = if raw.trim().is_empty() {
        None
    } else {
        serde_json::from_str(raw).ok()
    };
    let Some(body) = parsed else {
        return (fallback_message(status, raw), None);
    };
    let message = body["err"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_message(status, raw));
    let ecode = body["ECODE"].as_str().map(str::to_string);
    (message, ecode)
}

fn fallback_message(status: reqwest::StatusCode, raw: &str) -> String {
    if raw.trim().is_empty() {
        status
            .canonical_reason()
            .unwrap_or("request failed")
            .to_string()
    } else {
        raw.chars().take(MAX_BODY_ECHO_CHARS).collect()
    }
}
