#[tokio::main]
async fn main() {
    std::process::exit(clickup_cli::cli::cli_main().await);
}
