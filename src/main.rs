//! cco binary entry.
//!
//! [INPUT]: argv via clap · env RUST_LOG
//! [OUTPUT]: process exit code from cli::execute
//! [POS]: CLI 入口；逻辑在 cli/ 与 services
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

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
