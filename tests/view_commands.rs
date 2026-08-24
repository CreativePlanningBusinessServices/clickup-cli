mod common;

use clickup_cli::commands::view::{self, ViewLocation};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn view_list_hits_all_four_location_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"views": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/790/view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"views": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/folder/457/view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"views": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/view"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"views": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    view::list(&client, &ViewLocation::Workspace).await.unwrap();
    view::list(&client, &ViewLocation::Space("790"))
        .await
        .unwrap();
    view::list(&client, &ViewLocation::Folder("457"))
        .await
        .unwrap();
    view::list(&client, &ViewLocation::List("124"))
        .await
        .unwrap();
}

#[tokio::test]
async fn view_create_hits_the_same_four_location_paths() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/list/124/view"))
        .and(body_json(json!({"name": "Board"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "vw1"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = view::create(
        &client,
        &ViewLocation::List("124"),
        json!({"name": "Board"}),
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "vw1");
}

#[tokio::test]
async fn view_get_hits_view_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/view/vw1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "vw1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = view::get(&common::client_for(&server), "vw1")
        .await
        .unwrap();
    assert_eq!(result["id"], "vw1");
}

#[tokio::test]
async fn view_update_hits_view_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/view/vw1"))
        .and(body_json(json!({"name": "Renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "vw1"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = view::update(
        &common::client_for(&server),
        "vw1",
        json!({"name": "Renamed"}),
    )
    .await
    .unwrap();
    assert_eq!(result["id"], "vw1");
}

#[tokio::test]
async fn view_delete_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/view/vw1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = view::delete(&common::client_for(&server), "vw1")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "vw1"}));
}

#[tokio::test]
async fn view_tasks_defaults_page_zero_and_merges_all() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/view/vw1/task"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": [{"id": "task-0"}],
            "last_page": false,
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/view/vw1/task"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": [{"id": "task-1"}],
            "last_page": true,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = view::tasks(&common::client_for(&server), "vw1", None, true)
        .await
        .unwrap();

    assert_eq!(result["count"], 2);
    assert_eq!(result["hasMore"], false);
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn view_tasks_without_all_fetches_a_single_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/view/vw1/task"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tasks": [{"id": "task-2"}],
            "last_page": false,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = view::tasks(&common::client_for(&server), "vw1", Some(2), false)
        .await
        .unwrap();
    assert_eq!(result["tasks"].as_array().unwrap().len(), 1);
}
