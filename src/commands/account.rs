use std::io::{IsTerminal, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::client::CuClient;
use crate::config::{AccountEntry, Config};
use crate::error::CliError;
use crate::secrets::{AccountSecrets, SecretStore};

pub async fn add(
    config_path: &Path,
    store: &dyn SecretStore,
    base: &str,
    alias: &str,
    token: String,
    workspace_id_flag: Option<String>,
) -> Result<Value, CliError> {
    // No workspace id yet — that's exactly what this call is figuring out — so the
    // validation client is built without one.
    let client = CuClient::new(base.to_string(), token.clone(), None);

    let user_response = client
        .request(reqwest::Method::GET, "/api/v2/user", &[], None)
        .await?;
    let user_body = user_response
        .body
        .ok_or_else(|| CliError::Auth("ClickUp returned an empty /api/v2/user response".into()))?;
    let user_id = user_body["user"]["id"].clone();
    let username = user_body["user"]["username"].clone();

    let team_response = client
        .request(reqwest::Method::GET, "/api/v2/team", &[], None)
        .await?;
    let team_body = team_response
        .body
        .ok_or_else(|| CliError::Auth("ClickUp returned an empty /api/v2/team response".into()))?;
    let teams = team_body["teams"].as_array().cloned().unwrap_or_default();
    if teams.is_empty() {
        return Err(CliError::Auth(
            "this token has access to no ClickUp workspaces".into(),
        ));
    }
    let workspace = pick_workspace(&teams, workspace_id_flag.as_deref())?;
    let workspace_id = workspace["id"].as_str().unwrap_or_default().to_string();
    let workspace_name = workspace["name"].as_str().unwrap_or_default().to_string();

    // Write the config entry before touching the keychain: if the secrets write below
    // fails, the config still points at an alias with no stored token, which is a
    // self-describing and re-runnable state (context::context_for reports "no token
    // stored for '<alias>' — run `clickup-cli account add <alias>`"). The reverse order
    // can leave a token under an alias the config never learns about.
    let mut config = Config::load(config_path)?;
    config.accounts.insert(
        alias.to_string(),
        AccountEntry {
            workspace_id: workspace_id.clone(),
        },
    );
    if config.default_account.is_none() {
        config.default_account = Some(alias.to_string());
    }
    config.save(config_path)?;

    store.set(alias, &AccountSecrets::Token { token })?;

    Ok(json!({
        "ok": true,
        "alias": alias,
        "user": {"id": user_id, "username": username},
        "workspace": {"id": workspace_id, "name": workspace_name},
    }))
}

/// Chooses which workspace (ClickUp calls them "teams") to store for this account:
/// `--workspace-id` if given (must be among the token's workspaces), else the only
/// workspace, else an interactive numbered prompt on a TTY, else a usage error listing
/// what's available so a non-interactive caller can pass `--workspace-id` next time.
fn pick_workspace<'a>(
    teams: &'a [Value],
    workspace_id_flag: Option<&str>,
) -> Result<&'a Value, CliError> {
    if let Some(requested_id) = workspace_id_flag {
        return teams
            .iter()
            .find(|team| team["id"].as_str() == Some(requested_id))
            .ok_or_else(|| {
                CliError::Usage(format!(
                    "unknown workspace id '{requested_id}' — available: {}",
                    describe_workspaces(teams)
                ))
            });
    }
    if let [only_team] = teams {
        return Ok(only_team);
    }
    if std::io::stdin().is_terminal() {
        return prompt_for_workspace(teams);
    }
    Err(CliError::Usage(format!(
        "multiple workspaces — pass --workspace-id; available: {}",
        describe_workspaces(teams)
    )))
}

fn describe_workspaces(teams: &[Value]) -> String {
    teams
        .iter()
        .map(|team| {
            format!(
                "{}={}",
                team["id"].as_str().unwrap_or("?"),
                team["name"].as_str().unwrap_or("?")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn prompt_for_workspace(teams: &[Value]) -> Result<&Value, CliError> {
    eprintln!("Multiple workspaces available:");
    for (index, team) in teams.iter().enumerate() {
        eprintln!(
            "  {}) {} ({})",
            index + 1,
            team["name"].as_str().unwrap_or("?"),
            team["id"].as_str().unwrap_or("?")
        );
    }
    eprint!("Select a workspace [1-{}]: ", teams.len());
    std::io::stderr().flush().ok();
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|read_error| CliError::Usage(format!("failed to read selection: {read_error}")))?;
    let selection: usize = input
        .trim()
        .parse()
        .map_err(|_| CliError::Usage(format!("invalid selection '{}'", input.trim())))?;
    selection
        .checked_sub(1)
        .and_then(|index| teams.get(index))
        .ok_or_else(|| CliError::Usage("selection out of range".into()))
}

pub fn list(config_path: &Path) -> Result<Value, CliError> {
    let config = Config::load(config_path)?;
    let accounts: Vec<Value> = config
        .accounts
        .iter()
        .map(|(alias, entry)| json!({"alias": alias, "workspaceId": entry.workspace_id}))
        .collect();
    Ok(json!({
        "count": accounts.len(),
        "defaultAccount": config.default_account,
        "accounts": accounts,
    }))
}

pub fn set_default(config_path: &Path, alias: &str) -> Result<Value, CliError> {
    let mut config = Config::load(config_path)?;
    if !config.accounts.contains_key(alias) {
        return Err(CliError::Usage(format!(
            "unknown account alias '{alias}' — run `clickup-cli account list`"
        )));
    }
    config.default_account = Some(alias.to_string());
    config.save(config_path)?;
    Ok(json!({"defaultAccount": alias}))
}

pub fn remove(config_path: &Path, store: &dyn SecretStore, alias: &str) -> Result<Value, CliError> {
    let mut config = Config::load(config_path)?;
    if config.accounts.remove(alias).is_none() {
        return Err(CliError::Usage(format!(
            "unknown account alias '{alias}' — run `clickup-cli account list`"
        )));
    }
    if config.default_account.as_deref() == Some(alias) {
        config.default_account = None;
    }
    // Save the config removal before deleting keychain secrets: if the secrets delete
    // below fails, the alias is already gone from config (invisible to the CLI, and a
    // future `account add` for the same alias just overwrites the orphaned keychain
    // entry). The reverse order can leave the alias in config with no secrets behind it,
    // which then surfaces as a confusing "no token stored" error on unrelated commands.
    config.save(config_path)?;
    store.delete(alias)?;
    Ok(json!({"removed": alias}))
}

pub async fn test(client: &CuClient, alias: &str) -> Result<Value, CliError> {
    let response = client
        .request(reqwest::Method::GET, "/api/v2/user", &[], None)
        .await?;
    let user = response
        .body
        .map(|body| body["user"].clone())
        .unwrap_or(Value::Null);
    Ok(json!({"ok": true, "alias": alias, "user": user}))
}
