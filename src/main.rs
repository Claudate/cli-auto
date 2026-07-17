use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use cco::cli::{self, Cli};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let code = cli::execute(cli).await?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
