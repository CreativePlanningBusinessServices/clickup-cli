use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{merge_flag, pass_through};
use crate::error::CliError;

pub struct TimeListArgs {
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub assignee: Option<String>,
    pub task_id: Option<String>,
    pub list_id: Option<String>,
    pub space_id: Option<String>,
    pub folder_id: Option<String>,
    pub billable: Option<bool>,
    pub include_task_tags: bool,
    pub include_location_names: bool,
}

/// Pure query-building for `time list`, so the date-window/location/billable filters are
/// testable without a mock server.
pub fn build_list_query(args: &TimeListArgs) -> Vec<(String, String)> {
    let mut query = Vec::new();
    if let Some(start_date) = args.start_date {
        query.push(("start_date".to_string(), start_date.to_string()));
    }
    if let Some(end_date) = args.end_date {
        query.push(("end_date".to_string(), end_date.to_string()));
    }
    if let Some(assignee) = &args.assignee {
        query.push(("assignee".to_string(), assignee.clone()));
    }
    if let Some(task_id) = &args.task_id {
        query.push(("task_id".to_string(), task_id.clone()));
    }
    if let Some(list_id) = &args.list_id {
        query.push(("list_id".to_string(), list_id.clone()));
    }
    if let Some(space_id) = &args.space_id {
        query.push(("space_id".to_string(), space_id.clone()));
    }
    if let Some(folder_id) = &args.folder_id {
        query.push(("folder_id".to_string(), folder_id.clone()));
    }
    if let Some(billable) = args.billable {
        query.push(("is_billable".to_string(), billable.to_string()));
    }
    if args.include_task_tags {
        query.push(("include_task_tags".to_string(), "true".to_string()));
    }
    if args.include_location_names {
        query.push(("include_location_names".to_string(), "true".to_string()));
    }
    query
}

/// Time entries within a date window; every filter is optional. Note: a negative `duration`
/// on an entry in the response means that timer is currently running (ClickUp counts up from a
/// negative "elapsed so far" value rather than sending a separate running flag) — surfaced
/// as-is, not translated, since agents consuming this JSON should see the raw API shape.
pub async fn list(client: &CuClient, args: &TimeListArgs) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries");
    let query = build_list_query(args);
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn get(client: &CuClient, timer_id: &str) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/{timer_id}");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// Builds the body `time create`/`time start` send: `--task` merges as `data["tid"]` (ClickUp's
/// time-entry field name for the associated task id); `--description`/`--billable` merge into
/// `data["description"]`/`data["billable"]`. All three merge only when given.
pub fn build_entry_body(
    mut data: Value,
    task: Option<&str>,
    description: Option<&str>,
    billable: Option<bool>,
) -> Value {
    merge_flag(&mut data, "tid", task.map(str::to_string));
    merge_flag(&mut data, "description", description.map(str::to_string));
    merge_flag(&mut data, "billable", billable);
    data
}

/// `data["start"]` plus either `data["duration"]` or `data["end"]` — which combination is
/// required/accepted is validated by the API, not the CLI.
pub async fn create(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, timer_id: &str, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/{timer_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, timer_id: &str) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/{timer_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": timer_id}))
}

pub async fn history(client: &CuClient, timer_id: &str) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/{timer_id}/history");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

/// The currently running timer for `assignee` (or the token's own user when omitted). Note: a
/// negative `duration` in the response means the timer is running — see `list`'s doc comment.
pub async fn current(client: &CuClient, assignee: Option<&str>) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/current");
    let mut query = Vec::new();
    if let Some(assignee) = assignee {
        query.push(("assignee".to_string(), assignee.to_string()));
    }
    let response = client
        .request(reqwest::Method::GET, &path, &query, None)
        .await?;
    Ok(pass_through(response))
}

pub async fn start(client: &CuClient, data: Value) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/start");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn stop(client: &CuClient) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/stop");
    let response = client
        .request(reqwest::Method::POST, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn tags(client: &CuClient) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/tags");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn tag_add(
    client: &CuClient,
    entry_ids: &[String],
    tags: &[String],
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/tags");
    let body = json!({"time_entry_ids": entry_ids, "tags": tags});
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

/// Unlike most DELETEs in this CLI, ClickUp's remove-tags-from-time-entries endpoint takes the
/// ids/tags in a JSON body sent along with the DELETE request rather than query params —
/// `CuClient::request` supports a body on any HTTP method.
pub async fn tag_remove(
    client: &CuClient,
    entry_ids: &[String],
    tags: &[String],
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/tags");
    let body = json!({"time_entry_ids": entry_ids, "tags": tags});
    let response = client
        .request(reqwest::Method::DELETE, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

pub async fn tag_rename(
    client: &CuClient,
    name: &str,
    new_name: &str,
    fg: Option<&str>,
    bg: Option<&str>,
) -> Result<Value, CliError> {
    let workspace_id = client.workspace_id()?;
    let path = format!("/api/v2/team/{workspace_id}/time_entries/tags");
    let mut body = json!({"name": name, "new_name": new_name});
    merge_flag(&mut body, "tag_fg", fg.map(str::to_string));
    merge_flag(&mut body, "tag_bg", bg.map(str::to_string));
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(body))
        .await?;
    Ok(pass_through(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> TimeListArgs {
        TimeListArgs {
            start_date: None,
            end_date: None,
            assignee: None,
            task_id: None,
            list_id: None,
            space_id: None,
            folder_id: None,
            billable: None,
            include_task_tags: false,
            include_location_names: false,
        }
    }

    #[test]
    fn build_list_query_includes_only_given_filters() {
        assert_eq!(
            build_list_query(&base_args()),
            Vec::<(String, String)>::new()
        );

        let mut args = base_args();
        args.start_date = Some(1735689600000);
        args.end_date = Some(1738368000000);
        args.assignee = Some("75413867".to_string());
        args.billable = Some(true);
        args.include_task_tags = true;
        args.include_location_names = true;
        assert_eq!(
            build_list_query(&args),
            vec![
                ("start_date".to_string(), "1735689600000".to_string()),
                ("end_date".to_string(), "1738368000000".to_string()),
                ("assignee".to_string(), "75413867".to_string()),
                ("is_billable".to_string(), "true".to_string()),
                ("include_task_tags".to_string(), "true".to_string()),
                ("include_location_names".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn build_entry_body_merges_task_as_tid_plus_description_and_billable() {
        let body = build_entry_body(json!({}), Some("868k5a5wa"), Some("pairing"), Some(true));
        assert_eq!(
            body,
            json!({"tid": "868k5a5wa", "description": "pairing", "billable": true})
        );
    }

    #[test]
    fn build_entry_body_leaves_data_untouched_when_flags_absent() {
        let body = build_entry_body(json!({"start": 1735689600000_i64}), None, None, None);
        assert_eq!(body, json!({"start": 1735689600000_i64}));
    }
}
