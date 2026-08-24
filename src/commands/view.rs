use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_task_pages, pass_through};
use crate::error::CliError;

/// Where `view list`/`view create` operate. Exactly one of `--space`/`--folder`/`--list`
/// selects a location-scoped view collection; giving none of them (the clap `ArgGroup` on both
/// verbs only forbids giving more than one, not zero — the same precedent as `field list`'s
/// `FieldLocation`, since a bare `--workspace` flag would collide with the global
/// `--workspace <id>` override) falls back to `Workspace` — team-wide views via the team path.
pub enum ViewLocation<'a> {
    Workspace,
    Space(&'a str),
    Folder(&'a str),
    List(&'a str),
}

/// Builds a `ViewLocation` from the CLI's `--space`/`--folder`/`--list` options. Anything other
/// than exactly one of the three set (including all unset) resolves to `Workspace`.
pub fn location_from<'a>(
    space_id: Option<&'a str>,
    folder_id: Option<&'a str>,
    list_id: Option<&'a str>,
) -> ViewLocation<'a> {
    match (space_id, folder_id, list_id) {
        (Some(space_id), None, None) => ViewLocation::Space(space_id),
        (None, Some(folder_id), None) => ViewLocation::Folder(folder_id),
        (None, None, Some(list_id)) => ViewLocation::List(list_id),
        _ => ViewLocation::Workspace,
    }
}

impl ViewLocation<'_> {
    fn path(&self, client: &CuClient) -> Result<String, CliError> {
        Ok(match self {
            ViewLocation::Workspace => {
                let workspace_id = client.workspace_id()?;
                format!("/api/v2/team/{workspace_id}/view")
            }
            ViewLocation::Space(space_id) => format!("/api/v2/space/{space_id}/view"),
            ViewLocation::Folder(folder_id) => format!("/api/v2/folder/{folder_id}/view"),
            ViewLocation::List(list_id) => format!("/api/v2/list/{list_id}/view"),
        })
    }
}

pub async fn list(client: &CuClient, location: &ViewLocation<'_>) -> Result<Value, CliError> {
    let path = location.path(client)?;
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn create(
    client: &CuClient,
    location: &ViewLocation<'_>,
    data: Value,
) -> Result<Value, CliError> {
    let path = location.path(client)?;
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, view_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/view/{view_id}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, view_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/view/{view_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, view_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/view/{view_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": view_id}))
}

/// `page` is REQUIRED by the API (unlike `task list`/`task filter`, which default it to 0
/// themselves via `merge_task_pages`) — but that default already lives in `merge_task_pages`,
/// so this just forwards `page`/`fetch_all` straight through, reusing the same last_page-driven
/// merge loop task pagination uses (Task 10).
pub async fn tasks(
    client: &CuClient,
    view_id: &str,
    page: Option<u64>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/view/{view_id}/task");
    merge_task_pages(client, &path, &[], page, fetch_all).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_from_picks_the_one_given_location() {
        assert!(matches!(
            location_from(Some("790"), None, None),
            ViewLocation::Space("790")
        ));
        assert!(matches!(
            location_from(None, Some("457"), None),
            ViewLocation::Folder("457")
        ));
        assert!(matches!(
            location_from(None, None, Some("124")),
            ViewLocation::List("124")
        ));
    }

    #[test]
    fn location_from_falls_back_to_workspace_when_none_given() {
        assert!(matches!(
            location_from(None, None, None),
            ViewLocation::Workspace
        ));
    }
}
