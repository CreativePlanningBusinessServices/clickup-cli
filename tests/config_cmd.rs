use clickup_cli::commands::config_cmd;
use clickup_cli::config::{AccountEntry, Config};
use clickup_cli::error::CliError;
use serde_json::json;

fn config_with_work_account() -> Config {
    let mut config = Config::default();
    config.accounts.insert(
        "work".into(),
        AccountEntry {
            workspace_id: "9001".into(),
        },
    );
    config.default_account = Some("work".into());
    config
}

#[test]
fn set_default_account_validates_membership() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    Config::default().save(&config_path).unwrap();

    let error = config_cmd::set(&config_path, "default_account", "ghost").unwrap_err();
    match error {
        CliError::Usage(message) => assert!(message.contains("ghost"), "got: {message}"),
        other => panic!("expected Usage error, got {other:?}"),
    }
}

#[test]
fn set_default_account_succeeds_with_known_alias() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    config_with_work_account().save(&config_path).unwrap();

    let result = config_cmd::set(&config_path, "default_account", "work").unwrap();
    assert_eq!(result, json!({"default_account": "work"}));

    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.default_account.as_deref(), Some("work"));
}

#[test]
fn get_single_key_and_whole_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    config_with_work_account().save(&config_path).unwrap();

    let single = config_cmd::get(&config_path, Some("default_account")).unwrap();
    assert_eq!(single, json!({"default_account": "work"}));

    let whole = config_cmd::get(&config_path, None).unwrap();
    assert_eq!(whole["default_account"], "work");
    assert_eq!(whole["accounts"]["work"]["workspace_id"], "9001");
    assert!(
        whole.to_string().contains("workspace_id"),
        "whole config should include accounts: {whole}"
    );
}

#[test]
fn get_and_set_reject_unknown_key() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    Config::default().save(&config_path).unwrap();

    let get_error = config_cmd::get(&config_path, Some("bogus")).unwrap_err();
    match get_error {
        CliError::Usage(message) => {
            assert!(message.contains("bogus"), "got: {message}");
            assert!(message.contains("default_account"), "got: {message}");
        }
        other => panic!("expected Usage error, got {other:?}"),
    }

    let set_error = config_cmd::set(&config_path, "bogus", "value").unwrap_err();
    assert!(matches!(set_error, CliError::Usage(_)));
}
