use std::path::Path;

use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{
    custom_id_query, merge_cursor_pages, merge_flag, merge_task_pages, pass_through,
};
use crate::error::CliError;

pub async fn create(client: &CuClient, list_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}/task");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn get(
    client: &CuClient,
    task_id: &str,
    custom_id: bool,
    subtasks: bool,
    markdown: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}");
    let mut query = custom_id_query(client, custom_id)?;
    query.push(("include_subtasks".to_string(), subtasks.to_string()));
    query.push((
        "include_markdown_description".to_string(),
        markdown.to_string(),
    ));
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn update(
    client: &CuClient,
    task_id: &str,
    custom_id: bool,
    data: Value,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}");
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::PUT, &path, &query, Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, task_id: &str, custom_id: bool) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}");
    let query = custom_id_query(client, custom_id)?;
    client
        .request(reqwest::Method::DELETE, &path, &query, None)
        .await?;
    Ok(json!({"deleted": true, "id": task_id}))
}

pub struct TaskListArgs {
    pub archived: bool,
    pub include_closed: bool,
    pub subtasks: bool,
    pub markdown: bool,
    pub statuses: Vec<String>,
    pub assignees: Vec<String>,
    pub order_by: Option<String>,
    pub reverse: bool,
    /// Raw `key=value` pairs from repeatable `--query` flags, passed through verbatim — this
    /// is also where the CLI layer folds in `filter`'s `--space`/`--folder`/`--list` ids
    /// (as `space_ids[]=`/`project_ids[]=`/`list_ids[]=`) so this function stays generic to
    /// both `list` and `filter`.
    pub extra_query: Vec<String>,
}

/// Pure query-building shared by `list` and `filter`. Repeatable `--status`/`--assignee`
/// fan out to one `statuses[]`/`assignees[]` pair per value, matching how ClickUp expects
/// repeated array query params.
pub fn build_task_query(args: &TaskListArgs) -> Result<Vec<(String, String)>, CliError> {
    let mut query = vec![
        ("archived".to_string(), args.archived.to_string()),
        (
            "include_closed".to_string(),
            args.include_closed.to_string(),
        ),
        ("subtasks".to_string(), args.subtasks.to_string()),
        (
            "include_markdown_description".to_string(),
            args.markdown.to_string(),
        ),
    ];
    for status in &args.statuses {
        query.push(("statuses[]".to_string(), status.clone()));
    }
    for assignee in &args.assignees {
        query.push(("assignees[]".to_string(), assignee.clone()));
    }
    if let Some(order_by) = &args.order_by {
        query.push(("order_by".to_string(), order_by.clone()));
    }
    if args.reverse {
        query.push(("reverse".to_string(), "true".to_string()));
    }
    query.extend(crate::commands::parse_key_value_pairs(&args.extra_query)?);
    Ok(query)
}

pub async fn list(
    client: &CuClient,
    list_id: &str,
    args: &TaskListArgs,
    page: Option<u64>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}/task");
    let query = build_task_query(args)?;
    merge_task_pages(client, &path, &query, page, fetch_all).await
}

/// Team-scoped "filtered team tasks" endpoint. Unlike `list`, ClickUp doesn't send a
/// `last_page` flag here — `merge_task_pages` falls back to stopping on a short page.
pub async fn filter(
    client: &CuClient,
    args: &TaskListArgs,
    page: Option<u64>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/task");
    let query = build_task_query(args)?;
    merge_task_pages(client, &path, &query, page, fetch_all).await
}

/// Moves a task to a different list — ClickUp's v3 "home list" endpoint, distinct from `list
/// add-task`/`list remove-task` (which add/remove additional list memberships when the Tasks
/// in Multiple Lists ClickApp is enabled).
pub async fn move_task(client: &CuClient, task_id: &str, list_id: &str) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v3/workspaces/{workspace_id}/tasks/{task_id}/home_list/{list_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn merge(
    client: &CuClient,
    target_task_id: &str,
    source_task_ids: &[String],
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{target_task_id}/merge");
    let body = json!({"source_task_ids": source_task_ids});
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn from_template(
    client: &CuClient,
    list_id: &str,
    template_id: &str,
    name: &str,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/list/{list_id}/taskTemplate/{template_id}");
    let body = json!({"name": name});
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn members(client: &CuClient, task_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/member");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// A single task id hits the per-task endpoint; more than one switches to the bulk endpoint,
/// which takes one `task_ids` query pair per id instead of a path segment.
pub async fn time_in_status(
    client: &CuClient,
    task_ids: &[String],
    custom_id: bool,
) -> Result<Value, CliError> {
    let mut query = custom_id_query(client, custom_id)?;
    let path = if task_ids.len() == 1 {
        format!("/api/v2/task/{}/time_in_status", task_ids[0])
    } else {
        for task_id in task_ids {
            query.push(("task_ids".to_string(), task_id.clone()));
        }
        "/api/v2/task/bulk_time_in_status/task_ids".to_string()
    };
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn tag_add(
    client: &CuClient,
    task_id: &str,
    tag_name: &str,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/tag/{tag_name}");
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::POST, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn tag_remove(
    client: &CuClient,
    task_id: &str,
    tag_name: &str,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/tag/{tag_name}");
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::DELETE, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn dependency_add(
    client: &CuClient,
    task_id: &str,
    depends_on: Option<&str>,
    dependency_of: Option<&str>,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/dependency");
    let query = custom_id_query(client, custom_id)?;
    let mut body = json!({});
    merge_flag(&mut body, "depends_on", depends_on.map(str::to_string));
    merge_flag(
        &mut body,
        "dependency_of",
        dependency_of.map(str::to_string),
    );
    let response = client
        .request(reqwest::Method::POST, &path, &query, Some(body))
        .await?;
    Ok(pass_through(response))
}

/// Unlike `dependency_add`, ClickUp's delete takes `depends_on`/`dependency_of` as query
/// params rather than a JSON body.
pub async fn dependency_remove(
    client: &CuClient,
    task_id: &str,
    depends_on: Option<&str>,
    dependency_of: Option<&str>,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/dependency");
    let mut query = custom_id_query(client, custom_id)?;
    if let Some(depends_on_value) = depends_on {
        query.push(("depends_on".to_string(), depends_on_value.to_string()));
    }
    if let Some(dependency_of_value) = dependency_of {
        query.push(("dependency_of".to_string(), dependency_of_value.to_string()));
    }
    let response = client
        .request(reqwest::Method::DELETE, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn link_add(
    client: &CuClient,
    task_id: &str,
    links_to: &str,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/link/{links_to}");
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::POST, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn link_remove(
    client: &CuClient,
    task_id: &str,
    links_to: &str,
    custom_id: bool,
) -> Result<Value, CliError> {
    let path = format!("/api/v2/task/{task_id}/link/{links_to}");
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::DELETE, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

/// Uploads a local file as a task attachment via multipart form field `"attachment"`. Reads
/// the whole file into memory up front — `request_multipart` sends once with no retry (a
/// `reqwest::multipart::Form` isn't `Clone`, so it can't be rebuilt per attempt), so a
/// missing/unreadable file is caught here as a `Usage` error before any request goes out.
pub async fn attach(
    client: &CuClient,
    task_id: &str,
    file_path: &Path,
    filename: Option<String>,
    custom_id: bool,
) -> Result<Value, CliError> {
    let file_bytes = std::fs::read(file_path).map_err(|io_error| {
        CliError::Usage(format!("cannot read {}: {io_error}", file_path.display()))
    })?;
    let attachment_name = filename.unwrap_or_else(|| {
        file_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "attachment".to_string())
    });
    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(attachment_name);
    let form = reqwest::multipart::Form::new().part("attachment", part);
    let path = format!("/api/v2/task/{task_id}/attachment");
    let query = custom_id_query(client, custom_id)?;
    let response = client.request_multipart(&path, &query, form).await?;
    Ok(pass_through(response))
}

/// Lists a task's attachments via the v3 cursor-paginated endpoint. `cursor` fetches one page
/// from a specific offset; `fetch_all` (`--all`) follows `next_cursor` via `merge_cursor_pages`
/// until it stops, merging every page's `"attachments"` array into one envelope.
pub async fn attachments(
    client: &CuClient,
    task_id: &str,
    cursor: Option<String>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v3/workspaces/{workspace_id}/attachments/{task_id}/attachments");
    let mut query = Vec::new();
    if let Some(cursor_value) = cursor {
        query.push(("cursor".to_string(), cursor_value));
    }
    merge_cursor_pages(client, &path, &query, "attachments", fetch_all).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> TaskListArgs {
        TaskListArgs {
            archived: false,
            include_closed: false,
            subtasks: false,
            markdown: false,
            statuses: Vec::new(),
            assignees: Vec::new(),
            order_by: None,
            reverse: false,
            extra_query: Vec::new(),
        }
    }

    #[test]
    fn statuses_fan_out_to_repeated_statuses_bracket_pairs() {
        let mut args = base_args();
        args.statuses = vec!["in progress".to_string(), "open".to_string()];
        let query = build_task_query(&args).unwrap();
        assert_eq!(
            query
                .iter()
                .filter(|(key, _)| key == "statuses[]")
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                ("statuses[]".to_string(), "in progress".to_string()),
                ("statuses[]".to_string(), "open".to_string()),
            ]
        );
    }

    #[test]
    fn assignees_fan_out_to_repeated_assignees_bracket_pairs() {
        let mut args = base_args();
        args.assignees = vec!["123".to_string(), "456".to_string()];
        let query = build_task_query(&args).unwrap();
        assert_eq!(
            query
                .iter()
                .filter(|(key, _)| key == "assignees[]")
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                ("assignees[]".to_string(), "123".to_string()),
                ("assignees[]".to_string(), "456".to_string()),
            ]
        );
    }

    #[test]
    fn bad_query_pair_without_equals_is_a_usage_error() {
        let mut args = base_args();
        args.extra_query = vec!["due_date_lt".to_string()];
        let result = build_task_query(&args);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }
}
