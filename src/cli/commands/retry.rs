//! CLI `cco retry` — re-run one failed/stopped/timeout task (not re-split).
//!
//! [INPUT]: Config · clap fields (run_id, task, optional provider)
//! [OUTPUT]: exit code
//! [POS]: cli/commands；A1-7 无重试策略（provider override patch 在 app/services）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::app::run as run_uc;
use crate::config::Config;

/// `cco retry <run_id> --task <id> [--provider <name>]`
///
/// Thin shell over [`run_uc::retry_task`]. When `--provider` is given, the task's
/// provider is patched in `plan.resolved.json` before the retry spawn (same path
/// as the desktop "切换通道" button — not a second open-run entry).
pub fn run(
    config: &Config,
    run_id: String,
    task: String,
    provider: Option<String>,
) -> Result<i32> {
    run_uc::retry_task(config.clone(), &run_id, &task, provider.as_deref(), None)?;
    match provider.as_deref() {
        Some(p) => println!("retrying task {task} on run {run_id} via provider {p}"),
        None => println!("retrying task {task} on run {run_id}"),
    }
    Ok(0)
}
