mod common;

use clickup_cli::commands::workspace;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn workspace_seats_hits_team_scoped_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/seats"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"members": {"filled_members_seats": 3}})),
        )
        .mount(&server)
        .await;

    let result = workspace::seats(&common::client_for(&server))
        .await
        .unwrap();
    assert_eq!(result["members"]["filled_members_seats"], 3);
}

#[tokio::test]
async fn list_hits_team_path_without_workspace_scoping() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"teams": []})))
        .mount(&server)
        .await;

    let result = workspace::list(&common::client_for(&server)).await.unwrap();
    assert_eq!(result, json!({"teams": []}));
}

#[tokio::test]
async fn plan_hits_team_scoped_plan_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/plan"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"plan_name": "Business"})))
        .mount(&server)
        .await;

    let result = workspace::plan(&common::client_for(&server)).await.unwrap();
    assert_eq!(result["plan_name"], "Business");
}

#[tokio::test]
async fn shared_hits_team_scoped_shared_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/shared"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"shared": {"lists": []}})))
        .mount(&server)
        .await;

    let result = workspace::shared(&common::client_for(&server))
        .await
        .unwrap();
    assert_eq!(result, json!({"shared": {"lists": []}}));
}

#[tokio::test]
async fn task_types_hits_custom_item_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/custom_item"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"custom_items": []})))
        .mount(&server)
        .await;

    let result = workspace::task_types(&common::client_for(&server))
        .await
        .unwrap();
    assert_eq!(result, json!({"custom_items": []}));
}
