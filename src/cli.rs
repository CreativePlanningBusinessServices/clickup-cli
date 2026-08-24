use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::commands;
use crate::commands::{account, config_cmd, folder, raw, space, workspace};
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
    /// Send an arbitrary request to any ClickUp API endpoint
    #[command(
        after_help = "Example: clickup-cli raw GET /api/v2/team/9001/guest/42\n\nUse this to reach enterprise-only endpoints (like guests) that don't have a dedicated subcommand."
    )]
    Raw {
        #[arg(value_enum, ignore_case = true)]
        method: HttpMethodArg,
        path: String,
        /// Repeatable key=value query parameter
        #[arg(long = "query")]
        query: Vec<String>,
        /// Repeatable 'Name: value' header
        #[arg(long = "header")]
        header: Vec<String>,
        /// Request body: inline JSON, @file, or - for stdin
        #[arg(long)]
        data: Option<String>,
    },
    /// Workspaces (ClickUp calls them "teams") the token can see
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Spaces within a workspace
    Space {
        #[command(subcommand)]
        action: SpaceAction,
    },
    /// Folders within a space
    Folder {
        #[command(subcommand)]
        action: FolderAction,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum HttpMethodArg {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethodArg {
    fn to_method(self) -> reqwest::Method {
        match self {
            HttpMethodArg::Get => reqwest::Method::GET,
            HttpMethodArg::Post => reqwest::Method::POST,
            HttpMethodArg::Put => reqwest::Method::PUT,
            HttpMethodArg::Patch => reqwest::Method::PATCH,
            HttpMethodArg::Delete => reqwest::Method::DELETE,
        }
    }
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

#[derive(Subcommand)]
pub enum WorkspaceAction {
    /// List every workspace the token can see
    #[command(after_help = "Example: clickup-cli workspace list")]
    List,
    /// Seat usage and limits for the current workspace
    #[command(after_help = "Example: clickup-cli workspace seats")]
    Seats,
    /// Billing plan details for the current workspace
    #[command(after_help = "Example: clickup-cli workspace plan")]
    Plan,
    /// Hierarchy shared with the current user in the current workspace
    #[command(after_help = "Example: clickup-cli workspace shared")]
    Shared,
    /// Custom task types configured on the current workspace
    #[command(after_help = "Example: clickup-cli workspace task-types")]
    TaskTypes,
}

#[derive(Subcommand)]
pub enum SpaceAction {
    /// List spaces in the current workspace
    #[command(after_help = "Example: clickup-cli space list --archived")]
    List {
        #[arg(long)]
        archived: bool,
    },
    /// Get a single space
    #[command(after_help = "Example: clickup-cli space get 90123")]
    Get { space_id: String },
    /// Create a space in the current workspace
    #[command(
        after_help = "Examples:\n  clickup-cli space create --name \"Engineering\"\n  clickup-cli space create --name \"Engineering\" --data '{\"multiple_assignees\": true}'\n\n--data supplies fields --name doesn't cover, like multiple_assignees or features; --name always wins if both set name."
    )]
    Create {
        #[arg(long)]
        name: String,
        /// Request body: inline JSON, @file, or - for stdin; merged under --name
        #[arg(long)]
        data: Option<String>,
    },
    /// Update a space
    #[command(
        after_help = "Example: clickup-cli space update 90123 --data '{\"name\": \"Renamed\"}'"
    )]
    Update {
        space_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a space
    #[command(after_help = "Example: clickup-cli space delete 90123")]
    Delete { space_id: String },
}

#[derive(Subcommand)]
pub enum FolderAction {
    /// List folders in a space
    #[command(after_help = "Example: clickup-cli folder list 90123 --archived")]
    List {
        space_id: String,
        #[arg(long)]
        archived: bool,
    },
    /// Get a single folder
    #[command(after_help = "Example: clickup-cli folder get 457 --include-subfolders")]
    Get {
        folder_id: String,
        #[arg(long)]
        include_subfolders: bool,
    },
    /// Create a folder in a space
    #[command(after_help = "Example: clickup-cli folder create 90123 --name \"Sprints\"")]
    Create {
        space_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Update a folder
    #[command(
        after_help = "Example: clickup-cli folder update 457 --data '{\"name\": \"Renamed\"}'"
    )]
    Update {
        folder_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a folder
    #[command(after_help = "Example: clickup-cli folder delete 457")]
    Delete { folder_id: String },
    /// Move a folder into a different space
    #[command(after_help = "Example: clickup-cli folder move 457 --data '{\"space_id\": \"123\"}'")]
    Move {
        folder_id: String,
        /// Request body: inline JSON, @file, or - for stdin; e.g. {"space_id": "123"}
        #[arg(long)]
        data: String,
    },
    /// Create a folder from a folder template
    #[command(
        after_help = "Example: clickup-cli folder from-template 90123 tmpl_1 --name \"Sprint 12\""
    )]
    FromTemplate {
        space_id: String,
        template_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
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
        Command::Raw {
            method,
            path,
            query,
            header,
            data,
        } => {
            let query_pairs = commands::parse_key_value_pairs(query)?;
            let header_pairs = commands::parse_header_pairs(header)?;
            let body = data
                .as_ref()
                .map(|arg| commands::read_data_arg(arg))
                .transpose()?;
            let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
            raw::call(
                &context.client,
                method.to_method(),
                path,
                &query_pairs,
                &header_pairs,
                body,
            )
            .await
        }
        Command::Workspace { action } => dispatch_workspace(cli, action).await,
        Command::Space { action } => dispatch_space(cli, action).await,
        Command::Folder { action } => dispatch_folder(cli, action).await,
    }
}

async fn dispatch_workspace(cli: &Cli, action: &WorkspaceAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        WorkspaceAction::List => workspace::list(&context.client).await,
        WorkspaceAction::Seats => workspace::seats(&context.client).await,
        WorkspaceAction::Plan => workspace::plan(&context.client).await,
        WorkspaceAction::Shared => workspace::shared(&context.client).await,
        WorkspaceAction::TaskTypes => workspace::task_types(&context.client).await,
    }
}

async fn dispatch_space(cli: &Cli, action: &SpaceAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        SpaceAction::List { archived } => space::list(&context.client, *archived).await,
        SpaceAction::Get { space_id } => space::get(&context.client, space_id).await,
        SpaceAction::Create { name, data } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            space::create(&context.client, body).await
        }
        SpaceAction::Update { space_id, data } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            space::update(&context.client, space_id, body).await
        }
        SpaceAction::Delete { space_id } => space::delete(&context.client, space_id).await,
    }
}

async fn dispatch_folder(cli: &Cli, action: &FolderAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        FolderAction::List { space_id, archived } => {
            folder::list(&context.client, space_id, *archived).await
        }
        FolderAction::Get {
            folder_id,
            include_subfolders,
        } => folder::get(&context.client, folder_id, *include_subfolders).await,
        FolderAction::Create {
            space_id,
            name,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            folder::create(&context.client, space_id, body).await
        }
        FolderAction::Update { folder_id, data } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            folder::update(&context.client, folder_id, body).await
        }
        FolderAction::Delete { folder_id } => folder::delete(&context.client, folder_id).await,
        FolderAction::Move { folder_id, data } => {
            let body = commands::read_data_arg(data)?;
            folder::move_folder(&context.client, folder_id, body).await
        }
        FolderAction::FromTemplate {
            space_id,
            template_id,
            name,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            folder::from_template(&context.client, space_id, template_id, body).await
        }
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
