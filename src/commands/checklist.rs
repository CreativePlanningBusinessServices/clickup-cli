use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{custom_id_query, pass_through};
use crate::error::CliError;

pub async fn create(
    client: &CuClient,
    task_id: &str,
    custom_id: bool,
    name: &str,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/checklist");
    let query = custom_id_query(client, custom_id)?;
    let body = json!({"name": name});
    let response = client
        .request(reqwest::Method::POST, &path, &query, Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, checklist_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/checklist/{checklist_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, checklist_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/checklist/{checklist_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": checklist_id}))
}

pub async fn item_add(
    client: &CuClient,
    checklist_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/checklist/{checklist_id}/checklist_item");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn item_update(
    client: &CuClient,
    checklist_id: &str,
    item_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/checklist/{checklist_id}/checklist_item/{item_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn item_remove(
    client: &CuClient,
    checklist_id: &str,
    item_id: &str,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/checklist/{checklist_id}/checklist_item/{item_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": item_id}))
}
