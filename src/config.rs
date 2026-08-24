use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CliError;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_account: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accounts: BTreeMap<String, AccountEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEntry {
    pub workspace_id: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, CliError> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|read_error| {
            CliError::Usage(format!(
                "cannot read config {}: {read_error}",
                path.display()
            ))
        })?;
        toml::from_str(&raw).map_err(|parse_error| {
            CliError::Usage(format!("invalid config {}: {parse_error}", path.display()))
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), CliError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|io_error| {
                CliError::Usage(format!("cannot create {}: {io_error}", parent.display()))
            })?;
        }
        let raw = toml::to_string_pretty(self).expect("config serializable");
        std::fs::write(path, raw).map_err(|io_error| {
            CliError::Usage(format!("cannot write {}: {io_error}", path.display()))
        })
    }

    pub fn resolve_alias(
        &self,
        flag: Option<&str>,
        env_alias: Option<&str>,
    ) -> Result<String, CliError> {
        let alias = flag
            .map(str::to_string)
            .or_else(|| env_alias.map(str::to_string))
            .or_else(|| self.default_account.clone())
            .ok_or_else(|| {
                CliError::Usage(
                    "no account selected: pass --account, set CLICKUP_ACCOUNT, or run `clickup-cli account set-default`"
                        .into(),
                )
            })?;
        if !self.accounts.contains_key(&alias) {
            return Err(CliError::Usage(format!(
                "unknown account '{alias}' — run `clickup-cli account list`"
            )));
        }
        Ok(alias)
    }
}

pub fn default_config_path() -> Result<PathBuf, CliError> {
    directories::ProjectDirs::from("com", "CreativePlanning", "clickup-cli")
        .map(|project_dirs| project_dirs.config_dir().join("config.toml"))
        .ok_or_else(|| CliError::Usage("cannot resolve home directory for config path".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        let mut config = Config::default();
        config.accounts.insert(
            "prod".into(),
            AccountEntry {
                workspace_id: "9001".into(),
            },
        );
        config.accounts.insert(
            "sandbox".into(),
            AccountEntry {
                workspace_id: "9002".into(),
            },
        );
        config.default_account = Some("prod".into());
        config
    }

    #[test]
    fn round_trips_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        sample_config().save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.default_account, sample_config().default_account);
        assert_eq!(loaded.accounts.len(), sample_config().accounts.len());
        for (alias, entry) in &sample_config().accounts {
            assert_eq!(loaded.accounts[alias].workspace_id, entry.workspace_id);
        }

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("[accounts.prod]"), "got: {raw}");
    }

    #[test]
    fn resolve_alias_prefers_flag_then_env_then_default() {
        let config = sample_config();
        assert_eq!(
            config.resolve_alias(Some("sandbox"), Some("prod")).unwrap(),
            "sandbox"
        );
        assert_eq!(
            config.resolve_alias(None, Some("sandbox")).unwrap(),
            "sandbox"
        );
        assert_eq!(config.resolve_alias(None, None).unwrap(), "prod");
    }

    #[test]
    fn resolve_alias_without_any_selection_is_usage_error() {
        let config = Config::default();
        let error = config.resolve_alias(None, None).unwrap_err();
        assert!(matches!(error, CliError::Usage(_)));
    }

    #[test]
    fn resolve_alias_unknown_alias_is_usage_error() {
        let config = sample_config();
        let error = config.resolve_alias(Some("ghost"), None).unwrap_err();
        match error {
            CliError::Usage(message) => {
                assert!(message.contains("account list"), "got message: {message}")
            }
            other => panic!("expected Usage error, got {other:?}"),
        }
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Config::load(&dir.path().join("nope.toml")).unwrap();
        assert!(loaded.accounts.is_empty());
        assert!(loaded.default_account.is_none());
    }
}
