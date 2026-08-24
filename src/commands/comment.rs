use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{MAX_PAGES, custom_id_query, pass_through};
use crate::error::CliError;

/// Exactly one of `--task`/`--list`/`--view` (enforced by a clap `ArgGroup` on every CLI verb
/// that builds one of these) selects which ClickUp comment thread a `list`/`create` call
/// targets.
pub enum CommentLocation<'a> {
    Task(&'a str),
    List(&'a str),
    View(&'a str),
}

impl CommentLocation<'_> {
    fn path(&self) -> String {
        match self {
            CommentLocation::Task(task_id) => format!("/api/v2/task/{task_id}/comment"),
            CommentLocation::List(list_id) => format!("/api/v2/list/{list_id}/comment"),
            CommentLocation::View(view_id) => format!("/api/v2/view/{view_id}/comment"),
        }
    }
}

/// Builds a `CommentLocation` from the CLI's `--task`/`--list`/`--view` options. The clap
/// `ArgGroup` on every verb that calls this already guarantees exactly one is `Some`, so the
/// fallback error only guards a caller that skips the CLI layer entirely.
pub fn location_from<'a>(
    task_id: Option<&'a str>,
    list_id: Option<&'a str>,
    view_id: Option<&'a str>,
) -> Result<CommentLocation<'a>, CliError> {
    match (task_id, list_id, view_id) {
        (Some(task_id), None, None) => Ok(CommentLocation::Task(task_id)),
        (None, Some(list_id), None) => Ok(CommentLocation::List(list_id)),
        (None, None, Some(view_id)) => Ok(CommentLocation::View(view_id)),
        _ => Err(CliError::Usage(
            "exactly one of --task, --list, or --view is required".into(),
        )),
    }
}

/// Lists comments newest-first, 25 per page. Without `fetch_all`, issues a single request and
/// passes its body through unchanged. With `fetch_all`, loops, merging each page's `"comments"`
/// array into one envelope: `{"items": [...], "count": n, "hasMore": bool}`. Unlike
/// `merge_task_pages`/`merge_cursor_pages`, ClickUp comment pagination isn't `page=`/`cursor=`
/// based — page 2+ requires both `start` (the previous page's last comment's `date`) and
/// `start_id` (that comment's `id`) as query params. A page stops the loop when it's empty or
/// has fewer than 25 items — ClickUp's fixed page size — since there's no `hasMore`/`last_page`
/// flag in the response to key off of.
pub async fn list(
    client: &CuClient,
    location: &CommentLocation<'_>,
    custom_id: bool,
    fetch_all: bool,
) -> Result<Value, CliError> {
    let path = location.path();
    let base_query = custom_id_query(client, custom_id)?;

    if !fetch_all {
        let response = client
            .request(reqwest::Method::GET, &path, &base_query, None)
            .await?;
        return Ok(pass_through(response));
    }

    let mut items: Vec<Value> = Vec::new();
    let mut has_more = false;
    let mut start: Option<(String, String)> = None;
    for page_number in 0..MAX_PAGES {
        let mut query = base_query.clone();
        if let Some((date, id)) = &start {
            query.push(("start".to_string(), date.clone()));
            query.push(("start_id".to_string(), id.clone()));
        }
        let response = client
            .request(reqwest::Method::GET, &path, &query, None)
            .await?;
        let body = pass_through(response);
        let page_comments = body["comments"].as_array().cloned().unwrap_or_default();
        let page_len = page_comments.len();
        let last_comment = page_comments.last().cloned();
        items.extend(page_comments);

        if page_len < 25 {
            break;
        }
        let Some(last_comment) = last_comment else {
            break;
        };
        let date = last_comment["date"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let id = last_comment["id"].as_str().unwrap_or_default().to_string();
        start = Some((date, id));
        if page_number + 1 == MAX_PAGES {
            has_more = true;
        }
    }

    Ok(json!({"items": items, "count": items.len(), "hasMore": has_more}))
}

pub async fn create(
    client: &CuClient,
    location: &CommentLocation<'_>,
    custom_id: bool,
    data: Value,
) -> Result<Value, CliError> {
    let path = location.path();
    let query = custom_id_query(client, custom_id)?;
    let response = client
        .request(reqwest::Method::POST, &path, &query, Some(data))
        .await?;
    Ok(pass_through(response))
}

pub async fn update(client: &CuClient, comment_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/comment/{comment_id}");
    let response = client
        .request(reqwest::Method::PUT, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

/// ClickUp's delete returns 204 with no body; synthesize an acknowledgement instead of
/// surfacing the bare status code.
pub async fn delete(client: &CuClient, comment_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/comment/{comment_id}");
    client
        .request(reqwest::Method::DELETE, &path, &[], None)
        .await?;
    Ok(json!({"deleted": true, "id": comment_id}))
}

pub async fn replies(client: &CuClient, comment_id: &str) -> Result<Value, CliError> {
    let path = format!("/api/v2/comment/{comment_id}/reply");
    let response = client
        .request(reqwest::Method::GET, &path, &[], None)
        .await?;
    Ok(pass_through(response))
}

pub async fn reply(client: &CuClient, comment_id: &str, data: Value) -> Result<Value, CliError> {
    let path = format!("/api/v2/comment/{comment_id}/reply");
    let response = client
        .request(reqwest::Method::POST, &path, &[], Some(data))
        .await?;
    Ok(pass_through(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_from_requires_exactly_one() {
        assert!(location_from(None, None, None).is_err());
        assert!(location_from(Some("t1"), Some("l1"), None).is_err());
        assert!(matches!(
            location_from(Some("t1"), None, None),
            Ok(CommentLocation::Task("t1"))
        ));
        assert!(matches!(
            location_from(None, Some("l1"), None),
            Ok(CommentLocation::List("l1"))
        ));
        assert!(matches!(
            location_from(None, None, Some("v1")),
            Ok(CommentLocation::View("v1"))
        ));
    }
}
