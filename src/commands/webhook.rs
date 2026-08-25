use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

pub async fn list(client: &CuClient) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/webhook");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `webhook create` sends: `--endpoint` always overwrites `data["endpoint"]`;
/// `--event` (repeatable; the literal `"*"` subscribes to everything) merges into
/// `data["events"]` as an array, only when at least one is given.
pub fn build_create_body(mut data: Value, endpoint: &str, events: &[String]) -> Value {
    data["endpoint"] = json!(endpoint);
    if !events.is_empty() {
        data["events"] = json!(events);
    }
    data
}

/// Response includes `webhook.secret` (needed by the caller for `X-Signature` verification) —
/// `pass_through` returns the body unmodified, so the secret reaches the caller untouched.
pub async fn create(
    client: &CuClient,
    endpoint: &str,
    events: &[String],
    data: Value,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/webhook");
    let body = build_create_body(data, endpoint, events);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, webhook_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/webhook/{webhook_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, webhook_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/webhook/{webhook_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "webhook_id": webhook_id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_create_body_merges_endpoint_and_events() {
        let body = build_create_body(
            json!({}),
            "https://x.example/hook",
            &["taskUpdated".to_string(), "taskDeleted".to_string()],
        );
        assert_eq!(
            body,
            json!({
                "endpoint": "https://x.example/hook",
                "events": ["taskUpdated", "taskDeleted"],
            })
        );
    }

    #[test]
    fn build_create_body_omits_events_when_none_given() {
        let body = build_create_body(json!({}), "https://x.example/hook", &[]);
        assert_eq!(body, json!({"endpoint": "https://x.example/hook"}));
    }

    #[test]
    fn build_create_body_endpoint_flag_overrides_data() {
        let body = build_create_body(
            json!({"endpoint": "https://old.example/hook"}),
            "https://new.example/hook",
            &[],
        );
        assert_eq!(body, json!({"endpoint": "https://new.example/hook"}));
    }
}
