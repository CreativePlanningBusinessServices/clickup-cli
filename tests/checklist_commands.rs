mod common;

use clickup_cli::commands::checklist;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn create_posts_name_to_task_checklist_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/868k5a5wa/checklist"))
        .and(body_json(json!({"name": "QA steps"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "cl1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::create(&common::client_for(&server), "868k5a5wa", false, "QA steps")
        .await
        .unwrap();
    assert_eq!(result["id"], "cl1");
}

#[tokio::test]
async fn create_with_custom_id_adds_custom_task_ids_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/DEV-123/checklist"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "9001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "cl1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::create(&common::client_for(&server), "DEV-123", true, "QA steps")
        .await
        .unwrap();
    assert_eq!(result["id"], "cl1");
}

#[tokio::test]
async fn update_puts_to_checklist_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/checklist/cl1"))
        .and(body_json(json!({"name": "Renamed"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "cl1", "name": "Renamed"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::update(
        &common::client_for(&server),
        "cl1",
        json!({"name": "Renamed"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Renamed");
}

#[tokio::test]
async fn delete_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/checklist/cl1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::delete(&common::client_for(&server), "cl1")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "cl1"}));
}

#[tokio::test]
async fn item_add_posts_to_checklist_item_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/checklist/cl1/checklist_item"))
        .and(body_json(
            json!({"name": "run tests", "assignee": 75413867}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "item1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::item_add(
        &common::client_for(&server),
        "cl1",
        json!({"name": "run tests", "assignee": 75413867}),
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "item1");
}

#[tokio::test]
async fn item_update_hits_nested_item_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/checklist/cl1/checklist_item/item1"))
        .and(body_json(json!({"resolved": true})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "item1", "resolved": true})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::item_update(
        &common::client_for(&server),
        "cl1",
        "item1",
        json!({"resolved": true}),
    )
    .await
    .unwrap();
    assert_eq!(result["resolved"], true);
}

#[tokio::test]
async fn item_remove_deletes_nested_item_path_and_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/checklist/cl1/checklist_item/item1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = checklist::item_remove(&common::client_for(&server), "cl1", "item1")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "item1"}));
}
