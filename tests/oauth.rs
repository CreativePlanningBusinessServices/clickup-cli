mod common;

use clickup_cli::oauth;
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn authorize_url_carries_client_id_redirect_and_state() {
    let url = oauth::authorize_url("ABC123", "http://localhost:8899", "state-xyz");
    assert!(url.starts_with("https://app.clickup.com/api?"));
    assert!(url.contains("client_id=ABC123"));
    assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8899"));
    assert!(url.contains("state=state-xyz"));
}

#[tokio::test]
async fn exchange_code_posts_credentials_and_returns_access_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/oauth/token"))
        .and(body_json(json!({
            "client_id": "ABC123",
            "client_secret": "shh",
            "code": "the-code",
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"access_token": "fresh_token"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let access_token = oauth::exchange_code(&server.uri(), "ABC123", "shh", "the-code")
        .await
        .unwrap();
    assert_eq!(access_token, "fresh_token");
}

#[tokio::test]
async fn exchange_code_maps_clickup_error_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/oauth/token"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"err": "Redirect URI does not match", "ECODE": "OAUTH_007"})),
        )
        .mount(&server)
        .await;

    let error = oauth::exchange_code(&server.uri(), "ABC123", "shh", "bad-code")
        .await
        .unwrap_err();
    match error {
        clickup_cli::error::CliError::Api {
            status,
            message,
            ecode,
        } => {
            assert_eq!(status, 400);
            assert!(message.contains("Redirect URI"));
            assert_eq!(ecode.as_deref(), Some("OAUTH_007"));
        }
        other => panic!("expected an api error, got {other:?}"),
    }
}

#[tokio::test]
async fn catch_redirect_returns_the_code_for_a_matching_state() {
    let listener = oauth::bind_loopback(0).unwrap(); // port 0 = any free port
    let port = listener.port();
    let redirect = tokio::spawn(async move {
        oauth::catch_redirect(listener, "state-xyz", Duration::from_secs(5)).await
    });

    // Give the acceptor a beat, then play the browser's part.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let response = reqwest::get(format!(
        "http://127.0.0.1:{port}/?code=the-code&state=state-xyz"
    ))
    .await
    .unwrap();
    assert!(response.status().is_success());
    let page = response.text().await.unwrap();
    assert!(page.to_lowercase().contains("close this tab"));

    let code = redirect.await.unwrap().unwrap();
    assert_eq!(code, "the-code");
}

#[tokio::test]
async fn catch_redirect_rejects_a_mismatched_state() {
    let listener = oauth::bind_loopback(0).unwrap();
    let port = listener.port();
    let redirect = tokio::spawn(async move {
        oauth::catch_redirect(listener, "state-xyz", Duration::from_secs(5)).await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let response = reqwest::get(format!(
        "http://127.0.0.1:{port}/?code=the-code&state=WRONG"
    ))
    .await
    .unwrap();
    assert_eq!(response.status().as_u16(), 400);

    let error = redirect.await.unwrap().unwrap_err();
    assert!(matches!(error, clickup_cli::error::CliError::Auth(_)));
}

#[tokio::test]
async fn catch_redirect_times_out() {
    let listener = oauth::bind_loopback(0).unwrap();
    let error = oauth::catch_redirect(listener, "state-xyz", Duration::from_millis(100))
        .await
        .unwrap_err();
    assert!(matches!(error, clickup_cli::error::CliError::Network(_)));
}

#[tokio::test]
async fn catch_redirect_fails_fast_when_authorization_is_denied() {
    let listener = oauth::bind_loopback(0).unwrap();
    let port = listener.port();
    let redirect = tokio::spawn(async move {
        oauth::catch_redirect(listener, "state-xyz", Duration::from_secs(5)).await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let response = reqwest::get(format!(
        "http://127.0.0.1:{port}/?error=access_denied&state=state-xyz"
    ))
    .await
    .unwrap();
    assert!(response.status().is_success());

    let error = redirect.await.unwrap().unwrap_err();
    match error {
        clickup_cli::error::CliError::Auth(message) => {
            assert!(message.contains("access_denied"), "got: {message}");
        }
        other => panic!("expected an auth error, got {other:?}"),
    }
}

#[tokio::test]
async fn catch_redirect_accepts_ipv6_localhost_connections() {
    let listener = oauth::bind_loopback(0).unwrap();
    if !listener.has_ipv6() {
        // Host without an IPv6 loopback — nothing to verify here.
        return;
    }
    let port = listener.port();
    let redirect = tokio::spawn(async move {
        oauth::catch_redirect(listener, "state-xyz", Duration::from_secs(5)).await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let response = reqwest::get(format!("http://[::1]:{port}/?code=v6-code&state=state-xyz"))
        .await
        .unwrap();
    assert!(response.status().is_success());

    let code = redirect.await.unwrap().unwrap();
    assert_eq!(code, "v6-code");
}
