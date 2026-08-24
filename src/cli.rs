use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::commands::{account, config_cmd};
use crate::context::{DEFAULT_BASE, context_for};
use crate::error::CliError;
use crate::output;
use crate::secrets::KeyringStore;

#[derive(Parser)]
#[command(
    name = "clickup-cli",
    version,
    about = "ClickUp API CLI for AI agents — tasks, lists, docs, comments, time tracking",
    propagate_version = true,
    color = clap::ColorChoice::Never
)]
pub struct Cli {
    /// Account alias (falls back to $CLICKUP_ACCOUNT, then the configured default)
    #[arg(long, global = true)]
    pub account: Option<String>,
    /// Workspace (team) id override (falls back to the account's configured workspace)
    #[arg(long, global = true)]
    pub workspace: Option<String>,
    /// Pretty-print JSON output
    #[arg(long, global = true)]
    pub pretty: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage stored account credentials and aliases
    Account {
        #[command(subcommand)]
        action: AccountAction,
    },
    /// Get or set configuration values
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum AccountAction {
    /// Add or overwrite an account alias; the first account added becomes the default
    #[command(
        after_help = "Examples:\n  clickup-cli account add work\n  clickup-cli account add work --workspace-id 9001\n\nThe token is never a flag: set CLICKUP_CLI_TOKEN, or run interactively to be prompted for it."
    )]
    Add {
        alias: String,
        /// Workspace (team) id to use when the token has access to more than one
        #[arg(long = "workspace-id")]
        workspace_id: Option<String>,
    },
    /// List configured account aliases (never prints secrets)
    #[command(after_help = "Example: clickup-cli account list")]
    List,
    /// Change which alias is used when --account/$CLICKUP_ACCOUNT is not given
    #[command(after_help = "Example: clickup-cli account set-default work")]
    SetDefault { alias: String },
    /// Remove an account alias and its stored token
    #[command(after_help = "Example: clickup-cli account remove work")]
    Remove { alias: String },
    /// Verify the stored token by calling GET /api/v2/user
    #[command(after_help = "Example: clickup-cli account test --account work")]
    Test,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Get one or all configuration values
    #[command(
        after_help = "Examples:\n  clickup-cli config get\n  clickup-cli config get default_account"
    )]
    Get {
        /// Configuration key to retrieve; omit to get all values
        key: Option<String>,
    },
    /// Set a configuration value
    #[command(after_help = "Example: clickup-cli config set default_account work")]
    Set { key: String, value: String },
}

pub async fn cli_main() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(clap_error) => return handle_clap_error(clap_error),
    };
    match dispatch(&cli).await {
        Ok(value) => {
            output::print_json(&value, cli.pretty);
            0
        }
        Err(error) => {
            output::print_error(&error);
            error.exit_code() as i32
        }
    }
}

/// clap's own help/version/error rendering is human-readable text, not our stdout-JSON /
/// stderr-JSON contract. `--help`/`--version` are a documented exception (human text on
/// stdout, exit 0, matching what every other CLI does); every other parse failure (bad
/// flag, missing required arg, invalid enum value, etc.) is folded into the same
/// `CliError::Usage` envelope every other invocation error goes through, so an agent
/// never has to special-case clap's output.
fn handle_clap_error(error: clap::Error) -> i32 {
    use clap::error::ErrorKind;
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            print!("{error}");
            0
        }
        _ => {
            let usage_error = CliError::Usage(error.render().to_string().trim().to_string());
            output::print_error(&usage_error);
            usage_error.exit_code() as i32
        }
    }
}

async fn dispatch(cli: &Cli) -> Result<Value, CliError> {
    match &cli.command {
        Command::Account { action } => dispatch_account(cli, action).await,
        Command::Config { action } => dispatch_config(action),
    }
}

fn dispatch_config(action: &ConfigAction) -> Result<Value, CliError> {
    let config_path = crate::config::default_config_path()?;
    match action {
        ConfigAction::Get { key } => config_cmd::get(&config_path, key.as_deref()),
        ConfigAction::Set { key, value } => config_cmd::set(&config_path, key, value),
    }
}

async fn dispatch_account(cli: &Cli, action: &AccountAction) -> Result<Value, CliError> {
    let config_path = crate::config::default_config_path()?;
    let store = KeyringStore;
    match action {
        AccountAction::Add {
            alias,
            workspace_id,
        } => {
            let token = resolve_token()?;
            let base =
                std::env::var("CLICKUP_API_BASE").unwrap_or_else(|_| DEFAULT_BASE.to_string());
            account::add(
                &config_path,
                &store,
                &base,
                alias,
                token,
                workspace_id.clone(),
            )
            .await
        }
        AccountAction::List => account::list(&config_path),
        AccountAction::SetDefault { alias } => account::set_default(&config_path, alias),
        AccountAction::Remove { alias } => account::remove(&config_path, &store, alias),
        AccountAction::Test => {
            let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
            account::test(&context.client, &context.alias).await
        }
    }
}

/// The token is never accepted as a CLI flag — that would leak it into shell history and
/// `ps` output. Resolution order: env var (for CI/non-interactive use), then a hidden
/// interactive prompt.
fn resolve_token() -> Result<String, CliError> {
    if let Ok(token) = std::env::var("CLICKUP_CLI_TOKEN") {
        return Ok(token);
    }
    if std::io::stdin().is_terminal() {
        return rpassword::prompt_password("ClickUp API token: ")
            .map_err(|read_error| CliError::Usage(format!("failed to read token: {read_error}")));
    }
    Err(CliError::Usage(
        "set CLICKUP_CLI_TOKEN or run interactively to be prompted".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    /// `Cli` intentionally has no `Debug` impl, so `Result::unwrap_err` (which requires
    /// `T: Debug`) doesn't work here; extract the error by hand instead.
    fn expect_parse_error(args: &[&str]) -> clap::Error {
        match Cli::try_parse_from(args) {
            Ok(_) => panic!("expected {args:?} to fail parsing"),
            Err(clap_error) => clap_error,
        }
    }

    #[test]
    fn bad_flag_yields_a_usage_kind_error_and_exit_code_two() {
        let clap_error = expect_parse_error(&["clickup-cli", "--bogus-flag"]);
        assert_ne!(clap_error.kind(), ErrorKind::DisplayHelp);
        assert_ne!(clap_error.kind(), ErrorKind::DisplayVersion);
        assert_eq!(handle_clap_error(clap_error), 2);
    }

    #[test]
    fn help_flag_yields_display_help_kind_and_exit_code_zero() {
        let clap_error = expect_parse_error(&["clickup-cli", "--help"]);
        assert_eq!(clap_error.kind(), ErrorKind::DisplayHelp);
        assert_eq!(handle_clap_error(clap_error), 0);
    }

    #[test]
    fn version_flag_yields_display_version_kind_and_exit_code_zero() {
        let clap_error = expect_parse_error(&["clickup-cli", "--version"]);
        assert_eq!(clap_error.kind(), ErrorKind::DisplayVersion);
        assert_eq!(handle_clap_error(clap_error), 0);
    }

    #[test]
    fn account_add_parses_alias_and_workspace_id() {
        let cli = Cli::try_parse_from([
            "clickup-cli",
            "account",
            "add",
            "work",
            "--workspace-id",
            "9001",
        ])
        .expect("add should parse");
        let Command::Account {
            action:
                AccountAction::Add {
                    alias,
                    workspace_id,
                },
        } = cli.command
        else {
            panic!("wrong variant")
        };
        assert_eq!(alias, "work");
        assert_eq!(workspace_id.as_deref(), Some("9001"));
    }

    #[test]
    fn account_add_without_workspace_id_parses() {
        let cli = Cli::try_parse_from(["clickup-cli", "account", "add", "work"])
            .expect("add should parse");
        let Command::Account {
            action: AccountAction::Add { workspace_id, .. },
        } = cli.command
        else {
            panic!("wrong variant")
        };
        assert_eq!(workspace_id, None);
    }

    #[test]
    fn account_subcommands_parse() {
        Cli::try_parse_from(["clickup-cli", "account", "list"]).expect("list parses");
        Cli::try_parse_from(["clickup-cli", "account", "set-default", "work"])
            .expect("set-default parses");
        Cli::try_parse_from(["clickup-cli", "account", "remove", "work"]).expect("remove parses");
        Cli::try_parse_from(["clickup-cli", "account", "test"]).expect("test parses");
    }

    #[test]
    fn global_flags_parse_before_subcommand() {
        Cli::try_parse_from([
            "clickup-cli",
            "--account",
            "work",
            "--workspace",
            "9001",
            "--pretty",
            "account",
            "list",
        ])
        .expect("global flags before subcommand should parse");
    }

    #[test]
    fn global_flags_parse_after_subcommand() {
        let cli = Cli::try_parse_from([
            "clickup-cli",
            "account",
            "list",
            "--account",
            "work",
            "--workspace",
            "9001",
            "--pretty",
        ])
        .expect("global flags after subcommand should parse");
        assert_eq!(cli.account.as_deref(), Some("work"));
        assert_eq!(cli.workspace.as_deref(), Some("9001"));
        assert!(cli.pretty);
    }
}
