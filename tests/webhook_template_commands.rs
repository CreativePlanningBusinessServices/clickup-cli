mod common;

use clickup_cli::commands::template::TemplateKind;
use clickup_cli::commands::{template, webhook};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn webhook_list_hits_team_webhook_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/webhook"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"webhooks": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = webhook::list(&common::client_for(&server)).await.unwrap();
    assert_eq!(result["webhooks"], json!([]));
}

#[tokio::test]
async fn webhook_create_merges_endpoint_and_events() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/webhook"))
        .and(body_json(json!({
            "endpoint": "https://x.example/hook",
            "events": ["taskUpdated", "taskDeleted"],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "webhook": {"id": "wh1", "secret": "shh"},
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = webhook::create(
        &common::client_for(&server),
        "https://x.example/hook",
        &["taskUpdated".to_string(), "taskDeleted".to_string()],
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(result["webhook"]["secret"], "shh");
}

#[tokio::test]
async fn webhook_create_star_event_passes_through() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/webhook"))
        .and(body_json(json!({
            "endpoint": "https://x.example/hook",
            "events": ["*"],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "wh1"})))
        .expect(1)
        .mount(&server)
        .await;

    webhook::create(
        &common::client_for(&server),
        "https://x.example/hook",
        &["*".to_string()],
        json!({}),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn webhook_update_and_delete_hit_webhook_id_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/webhook/wh1"))
        .and(body_json(json!({"status": "active"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "wh1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/webhook/wh1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    webhook::update(&client, "wh1", json!({"status": "active"}))
        .await
        .unwrap();
    let delete_result = webhook::delete(&client, "wh1").await.unwrap();
    assert_eq!(delete_result, json!({"deleted": true, "webhook_id": "wh1"}));
}

#[tokio::test]
async fn template_list_defaults_page_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/taskTemplate"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"templates": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = template::list(&common::client_for(&server), TemplateKind::Task, None)
        .await
        .unwrap();
    assert_eq!(result["templates"], json!([]));
}

#[tokio::test]
async fn template_list_passes_page_and_kind_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/list_template"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"templates": []})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/folder_template"))
        .and(query_param("page", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"templates": []})))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    template::list(&client, TemplateKind::List, Some(2))
        .await
        .unwrap();
    template::list(&client, TemplateKind::Folder, None)
        .await
        .unwrap();
}
