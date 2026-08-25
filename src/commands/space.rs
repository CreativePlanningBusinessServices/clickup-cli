use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

pub async fn list(client: &CuClient, archived: bool) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/space");
    let query = [("archived".to_string(), archived.to_string())];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, space_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/space");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, space_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, space_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": space_id}))
}
