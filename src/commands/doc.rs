use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_cursor_pages, merge_flag, pass_through};
use crate::error::CliError;

fn docs_path(client: &CuClient) -> Result<String, CliError> {
    let workspace_id = client.workspace_id()?;
    Ok(format!("/api/v3/workspaces/{workspace_id}/docs"))
}

/// Pure query-building for `doc search`'s filters, split out so it's testable without a mock
/// server — each flag maps to a same-named query param only when given.
#[allow(clippy::too_many_arguments)]
pub fn build_search_query(
    parent_id: Option<&str>,
    parent_type: Option<i64>,
    creator: Option<&str>,
    deleted: Option<bool>,
    archived: Option<bool>,
    limit: Option<i64>,
    cursor: Option<&str>,
) -> Vec<(String, String)> {
    let mut query = Vec::new();
    if let Some(value) = parent_id {
        query.push(("parent_id".to_string(), value.to_string()));
    }
    if let Some(value) = parent_type {
        query.push(("parent_type".to_string(), value.to_string()));
    }
    if let Some(value) = creator {
        query.push(("creator".to_string(), value.to_string()));
    }
    if let Some(value) = deleted {
        query.push(("deleted".to_string(), value.to_string()));
    }
    if let Some(value) = archived {
        query.push(("archived".to_string(), value.to_string()));
    }
    if let Some(value) = limit {
        query.push(("limit".to_string(), value.to_string()));
    }
    if let Some(value) = cursor {
        query.push(("cursor".to_string(), value.to_string()));
    }
    query
}

pub async fn search(
    client: &CuClient,
    query: &[(String, String)],
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = docs_path(client)?;
    merge_cursor_pages(client, &path, query, "docs", fetch_all).await
}

/// Builds the body `doc create` sends: `--name` merges into `data["name"]`; `--parent-id`/
/// `--parent-type` merge into `data["parent"]["id"]`/`data["parent"]["type"]`, layered over
/// whatever `--data` already had at `"parent"` (so `--data` can still supply other parent-object
/// fields ClickUp accepts).
pub fn build_create_body(mut data: Value, name: &str, parent_id: &str, parent_type: i64) -> Value {
    merge_flag(&mut data, "name", Some(name.to_string()));
    let mut parent = data.get("parent").cloned().unwrap_or_else(|| json!({}));
    merge_flag(&mut parent, "id", Some(parent_id.to_string()));
    merge_flag(&mut parent, "type", Some(parent_type));
    data["parent"] = parent;
    data
}

pub async fn create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let path = docs_path(client)?;
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, doc_id: &str) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}", docs_path(client)?);
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn page_listing(
    client: &CuClient,
    doc_id: &str,
    depth: Option<i64>,
) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}/page_listing", docs_path(client)?);
    let query = depth_query(depth);
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn pages(
    client: &CuClient,
    doc_id: &str,
    depth: Option<i64>,
    format: Option<&str>,
) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}/pages", docs_path(client)?);
    let mut query = depth_query(depth);
    if let Some(format) = format {
        query.push(("content_format".to_string(), format.to_string()));
    }
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn page_get(
    client: &CuClient,
    doc_id: &str,
    page_id: &str,
    format: Option<&str>,
) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}/pages/{page_id}", docs_path(client)?);
    let mut query = Vec::new();
    if let Some(format) = format {
        query.push(("content_format".to_string(), format.to_string()));
    }
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `doc page create` sends: `--parent-page-id`/`--name`/`--sub-title`/
/// `--content`/`--content-format` merge into their same-named keys, only when given. `content`
/// is already the read-and-resolved string (via `read_text_arg`'s `@file`/`-` handling) by the
/// time it reaches here — this layer never parses it as JSON.
pub fn build_page_create_body(
    mut data: Value,
    parent_page_id: Option<&str>,
    name: Option<&str>,
    sub_title: Option<&str>,
    content: Option<String>,
    content_format: Option<&str>,
) -> Value {
    merge_flag(
        &mut data,
        "parent_page_id",
        parent_page_id.map(str::to_string),
    );
    merge_flag(&mut data, "name", name.map(str::to_string));
    merge_flag(&mut data, "sub_title", sub_title.map(str::to_string));
    merge_flag(&mut data, "content", content);
    merge_flag(
        &mut data,
        "content_format",
        content_format.map(str::to_string),
    );
    data
}

pub async fn page_create(client: &CuClient, doc_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}/pages", docs_path(client)?);
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `doc page update` sends: like `build_page_create_body`, but also merges
/// `--mode` into `data["content_edit_mode"]` (`replace`/`append`/`prepend`) instead of a
/// `parent_page_id`, which only applies at creation.
pub fn build_page_update_body(
    mut data: Value,
    name: Option<&str>,
    sub_title: Option<&str>,
    content: Option<String>,
    mode: Option<&str>,
    content_format: Option<&str>,
) -> Value {
    merge_flag(&mut data, "name", name.map(str::to_string));
    merge_flag(&mut data, "sub_title", sub_title.map(str::to_string));
    merge_flag(&mut data, "content", content);
    merge_flag(&mut data, "content_edit_mode", mode.map(str::to_string));
    merge_flag(
        &mut data,
        "content_format",
        content_format.map(str::to_string),
    );
    data
}

pub async fn page_update(
    client: &CuClient,
    doc_id: &str,
    page_id: &str,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("{}/{doc_id}/pages/{page_id}", docs_path(client)?);
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

fn depth_query(depth: Option<i64>) -> Vec<(String, String)> {
    depth
        .map(|depth_value| vec![("max_page_depth".to_string(), depth_value.to_string())])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_search_query_includes_only_given_filters() {
        let query = build_search_query(Some("790"), Some(4), None, None, None, None, None);
        assert_eq!(
            query,
            vec![
                ("parent_id".to_string(), "790".to_string()),
                ("parent_type".to_string(), "4".to_string()),
            ]
        );
    }

    #[test]
    fn build_search_query_empty_when_nothing_given() {
        assert_eq!(
            build_search_query(None, None, None, None, None, None, None),
            Vec::<(String, String)>::new()
        );
    }

    #[test]
    fn build_create_body_sets_name_and_parent_object() {
        let body = build_create_body(json!({}), "Runbook", "790", 4);
        assert_eq!(
            body,
            json!({"name": "Runbook", "parent": {"id": "790", "type": 4}})
        );
    }

    #[test]
    fn build_create_body_preserves_other_data_fields() {
        let body = build_create_body(json!({"visibility": "PUBLIC"}), "Runbook", "790", 4);
        assert_eq!(
            body,
            json!({
                "name": "Runbook",
                "visibility": "PUBLIC",
                "parent": {"id": "790", "type": 4},
            })
        );
    }

    #[test]
    fn build_page_create_body_merges_only_given_fields() {
        let body = build_page_create_body(
            json!({}),
            Some("page1"),
            Some("Setup"),
            None,
            Some("hello world".to_string()),
            None,
        );
        assert_eq!(
            body,
            json!({"parent_page_id": "page1", "name": "Setup", "content": "hello world"})
        );
    }

    #[test]
    fn build_page_update_body_sends_mode_and_string_content() {
        let body = build_page_update_body(
            json!({}),
            None,
            None,
            Some(r#"{"looks":"like json"}"#.to_string()),
            Some("append"),
            None,
        );
        assert_eq!(
            body,
            json!({"content": r#"{"looks":"like json"}"#, "content_edit_mode": "append"})
        );
    }
}
