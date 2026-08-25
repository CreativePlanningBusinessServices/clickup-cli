use serde_json::Value;

use crate::client::CuClient;
use crate::commands::pass_through;
use crate::error::CliError;

/// GET /api/v2/team — every workspace (ClickUp calls them "teams") the token can see. Not
/// workspace-scoped: this is how a caller discovers workspace ids in the first place.
pub async fn list(client: &CuClient) -> Result<Value, CliError> {
    let response = client
        .request(reqwest::Method::GET, "/api/v2/team", &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn seats(client: &CuClient) -> Result<Value, CliError> {
    team_scoped_get(client, "seats").await
}

pub async fn plan(client: &CuClient) -> Result<Value, CliError> {
    team_scoped_get(client, "plan").await
}

pub async fn shared(client: &CuClient) -> Result<Value, CliError> {
    team_scoped_get(client, "shared").await
}

/// GET /api/v2/team/{ws}/custom_item — ClickUp's own endpoint segment is `custom_item` despite
/// the CLI verb being `task-types`.
pub async fn task_types(client: &CuClient) -> Result<Value, CliError> {
    team_scoped_get(client, "custom_item").await
}

async fn team_scoped_get(client: &CuClient, segment: &str) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/{segment}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}
