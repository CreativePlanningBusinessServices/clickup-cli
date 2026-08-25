mod common;

use clickup_cli::commands::list;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_via_folder_and_via_space_hit_different_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/folder/457/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"lists": [{"id": "124"}]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/790/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"lists": [{"id": "125"}]})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let via_folder = list::list(&client, Some("457"), None, false).await.unwrap();
    assert_eq!(via_folder["lists"][0]["id"], "124");

    let via_space = list::list(&client, None, Some("790"), false).await.unwrap();
    assert_eq!(via_space["lists"][0]["id"], "125");
}

#[tokio::test]
async fn create_folderless_posts_to_space_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/space/790/list"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "124", "name": "Sprint 12"})),
        )
        .mount(&server)
        .await;

    let result = list::create(
        &common::client_for(&server),
        None,
        Some("790"),
        json!({"name": "Sprint 12"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Sprint 12");
}

#[tokio::test]
async fn add_task_posts_and_synthesizes_ok() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/list/124/task/abc123"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let result = list::add_task(&common::client_for(&server), "124", "abc123")
        .await
        .unwrap();
    assert_eq!(
        result,
        json!({"ok": true, "listId": "124", "taskId": "abc123"})
    );
}

#[tokio::test]
async fn remove_task_deletes_and_synthesizes_ok() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/list/124/task/abc123"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let result = list::remove_task(&common::client_for(&server), "124", "abc123")
        .await
        .unwrap();
    assert_eq!(
        result,
        json!({"ok": true, "listId": "124", "taskId": "abc123"})
    );
}

#[tokio::test]
async fn get_hits_list_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "124"})))
        .mount(&server)
        .await;

    let result = list::get(&common::client_for(&server), "124")
        .await
        .unwrap();
    assert_eq!(result["id"], "124");
}

#[tokio::test]
async fn members_hits_member_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/member"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"members": []})))
        .mount(&server)
        .await;

    let result = list::members(&common::client_for(&server), "124")
        .await
        .unwrap();
    assert_eq!(result, json!({"members": []}));
}

#[tokio::test]
async fn from_template_posts_to_folder_template_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/folder/457/list_template/tmpl_1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "124", "name": "Sprint 12"})),
        )
        .mount(&server)
        .await;

    let result = list::from_template(
        &common::client_for(&server),
        Some("457"),
        None,
        "tmpl_1",
        json!({"name": "Sprint 12"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Sprint 12");
}

#[tokio::test]
async fn update_puts_to_list_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/list/124"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "124", "name": "Renamed"})),
        )
        .mount(&server)
        .await;

    let result = list::update(
        &common::client_for(&server),
        "124",
        json!({"name": "Renamed"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "Renamed");
}

#[tokio::test]
async fn delete_synthesizes_ack_on_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/list/124"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = list::delete(&common::client_for(&server), "124")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "124"}));
}
