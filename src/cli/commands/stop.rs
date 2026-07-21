//! CLI `cco stop` — thin presentation over [`crate::app::run`].
//!
//! [INPUT]: Config · clap fields (run_id, optional task)
//! [OUTPUT]: exit code
//! [POS]: cli/commands；A1-7 无 stop 策略（Pending 冻结在 app/services）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::app::run as run_uc;
use crate::config::Config;
use crate::state;

pub fn run(config: &Config, run_id: Option<String>, task: Option<String>) -> Result<i32> {
    let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
    let st = state::RunState::load(&dir)?;
    let rid = st.run_id.clone();

    match task.as_deref() {
        None => {
            // Whole-run: freeze Pending + kill active (A0-R2) via app.
            run_uc::stop(config, &rid)?;
            println!("stopped run {rid} (aborted; pending frozen)");
        }
        Some(tid) => {
            run_uc::stop_task(config, &rid, Some(tid))?;
            println!("stopped task {tid} on run {rid}");
        }
    }
    Ok(0)
}
