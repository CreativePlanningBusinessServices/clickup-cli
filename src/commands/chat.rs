use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_cursor_pages, merge_flag, pass_through};
use crate::error::CliError;

fn chat_path(client: &CuClient) -> Result<String, CliError> {
    let workspace_id = client.workspace_id()?;
    Ok(format!("/api/v3/workspaces/{workspace_id}/chat"))
}

fn cursor_query(cursor: Option<String>) -> Vec<(String, String)> {
    cursor
        .map(|cursor_value| vec![("cursor".to_string(), cursor_value)])
        .unwrap_or_default()
}

pub async fn channels(
    client: &CuClient,
    query: &[(String, String)],
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/channels", chat_path(client)?);
    merge_cursor_pages(client, &path, query, "data", fetch_all).await
}

/// Builds the body `chat channel create` sends: `--name` merges into `data["name"]`.
pub fn build_channel_create_body(mut data: Value, name: &str) -> Value {
    merge_flag(&mut data, "name", Some(name.to_string()));
    data
}

pub async fn channel_create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let path = format!("{}/channels", chat_path(client)?);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `chat channel create-location` sends: `--location` (already-parsed JSON)
/// merges into `data["location"]`.
pub fn build_channel_create_location_body(mut data: Value, location: Value) -> Value {
    merge_flag(&mut data, "location", Some(location));
    data
}

pub async fn channel_create_location(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let path = format!("{}/channels/location", chat_path(client)?);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn channel_create_dm(client: &CuClient, user_ids: &[String]) -> Result<Value, CliError> {
    let path = format!("{}/channels/direct_message", chat_path(client)?);
    let body = json!({ "user_ids": user_ids });
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn channel_get(client: &CuClient, channel_id: &str) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}", chat_path(client)?);
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn channel_update(
    client: &CuClient,
    channel_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}", chat_path(client)?);
    let response = client
        .request(reqwest::Method::PATCH, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn channel_delete(client: &CuClient, channel_id: &str) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}", chat_path(client)?);
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "channel_id": channel_id}))
}

pub async fn channel_members(
    client: &CuClient,
    channel_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}/members", chat_path(client)?);
    merge_cursor_pages(client, &path, &cursor_query(cursor), "data", fetch_all).await
}

pub async fn channel_followers(
    client: &CuClient,
    channel_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}/followers", chat_path(client)?);
    merge_cursor_pages(client, &path, &cursor_query(cursor), "data", fetch_all).await
}

pub async fn messages(
    client: &CuClient,
    channel_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}/messages", chat_path(client)?);
    merge_cursor_pages(client, &path, &cursor_query(cursor), "data", fetch_all).await
}

/// Builds the body `chat send`/`chat reply` sends: `type` defaults to `"message"` only when
/// neither `--type` nor an existing `--data` `"type"` key is present — an explicit `--type`
/// always wins, and an unset `--type` never clobbers a type already supplied via `--data`.
/// `--content` always overwrites `data["content"]`, same as every other flag-over-data merge.
pub fn build_send_body(mut data: Value, content: &str, message_type: Option<&str>) -> Value {
    if let Some(message_type) = message_type {
        merge_flag(&mut data, "type", Some(message_type.to_string()));
    } else if data.get("type").is_none() {
        data["type"] = json!("message");
    }
    merge_flag(&mut data, "content", Some(content.to_string()));
    data
}

pub async fn send(client: &CuClient, channel_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("{}/channels/{channel_id}/messages", chat_path(client)?);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn message_update(
    client: &CuClient,
    message_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}", chat_path(client)?);
    let response = client
        .request(reqwest::Method::PATCH, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn message_delete(client: &CuClient, message_id: &str) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}", chat_path(client)?);
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "message_id": message_id}))
}

pub async fn replies(
    client: &CuClient,
    message_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}/replies", chat_path(client)?);
    merge_cursor_pages(client, &path, &cursor_query(cursor), "data", fetch_all).await
}

/// Builds the body `chat reply` sends: `--content` merges into `data["content"]`.
pub fn build_reply_body(mut data: Value, content: &str) -> Value {
    merge_flag(&mut data, "content", Some(content.to_string()));
    data
}

pub async fn reply(client: &CuClient, message_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}/replies", chat_path(client)?);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn reactions(
    client: &CuClient,
    message_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}/reactions", chat_path(client)?);
    merge_cursor_pages(client, &path, &cursor_query(cursor), "data", fetch_all).await
}

pub async fn react(client: &CuClient, message_id: &str, reaction: &str) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}/reactions", chat_path(client)?);
    let body = json!({"reaction": reaction});
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's unreact returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn unreact(
    client: &CuClient,
    message_id: &str,
    reaction: &str,
) -> Result<Value, CliError> {
    let path = format!(
        "{}/messages/{message_id}/reactions/{reaction}",
        chat_path(client)?
    );
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "message_id": message_id, "reaction": reaction}))
}

pub async fn tagged_users(client: &CuClient, message_id: &str) -> Result<Value, CliError> {
    let path = format!("{}/messages/{message_id}/tagged_users", chat_path(client)?);
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_channel_create_body_merges_name() {
        let body = build_channel_create_body(json!({}), "eng-updates");
        assert_eq!(body, json!({"name": "eng-updates"}));
    }

    #[test]
    fn build_channel_create_location_body_merges_location() {
        let body = build_channel_create_location_body(json!({}), json!({"folder": {"id": "457"}}));
        assert_eq!(body, json!({"location": {"folder": {"id": "457"}}}));
    }

    #[test]
    fn build_send_body_defaults_type_to_message_when_absent() {
        let body = build_send_body(json!({}), "build is green", None);
        assert_eq!(
            body,
            json!({"type": "message", "content": "build is green"})
        );
    }

    #[test]
    fn build_send_body_type_flag_overrides_data() {
        let body = build_send_body(json!({"type": "message"}), "posting", Some("post"));
        assert_eq!(body, json!({"type": "post", "content": "posting"}));
    }

    #[test]
    fn build_send_body_does_not_clobber_data_type_when_flag_absent() {
        let body = build_send_body(json!({"type": "post"}), "posting", None);
        assert_eq!(body, json!({"type": "post", "content": "posting"}));
    }

    #[test]
    fn build_reply_body_merges_content() {
        let body = build_reply_body(json!({}), "ack");
        assert_eq!(body, json!({"content": "ack"}));
    }
}
