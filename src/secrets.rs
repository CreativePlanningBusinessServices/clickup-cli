use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::CliError;

pub const KEYRING_SERVICE: &str = "clickup-cli";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AccountSecrets {
    // The tag keeps a future OAuth variant migration-free.
    Token { token: String },
}

pub trait SecretStore: Send + Sync {
    fn get(&self, alias: &str) -> Result<Option<AccountSecrets>, CliError>;
    fn set(&self, alias: &str, secrets: &AccountSecrets) -> Result<(), CliError>;
    fn delete(&self, alias: &str) -> Result<(), CliError>;
}

pub struct KeyringStore;

impl KeyringStore {
    fn entry(alias: &str) -> Result<keyring::Entry, CliError> {
        keyring::Entry::new(KEYRING_SERVICE, alias).map_err(|keyring_error| {
            CliError::Auth(format!("keychain unavailable: {keyring_error}"))
        })
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, alias: &str) -> Result<Option<AccountSecrets>, CliError> {
        match Self::entry(alias)?.get_password() {
            Ok(raw) => serde_json::from_str(&raw).map(Some).map_err(|parse_error| {
                CliError::Auth(format!("corrupt keychain entry '{alias}': {parse_error}"))
            }),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(keyring_error) => Err(CliError::Auth(format!(
                "keychain read failed: {keyring_error}"
            ))),
        }
    }

    fn set(&self, alias: &str, secrets: &AccountSecrets) -> Result<(), CliError> {
        Self::entry(alias)?
            .set_password(&serde_json::to_string(secrets).expect("serializable"))
            .map_err(|keyring_error| {
                CliError::Auth(format!("keychain write failed: {keyring_error}"))
            })
    }

    fn delete(&self, alias: &str) -> Result<(), CliError> {
        match Self::entry(alias)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(keyring_error) => Err(CliError::Auth(format!(
                "keychain delete failed: {keyring_error}"
            ))),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, AccountSecrets>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, alias: &str) -> Result<Option<AccountSecrets>, CliError> {
        Ok(self.entries.lock().unwrap().get(alias).cloned())
    }

    fn set(&self, alias: &str, secrets: &AccountSecrets) -> Result<(), CliError> {
        self.entries
            .lock()
            .unwrap()
            .insert(alias.into(), secrets.clone());
        Ok(())
    }

    fn delete(&self, alias: &str) -> Result<(), CliError> {
        self.entries.lock().unwrap().remove(alias);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn account_secrets_serialize_with_kind_tag() {
        let secrets = AccountSecrets::Token {
            token: "pk_123".into(),
        };
        let value = serde_json::to_value(&secrets).unwrap();
        assert_eq!(value, json!({"kind": "token", "token": "pk_123"}));

        let round_tripped: AccountSecrets = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, secrets);
    }

    #[test]
    fn memory_store_set_get_delete_round_trip() {
        let store = MemoryStore::default();
        let secrets = AccountSecrets::Token {
            token: "pk_abc".into(),
        };
        store.set("prod", &secrets).unwrap();
        assert_eq!(store.get("prod").unwrap(), Some(secrets));

        store.delete("prod").unwrap();
        assert_eq!(store.get("prod").unwrap(), None);
    }

    #[test]
    fn memory_store_get_missing_alias_is_none() {
        let store = MemoryStore::default();
        assert_eq!(store.get("absent").unwrap(), None);
    }

    #[test]
    fn delete_missing_alias_is_ok() {
        let store = MemoryStore::default();
        assert!(store.delete("absent").is_ok());
    }
}
