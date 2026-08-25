use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

pub async fn list(client: &CuClient, include_completed: bool) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/goal");
    let query = vec![(
        "include_completed".to_string(),
        include_completed.to_string(),
    )];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, goal_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/goal/{goal_id}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/goal");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, goal_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/goal/{goal_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, goal_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/goal/{goal_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": goal_id}))
}

pub async fn key_result_add(
    client: &CuClient,
    goal_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/goal/{goal_id}/key_result");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn key_result_update(
    client: &CuClient,
    key_result_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/key_result/{key_result_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn key_result_remove(client: &CuClient, key_result_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/key_result/{key_result_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": key_result_id}))
}
