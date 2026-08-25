use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_flag, pass_through};
use crate::error::CliError;

pub async fn list(client: &CuClient, group_ids: Option<String>) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let mut query = vec![("team_id".to_string(), workspace_id.to_string())];
    if let Some(group_ids) = group_ids {
        query.push(("group_ids".to_string(), group_ids));
    }
    let response = client
        .request(reqwest::Method::GET, "/api/v2/group", &query, None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `group create` sends: `--name` merges into `data["name"]`; `--member`
/// (repeatable) merges into `data["members"]` as an array, only when at least one is given.
/// Members are numeric: ClickUp's Create Group endpoint requires int32 user ids and
/// rejects string-typed members.
pub fn build_create_body(mut data: Value, name: &str, members: &[i64]) -> Value {
    merge_flag(&mut data, "name", Some(name.to_string()));
    if !members.is_empty() {
        data["members"] = json!(members);
    }
    data
}

pub async fn create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/group");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, group_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/group/{group_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, group_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/group/{group_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "group_id": group_id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_create_body_merges_name_and_members() {
        let body = build_create_body(json!({}), "eng", &[75413867, 123]);
        assert_eq!(body, json!({"name": "eng", "members": [75413867, 123]}));
    }

    #[test]
    fn build_create_body_omits_members_when_none_given() {
        let body = build_create_body(json!({}), "eng", &[]);
        assert_eq!(body, json!({"name": "eng"}));
    }
}
