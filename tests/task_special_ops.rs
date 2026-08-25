mod common;

use std::io::Write;

use clickup_cli::commands::task;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn move_hits_v3_home_list_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v3/workspaces/9001/tasks/abc/home_list/125"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "abc"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::move_task(&common::client_for(&server), "abc", "125")
        .await
        .unwrap();
    assert_eq!(result["id"], "abc");
}

#[tokio::test]
async fn merge_posts_source_ids() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/target1/merge"))
        .and(body_json(
            json!({"source_task_ids": ["source1", "source2"]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "target1"})))
        .expect(1)
        .mount(&server)
        .await;

    let source_ids = vec!["source1".to_string(), "source2".to_string()];
    let result = task::merge(&common::client_for(&server), "target1", &source_ids)
        .await
        .unwrap();
    assert_eq!(result["id"], "target1");
}

#[tokio::test]
async fn time_in_status_uses_single_path_for_one_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/task1/time_in_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status_history": []})))
        .expect(1)
        .mount(&server)
        .await;

    let task_ids = vec!["task1".to_string()];
    let result = task::time_in_status(&common::client_for(&server), &task_ids, false)
        .await
        .unwrap();
    assert_eq!(result["status_history"], json!([]));
}

#[tokio::test]
async fn time_in_status_switches_to_bulk_for_multiple_ids() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/bulk_time_in_status/task_ids"))
        .and(query_param("task_ids", "a"))
        .and(query_param("task_ids", "b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"a": {}, "b": {}})))
        .expect(1)
        .mount(&server)
        .await;

    let task_ids = vec!["a".to_string(), "b".to_string()];
    let result = task::time_in_status(&common::client_for(&server), &task_ids, false)
        .await
        .unwrap();
    assert!(result.get("a").is_some());
}

#[tokio::test]
async fn dependency_delete_passes_query_params() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc123/dependency"))
        .and(query_param("depends_on", "other1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"deleted": true})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::dependency_remove(
        &common::client_for(&server),
        "abc123",
        Some("other1"),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["deleted"], true);
}

#[tokio::test]
async fn attach_uploads_multipart_field_named_attachment() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/abc123/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "att1"})))
        .expect(1)
        .mount(&server)
        .await;

    let mut temp_file = tempfile::NamedTempFile::new().unwrap();
    write!(temp_file, "report contents").unwrap();

    let result = task::attach(
        &common::client_for(&server),
        "abc123",
        temp_file.path(),
        None,
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "att1");

    let received_requests = server.received_requests().await.unwrap();
    assert_eq!(received_requests.len(), 1);
    let request = &received_requests[0];
    let content_type = request
        .headers
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("multipart/form-data"));
    assert!(String::from_utf8_lossy(&request.body).contains("name=\"attachment\""));
}

/// Tag names can contain spaces (e.g. "high priority") or a `/`; `tag add`/`tag remove` must
/// percent-encode the name before splicing it into the URL path.
#[tokio::test]
async fn tag_add_hits_encoded_name_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/abc123/tag/high%20priority"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"tag": "high priority"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::tag_add(
        &common::client_for(&server),
        "abc123",
        "high priority",
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["tag"], "high priority");
}

#[tokio::test]
async fn tag_add_encodes_a_slash_in_the_tag_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/abc123/tag/a%2Fb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"tag": "a/b"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::tag_add(&common::client_for(&server), "abc123", "a/b", false)
        .await
        .unwrap();
    assert_eq!(result["tag"], "a/b");
}

#[tokio::test]
async fn tag_remove_hits_encoded_name_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc123/tag/high%20priority"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::tag_remove(
        &common::client_for(&server),
        "abc123",
        "high priority",
        false,
    )
    .await
    .unwrap();
    assert_eq!(result, json!({"status": 204}));
}

#[tokio::test]
async fn tag_remove_encodes_a_slash_in_the_tag_name() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc123/tag/a%2Fb"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::tag_remove(&common::client_for(&server), "abc123", "a/b", false)
        .await
        .unwrap();
    assert_eq!(result, json!({"status": 204}));
}

#[tokio::test]
async fn attachments_all_follows_next_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v3/workspaces/9001/attachments/abc123/attachments",
        ))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "attachments": [{"id": "att1"}],
            "next_cursor": "c2",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/api/v3/workspaces/9001/attachments/abc123/attachments",
        ))
        .and(query_param("cursor", "c2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "attachments": [{"id": "att2"}],
            "next_cursor": null,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::attachments(&common::client_for(&server), "abc123", None, true)
        .await
        .unwrap();

    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
}
