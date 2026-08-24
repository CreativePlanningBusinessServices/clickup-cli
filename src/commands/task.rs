use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{custom_id_query, merge_task_pages, pass_through};
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
