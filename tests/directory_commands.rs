mod common;

use clickup_cli::commands::{group, user};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn user_get_hits_team_user_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/user/75413867"))
        .and(query_param_is_missing("include_shared"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user": {"id": 75413867}})))
        .expect(1)
        .mount(&server)
        .await;

    let result = user::get(&common::client_for(&server), "75413867", false)
        .await
        .unwrap();
    assert_eq!(result["user"]["id"], 75413867);
}

#[tokio::test]
async fn user_get_with_include_shared_adds_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/user/75413867"))
        .and(query_param("include_shared", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user": {"id": 75413867}})))
        .expect(1)
        .mount(&server)
        .await;

    user::get(&common::client_for(&server), "75413867", true)
        .await
        .unwrap();
}

#[tokio::test]
async fn user_invite_posts_email() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/user"))
        .and(body_json(json!({"email": "jo@cp.com"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"user": {"email": "jo@cp.com"}})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = user::invite(&common::client_for(&server), "jo@cp.com", false, None)
        .await
        .unwrap();
    assert_eq!(result["user"]["email"], "jo@cp.com");
}

#[tokio::test]
async fn user_invite_admin_and_custom_role_id_merge_into_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/user"))
        .and(body_json(
            json!({"email": "jo@cp.com", "admin": true, "custom_role_id": 42}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "u1"})))
        .expect(1)
        .mount(&server)
        .await;

    user::invite(&common::client_for(&server), "jo@cp.com", true, Some(42))
        .await
        .unwrap();
}

#[tokio::test]
async fn user_edit_puts_to_team_user_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/team/9001/user/75413867"))
        .and(body_json(
            json!({"username": "Jo", "admin": false, "custom_role_id": null}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "u1"})))
        .expect(1)
        .mount(&server)
        .await;

    let data = json!({"username": "Jo", "admin": false, "custom_role_id": null});
    user::edit(&common::client_for(&server), "75413867", data)
        .await
        .unwrap();
}

#[tokio::test]
async fn user_remove_synthesizes_ack() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/team/9001/user/75413867"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let result = user::remove(&common::client_for(&server), "75413867")
        .await
        .unwrap();
    assert_eq!(result, json!({"removed": true, "user_id": "75413867"}));
}

#[tokio::test]
async fn group_list_passes_team_id_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/group"))
        .and(query_param("team_id", "9001"))
        .and(query_param_is_missing("group_ids"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"groups": []})))
        .expect(1)
        .mount(&server)
        .await;

    let result = group::list(&common::client_for(&server), None)
        .await
        .unwrap();
    assert_eq!(result["groups"], json!([]));
}

#[tokio::test]
async fn group_list_passes_group_ids_when_given() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/group"))
        .and(query_param("team_id", "9001"))
        .and(query_param("group_ids", "g1,g2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"groups": []})))
        .expect(1)
        .mount(&server)
        .await;

    group::list(&common::client_for(&server), Some("g1,g2".to_string()))
        .await
        .unwrap();
}

#[tokio::test]
async fn group_create_posts_to_team_group_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/team/9001/group"))
        .and(body_json(
            json!({"name": "eng", "members": [75413867, 123]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "grp1"})))
        .expect(1)
        .mount(&server)
        .await;

    let body = group::build_create_body(json!({}), "eng", &[75413867, 123]);
    let result = group::create(&common::client_for(&server), body)
        .await
        .unwrap();
    assert_eq!(result["id"], "grp1");
}

#[tokio::test]
async fn group_update_and_delete_hit_group_id_path() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v2/group/grp1"))
        .and(body_json(json!({"name": "renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "grp1"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/group/grp1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    group::update(&client, "grp1", json!({"name": "renamed"}))
        .await
        .unwrap();
    let delete_result = group::delete(&client, "grp1").await.unwrap();
    assert_eq!(delete_result, json!({"deleted": true, "group_id": "grp1"}));
}
