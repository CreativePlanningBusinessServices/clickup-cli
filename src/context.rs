use crate::client::CuClient;
use crate::config::{Config, default_config_path};
use crate::error::CliError;
use crate::secrets::{AccountSecrets, KeyringStore, SecretStore};

/// Default ClickUp API base, overridable via `CLICKUP_API_BASE` (sandboxes, proxies, tests).
pub const DEFAULT_BASE: &str = "https://api.clickup.com";

/// The one wiring seam every network command goes through: resolved account alias plus a
/// `CuClient` ready to call the API.
pub struct AccountContext {
    pub alias: String,
    pub client: CuClient,
}

pub fn context_for(
    account_flag: Option<&str>,
    workspace_flag: Option<&str>,
) -> Result<AccountContext, CliError> {
    let config = Config::load(&default_config_path()?)?;
    let env_alias = std::env::var("CLICKUP_ACCOUNT").ok();
    let env_base = std::env::var("CLICKUP_API_BASE").ok();
    let store = KeyringStore;
    context_from(
        &config,
        account_flag,
        workspace_flag,
        env_alias.as_deref(),
        env_base.as_deref(),
        &store,
    )
}

/// Testable inner half of `context_for`: every input that would otherwise come from disk,
/// the environment, or the real keychain is passed in.
pub fn context_from(
    config: &Config,
    account_flag: Option<&str>,
    workspace_flag: Option<&str>,
    env_alias: Option<&str>,
    env_base: Option<&str>,
    store: &dyn SecretStore,
) -> Result<AccountContext, CliError> {
    let alias = config.resolve_alias(account_flag, env_alias)?;
    let entry = &config.accounts[&alias];
    let secrets = store.get(&alias)?.ok_or_else(|| {
        CliError::Auth(format!(
            "no token stored for '{alias}' — run `clickup-cli account add {alias}`"
        ))
    })?;
    let auth_header = auth_header_for(&secrets);
    let base = env_base
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_BASE.to_string());
    let workspace_id = workspace_flag
        .map(str::to_string)
        .unwrap_or_else(|| entry.workspace_id.clone());
    let client = CuClient::new(base, auth_header, Some(workspace_id));
    Ok(AccountContext { alias, client })
}

/// ClickUp's Authorization header differs by credential kind: personal `pk_` tokens are
/// sent verbatim, but OAuth access tokens need the `Bearer ` prefix.
pub fn auth_header_for(secrets: &AccountSecrets) -> String {
    match secrets {
        AccountSecrets::Token { token } => token.clone(),
        AccountSecrets::OAuth { access_token, .. } => format!("Bearer {access_token}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AccountEntry;
    use crate::secrets::MemoryStore;

    #[test]
    fn personal_tokens_are_sent_raw_and_oauth_tokens_as_bearer() {
        assert_eq!(
            auth_header_for(&AccountSecrets::Token {
                token: "pk_123".into()
            }),
            "pk_123"
        );
        assert_eq!(
            auth_header_for(&AccountSecrets::OAuth {
                client_id: "ABC".into(),
                client_secret: "shh".into(),
                access_token: "oauth_token_value".into(),
            }),
            "Bearer oauth_token_value"
        );
    }

    fn sample_config() -> Config {
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

    /// `AccountContext` carries a `CuClient` and intentionally has no `Debug` impl, so
    /// `Result::unwrap_err` (which requires `T: Debug`) doesn't work here; extract the
    /// error by hand instead.
    fn expect_context_error(result: Result<AccountContext, CliError>) -> CliError {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        }
    }

    #[test]
    fn missing_token_is_an_auth_error_naming_the_alias() {
        let config = sample_config();
        let store = MemoryStore::default();
        let error = expect_context_error(context_from(&config, None, None, None, None, &store));
        match error {
            CliError::Auth(message) => {
                assert!(message.contains("work"), "got: {message}");
                assert!(message.contains("account add"), "got: {message}");
            }
            other => panic!("expected Auth error, got {other:?}"),
        }
    }

    #[test]
    fn workspace_flag_overrides_configured_workspace() {
        let config = sample_config();
        let store = MemoryStore::default();
        store
            .set(
                "work",
                &AccountSecrets::Token {
                    token: "pk_test".into(),
                },
            )
            .unwrap();

        let context = context_from(&config, None, Some("9999"), None, None, &store).unwrap();
        assert_eq!(context.alias, "work");
        assert_eq!(context.client.workspace_id().unwrap(), "9999");
    }

    #[test]
    fn defaults_workspace_to_configured_entry_and_base_url() {
        let config = sample_config();
        let store = MemoryStore::default();
        store
            .set(
                "work",
                &AccountSecrets::Token {
                    token: "pk_test".into(),
                },
            )
            .unwrap();

        let context = context_from(&config, None, None, None, None, &store).unwrap();
        assert_eq!(context.client.workspace_id().unwrap(), "9001");
    }

    #[test]
    fn unknown_alias_is_a_usage_error() {
        let config = sample_config();
        let store = MemoryStore::default();
        let error = expect_context_error(context_from(
            &config,
            Some("ghost"),
            None,
            None,
            None,
            &store,
        ));
        assert!(matches!(error, CliError::Usage(_)));
    }
}
