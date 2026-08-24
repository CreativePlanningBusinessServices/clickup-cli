mod common;

use clickup_cli::commands::tag;
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn tag_list_hits_space_tag_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/space/790/tag"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"tags": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = tag::list(&common::client_for(&server), "790")
        .await
        .unwrap();
    assert_eq!(result["tags"], json!([]));
}

#[tokio::test]
async fn tag_create_nests_tag_object() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/space/790/tag"))
        .and(body_json(json!({
            "tag": {"name": "urgent", "tag_fg": "#ffffff", "tag_bg": "#ff0000"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "urgent"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = tag::create(
        &common::client_for(&server),
        "790",
        "urgent",
        Some("#ffffff"),
        Some("#ff0000"),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "urgent");
}

#[tokio::test]
async fn tag_create_without_colors_omits_them() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/space/790/tag"))
        .and(body_json(json!({"tag": {"name": "urgent"}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "urgent"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = tag::create(&common::client_for(&server), "790", "urgent", None, None)
        .await
        .unwrap();
    assert_eq!(result["name"], "urgent");
}

/// Tag names can contain spaces (e.g. "high priority"); `update`/`delete` must percent-encode
/// the name before splicing it into the URL path.
#[tokio::test]
async fn tag_update_hits_name_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/space/790/tag/high%20priority"))
        .and(body_json(json!({"tag": {"name": "blocked"}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "blocked"})))
        .expect(1)
        .mount(&server)
        .await;

    let result = tag::update(
        &common::client_for(&server),
        "790",
        "high priority",
        json!({"name": "blocked"}),
    )
    .await
    .unwrap();
    assert_eq!(result["name"], "blocked");
}

#[tokio::test]
async fn tag_delete_hits_encoded_name_path_and_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/space/790/tag/high%20priority"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = tag::delete(&common::client_for(&server), "790", "high priority")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "name": "high priority"}));
}
