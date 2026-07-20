//! Shared helpers for services submodules.
//!
//! [INPUT]: Path · RunStatus · TaskStatus · log paths
//! [OUTPUT]: kill_pid · paths_match · status helpers · log tail
//! [POS]: services 内部工具
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

use std::path::Path;

use crate::runtime::log_events;
use crate::runtime::provider::TaskStatus;
use crate::state::RunStatus;

/// Best-effort stop a worker OS process. Unix: SIGTERM then SIGKILL so hung
/// Claude/Codex CLIs actually die (desktop stop used to send only SIGTERM).
pub(super) fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15); // SIGTERM
            let _ = kill(pid as i32, 9); // SIGKILL (idempotent if already gone)
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}

pub(super) fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(aa), Ok(bb)) => aa == bb,
        _ => a.to_string_lossy() == b.to_string_lossy(),
    }
}

pub(super) fn status_str(s: &RunStatus) -> String {
    format!("{s:?}").to_ascii_lowercase()
}

pub(super) fn task_status_str(s: &TaskStatus) -> String {
    format!("{s:?}").to_ascii_lowercase()
}

pub(super) fn is_live_task(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Running | TaskStatus::Starting | TaskStatus::Queued
    )
}

/// 行边界 tail（委托 log_events，避免半截 NDJSON）。
pub(super) fn read_log_tail(path: &Path, max_bytes: usize) -> (String, u64) {
    log_events::read_text_tail(path, max_bytes)
}

/// Shrink raw log_tail for live IPC when structured events are present (P1-1).
///
/// Crash reports CCO-2026-07-18-* panicked here: byte-budget cut landed mid
/// CJK rune while `project_live_view` polled a running Claude worker.
pub(super) fn compact_log_tail_for_live(full: &str, has_events: bool, soft_cap: usize) -> String {
    if !has_events {
        return full.to_string();
    }
    log_events::compact_text_tail(full, soft_cap, "… (live compact)\n")
}
