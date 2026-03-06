use scriptor::cli::interface::run_cli;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("scriptor=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    if let Err(e) = run_cli().await {
        eprintln!("{:#}", e);
        std::process::exit(1);
    }
}
