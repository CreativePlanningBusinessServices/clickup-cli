mod common;

use clickup_cli::commands::comment::{self, CommentLocation, build_create_body};
use serde_json::{Value, json};
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 25 comments, newest-first; the last one is `{"id":"c25","date":"1712345"}` — the value the
/// second page's `start`/`start_id` query params must carry.
fn page_one_comments() -> Vec<Value> {
    let mut comments: Vec<Value> = (1..25)
        .map(|n| json!({"id": format!("c{n}"), "date": format!("17123{n:02}")}))
        .collect();
    comments.push(json!({"id": "c25", "date": "1712345"}));
    comments
}

#[tokio::test]
async fn list_all_pages_with_start_and_start_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/868k5a5wa/comment"))
        .and(query_param_is_missing("start"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"comments": page_one_comments()})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/868k5a5wa/comment"))
        .and(query_param("start", "1712345"))
        .and(query_param("start_id", "c25"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"comments": [
                {"id": "c26", "date": "1712346"},
                {"id": "c27", "date": "1712347"},
                {"id": "c28", "date": "1712348"},
            ]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let location = CommentLocation::Task("868k5a5wa");
    let result = comment::list(&client, &location, false, true)
        .await
        .unwrap();

    assert_eq!(result["count"], 28);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 28);
}

/// Drives `create` through `build_create_body` — the same merge function `dispatch_comment`
/// calls in `cli.rs` — rather than a pre-merged body, so this test actually exercises the
/// `--text`/`--notify-all` merge logic instead of just `create`'s passthrough-to-HTTP behavior.
#[tokio::test]
async fn create_merges_text_and_notify_flags() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/868k5a5wa/comment"))
        .and(body_json(
            json!({"comment_text": "Deployed.", "notify_all": true}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let location = CommentLocation::Task("868k5a5wa");
    let body = build_create_body(json!({}), "Deployed.", true);
    let result = comment::create(&client, &location, false, body)
        .await
        .unwrap();
    assert_eq!(result["id"], "456");
}

#[tokio::test]
async fn view_location_hits_view_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/view/vw1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"comments": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let location = CommentLocation::View("vw1");
    let result = comment::list(&client, &location, false, false)
        .await
        .unwrap();
    assert_eq!(result["comments"], json!([]));
}

#[tokio::test]
async fn update_and_delete_hit_comment_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/comment/456"))
        .and(body_json(json!({"comment_text": "edited"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "456"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/comment/456"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let update_result = comment::update(&client, "456", json!({"comment_text": "edited"}))
        .await
        .unwrap();
    assert_eq!(update_result["id"], "456");

    let delete_result = comment::delete(&client, "456").await.unwrap();
    assert_eq!(delete_result, json!({"deleted": true, "id": "456"}));
}

#[tokio::test]
async fn replies_and_reply_hit_comment_reply_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/comment/456/reply"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"replies": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/comment/456/reply"))
        .and(body_json(json!({"comment_text": "thanks"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "789"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let replies_result = comment::replies(&client, "456").await.unwrap();
    assert_eq!(replies_result["replies"], json!([]));

    let reply_result = comment::reply(&client, "456", json!({"comment_text": "thanks"}))
        .await
        .unwrap();
    assert_eq!(reply_result["id"], "789");
}

#[tokio::test]
async fn list_via_list_location_hits_list_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"comments": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let location = CommentLocation::List("124");
    let result = comment::list(&client, &location, false, false)
        .await
        .unwrap();
    assert_eq!(result["comments"], json!([]));
}

#[tokio::test]
async fn list_with_custom_id_adds_custom_task_ids_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/DEV-123/comment"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "9001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"comments": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let location = CommentLocation::Task("DEV-123");
    let result = comment::list(&client, &location, true, false)
        .await
        .unwrap();
    assert_eq!(result["comments"], json!([]));
}
