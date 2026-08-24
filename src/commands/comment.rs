use serde_json::{Value, json};

use crate::client::CuClient;
use crate::commands::{MAX_PAGES, custom_id_query, merge_flag, pass_through};
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

/// Builds the body `comment create` sends: `--text` always merges into `data["comment_text"]`;
/// `--notify-all` merges `data["notify_all"] = true` only when the flag is actually passed, so
/// a bare (unset) `--notify-all` can't clobber a `notify_all` value supplied via `--data`.
pub fn build_create_body(mut data: Value, text: &str, notify_all: bool) -> Value {
    merge_flag(&mut data, "comment_text", Some(text.to_string()));
    if notify_all {
        merge_flag(&mut data, "notify_all", Some(true));
    }
    data
}

/// Builds the body `comment update` sends: `--text`/`--resolved` merge into
/// `data["comment_text"]`/`data["resolved"]` only when present.
pub fn build_update_body(mut data: Value, text: Option<&str>, resolved: Option<bool>) -> Value {
    merge_flag(&mut data, "comment_text", text.map(str::to_string));
    merge_flag(&mut data, "resolved", resolved);
    data
}

/// Builds the body `comment reply` sends: `--text` merges into `data["comment_text"]`.
pub fn build_reply_body(mut data: Value, text: &str) -> Value {
    merge_flag(&mut data, "comment_text", Some(text.to_string()));
    data
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

    #[test]
    fn build_create_body_merges_text_and_sets_notify_all_true_when_flag_passed() {
        let body = build_create_body(json!({}), "Deployed.", true);
        assert_eq!(
            body,
            json!({"comment_text": "Deployed.", "notify_all": true})
        );
    }

    #[test]
    fn build_create_body_omits_notify_all_when_flag_absent() {
        let body = build_create_body(json!({}), "Deployed.", false);
        assert_eq!(body, json!({"comment_text": "Deployed."}));
    }

    #[test]
    fn build_create_body_does_not_clobber_data_notify_all_when_flag_absent() {
        // The `--notify-all` conditional (`if notify_all { merge_flag(...) }`) exists
        // precisely so an unset flag can't overwrite a `notify_all` value already present in
        // `--data` with `Some(false)`.
        let body = build_create_body(json!({"notify_all": true}), "Deployed.", false);
        assert_eq!(
            body,
            json!({"comment_text": "Deployed.", "notify_all": true})
        );
    }

    #[test]
    fn build_create_body_text_flag_overrides_data_comment_text() {
        let body = build_create_body(json!({"comment_text": "from data"}), "from flag", false);
        assert_eq!(body, json!({"comment_text": "from flag"}));
    }

    #[test]
    fn build_update_body_merges_only_present_fields() {
        let body = build_update_body(json!({}), Some("edited"), Some(true));
        assert_eq!(body, json!({"comment_text": "edited", "resolved": true}));

        let body = build_update_body(json!({"resolved": false}), None, None);
        assert_eq!(body, json!({"resolved": false}));
    }

    #[test]
    fn build_reply_body_merges_text() {
        let body = build_reply_body(json!({}), "thanks");
        assert_eq!(body, json!({"comment_text": "thanks"}));
    }
}
