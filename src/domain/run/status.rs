//! Pure run / task status rules (wire schema stays in `state`).
//!
//! [INPUT]: stop flags · failed set · on_failure · live/terminal labels
//! [OUTPUT]: FinalRunStatus · freeze/merge decisions
//! [POS]: domain/run — no run.json IO
//! [PROTOCOL]: 变更时更新 domain/run/mod.rs

/// Final run outcome after the orchestrator loop exits (maps 1:1 to `state::RunStatus`
/// Completed / Failed / Paused / Aborted — no Init/Validated/Running here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalRunStatus {
    Completed,
    Failed,
    Paused,
    Aborted,
}

/// User stop (provider STOP / desktop stop_run) → Aborted, not Completed.
/// Stopped tasks sit in `done` so they do not trip on_failure Pause mid-graph.
pub fn resolve_final_run_status(
    any_stopped: bool,
    has_failed: bool,
    on_failure_pause: bool,
) -> FinalRunStatus {
    if any_stopped {
        FinalRunStatus::Aborted
    } else if has_failed {
        if on_failure_pause {
            FinalRunStatus::Paused
        } else {
            FinalRunStatus::Failed
        }
    } else {
        FinalRunStatus::Completed
    }
}

/// Disk run status means the user (or CLI) requested an external stop.
pub fn is_external_stop(run_status_snake: &str) -> bool {
    matches!(run_status_snake, "aborted" | "paused")
}

/// Live (non-terminal) task phases that external stop freezes to Stopped.
/// Matches `TaskStatus::{Pending,Queued,Starting,Running}` snake_case serde.
pub fn is_live_task_status(status_snake: &str) -> bool {
    matches!(status_snake, "pending" | "queued" | "starting" | "running")
}

/// How a terminal task status from disk merges into in-memory done/failed sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeDiskEffect {
    /// Done / Stopped / Skipped → settle into done set.
    SettleDone,
    /// Failed / Timeout → settle into failed set.
    SettleFailed,
    /// Unknown terminal — ignore.
    Ignore,
}

/// Map terminal task status (snake_case) to done/failed settlement.
pub fn merge_disk_terminal(status_snake: &str) -> MergeDiskEffect {
    match status_snake {
        "stopped" | "skipped" | "done" => MergeDiskEffect::SettleDone,
        "failed" | "timeout" => MergeDiskEffect::SettleFailed,
        _ => MergeDiskEffect::Ignore,
    }
}

/// Per-provider parallel cap: no entry ⇒ unlimited.
pub fn provider_slot_open(used: usize, cap: Option<usize>) -> bool {
    match cap {
        None => true,
        Some(c) => used < c,
    }
}

/// Run-level USD budget breached (epsilon for float noise).
pub fn budget_exceeded(spent: f64, cap: f64) -> bool {
    spent > cap + f64::EPSILON
}

/// Stall patrol: idle without stdout growth ≥ threshold.
pub fn stall_triggered(idle: std::time::Duration, stall_for: std::time::Duration) -> bool {
    idle >= stall_for
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn final_status_stop_wins() {
        assert_eq!(
            resolve_final_run_status(true, true, true),
            FinalRunStatus::Aborted
        );
        assert_eq!(
            resolve_final_run_status(false, true, true),
            FinalRunStatus::Paused
        );
        assert_eq!(
            resolve_final_run_status(false, true, false),
            FinalRunStatus::Failed
        );
        assert_eq!(
            resolve_final_run_status(false, false, true),
            FinalRunStatus::Completed
        );
    }

    #[test]
    fn external_stop_and_live() {
        assert!(is_external_stop("aborted"));
        assert!(is_external_stop("paused"));
        assert!(!is_external_stop("running"));
        assert!(is_live_task_status("pending"));
        assert!(!is_live_task_status("stopped"));
    }

    #[test]
    fn merge_disk_effects() {
        assert_eq!(merge_disk_terminal("stopped"), MergeDiskEffect::SettleDone);
        assert_eq!(merge_disk_terminal("failed"), MergeDiskEffect::SettleFailed);
        assert_eq!(merge_disk_terminal("running"), MergeDiskEffect::Ignore);
    }

    #[test]
    fn stall_and_budget() {
        assert!(stall_triggered(
            Duration::from_secs(10),
            Duration::from_secs(5)
        ));
        assert!(!stall_triggered(
            Duration::from_secs(3),
            Duration::from_secs(5)
        ));
        assert!(budget_exceeded(10.1, 10.0));
        assert!(!budget_exceeded(9.9, 10.0));
    }

    #[test]
    fn provider_slots() {
        assert!(provider_slot_open(99, None));
        assert!(provider_slot_open(1, Some(2)));
        assert!(!provider_slot_open(2, Some(2)));
    }
}
