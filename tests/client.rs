mod common;

use clickup_cli::error::CliError;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn sends_raw_token_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .and(header("Authorization", "TEST_TOKEN"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"user": {"id": 42}})))
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let response = client
        .request(reqwest::Method::GET, "/api/v2/user", &[], None)
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body.unwrap()["user"]["id"], 42);
}

#[tokio::test]
async fn maps_err_and_ecode_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/999"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({"err": "Team not found", "ECODE": "TEAM_001"})),
        )
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let error = client
        .request(reqwest::Method::GET, "/api/v2/team/999", &[], None)
        .await
        .unwrap_err();

    match error {
        CliError::Api {
            status,
            message,
            ecode,
        } => {
            assert_eq!(status, 404);
            assert_eq!(message, "Team not found");
            assert_eq!(ecode.as_deref(), Some("TEAM_001"));
        }
        other => panic!("expected CliError::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn maps_error_key_variant() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/999"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({"error": "not authorized", "ECODE": "OAUTH_119"})),
        )
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let error = client
        .request(reqwest::Method::GET, "/api/v2/team/999", &[], None)
        .await
        .unwrap_err();

    match error {
        CliError::Api {
            status, message, ..
        } => {
            assert_eq!(status, 403);
            assert_eq!(message, "not authorized");
        }
        other => panic!("expected CliError::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn retries_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/limited"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .with_priority(2)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let response = client
        .request(reqwest::Method::GET, "/api/v2/limited", &[], None)
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn unauthorized_fails_fast_without_retry() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"err": "Token invalid", "ECODE": "OAUTH_025"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let error = client
        .request(reqwest::Method::GET, "/api/v2/user", &[], None)
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Auth(_)), "got {error:?}");
}

#[tokio::test]
async fn empty_body_yields_none() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let response = client
        .request(reqwest::Method::DELETE, "/api/v2/task/abc", &[], None)
        .await
        .unwrap();

    assert_eq!(response.status, 204);
    assert!(response.body.is_none());
}
