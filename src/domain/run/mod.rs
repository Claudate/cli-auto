//! Domain run machine (A1-3 · P2-17).
//!
//! Pure status / retry / ready-set rules extracted from the scheduler.
//! **No** path IO, **no** provider spawn, **no** VERDICT text parse.
//!
//! [INPUT]: status labels · attempt counters · plan filters
//! [OUTPUT]: transition decisions for the orchestrator loop
//! [POS]: domain/run — wire types stay in `state` (`cco-run/v1` unchanged)
//! [PROTOCOL]: 变更时更新此头部与 src/domain/CLAUDE.md

mod active;
mod retry;
mod status;
mod status_line;

pub use active::{expand_from_task, resolve_active_ids, ActiveFilter};
pub use retry::{
    attempt_budget, can_same_provider_retry, classify_retry, default_failover_order,
    effective_retry_max, is_inspect_gate_error, is_non_failover_provider, is_non_retryable,
    next_failover_target, production_failover_target, RetryKind,
};
pub use status::{
    budget_exceeded, is_external_stop, is_live_task_status, merge_disk_terminal,
    provider_slot_open, resolve_final_run_status, stall_triggered, FinalRunStatus, MergeDiskEffect,
};
pub use status_line::{
    from_plan_job, from_run, resolve_status_one_liner, PlanJobSnap, StatusOneLiner, StatusPhase,
    TaskStatusSnap,
};
