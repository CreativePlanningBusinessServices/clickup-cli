mod common;

use clickup_cli::commands::space;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn space_list_passes_archived_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/space"))
        .and(query_param("archived", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"spaces": []})))
        .mount(&server)
        .await;

    let result = space::list(&common::client_for(&server), true)
        .await
        .unwrap();
    assert_eq!(result, json!({"spaces": []}));
}

#[tokio::test]
async fn space_create_merges_name_over_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/space"))
        .and(body_json(
            json!({"name": "FlagName", "multiple_assignees": true}),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "90123", "name": "FlagName"})),
        )
        .mount(&server)
        .await;

    let mut body = json!({"name": "DataName", "multiple_assignees": true});
    clickup_cli::commands::merge_flag(&mut body, "name", Some("FlagName".to_string()));

    let result = space::create(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["name"], "FlagName");
}

#[tokio::test]
async fn get_hits_space_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/90123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "90123"})))
        .mount(&server)
        .await;

    let result = space::get(&common::client_for(&server), "90123")
        .await
        .unwrap();
    assert_eq!(result["id"], "90123");
}

#[tokio::test]
async fn update_puts_to_space_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/space/90123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "90123", "name": "Renamed"})),
        )
        .mount(&server)
        .await;

    let result = space::update(
        &common::client_for(&server),
        "90123",
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
        .and(path("/api/v2/space/90123"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = space::delete(&common::client_for(&server), "90123")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "90123"}));
}
