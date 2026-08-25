mod common;

use clickup_cli::commands::folder;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn folder_get_passes_include_subfolders() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/folder/457"))
        .and(query_param("include_subfolders", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "457"})))
        .mount(&server)
        .await;

    let result = folder::get(&common::client_for(&server), "457", true)
        .await
        .unwrap();
    assert_eq!(result["id"], "457");
}

#[tokio::test]
async fn folder_delete_returns_synthesized_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/folder/457"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = folder::delete(&common::client_for(&server), "457")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "457"}));
}

#[tokio::test]
async fn folder_from_template_posts_to_template_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/space/90123/folder_template/tmpl_1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "457", "name": "Sprint 12"})),
        )
        .mount(&server)
        .await;

    let result = folder::from_template(
        &common::client_for(&server),
        "90123",
        "tmpl_1",
        json!({"name": "Sprint 12"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Sprint 12");
}

#[tokio::test]
async fn list_passes_archived_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/90123/folder"))
        .and(query_param("archived", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"folders": []})))
        .mount(&server)
        .await;

    let result = folder::list(&common::client_for(&server), "90123", false)
        .await
        .unwrap();
    assert_eq!(result, json!({"folders": []}));
}

#[tokio::test]
async fn create_posts_to_space_folder_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/space/90123/folder"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "457", "name": "Sprints"})),
        )
        .mount(&server)
        .await;

    let result = folder::create(
        &common::client_for(&server),
        "90123",
        json!({"name": "Sprints"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Sprints");
}

#[tokio::test]
async fn update_puts_to_folder_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/folder/457"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "457", "name": "Renamed"})),
        )
        .mount(&server)
        .await;

    let result = folder::update(
        &common::client_for(&server),
        "457",
        json!({"name": "Renamed"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Renamed");
}

#[tokio::test]
async fn move_folder_puts_to_position_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/folder/457/position"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "457", "space": {"id": "123"}})),
        )
        .mount(&server)
        .await;

    let result = folder::move_folder(
        &common::client_for(&server),
        "457",
        json!({"space_id": "123"}),
    )
    .await
    .unwrap();
    assert_eq!(result["space"]["id"], "123");
}
