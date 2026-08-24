use std::io::IsTerminal;
use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};
use serde_json::Value;

use crate::client::CuClient;
use crate::commands;
use crate::commands::{
    account, checklist, comment, config_cmd, field, folder, list, raw, space, tag, task, workspace,
};
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
    /// Lists within a folder or a space
    List {
        #[command(subcommand)]
        action: ListAction,
    },
    /// Tasks: CRUD, list-scoped listing, and workspace-wide filtering
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Comments on a task, list, or view, and their replies
    Comment {
        #[command(subcommand)]
        action: CommentAction,
    },
    /// Checklists on a task, and their items
    Checklist {
        #[command(subcommand)]
        action: ChecklistAction,
    },
    /// Custom fields, scoped to a list/folder/space/workspace, and per-task field values
    Field {
        #[command(subcommand)]
        action: FieldAction,
    },
    /// Tags defined on a space (task tagging lives under `task tag`)
    Tag {
        #[command(subcommand)]
        action: TagAction,
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

#[derive(Subcommand)]
pub enum ListAction {
    /// List lists in a folder, or the folderless lists directly in a space
    #[command(
        after_help = "Examples:\n  clickup-cli list list --folder 457\n  clickup-cli list list --space 790 --archived",
        group(ArgGroup::new("list_location").args(["folder", "space"]).required(true).multiple(false))
    )]
    List {
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        archived: bool,
    },
    /// Get a single list
    #[command(after_help = "Example: clickup-cli list get 124")]
    Get { list_id: String },
    /// Create a list in a folder, or a folderless list directly in a space
    #[command(
        after_help = "Example: clickup-cli list create --space 790 --name \"Sprint 12\"",
        group(ArgGroup::new("create_location").args(["folder", "space"]).required(true).multiple(false))
    )]
    Create {
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Update a list
    #[command(after_help = "Example: clickup-cli list update 124 --data '{\"name\": \"Renamed\"}'")]
    Update {
        list_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a list
    #[command(after_help = "Example: clickup-cli list delete 124")]
    Delete { list_id: String },
    /// List the members of a list
    #[command(after_help = "Example: clickup-cli list members 124")]
    Members { list_id: String },
    /// Create a list from a list template
    #[command(
        after_help = "Example: clickup-cli list from-template --folder 457 tmpl_1 --name \"Sprint 12\"",
        group(ArgGroup::new("template_location").args(["folder", "space"]).required(true).multiple(false))
    )]
    FromTemplate {
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        space: Option<String>,
        template_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Add a task to a list; requires the Tasks in Multiple Lists ClickApp
    #[command(after_help = "Example: clickup-cli list add-task 124 abc123")]
    AddTask { list_id: String, task_id: String },
    /// Remove a task from a list
    #[command(after_help = "Example: clickup-cli list remove-task 124 abc123")]
    RemoveTask { list_id: String, task_id: String },
}

#[derive(Subcommand)]
pub enum TaskAction {
    /// Create a task in a list
    #[command(
        after_help = "Example: clickup-cli task create 124 --name \"Fix login\"\n\n--data supplies fields --name doesn't cover; --name always wins if both set name."
    )]
    Create {
        list_id: String,
        #[arg(long)]
        name: String,
        /// Request body: inline JSON, @file, or - for stdin; merged under --name
        #[arg(long)]
        data: Option<String>,
    },
    /// Get a single task
    #[command(
        after_help = "Examples:\n  clickup-cli task get 868k5a5wa --subtasks --markdown\n  clickup-cli task get DEV-123 --custom-id\n\n--custom-id resolves task_id as a custom id (e.g. DEV-123) instead of a ClickUp id."
    )]
    Get {
        task_id: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
        #[arg(long)]
        subtasks: bool,
        #[arg(long)]
        markdown: bool,
    },
    /// Update a task
    #[command(
        after_help = "Example: clickup-cli task update 868k5a5wa --data '{\"status\":\"done\"}'"
    )]
    Update {
        task_id: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a task
    #[command(after_help = "Example: clickup-cli task delete 868k5a5wa")]
    Delete {
        task_id: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// List tasks in a list
    #[command(
        after_help = "Example: clickup-cli task list 124 --status \"in progress\" --all\n\n--all merges every page into one {items,count,hasMore} envelope; omit it to fetch a single --page (default 0)."
    )]
    List {
        list_id: String,
        #[arg(long)]
        archived: bool,
        #[arg(long = "include-closed")]
        include_closed: bool,
        #[arg(long)]
        subtasks: bool,
        #[arg(long)]
        markdown: bool,
        /// Repeatable; fans out to one statuses[] query pair per value
        #[arg(long = "status")]
        status: Vec<String>,
        /// Repeatable; fans out to one assignees[] query pair per value
        #[arg(long = "assignee")]
        assignee: Vec<String>,
        #[arg(long = "order-by")]
        order_by: Option<String>,
        #[arg(long)]
        reverse: bool,
        /// Repeatable key=value query parameter, passed through verbatim (date windows,
        /// custom_fields JSON, etc.)
        #[arg(long = "query")]
        query: Vec<String>,
        #[arg(long)]
        page: Option<u64>,
        /// Fetch and merge every page instead of just --page
        #[arg(long)]
        all: bool,
    },
    /// Filter tasks across the whole workspace
    #[command(
        after_help = "Example: clickup-cli task filter --assignee 75413867 --query due_date_lt=1735689600000 --all"
    )]
    Filter {
        #[arg(long)]
        archived: bool,
        #[arg(long = "include-closed")]
        include_closed: bool,
        #[arg(long)]
        subtasks: bool,
        #[arg(long)]
        markdown: bool,
        /// Repeatable; fans out to one statuses[] query pair per value
        #[arg(long = "status")]
        status: Vec<String>,
        /// Repeatable; fans out to one assignees[] query pair per value
        #[arg(long = "assignee")]
        assignee: Vec<String>,
        /// Repeatable space id; fans out to one space_ids[] query pair per value
        #[arg(long = "space")]
        space: Vec<String>,
        /// Repeatable folder id; fans out to one project_ids[] query pair per value
        /// (ClickUp's filtered-tasks endpoint still calls folders "projects", its pre-Folder
        /// legacy name)
        #[arg(long = "folder")]
        folder: Vec<String>,
        /// Repeatable list id; fans out to one list_ids[] query pair per value
        #[arg(long = "list")]
        list: Vec<String>,
        #[arg(long = "order-by")]
        order_by: Option<String>,
        #[arg(long)]
        reverse: bool,
        /// Repeatable key=value query parameter, passed through verbatim (date windows,
        /// custom_fields JSON, etc.)
        #[arg(long = "query")]
        query: Vec<String>,
        #[arg(long)]
        page: Option<u64>,
        /// Fetch and merge every page instead of just --page
        #[arg(long)]
        all: bool,
    },
    /// Move a task into a different list
    #[command(after_help = "Example: clickup-cli task move 868k5a5wa 125")]
    Move { task_id: String, list_id: String },
    /// Merge one or more source tasks into a target task
    #[command(after_help = "Example: clickup-cli task merge TARGET --source id1 --source id2")]
    Merge {
        target_task_id: String,
        /// Repeatable; task ids to merge into target_task_id
        #[arg(long = "source", required = true)]
        source: Vec<String>,
    },
    /// Create a task from a list's task template
    #[command(
        after_help = "Example: clickup-cli task from-template 124 tmpl_1 --name \"Sprint 12 kickoff\""
    )]
    FromTemplate {
        list_id: String,
        template_id: String,
        #[arg(long)]
        name: String,
    },
    /// List the members assigned to a task
    #[command(after_help = "Example: clickup-cli task members 868k5a5wa")]
    Members { task_id: String },
    /// Time a task (or tasks) has spent in each status
    #[command(
        after_help = "Examples:\n  clickup-cli task time-in-status 868k5a5wa\n  clickup-cli task time-in-status id1 id2\n\nMore than one id switches to the bulk endpoint."
    )]
    TimeInStatus {
        #[arg(required = true)]
        task_ids: Vec<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Add or remove a tag on a task
    Tag {
        #[command(subcommand)]
        action: TaskTagAction,
    },
    /// Add or remove a dependency link between two tasks
    Dependency {
        #[command(subcommand)]
        action: TaskDependencyAction,
    },
    /// Add or remove a link between two tasks
    Link {
        #[command(subcommand)]
        action: TaskLinkAction,
    },
    /// Attach a local file to a task
    #[command(after_help = "Example: clickup-cli task attach 868k5a5wa ./report.pdf")]
    Attach {
        task_id: String,
        file_path: PathBuf,
        /// Filename to send with the upload; defaults to file_path's own filename
        #[arg(long)]
        filename: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// List a task's attachments
    #[command(after_help = "Example: clickup-cli task attachments 868k5a5wa --all")]
    Attachments {
        task_id: String,
        /// Fetch one page starting from this cursor
        #[arg(long)]
        cursor: Option<String>,
        /// Fetch and merge every page instead of just one
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
pub enum TaskTagAction {
    /// Add a tag to a task
    #[command(after_help = "Example: clickup-cli task tag add 868k5a5wa urgent")]
    Add {
        task_id: String,
        tag_name: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Remove a tag from a task
    #[command(after_help = "Example: clickup-cli task tag remove 868k5a5wa urgent")]
    Remove {
        task_id: String,
        tag_name: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
}

#[derive(Subcommand)]
pub enum TaskDependencyAction {
    /// Add a dependency link between two tasks
    #[command(
        after_help = "Example: clickup-cli task dependency add 868k5a5wa --depends-on other1",
        group(ArgGroup::new("dependency_direction_add").args(["depends_on", "dependency_of"]).required(true).multiple(false))
    )]
    Add {
        task_id: String,
        #[arg(long = "depends-on")]
        depends_on: Option<String>,
        #[arg(long = "dependency-of")]
        dependency_of: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Remove a dependency link between two tasks
    #[command(
        after_help = "Example: clickup-cli task dependency remove 868k5a5wa --depends-on other1",
        group(ArgGroup::new("dependency_direction_remove").args(["depends_on", "dependency_of"]).required(true).multiple(false))
    )]
    Remove {
        task_id: String,
        #[arg(long = "depends-on")]
        depends_on: Option<String>,
        #[arg(long = "dependency-of")]
        dependency_of: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
}

#[derive(Subcommand)]
pub enum TaskLinkAction {
    /// Link two tasks together
    #[command(after_help = "Example: clickup-cli task link add 868k5a5wa other2")]
    Add {
        task_id: String,
        links_to: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Remove a link between two tasks
    #[command(after_help = "Example: clickup-cli task link remove 868k5a5wa other2")]
    Remove {
        task_id: String,
        links_to: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
}

#[derive(Subcommand)]
pub enum CommentAction {
    /// List comments on a task, list, or view
    #[command(
        after_help = "Example: clickup-cli comment list --task 868k5a5wa --all\n\n--all merges every page into one {items,count,hasMore} envelope, following ClickUp's start/start_id cursor across pages; omit it to fetch just the first (newest) page.",
        group(ArgGroup::new("comment_list_location").args(["task", "list", "view"]).required(true).multiple(false))
    )]
    List {
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        list: Option<String>,
        #[arg(long)]
        view: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
        /// Fetch and merge every page instead of just the first
        #[arg(long)]
        all: bool,
    },
    /// Create a comment on a task, list, or view
    #[command(
        after_help = "Example: clickup-cli comment create --task 868k5a5wa --text \"Deployed.\" --notify-all\n\n--text merges into data[\"comment_text\"]; --notify-all merges into data[\"notify_all\"]. ClickUp requires an assignee on list comments and validates notify_all — the CLI passes those through as API errors.",
        group(ArgGroup::new("comment_create_location").args(["task", "list", "view"]).required(true).multiple(false))
    )]
    Create {
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        list: Option<String>,
        #[arg(long)]
        view: Option<String>,
        #[arg(long)]
        text: String,
        #[arg(long = "notify-all")]
        notify_all: bool,
        /// Request body: inline JSON, @file, or - for stdin; merged under --text/--notify-all
        #[arg(long)]
        data: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Update a comment
    #[command(
        after_help = "Example: clickup-cli comment update 456 --text \"edited\" --resolved true"
    )]
    Update {
        comment_id: String,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        resolved: Option<bool>,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a comment
    #[command(after_help = "Example: clickup-cli comment delete 456")]
    Delete { comment_id: String },
    /// List a comment's threaded replies
    #[command(after_help = "Example: clickup-cli comment replies 456")]
    Replies { comment_id: String },
    /// Reply to a comment
    #[command(after_help = "Example: clickup-cli comment reply 456 --text \"thanks\"")]
    Reply {
        comment_id: String,
        #[arg(long)]
        text: String,
        /// Request body: inline JSON, @file, or - for stdin; merged under --text
        #[arg(long)]
        data: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ChecklistAction {
    /// Create a checklist on a task
    #[command(after_help = "Example: clickup-cli checklist create 868k5a5wa --name \"QA steps\"")]
    Create {
        task_id: String,
        #[arg(long)]
        name: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Update a checklist
    #[command(
        after_help = "Example: clickup-cli checklist update cl1 --data '{\"name\": \"Renamed\"}'"
    )]
    Update {
        checklist_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a checklist
    #[command(after_help = "Example: clickup-cli checklist delete cl1")]
    Delete { checklist_id: String },
    /// Add, update, or remove an item on a checklist
    Item {
        #[command(subcommand)]
        action: ChecklistItemAction,
    },
}

#[derive(Subcommand)]
pub enum ChecklistItemAction {
    /// Add an item to a checklist
    #[command(
        after_help = "Example: clickup-cli checklist item add CHECKLIST_ID --name \"run tests\" --assignee 75413867"
    )]
    Add {
        checklist_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        assignee: Option<String>,
        /// Request body: inline JSON, @file, or - for stdin; merged under --name/--assignee
        #[arg(long)]
        data: Option<String>,
    },
    /// Update a checklist item
    #[command(
        after_help = "Example: clickup-cli checklist item update CHECKLIST_ID ITEM_ID --data '{\"resolved\":true}'"
    )]
    Update {
        checklist_id: String,
        item_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        assignee: Option<String>,
        /// Request body: inline JSON, @file, or - for stdin; merged under --name/--assignee
        #[arg(long)]
        data: Option<String>,
    },
    /// Remove an item from a checklist
    #[command(after_help = "Example: clickup-cli checklist item remove CHECKLIST_ID ITEM_ID")]
    Remove {
        checklist_id: String,
        item_id: String,
    },
}

#[derive(Subcommand)]
pub enum FieldAction {
    /// List custom fields on a list, folder, space, or (with none of those given) the current
    /// workspace
    #[command(
        after_help = "Examples:\n  clickup-cli field list --list 124\n  clickup-cli field list --workspace 9001\n  clickup-cli field list\n\nGive at most one of --list/--folder/--space; with none given, lists workspace-wide fields for --workspace (global flag) or the account's default workspace.",
        group(ArgGroup::new("field_list_location").args(["list", "folder", "space"]).multiple(false))
    )]
    List {
        #[arg(long)]
        list: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        space: Option<String>,
    },
    /// Set a custom field's value on a task
    #[command(
        after_help = "Examples:\n  clickup-cli field set 868k5a5wa FIELD_UUID --value '\"http://example.com\"'\n  clickup-cli field set 868k5a5wa FIELD_UUID --value urgent\n  clickup-cli field set 868k5a5wa FIELD_UUID --value '{\"add\":[\"uuid1\"]}'\n\n--value accepts JSON, or a bare word treated as a plain string (no need to double-quote \"urgent\")."
    )]
    Set {
        task_id: String,
        field_id: String,
        #[arg(long)]
        value: String,
        /// value_options, e.g. {"time":true} for date fields — inline JSON, @file, or -
        #[arg(long)]
        options: Option<String>,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
    /// Remove a custom field's value from a task
    #[command(after_help = "Example: clickup-cli field remove 868k5a5wa FIELD_UUID")]
    Remove {
        task_id: String,
        field_id: String,
        #[arg(long = "custom-id")]
        custom_id: bool,
    },
}

#[derive(Subcommand)]
pub enum TagAction {
    /// List tags defined in a space
    #[command(after_help = "Example: clickup-cli tag list 790")]
    List { space_id: String },
    /// Create a tag in a space
    #[command(
        after_help = "Example: clickup-cli tag create 790 --name urgent --fg '#ffffff' --bg '#ff0000'"
    )]
    Create {
        space_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        fg: Option<String>,
        #[arg(long)]
        bg: Option<String>,
    },
    /// Update a tag in a space
    #[command(
        after_help = "Example: clickup-cli tag update 790 \"high priority\" --data '{\"name\":\"blocked\",\"tag_fg\":\"#ffffff\",\"tag_bg\":\"#ff0000\"}'"
    )]
    Update {
        space_id: String,
        tag_name: String,
        #[arg(long)]
        data: Option<String>,
    },
    /// Delete a tag from a space
    #[command(after_help = "Example: clickup-cli tag delete 790 urgent")]
    Delete { space_id: String, tag_name: String },
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
        Command::List { action } => dispatch_list(cli, action).await,
        Command::Task { action } => dispatch_task(cli, action).await,
        Command::Comment { action } => dispatch_comment(cli, action).await,
        Command::Checklist { action } => dispatch_checklist(cli, action).await,
        Command::Field { action } => dispatch_field(cli, action).await,
        Command::Tag { action } => dispatch_tag(cli, action).await,
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

async fn dispatch_list(cli: &Cli, action: &ListAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        ListAction::List {
            folder,
            space,
            archived,
        } => {
            list::list(
                &context.client,
                folder.as_deref(),
                space.as_deref(),
                *archived,
            )
            .await
        }
        ListAction::Get { list_id } => list::get(&context.client, list_id).await,
        ListAction::Create {
            folder,
            space,
            name,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            list::create(&context.client, folder.as_deref(), space.as_deref(), body).await
        }
        ListAction::Update { list_id, data } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            list::update(&context.client, list_id, body).await
        }
        ListAction::Delete { list_id } => list::delete(&context.client, list_id).await,
        ListAction::Members { list_id } => list::members(&context.client, list_id).await,
        ListAction::FromTemplate {
            folder,
            space,
            template_id,
            name,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            list::from_template(
                &context.client,
                folder.as_deref(),
                space.as_deref(),
                template_id,
                body,
            )
            .await
        }
        ListAction::AddTask { list_id, task_id } => {
            list::add_task(&context.client, list_id, task_id).await
        }
        ListAction::RemoveTask { list_id, task_id } => {
            list::remove_task(&context.client, list_id, task_id).await
        }
    }
}

async fn dispatch_task(cli: &Cli, action: &TaskAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        TaskAction::Create {
            list_id,
            name,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", Some(name.clone()));
            task::create(&context.client, list_id, body).await
        }
        TaskAction::Get {
            task_id,
            custom_id,
            subtasks,
            markdown,
        } => task::get(&context.client, task_id, *custom_id, *subtasks, *markdown).await,
        TaskAction::Update {
            task_id,
            custom_id,
            data,
        } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            task::update(&context.client, task_id, *custom_id, body).await
        }
        TaskAction::Delete { task_id, custom_id } => {
            task::delete(&context.client, task_id, *custom_id).await
        }
        TaskAction::List {
            list_id,
            archived,
            include_closed,
            subtasks,
            markdown,
            status,
            assignee,
            order_by,
            reverse,
            query,
            page,
            all,
        } => {
            let args = task::TaskListArgs {
                archived: *archived,
                include_closed: *include_closed,
                subtasks: *subtasks,
                markdown: *markdown,
                statuses: status.clone(),
                assignees: assignee.clone(),
                order_by: order_by.clone(),
                reverse: *reverse,
                extra_query: query.clone(),
            };
            task::list(&context.client, list_id, &args, *page, *all).await
        }
        TaskAction::Filter {
            archived,
            include_closed,
            subtasks,
            markdown,
            status,
            assignee,
            space,
            folder,
            list,
            order_by,
            reverse,
            query,
            page,
            all,
        } => {
            let mut extra_query = query.clone();
            extra_query.extend(
                space
                    .iter()
                    .map(|space_id| format!("space_ids[]={space_id}")),
            );
            // ClickUp's filtered-team-tasks endpoint still calls folders "projects" — its
            // pre-Folder legacy param name.
            extra_query.extend(
                folder
                    .iter()
                    .map(|folder_id| format!("project_ids[]={folder_id}")),
            );
            extra_query.extend(list.iter().map(|list_id| format!("list_ids[]={list_id}")));
            let args = task::TaskListArgs {
                archived: *archived,
                include_closed: *include_closed,
                subtasks: *subtasks,
                markdown: *markdown,
                statuses: status.clone(),
                assignees: assignee.clone(),
                order_by: order_by.clone(),
                reverse: *reverse,
                extra_query,
            };
            task::filter(&context.client, &args, *page, *all).await
        }
        TaskAction::Move { task_id, list_id } => {
            task::move_task(&context.client, task_id, list_id).await
        }
        TaskAction::Merge {
            target_task_id,
            source,
        } => task::merge(&context.client, target_task_id, source).await,
        TaskAction::FromTemplate {
            list_id,
            template_id,
            name,
        } => task::from_template(&context.client, list_id, template_id, name).await,
        TaskAction::Members { task_id } => task::members(&context.client, task_id).await,
        TaskAction::TimeInStatus {
            task_ids,
            custom_id,
        } => task::time_in_status(&context.client, task_ids, *custom_id).await,
        TaskAction::Tag { action } => dispatch_task_tag(&context.client, action).await,
        TaskAction::Dependency { action } => {
            dispatch_task_dependency(&context.client, action).await
        }
        TaskAction::Link { action } => dispatch_task_link(&context.client, action).await,
        TaskAction::Attach {
            task_id,
            file_path,
            filename,
            custom_id,
        } => {
            task::attach(
                &context.client,
                task_id,
                file_path,
                filename.clone(),
                *custom_id,
            )
            .await
        }
        TaskAction::Attachments {
            task_id,
            cursor,
            all,
        } => task::attachments(&context.client, task_id, cursor.clone(), *all).await,
    }
}

async fn dispatch_task_tag(client: &CuClient, action: &TaskTagAction) -> Result<Value, CliError> {
    match action {
        TaskTagAction::Add {
            task_id,
            tag_name,
            custom_id,
        } => task::tag_add(client, task_id, tag_name, *custom_id).await,
        TaskTagAction::Remove {
            task_id,
            tag_name,
            custom_id,
        } => task::tag_remove(client, task_id, tag_name, *custom_id).await,
    }
}

async fn dispatch_task_dependency(
    client: &CuClient,
    action: &TaskDependencyAction,
) -> Result<Value, CliError> {
    match action {
        TaskDependencyAction::Add {
            task_id,
            depends_on,
            dependency_of,
            custom_id,
        } => {
            task::dependency_add(
                client,
                task_id,
                depends_on.as_deref(),
                dependency_of.as_deref(),
                *custom_id,
            )
            .await
        }
        TaskDependencyAction::Remove {
            task_id,
            depends_on,
            dependency_of,
            custom_id,
        } => {
            task::dependency_remove(
                client,
                task_id,
                depends_on.as_deref(),
                dependency_of.as_deref(),
                *custom_id,
            )
            .await
        }
    }
}

async fn dispatch_task_link(client: &CuClient, action: &TaskLinkAction) -> Result<Value, CliError> {
    match action {
        TaskLinkAction::Add {
            task_id,
            links_to,
            custom_id,
        } => task::link_add(client, task_id, links_to, *custom_id).await,
        TaskLinkAction::Remove {
            task_id,
            links_to,
            custom_id,
        } => task::link_remove(client, task_id, links_to, *custom_id).await,
    }
}

async fn dispatch_comment(cli: &Cli, action: &CommentAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        CommentAction::List {
            task,
            list,
            view,
            custom_id,
            all,
        } => {
            let location =
                comment::location_from(task.as_deref(), list.as_deref(), view.as_deref())?;
            comment::list(&context.client, &location, *custom_id, *all).await
        }
        CommentAction::Create {
            task,
            list,
            view,
            text,
            notify_all,
            data,
            custom_id,
        } => {
            let location =
                comment::location_from(task.as_deref(), list.as_deref(), view.as_deref())?;
            let data = commands::parse_data_flag(data.as_deref())?;
            let body = comment::build_create_body(data, text, *notify_all);
            comment::create(&context.client, &location, *custom_id, body).await
        }
        CommentAction::Update {
            comment_id,
            text,
            resolved,
            data,
        } => {
            let data = commands::parse_data_flag(data.as_deref())?;
            let body = comment::build_update_body(data, text.as_deref(), *resolved);
            comment::update(&context.client, comment_id, body).await
        }
        CommentAction::Delete { comment_id } => comment::delete(&context.client, comment_id).await,
        CommentAction::Replies { comment_id } => {
            comment::replies(&context.client, comment_id).await
        }
        CommentAction::Reply {
            comment_id,
            text,
            data,
        } => {
            let data = commands::parse_data_flag(data.as_deref())?;
            let body = comment::build_reply_body(data, text);
            comment::reply(&context.client, comment_id, body).await
        }
    }
}

async fn dispatch_checklist(cli: &Cli, action: &ChecklistAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        ChecklistAction::Create {
            task_id,
            name,
            custom_id,
        } => checklist::create(&context.client, task_id, *custom_id, name).await,
        ChecklistAction::Update { checklist_id, data } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            checklist::update(&context.client, checklist_id, body).await
        }
        ChecklistAction::Delete { checklist_id } => {
            checklist::delete(&context.client, checklist_id).await
        }
        ChecklistAction::Item { action } => dispatch_checklist_item(&context.client, action).await,
    }
}

async fn dispatch_checklist_item(
    client: &CuClient,
    action: &ChecklistItemAction,
) -> Result<Value, CliError> {
    match action {
        ChecklistItemAction::Add {
            checklist_id,
            name,
            assignee,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", name.clone());
            commands::merge_flag(&mut body, "assignee", assignee.clone());
            checklist::item_add(client, checklist_id, body).await
        }
        ChecklistItemAction::Update {
            checklist_id,
            item_id,
            name,
            assignee,
            data,
        } => {
            let mut body = commands::parse_data_flag(data.as_deref())?;
            commands::merge_flag(&mut body, "name", name.clone());
            commands::merge_flag(&mut body, "assignee", assignee.clone());
            checklist::item_update(client, checklist_id, item_id, body).await
        }
        ChecklistItemAction::Remove {
            checklist_id,
            item_id,
        } => checklist::item_remove(client, checklist_id, item_id).await,
    }
}

async fn dispatch_field(cli: &Cli, action: &FieldAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        FieldAction::List {
            list,
            folder,
            space,
        } => {
            let location =
                field::location_from(list.as_deref(), folder.as_deref(), space.as_deref());
            field::list(&context.client, &location).await
        }
        FieldAction::Set {
            task_id,
            field_id,
            value,
            options,
            custom_id,
        } => {
            let value = commands::read_value_arg(value)?;
            let options = options
                .as_deref()
                .map(commands::read_data_arg)
                .transpose()?;
            field::set(
                &context.client,
                task_id,
                field_id,
                *custom_id,
                value,
                options,
            )
            .await
        }
        FieldAction::Remove {
            task_id,
            field_id,
            custom_id,
        } => field::remove(&context.client, task_id, field_id, *custom_id).await,
    }
}

async fn dispatch_tag(cli: &Cli, action: &TagAction) -> Result<Value, CliError> {
    let context = context_for(cli.account.as_deref(), cli.workspace.as_deref())?;
    match action {
        TagAction::List { space_id } => tag::list(&context.client, space_id).await,
        TagAction::Create {
            space_id,
            name,
            fg,
            bg,
        } => {
            tag::create(
                &context.client,
                space_id,
                name,
                fg.as_deref(),
                bg.as_deref(),
            )
            .await
        }
        TagAction::Update {
            space_id,
            tag_name,
            data,
        } => {
            let body = commands::parse_data_flag(data.as_deref())?;
            tag::update(&context.client, space_id, tag_name, body).await
        }
        TagAction::Delete { space_id, tag_name } => {
            tag::delete(&context.client, space_id, tag_name).await
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

    #[test]
    fn from_template_requires_exactly_one_location() {
        expect_parse_error(&[
            "clickup-cli",
            "list",
            "from-template",
            "--folder",
            "457",
            "--space",
            "790",
            "tmpl_1",
            "--name",
            "Sprint 12",
        ]);
    }

    #[test]
    fn from_template_requires_at_least_one_location() {
        expect_parse_error(&[
            "clickup-cli",
            "list",
            "from-template",
            "tmpl_1",
            "--name",
            "Sprint 12",
        ]);
    }

    #[test]
    fn comment_list_requires_exactly_one_location() {
        expect_parse_error(&["clickup-cli", "comment", "list"]);
        expect_parse_error(&[
            "clickup-cli",
            "comment",
            "list",
            "--task",
            "868k5a5wa",
            "--list",
            "124",
        ]);
    }

    #[test]
    fn comment_list_with_exactly_one_location_parses() {
        Cli::try_parse_from(["clickup-cli", "comment", "list", "--view", "vw1", "--all"])
            .expect("--view alone should parse");
    }

    #[test]
    fn comment_create_requires_text() {
        expect_parse_error(&["clickup-cli", "comment", "create", "--task", "868k5a5wa"]);
    }

    #[test]
    fn checklist_create_parses() {
        Cli::try_parse_from([
            "clickup-cli",
            "checklist",
            "create",
            "868k5a5wa",
            "--name",
            "QA steps",
        ])
        .expect("checklist create should parse");
    }

    #[test]
    fn checklist_item_add_and_update_parse() {
        Cli::try_parse_from([
            "clickup-cli",
            "checklist",
            "item",
            "add",
            "cl1",
            "--name",
            "run tests",
            "--assignee",
            "75413867",
        ])
        .expect("checklist item add should parse");
        Cli::try_parse_from([
            "clickup-cli",
            "checklist",
            "item",
            "update",
            "cl1",
            "item1",
            "--data",
            r#"{"resolved":true}"#,
        ])
        .expect("checklist item update should parse");
    }

    #[test]
    fn from_template_with_exactly_one_location_parses() {
        Cli::try_parse_from([
            "clickup-cli",
            "list",
            "from-template",
            "--folder",
            "457",
            "tmpl_1",
            "--name",
            "Sprint 12",
        ])
        .expect("exactly one of --folder/--space should parse");
    }

    #[test]
    fn field_list_rejects_more_than_one_location() {
        expect_parse_error(&[
            "clickup-cli",
            "field",
            "list",
            "--list",
            "124",
            "--folder",
            "457",
        ]);
    }

    #[test]
    fn field_list_with_no_location_parses_as_workspace_scoped() {
        Cli::try_parse_from(["clickup-cli", "field", "list"])
            .expect("no location flags should parse (falls back to workspace scope)");
    }

    #[test]
    fn field_list_honors_the_global_workspace_override_after_the_subcommand() {
        // The global `--workspace` flag isn't part of `field list`'s ArgGroup (it can't be —
        // clap forbids a local arg from sharing a long name with a propagated global one), so
        // this only exercises that passing it alongside `field list` still parses; the
        // workspace override itself is applied later via `context_for`.
        Cli::try_parse_from(["clickup-cli", "field", "list", "--workspace", "9001"])
            .expect("global --workspace override should parse alongside field list");
    }

    #[test]
    fn field_set_parses_bare_word_value() {
        Cli::try_parse_from([
            "clickup-cli",
            "field",
            "set",
            "868k5a5wa",
            "FIELD_UUID",
            "--value",
            "urgent",
        ])
        .expect("field set should parse");
    }

    #[test]
    fn tag_create_parses() {
        Cli::try_parse_from([
            "clickup-cli",
            "tag",
            "create",
            "790",
            "--name",
            "urgent",
            "--fg",
            "#ffffff",
            "--bg",
            "#ff0000",
        ])
        .expect("tag create should parse");
    }
}
