use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

pub async fn list(
    client: &CuClient,
    folder_id: Option<&str>,
    space_id: Option<&str>,
    archived: bool,
) -> Result<Value, CliError> {
    let path = location_path(folder_id, space_id, "list")?;
    let query = [("archived".to_string(), archived.to_string())];
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, list_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn create(
    client: &CuClient,
    folder_id: Option<&str>,
    space_id: Option<&str>,
    data: Value,
) -> Result<Value, CliError> {
    let path = location_path(folder_id, space_id, "list")?;
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, list_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, list_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": list_id}))
}

pub async fn members(client: &CuClient, list_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}/member");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn from_template(
    client: &CuClient,
    folder_id: Option<&str>,
    space_id: Option<&str>,
    template_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let segment = format!("list_template/{template_id}");
    let path = location_path(folder_id, space_id, &segment)?;
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// Requires the Tasks in Multiple Lists ClickApp to be enabled on the workspace; without it
/// ClickUp rejects the call and the error passes through unmodified as `kind: "api"`.
pub async fn add_task(client: &CuClient, list_id: &str, task_id: &str) -> Result<Value, CliError> {
    task_membership(client, reqwest::Method::POST, list_id, task_id).await
}

pub async fn remove_task(
    client: &CuClient,
    list_id: &str,
    task_id: &str,
) -> Result<Value, CliError> {
    task_membership(client, reqwest::Method::DELETE, list_id, task_id).await
}

async fn task_membership(
    client: &CuClient,
    method: reqwest::Method,
    list_id: &str,
    task_id: &str,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}/task/{task_id}");
    let response = client.request(method, &path, &[], None).await?;
    Ok(response
        .body
        .unwrap_or_else(|| json!({"ok": true, "listId": list_id, "taskId": task_id})))
}

/// Builds the folder- or space-scoped path for a `--folder`/`--space` verb. The clap
/// `ArgGroup` on every CLI verb that reaches here already guarantees exactly one of the two is
/// set, so the fallback error only guards a caller that skips the CLI layer entirely.
fn location_path(
    folder_id: Option<&str>,
    space_id: Option<&str>,
    segment: &str,
) -> Result<String, CliError> {
    match (folder_id, space_id) {
        (Some(folder_id), None) => Ok(format!("/api/v2/folder/{folder_id}/{segment}")),
        (None, Some(space_id)) => Ok(format!("/api/v2/space/{space_id}/{segment}")),
        _ => Err(CliError::Usage(
            "exactly one of --folder or --space is required".into(),
        )),
    }
}
