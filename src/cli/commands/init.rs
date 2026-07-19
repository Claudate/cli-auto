//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::config::Config;

pub fn run(force: bool) -> Result<i32> {
    let path = Config::config_path();
    if path.exists() && !force {
        println!("already exists: {} (use --force)", path.display());
        return Ok(0);
    }
    Config::write_template(&path)?;
    println!("wrote {}", path.display());
    Ok(0)
}
