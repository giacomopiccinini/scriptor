use anyhow::Result;
use scriptor::cli::interface::run_cli;

#[tokio::main]
async fn main() -> Result<()> {
    run_cli().await
}
