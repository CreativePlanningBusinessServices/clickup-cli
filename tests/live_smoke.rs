//! Real-API smoke tests. Excluded from CI (all #[ignore]) — CI compiles this file but never
//! runs these tests.
//!
//! Run: set up an account first (`clickup-cli account add <alias>`), then:
//!
//!   CLICKUP_LIVE_ALIAS=<alias> cargo test --test live_smoke -- --ignored --nocapture
use clickup_cli::commands::task::TaskListArgs;
use clickup_cli::commands::{account, task, workspace};
use clickup_cli::context::context_for;

fn live_alias() -> String {
    std::env::var("CLICKUP_LIVE_ALIAS")
        .expect("set CLICKUP_LIVE_ALIAS to a configured account alias")
}

#[tokio::test]
#[ignore = "live: set CLICKUP_LIVE_ALIAS and run cargo test --test live_smoke -- --ignored"]
async fn account_test_reaches_the_real_api() {
    let context = context_for(Some(&live_alias()), None).unwrap();
    let result = account::test(&context.client, &context.alias)
        .await
        .unwrap();
    println!("{result}");
}

#[tokio::test]
#[ignore = "live: set CLICKUP_LIVE_ALIAS and run cargo test --test live_smoke -- --ignored"]
async fn workspace_list_returns_teams() {
    let context = context_for(Some(&live_alias()), None).unwrap();
    let result = workspace::list(&context.client).await.unwrap();
    println!("{result}");
}

#[tokio::test]
#[ignore = "live: set CLICKUP_LIVE_ALIAS and run cargo test --test live_smoke -- --ignored"]
async fn task_filter_pages_the_workspace() {
    let context = context_for(Some(&live_alias()), None).unwrap();
    let args = TaskListArgs {
        archived: false,
        include_closed: false,
        subtasks: false,
        markdown: false,
        statuses: Vec::new(),
        assignees: Vec::new(),
        order_by: None,
        reverse: false,
        extra_query: Vec::new(),
    };
    let result = task::filter(&context.client, &args, Some(0), false)
        .await
        .unwrap();
    println!("{result}");
}
