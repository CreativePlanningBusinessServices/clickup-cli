mod common;

use clickup_cli::commands::raw;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_with_query_and_custom_header_reaches_mock() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team/9001/guest/42"))
        .and(query_param("include_shared", "true"))
        .and(header("X-Custom-Header", "test-value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "guest": {"user": {"id": 42}}
        })))
        .mount(&server)
        .await;

    let result = raw::call(
        &common::client_for(&server),
        reqwest::Method::GET,
        "/api/v2/team/9001/guest/42",
        &[("include_shared".to_string(), "true".to_string())],
        &[("X-Custom-Header".to_string(), "test-value".to_string())],
        None,
    )
    .await
    .unwrap();

    assert_eq!(result["guest"]["user"]["id"], 42);
}

#[tokio::test]
async fn empty_response_body_yields_status_json() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v2/task/abc"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = raw::call(
        &common::client_for(&server),
        reqwest::Method::DELETE,
        "/api/v2/task/abc",
        &[],
        &[],
        None,
    )
    .await
    .unwrap();

    assert_eq!(result["status"], 204);
}
