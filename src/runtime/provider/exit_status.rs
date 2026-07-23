//! Shared exit-code → task/worker status mapping for CLI providers.
//!
//! [INPUT]: meta exit_code · `.done` marker content
//! [OUTPUT]: TaskStatus / WorkerStatus · normalized exit code
//! [POS]: runtime/provider pure helper (no spawn IO)
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//!
//! Unix SIGKILL/SIGTERM often leaves `wait` with no status code → we use `-1`.
//! That is orchestrator stop / kill, **not** a business failure (exit 1).

use crate::ports::worker::{TaskStatus, WorkerStatus};

/// Prefer orchestrator stop marker (`.done` / meta `130`) over stream race (`-1`).
/// Normalize signal death (`-1`) to `130` so collect/UI agree with stop_run.
pub fn resolve_exit_code(meta_code: Option<i32>, done_code: Option<i32>) -> Option<i32> {
    if done_code == Some(130) || meta_code == Some(130) {
        return Some(130);
    }
    if done_code == Some(-1) || meta_code == Some(-1) {
        return Some(130);
    }
    meta_code.or(done_code)
}

/// Map resolved exit code → task status. `-1` / `130` = Stopped (user/orchestrator).
pub fn task_status_from_exit(code: Option<i32>) -> TaskStatus {
    match code {
        Some(0) => TaskStatus::Done,
        Some(124) => TaskStatus::Timeout,
        Some(130) | Some(-1) => TaskStatus::Stopped,
        Some(_) => TaskStatus::Failed,
        None => TaskStatus::Failed,
    }
}

/// Map `.done` integer for poll() → worker status.
pub fn worker_status_from_exit(code: i32) -> WorkerStatus {
    match code {
        0 => WorkerStatus::Done,
        124 => WorkerStatus::Timeout,
        130 | -1 => WorkerStatus::Stopped,
        _ => WorkerStatus::Failed,
    }
}

/// If stop_run already wrote `.done=130`, stream must not overwrite with `-1`.
pub fn finalize_stream_exit(done_path: &std::path::Path, stream_code: i32) -> i32 {
    let existing = std::fs::read_to_string(done_path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok());
    if existing == Some(130) {
        return 130;
    }
    if stream_code == -1 {
        return 130;
    }
    stream_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefer_done_130_over_meta_minus_one() {
        assert_eq!(resolve_exit_code(Some(-1), Some(130)), Some(130));
        assert_eq!(resolve_exit_code(Some(1), Some(130)), Some(130));
    }

    #[test]
    fn signal_death_is_stopped() {
        assert_eq!(task_status_from_exit(Some(-1)), TaskStatus::Stopped);
        assert_eq!(task_status_from_exit(Some(130)), TaskStatus::Stopped);
        assert_eq!(task_status_from_exit(Some(1)), TaskStatus::Failed);
        assert!(matches!(
            worker_status_from_exit(-1),
            WorkerStatus::Stopped
        ));
    }
}
