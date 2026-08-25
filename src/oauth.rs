//! ClickUp OAuth authorization-code flow: build the authorize URL, catch the browser
//! redirect on a localhost listener, and exchange the code for an access token.
//!
//! ClickUp's flow has no PKCE and no refresh tokens; access tokens don't expire. The
//! authorize page itself lives on app.clickup.com (browser-only), while the token
//! exchange goes through the API base so tests can point it at a mock server.

use std::net::TcpListener;
use std::time::Duration;

use percent_encoding::percent_decode_str;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::client::api_error;
use crate::commands::encode_path_segment;
use crate::error::CliError;

const AUTHORIZE_BASE: &str = "https://app.clickup.com/api";

pub fn authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    // encode_path_segment's set (everything but unreserved chars) is also valid for
    // query values, and leaves the state's hyphens readable.
    let encoded_redirect = encode_path_segment(redirect_uri);
    let encoded_state = encode_path_segment(state);
    format!(
        "{AUTHORIZE_BASE}?client_id={client_id}&redirect_uri={encoded_redirect}&state={encoded_state}"
    )
}

/// Binds the loopback listener up front so a busy port fails before the browser opens.
/// Port 0 asks the OS for any free port (used by tests).
pub fn bind_loopback(port: u16) -> Result<TcpListener, CliError> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|bind_error| {
        CliError::Usage(format!(
            "cannot listen on 127.0.0.1:{port} ({bind_error}) — pass a different --port or use --paste"
        ))
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|nonblocking_error| {
            CliError::Network(format!(
                "configuring the OAuth listener failed: {nonblocking_error}"
            ))
        })?;
    Ok(listener)
}

/// Waits for ClickUp's browser redirect and returns the authorization code. A request
/// whose `state` doesn't match is answered 400 and treated as a possible interception —
/// the whole login is aborted rather than kept open for a second attempt.
pub async fn catch_redirect(
    listener: TcpListener,
    expected_state: &str,
    deadline: Duration,
) -> Result<String, CliError> {
    let listener = tokio::net::TcpListener::from_std(listener).map_err(|convert_error| {
        CliError::Network(format!("OAuth listener setup failed: {convert_error}"))
    })?;

    let wait_for_code = async {
        loop {
            let (mut connection, _peer) = listener.accept().await.map_err(|accept_error| {
                CliError::Network(format!("OAuth listener accept failed: {accept_error}"))
            })?;

            let mut request_bytes = vec![0_u8; 4096];
            let bytes_read = connection.read(&mut request_bytes).await.unwrap_or(0);
            let request_text = String::from_utf8_lossy(&request_bytes[..bytes_read]);

            let Some(query) = request_query(&request_text) else {
                // Not the redirect (e.g. a favicon probe) — answer and keep waiting.
                respond(&mut connection, 404, "Not the OAuth redirect.").await;
                continue;
            };
            let code = query_param(query, "code");
            let state = query_param(query, "state");
            if code.is_none() {
                respond(&mut connection, 404, "Not the OAuth redirect.").await;
                continue;
            }
            if state.as_deref() != Some(expected_state) {
                respond(
                    &mut connection,
                    400,
                    "State mismatch — this login attempt was aborted.",
                )
                .await;
                return Err(CliError::Auth(
                    "OAuth state mismatch — possible interception; re-run the login".to_string(),
                ));
            }
            respond(
                &mut connection,
                200,
                "clickup-cli is authorized. You can close this tab.",
            )
            .await;
            return Ok(code.expect("checked above"));
        }
    };

    match tokio::time::timeout(deadline, wait_for_code).await {
        Ok(result) => result,
        Err(_elapsed) => Err(CliError::Network(format!(
            "timed out after {}s waiting for the OAuth redirect — re-run, or use --paste",
            deadline.as_secs()
        ))),
    }
}

pub async fn exchange_code(
    base: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<String, CliError> {
    let http = reqwest::Client::new();
    let response = http
        .post(format!("{base}/api/v2/oauth/token"))
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
        }))
        .send()
        .await
        .map_err(|send_error| {
            CliError::Network(format!("OAuth token exchange failed: {send_error}"))
        })?;

    let status = response.status();
    let raw = response.text().await.map_err(|read_error| {
        CliError::Network(format!(
            "reading the token exchange response failed: {read_error}"
        ))
    })?;
    if !status.is_success() {
        return Err(api_error(status, &raw));
    }
    let body: Value = serde_json::from_str(&raw).map_err(|parse_error| CliError::Api {
        status: status.as_u16(),
        message: format!("token exchange returned a non-JSON body ({parse_error})"),
        ecode: None,
    })?;
    body["access_token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| CliError::Api {
            status: status.as_u16(),
            message: "token exchange response carried no access_token".to_string(),
            ecode: None,
        })
}

fn request_query(request_text: &str) -> Option<&str> {
    let request_line = request_text.lines().next()?;
    let path_and_query = request_line.split_whitespace().nth(1)?;
    path_and_query.split_once('?').map(|(_path, query)| query)
}

fn query_param(query: &str, wanted_key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == wanted_key {
            Some(percent_decode_str(value).decode_utf8_lossy().into_owned())
        } else {
            None
        }
    })
}

async fn respond(connection: &mut tokio::net::TcpStream, status: u16, message: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let page = format!(
        "<!doctype html><html><body style=\"font-family: sans-serif; margin: 3rem\"><p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    // Best-effort: the browser page is a courtesy; the flow's result doesn't depend on it.
    let _ = connection.write_all(response.as_bytes()).await;
    let _ = connection.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_params_are_percent_decoded() {
        assert_eq!(
            query_param("code=abc%2Fdef&state=x", "code").as_deref(),
            Some("abc/def")
        );
        assert_eq!(query_param("code=abc", "state"), None);
    }

    #[test]
    fn request_query_extracts_the_query_string() {
        assert_eq!(
            request_query("GET /?code=a&state=b HTTP/1.1\r\nHost: x\r\n"),
            Some("code=a&state=b")
        );
        assert_eq!(request_query("GET /favicon.ico HTTP/1.1\r\n"), None);
    }
}
