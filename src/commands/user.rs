use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_flag, pass_through};
use crate::error::CliError;

fn user_path(client: &CuClient, user_id: &str) -> Result<String, CliError> {
    let workspace_id = client.workspace_id()?;
    Ok(format!("/api/v2/team/{workspace_id}/user/{user_id}"))
}

pub async fn get(
    client: &CuClient,
    user_id: &str,
    include_shared: bool,
) -> Result<Value, CliError> {
    let path = user_path(client, user_id)?;
    let mut query = Vec::new();
    if include_shared {
        query.push(("include_shared".to_string(), "true".to_string()));
    }
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `user invite` sends: `email` always set; `--admin` merges `data["admin"] =
/// true` only when the flag is actually passed; `--custom-role-id` merges when given.
pub fn build_invite_body(email: &str, admin: bool, custom_role_id: Option<i64>) -> Value {
    let mut body = json!({"email": email});
    if admin {
        merge_flag(&mut body, "admin", Some(true));
    }
    merge_flag(&mut body, "custom_role_id", custom_role_id);
    body
}

pub async fn invite(
    client: &CuClient,
    email: &str,
    admin: bool,
    custom_role_id: Option<i64>,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/user");
    let body = build_invite_body(email, admin, custom_role_id);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn edit(client: &CuClient, user_id: &str, data: Value) -> Result<Value, CliError> {
    let path = user_path(client, user_id)?;
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's remove returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn remove(client: &CuClient, user_id: &str) -> Result<Value, CliError> {
    let path = user_path(client, user_id)?;
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"removed": true, "user_id": user_id}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_invite_body_omits_admin_and_role_when_absent() {
        let body = build_invite_body("jo@cp.com", false, None);
        assert_eq!(body, json!({"email": "jo@cp.com"}));
    }

    #[test]
    fn build_invite_body_includes_admin_and_role_when_given() {
        let body = build_invite_body("jo@cp.com", true, Some(42));
        assert_eq!(
            body,
            json!({"email": "jo@cp.com", "admin": true, "custom_role_id": 42})
        );
    }
}
