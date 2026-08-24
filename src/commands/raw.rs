use serde_json::{Value, json};

use crate::client::CuClient;
use crate::error::CliError;

pub async fn call(
    client: &CuClient,
    method: reqwest::Method,
    path: &str,
    query: &[(String, String)],
    headers: &[(String, String)],
    body: Option<Value>,
) -> Result<Value, CliError> {
    let response = client
        .request_with_headers(method, path, query, headers, body)
        .await?;

    // Non-JSON (or empty) success bodies come back as `body: None`; surface the status
    // rather than erroring.
    match response.body {
        Some(json_body) => Ok(json_body),
        None => Ok(json!({"status": response.status})),
    }
}
