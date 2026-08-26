mod common;

use clickup_cli::commands::field::{self, FieldLocation};
use clickup_cli::commands::read_value_arg;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn field_list_workspace_uses_team_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = field::list(&client, &FieldLocation::Workspace)
        .await
        .unwrap();
    assert_eq!(result["fields"], json!([]));
}

#[tokio::test]
async fn field_list_hits_list_folder_and_space_paths() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/list/124/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/folder/457/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/790/field"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fields": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    field::list(&client, &FieldLocation::List("124"))
        .await
        .unwrap();
    field::list(&client, &FieldLocation::Folder("457"))
        .await
        .unwrap();
    field::list(&client, &FieldLocation::Space("790"))
        .await
        .unwrap();
}

/// A bare word (no quotes, fails JSON parsing) becomes a plain JSON string — the whole point of
/// `read_value_arg` is that agents don't need to double-quote simple values like "hello".
/// ClickUp answers `field set` with a bare `{}`, indistinguishable from a no-op, so `set`
/// synthesizes an acknowledgement the way `remove` does.
#[tokio::test]
async fn field_set_wraps_value_and_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/868k5a5wa/field/FIELD_UUID"))
        .and(body_json(json!({"value": "hello"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let value = read_value_arg("hello").unwrap();
    let result = field::set(&client, "868k5a5wa", "FIELD_UUID", false, value, None)
        .await
        .unwrap();
    assert_eq!(
        result,
        json!({"set": true, "task_id": "868k5a5wa", "field_id": "FIELD_UUID"})
    );
}

#[tokio::test]
async fn field_set_passes_object_values_through() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/868k5a5wa/field/FIELD_UUID"))
        .and(body_json(json!({"value": {"add": ["uuid1"]}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let value = read_value_arg(r#"{"add":["uuid1"]}"#).unwrap();
    let result = field::set(&client, "868k5a5wa", "FIELD_UUID", false, value, None)
        .await
        .unwrap();
    assert_eq!(result["set"], true);
}

#[tokio::test]
async fn field_set_with_options_adds_value_options() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/868k5a5wa/field/FIELD_UUID"))
        .and(body_json(
            json!({"value": 5, "value_options": {"time": true}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = field::set(
        &client,
        "868k5a5wa",
        "FIELD_UUID",
        false,
        json!(5),
        Some(json!({"time": true})),
    )
    .await
    .unwrap();
    assert_eq!(result["set"], true);
}

#[tokio::test]
async fn field_set_with_custom_id_adds_custom_task_ids_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/task/DEV-123/field/FIELD_UUID"))
        .and(query_param("custom_task_ids", "true"))
        .and(query_param("team_id", "9001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = field::set(
        &client,
        "DEV-123",
        "FIELD_UUID",
        true,
        json!("urgent"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        result,
        json!({"set": true, "task_id": "DEV-123", "field_id": "FIELD_UUID"})
    );
}

#[tokio::test]
async fn field_remove_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/868k5a5wa/field/FIELD_UUID"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = field::remove(&client, "868k5a5wa", "FIELD_UUID", false)
        .await
        .unwrap();
    assert_eq!(
        result,
        json!({"removed": true, "task_id": "868k5a5wa", "field_id": "FIELD_UUID"})
    );
}
