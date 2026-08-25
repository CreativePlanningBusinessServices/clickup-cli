mod common;

use clickup_cli::commands::account;
use clickup_cli::config::Config;
use clickup_cli::error::CliError;
use clickup_cli::secrets::{AccountSecrets, MemoryStore, SecretStore};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A `SecretStore` whose `set` always fails, used to prove that `account::add` writes the
/// config entry before it ever touches the keychain — so a keychain failure still leaves a
/// re-runnable config behind rather than an invisible half-added account.
struct FailingSecretStore {
    inner: MemoryStore,
}

impl FailingSecretStore {
    fn new() -> Self {
        Self {
            inner: MemoryStore::default(),
        }
    }
}

impl SecretStore for FailingSecretStore {
    fn get(&self, alias: &str) -> Result<Option<AccountSecrets>, CliError> {
        self.inner.get(alias)
    }
    fn set(&self, _alias: &str, _secrets: &AccountSecrets) -> Result<(), CliError> {
        Err(CliError::Auth("keychain unavailable: simulated".into()))
    }
    fn delete(&self, alias: &str) -> Result<(), CliError> {
        self.inner.delete(alias)
    }
}

async fn mount_single_team(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user": {"id": 183, "username": "tester"}})),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"teams": [{"id": "9001", "name": "CP"}]})),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn add_validates_token_and_stores_workspace() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .and(header("Authorization", "pk_test"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user": {"id": 183, "username": "Jon Lamb"}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team"))
        .and(header("Authorization", "pk_test"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"teams": [{"id": "9001", "name": "CP"}]})),
        )
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store = MemoryStore::default();

    let result = account::add(
        &config_path,
        &store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        json!({
            "ok": true,
            "alias": "work",
            "user": {"id": 183, "username": "Jon Lamb"},
            "workspace": {"id": "9001", "name": "CP"},
        })
    );

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.accounts["work"].workspace_id, "9001");

    match store.get("work").unwrap().expect("token stored") {
        AccountSecrets::Token { token } => assert_eq!(token, "pk_test"),
        other => panic!("expected a personal-token entry, got {other:?}"),
    }
}

#[tokio::test]
async fn add_with_multiple_workspaces_requires_flag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"user": {"id": 1, "username": "x"}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/team"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"teams": [
            {"id": "9001", "name": "CP"},
            {"id": "9002", "name": "Sandbox"}
        ]})))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store = MemoryStore::default();

    let error = account::add(
        &config_path,
        &store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await
    .unwrap_err();
    match error {
        CliError::Usage(message) => {
            assert!(message.contains("9001"), "got: {message}");
            assert!(message.contains("9002"), "got: {message}");
        }
        other => panic!("expected Usage error, got {other:?}"),
    }
}

#[tokio::test]
async fn first_account_becomes_default() {
    let server = MockServer::start().await;
    mount_single_team(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store = MemoryStore::default();

    account::add(
        &config_path,
        &store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.default_account.as_deref(), Some("work"));
}

#[tokio::test]
async fn add_with_failing_keychain_leaves_rerunnable_config() {
    let server = MockServer::start().await;
    mount_single_team(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let failing_store = FailingSecretStore::new();

    let add_result = account::add(
        &config_path,
        &failing_store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await;
    assert!(matches!(add_result, Err(CliError::Auth(_))));

    // The config-before-keyring ordering means the account entry is already on disk even
    // though the keychain write failed.
    let config = Config::load(&config_path).unwrap();
    assert!(config.accounts.contains_key("work"));
    assert_eq!(config.default_account.as_deref(), Some("work"));

    // Re-running add with a working store against the same config path succeeds.
    let working_store = MemoryStore::default();
    let retry_result = account::add(
        &config_path,
        &working_store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(retry_result["alias"], "work");
    assert!(working_store.get("work").unwrap().is_some());
}

#[tokio::test]
async fn list_never_contains_token_material() {
    let server = MockServer::start().await;
    mount_single_team(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store = MemoryStore::default();
    account::add(
        &config_path,
        &store,
        &server.uri(),
        "work",
        "pk_SECRETVALUE".into(),
        None,
    )
    .await
    .unwrap();

    let listed = account::list(&config_path).unwrap();
    assert_eq!(listed["count"], 1);
    assert!(!listed.to_string().contains("pk_"));
}

#[tokio::test]
async fn remove_deletes_config_then_secrets() {
    let server = MockServer::start().await;
    mount_single_team(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let store = MemoryStore::default();
    account::add(
        &config_path,
        &store,
        &server.uri(),
        "work",
        "pk_test".into(),
        None,
    )
    .await
    .unwrap();

    account::remove(&config_path, &store, "work").unwrap();
    assert!(store.get("work").unwrap().is_none());
    let listed = account::list(&config_path).unwrap();
    assert_eq!(listed["count"], 0);
}

#[tokio::test]
async fn test_command_hits_get_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/user"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"user": {"id": 183, "username": "Jon Lamb"}})),
        )
        .mount(&server)
        .await;

    let client = common::client_for(&server);
    let result = account::test(&client, "work").await.unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["alias"], "work");
    assert_eq!(result["user"]["id"], 183);
}
