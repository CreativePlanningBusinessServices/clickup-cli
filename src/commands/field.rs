use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{custom_id_query, pass_through};
use crate::error::CliError;

/// Where `field list` looks for custom fields. Exactly one of `--list`/`--folder`/`--space`
/// selects a location-scoped list; giving none of them (the clap `ArgGroup` on `field list`
/// only forbids giving more than one, not zero) falls back to `Workspace` — workspace-wide
/// fields via the team path, using the implicit `--workspace`/account-default workspace the
/// same way every other team-scoped command in this CLI does, rather than a redundant local
/// flag.
pub enum FieldLocation<'a> {
    List(&'a str),
    Folder(&'a str),
    Space(&'a str),
    Workspace,
}

/// Builds a `FieldLocation` from the CLI's `--list`/`--folder`/`--space` options. Anything
/// other than exactly one of the three set (including all unset) resolves to `Workspace`.
pub fn location_from<'a>(
    list_id: Option<&'a str>,
    folder_id: Option<&'a str>,
    space_id: Option<&'a str>,
) -> FieldLocation<'a> {
    match (list_id, folder_id, space_id) {
        (Some(list_id), None, None) => FieldLocation::List(list_id),
        (None, Some(folder_id), None) => FieldLocation::Folder(folder_id),
        (None, None, Some(space_id)) => FieldLocation::Space(space_id),
        _ => FieldLocation::Workspace,
    }
}

pub async fn list(client: &CuClient, location: &FieldLocation<'_>) -> Result<Value, CliError> {
    let path = match location {
        FieldLocation::List(list_id) => format!("/api/v2/list/{list_id}/field"),
        FieldLocation::Folder(folder_id) => format!("/api/v2/folder/{folder_id}/field"),
        FieldLocation::Space(space_id) => format!("/api/v2/space/{space_id}/field"),
        FieldLocation::Workspace => {
            let workspace_id = client.workspace_id()?;
            format!("/api/v2/team/{workspace_id}/field")
        }
    };
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `field set` sends: `{"value": value}` plus `"value_options"` only when
/// `--options` was given.
pub fn build_set_body(value: Value, options: Option<Value>) -> Value {
    let mut body = json!({"value": value});
    if let Some(options) = options {
        body["value_options"] = options;
    }
    body
}

/// ClickUp answers with a bare `{}` on success, indistinguishable from a no-op; synthesize an
/// acknowledgement the way `remove` does.
pub async fn set(
    client: &CuClient,
    task_id: &str,
    field_id: &str,
    custom_id: bool,
    value: Value,
    options: Option<Value>,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/field/{field_id}");
    let query = custom_id_query(client, custom_id)?;
    let body = build_set_body(value, options);
    client
        .request(reqwest::Method::POST, &path, &query, Some(body))
        .await?;
    Ok(json!({"set": true, "task_id": task_id, "field_id": field_id}))
}

/// ClickUp's remove returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn remove(
    client: &CuClient,
    task_id: &str,
    field_id: &str,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/field/{field_id}");
    let query = custom_id_query(client, custom_id)?;
    client
        .request(reqwest::Method::DELETE, &path, &query, None)
        .await?;
    Ok(json!({"removed": true, "task_id": task_id, "field_id": field_id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_from_picks_the_one_given_location() {
        assert!(matches!(
            location_from(Some("124"), None, None),
            FieldLocation::List("124")
        ));
        assert!(matches!(
            location_from(None, Some("457"), None),
            FieldLocation::Folder("457")
        ));
        assert!(matches!(
            location_from(None, None, Some("790")),
            FieldLocation::Space("790")
        ));
    }

    #[test]
    fn location_from_falls_back_to_workspace_when_none_given() {
        assert!(matches!(
            location_from(None, None, None),
            FieldLocation::Workspace
        ));
    }

    #[test]
    fn build_set_body_omits_value_options_when_absent() {
        let body = build_set_body(json!("urgent"), None);
        assert_eq!(body, json!({"value": "urgent"}));
    }

    #[test]
    fn build_set_body_includes_value_options_when_given() {
        let body = build_set_body(json!("urgent"), Some(json!({"time": true})));
        assert_eq!(
            body,
            json!({"value": "urgent", "value_options": {"time": true}})
        );
    }
}
