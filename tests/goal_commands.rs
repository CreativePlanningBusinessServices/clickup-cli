mod common;

use clickup_cli::commands::goal;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn goal_list_hits_team_path_with_include_completed_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/goal"))
        .and(query_param("include_completed", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"goals": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::list(&common::client_for(&server), true)
        .await
        .unwrap();
    assert_eq!(result["goals"], json!([]));
}

#[tokio::test]
async fn goal_create_posts_to_team_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/goal"))
        .and(body_json(
            json!({"name": "Q3 targets", "due_date": 1756684800000i64}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"goal": {"id": "g1"}})))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::create(
        &common::client_for(&server),
        json!({"name": "Q3 targets", "due_date": 1756684800000i64}),
    )
    .await
    .unwrap();
    assert_eq!(result["goal"]["id"], "g1");
}

#[tokio::test]
async fn goal_get_hits_goal_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/goal/g1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"goal": {"id": "g1"}})))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::get(&common::client_for(&server), "g1").await.unwrap();
    assert_eq!(result["goal"]["id"], "g1");
}

#[tokio::test]
async fn goal_update_hits_goal_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/goal/g1"))
        .and(body_json(json!({"name": "Renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"goal": {"id": "g1"}})))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::update(
        &common::client_for(&server),
        "g1",
        json!({"name": "Renamed"}),
    )
    .await
    .unwrap();
    assert_eq!(result["goal"]["id"], "g1");
}

#[tokio::test]
async fn goal_delete_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/goal/g1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::delete(&common::client_for(&server), "g1")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "g1"}));
}

#[tokio::test]
async fn key_result_add_hits_goal_key_result_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/goal/g1/key_result"))
        .and(body_json(json!({"name": "Ship it"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"key_result": {"id": "kr1"}})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::key_result_add(
        &common::client_for(&server),
        "g1",
        json!({"name": "Ship it"}),
    )
    .await
    .unwrap();
    assert_eq!(result["key_result"]["id"], "kr1");
}

#[tokio::test]
async fn key_result_update_hits_key_result_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/key_result/kr1"))
        .and(body_json(json!({"steps_current": 5})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"key_result": {"id": "kr1"}})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::key_result_update(
        &common::client_for(&server),
        "kr1",
        json!({"steps_current": 5}),
    )
    .await
    .unwrap();
    assert_eq!(result["key_result"]["id"], "kr1");
}

#[tokio::test]
async fn key_result_remove_hits_key_result_path_and_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/key_result/kr1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = goal::key_result_remove(&common::client_for(&server), "kr1")
        .await
        .unwrap();
    assert_eq!(result, json!({"deleted": true, "id": "kr1"}));
}
