//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;

use anyhow::Result;

use crate::config::Config;
use crate::doctor;
use crate::terminal::TerminalManager;

pub async fn run(config: &Config, project: Option<PathBuf>) -> Result<i32> {
    let report = doctor::run_doctor(&config, project.as_deref()).await?;
    doctor::print_report(&report);
    // show detected terminal launcher
    let tm = TerminalManager::for_run(
        PathBuf::from("/tmp").as_path(),
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    );
    println!(
        "  [info] external_launcher     {} (prefer={})",
        tm.detected_launcher().as_str(),
        config.terminal.external_launcher
    );
    Ok(if report.ok { 0 } else { 1 })
}
