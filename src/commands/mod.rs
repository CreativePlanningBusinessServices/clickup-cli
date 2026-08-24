pub mod account;
pub mod config_cmd;
pub mod folder;
pub mod list;
pub mod raw;
pub mod space;
pub mod task;
pub mod workspace;

use std::io::Read;

use serde_json::{Value, json};

use crate::client::{CuClient, CuResponse};
use crate::error::CliError;

/// Hard cap on pages `merge_task_pages` will fetch for a single `--all` request, so a
/// misbehaving or unbounded endpoint can't loop forever.
pub const MAX_PAGES: u32 = 1000;

/// Shared task-list pagination for `task list`/`task filter` (Task 10) and `view tasks`
/// (Task 16). Without `fetch_all`, issues a single request for `page` (defaulting to 0) and
/// passes its body through unchanged. With `fetch_all`, loops `page = 0..`, merging each
/// page's `"tasks"` array into one envelope: `{"items": [...], "count": n, "hasMore": bool}`.
/// A page stops the loop when it's empty, or when `last_page: true` is present in the body
/// (list-scoped task endpoints); endpoints that never send `last_page` (filtered/team-scoped
/// task endpoints) instead stop once a page comes back with fewer than 100 items — ClickUp's
/// fixed page size. Hitting `MAX_PAGES` without a natural stop truncates the loop and reports
/// `hasMore: true` rather than fetching forever.
pub async fn merge_task_pages(
    client: &CuClient,
    path: &str,
    base_query: &[(String, String)],
    page: Option<u64>,
    fetch_all: bool,
) -> Result<Value, CliError> {
    if !fetch_all {
        let mut query = base_query.to_vec();
        query.push(("page".to_string(), page.unwrap_or(0).to_string()));
        let response = client
            .request(reqwest::Method::GET, path, &query, None)
            .await?;
        return Ok(pass_through(response));
    }

    let mut items: Vec<Value> = Vec::new();
    let mut has_more = false;
    for page_number in 0..u64::from(MAX_PAGES) {
        let mut query = base_query.to_vec();
        query.push(("page".to_string(), page_number.to_string()));
        let response = client
            .request(reqwest::Method::GET, path, &query, None)
            .await?;
        let body = pass_through(response);
        let page_tasks = body["tasks"].as_array().cloned().unwrap_or_default();
        let page_len = page_tasks.len();
        items.extend(page_tasks);

        if page_len == 0 {
            break;
        }
        let is_last_page = body
            .get("last_page")
            .and_then(Value::as_bool)
            .unwrap_or(page_len < 100);
        if is_last_page {
            break;
        }
        if page_number + 1 == u64::from(MAX_PAGES) {
            has_more = true;
        }
    }

    Ok(json!({"items": items, "count": items.len(), "hasMore": has_more}))
}

/// Query pairs for `--custom-id`: ClickUp requires `custom_task_ids=true` and `team_id`
/// together to resolve a task by its custom id (e.g. `DEV-123`) instead of its ClickUp id.
/// Returns an empty query when `custom_id` is false.
pub fn custom_id_query(
    client: &CuClient,
    custom_id: bool,
) -> Result<Vec<(String, String)>, CliError> {
    if !custom_id {
        return Ok(Vec::new());
    }
    let workspace_id = client.workspace_id()?;
    Ok(vec![
        ("custom_task_ids".to_string(), "true".to_string()),
        ("team_id".to_string(), workspace_id.to_string()),
    ])
}

/// Reads a `--data`-style argument: inline JSON, `@file` to read a file, or `-` for stdin.
pub fn read_data_arg(data: &str) -> Result<Value, CliError> {
    let text = if data == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|io_error| CliError::Usage(format!("cannot read stdin: {io_error}")))?;
        buffer
    } else if let Some(file_path) = data.strip_prefix('@') {
        std::fs::read_to_string(file_path)
            .map_err(|io_error| CliError::Usage(format!("cannot read {file_path}: {io_error}")))?
    } else {
        data.to_string()
    };
    serde_json::from_str(&text)
        .map_err(|parse_error| CliError::Usage(format!("data is not valid JSON: {parse_error}")))
}

/// Parses an optional `--data` flag into a JSON body, defaulting to an empty object when the
/// flag is absent — the base every create/update dispatch arm layers scalar convenience flags
/// (via `merge_flag`) on top of.
pub fn parse_data_flag(data: Option<&str>) -> Result<Value, CliError> {
    data.map(read_data_arg)
        .transpose()
        .map(|body| body.unwrap_or_else(|| json!({})))
}

/// Sets `body[key]` from `value` when present, leaving `body` untouched otherwise. Every
/// create/update command layers a scalar convenience flag (e.g. `--name`) over the `--data`
/// JSON blob this way — the flag always wins over whatever the same key holds in `--data`.
pub fn merge_flag<T: Into<Value>>(body: &mut Value, key: &str, value: Option<T>) {
    if let Some(scalar) = value {
        body[key] = scalar.into();
    }
}

/// A successful JSON body passes through unchanged; a body-less success (some DELETEs return
/// 204 with no body) falls back to `{"status": <code>}` so the caller still gets JSON back.
pub fn pass_through(response: CuResponse) -> Value {
    let status = response.status;
    response.body.unwrap_or_else(|| json!({"status": status}))
}

/// Parses repeated `key=value` flags.
pub fn parse_key_value_pairs(pairs: &[String]) -> Result<Vec<(String, String)>, CliError> {
    pairs
        .iter()
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| CliError::Usage(format!("expected key=value, got '{pair}'")))?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

/// Parses repeated `--header 'Name: value'` flags for the raw command.
pub fn parse_header_pairs(pairs: &[String]) -> Result<Vec<(String, String)>, CliError> {
    pairs
        .iter()
        .map(|pair| {
            let (name, value) = pair.split_once(':').ok_or_else(|| {
                CliError::Usage(format!(
                    "--header must be in 'Name: value' form, got '{pair}'"
                ))
            })?;
            Ok((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn reads_inline_json() {
        let parsed = read_data_arg(r#"{"name": "Acme"}"#).unwrap();
        assert_eq!(parsed, serde_json::json!({"name": "Acme"}));
    }

    #[test]
    fn reads_json_from_file_argument() {
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        write!(temp_file, r#"{{"name": "Acme"}}"#).unwrap();
        let file_arg = format!("@{}", temp_file.path().display());

        let parsed = read_data_arg(&file_arg).unwrap();
        assert_eq!(parsed, serde_json::json!({"name": "Acme"}));
    }

    #[test]
    fn rejects_invalid_json() {
        let parse_result = read_data_arg("not json");
        assert!(matches!(parse_result, Err(CliError::Usage(_))));
    }

    #[test]
    fn rejects_missing_file() {
        let parse_result = read_data_arg("@/nonexistent");
        assert!(matches!(parse_result, Err(CliError::Usage(_))));
    }

    #[test]
    fn parses_key_value_pairs() {
        let pairs =
            parse_key_value_pairs(&["status=open".to_string(), "priority=1".to_string()]).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("status".to_string(), "open".to_string()),
                ("priority".to_string(), "1".to_string())
            ]
        );
    }

    #[test]
    fn rejects_pair_without_equals() {
        let result = parse_key_value_pairs(&["status".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn parses_header_pairs_trimming_whitespace() {
        let pairs = parse_header_pairs(&["X-Custom-Header:  test-value ".to_string()]).unwrap();
        assert_eq!(
            pairs,
            vec![("X-Custom-Header".to_string(), "test-value".to_string())]
        );
    }

    #[test]
    fn rejects_header_pair_without_colon() {
        let result = parse_header_pairs(&["X-Custom-Header".to_string()]);
        assert!(matches!(result, Err(CliError::Usage(_))));
    }

    #[test]
    fn parse_data_flag_defaults_to_empty_object_when_absent() {
        assert_eq!(parse_data_flag(None).unwrap(), serde_json::json!({}));
    }

    #[test]
    fn parse_data_flag_parses_the_given_json() {
        let parsed = parse_data_flag(Some(r#"{"name": "Acme"}"#)).unwrap();
        assert_eq!(parsed, serde_json::json!({"name": "Acme"}));
    }

    #[test]
    fn merge_flag_overrides_an_existing_key() {
        let mut body = serde_json::json!({"name": "DataName", "multiple_assignees": true});
        merge_flag(&mut body, "name", Some("FlagName".to_string()));
        assert_eq!(
            body,
            serde_json::json!({"name": "FlagName", "multiple_assignees": true})
        );
    }

    #[test]
    fn merge_flag_leaves_body_untouched_when_value_is_none() {
        let mut body = serde_json::json!({"name": "DataName"});
        merge_flag(&mut body, "name", None::<String>);
        assert_eq!(body, serde_json::json!({"name": "DataName"}));
    }

    #[test]
    fn pass_through_returns_the_body_when_present() {
        let response = CuResponse {
            status: 200,
            body: Some(serde_json::json!({"id": "90123"})),
        };
        assert_eq!(pass_through(response), serde_json::json!({"id": "90123"}));
    }

    #[test]
    fn pass_through_falls_back_to_status_when_body_is_empty() {
        let response = CuResponse {
            status: 204,
            body: None,
        };
        assert_eq!(pass_through(response), serde_json::json!({"status": 204}));
    }
}
