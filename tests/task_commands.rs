mod common;

use clickup_cli::commands::task::{self, TaskListArgs};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn empty_list_args() -> TaskListArgs {
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

fn stub_tasks(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| json!({"id": format!("task-{index}")}))
        .collect()
}

#[tokio::test]
async fn get_with_custom_id_sends_team_id_pair() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/task/DEV-123"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "9001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "abc123"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::get(&common::client_for(&server), "DEV-123", true, false, false)
        .await
        .unwrap();
    assert_eq!(result["id"], "abc123");
}

#[tokio::test]
async fn create_posts_to_list_path_with_merged_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/list/124/task"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "abc123", "name": "Fix login"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut body = json!({});
    clickup_cli::commands::merge_flag(&mut body, "name", Some("Fix login".to_string()));

    let result = task::create(&common::client_for(&server), "124", body)
        .await
        .unwrap();
    assert_eq!(result["name"], "Fix login");
}

#[tokio::test]
async fn list_all_merges_pages_until_last_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": stub_tasks(100),
            "last_page": false,
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/task"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": stub_tasks(1),
            "last_page": true,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::list(
        &common::client_for(&server),
        "124",
        &empty_list_args(),
        None,
        true,
    )
    .await
    .unwrap();

    assert_eq!(result["count"], 101);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 101);
}

#[tokio::test]
async fn filter_all_stops_on_short_page_without_last_page_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": stub_tasks(3),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::filter(&common::client_for(&server), &empty_list_args(), None, true)
        .await
        .unwrap();

    assert_eq!(result["count"], 3);
    assert_eq!(result["hasMore"], false);
}

#[tokio::test]
async fn filter_hits_team_scoped_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"tasks": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::filter(
        &common::client_for(&server),
        &empty_list_args(),
        Some(0),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result["tasks"], json!([]));
}

#[tokio::test]
async fn delete_returns_synthesized_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc123"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = task::delete(&common::client_for(&server), "abc123", false)
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "abc123"}));
}
