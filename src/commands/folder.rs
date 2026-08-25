use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

pub async fn list(client: &CuClient, space_id: &str, archived: bool) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/folder");
    let query = [("archived".to_string(), archived.to_string())];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn get(
    client: &CuClient,
    folder_id: &str,
    include_subfolders: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/folder/{folder_id}");
    let query = [(
        "include_subfolders".to_string(),
        include_subfolders.to_string(),
    )];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn create(client: &CuClient, space_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/folder");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, folder_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/folder/{folder_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, folder_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/folder/{folder_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": folder_id}))
}

pub async fn move_folder(
    client: &CuClient,
    folder_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/folder/{folder_id}/position");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn from_template(
    client: &CuClient,
    space_id: &str,
    template_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/space/{space_id}/folder_template/{template_id}");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}
