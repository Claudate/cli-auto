//! Run use case surface (A1-3 · A1-7 · **A5-1** CLI 1:1 · **S-run** multi-file).
//!
//! The orchestration **loop** still lives in [`crate::runtime::Scheduler`]
//! (multi-file under `runtime/scheduler/`). Pure status/retry rules are
//! [`crate::domain::run`]. IO adapters remain in `services::runs` / `live`
//! until RunStore ports land; this module is the **only** Application API
//! Presentation (CLI/Tauri) should call for run lifecycle.
//!
//! [INPUT]: Config · run_id / StartRunRequest / plan path · ForegroundOpts
//! [OUTPUT]: run_id · RunStatus · summaries · rework DTO · materialize / scheduler
//! [POS]: Presentation → app::run → domain/run + services facade + runtime/scheduler
//! [PROTOCOL]: **Mode B 开跑仍只经** [`super::split::confirm`] / [`super::split::confirm_materialize`]；
//!   ParseOnly 走 [`materialize_parse_only`]（文档化，非 Mode B 旁路）
//!
//! ## Submodules (S-run · vertical split · zero semantic diff)
//! | File | Responsibility |
//! |------|----------------|
//! | `mod.rs` (this) | lifecycle facade · domain maps · observe helpers · re-export |
//! | [`materialize`] | disk materialize / ParseOnly load+materialize（返回 ir · drop optional · **Ensure closeout**） |
//! | [`foreground`] | ForegroundOpts · prepare_scheduler · preflight · prepare_resume · finish |
//! | [`ensure_loop`] | Ensure E3 auto rework（docs-closeout · 非 Mode B 旁路） |
//! | [`route`] | soft/force provider override (A0-R3) |
//! | [`provenance`] | stamp `route_source` (P1-2) · compose live `route_label` (P1-3) |
//!
//! ## Presentation map (A1-7 + A5-1 + **A5-3 TUI**)
//! | CLI / Tauri / TUI | app::run |
//! |------------------|---------|
//! | `stop` / `stop_run_cmd` / TUI `s` | [`stop`] / [`stop_task`] |
//! | `resume` / `resume_run_cmd` | [`prepare_resume`] + [`prepare_scheduler`] / [`resume`] |
//! | `retry_task_cmd`（卡片再跑一次） | [`retry_task`]（单任务，非 re-split） |
//! | rework / residual | [`start_rework`] / [`accept_residual`] |
//! | `get_runs` / `get_run` / `cco status` / TUI reload | [`list`] / [`load`] / [`load_by_dir`] / [`handoff_paths`] |
//! | TUI Graph plan | [`load_resolved_plan`] |
//! | legacy ParseOnly `start_run` IPC | [`start_from_request`] (not Mode B) |
//! | `cco run` ParseOnly / `--skip-plan` | [`materialize_run`] / [`materialize_parse_only`]（drop optional · 返回 ir）+ [`prepare_scheduler`] |
//! | `cco run --provider` | [`apply_provider_override`] → domain soft/force |
//! | CLI foreground loop | [`prepare_scheduler`] · [`finish_with_reports`] |

mod ensure_loop;
mod foreground;
mod materialize;
pub mod provenance;
mod route;
pub mod status_line;

pub use ensure_loop::{maybe_auto_rework, maybe_auto_rework_quiet};
pub use foreground::{
    finish_with_reports, preflight_plan, prepare_resume, prepare_scheduler, ForegroundOpts,
};
pub use materialize::{
    apply_effort, apply_permission_mode, materialize_parse_only, materialize_run,
    materialize_run_with_route, materialize_run_with_route_opts, MaterializeRouteOpts,
};
pub use provenance::{
    compose_route_label, provider_product_label, stamp_cost_budget, stamp_cost_escalate,
    stamp_cost_route, stamp_failover, stamp_route_fill, stamp_route_inferred,
};
pub use route::{apply_provider_override, list_cost_route_available, preview_cost_route};
pub use status_line::{from_job_view, from_run_state, from_run_state_with_titles, resolve};

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::Config;
use crate::domain::run::{classify_retry, resolve_final_run_status, FinalRunStatus, RetryKind};
use crate::plan::PlanIR;
use crate::report;
use crate::services::{
    accept_run_residual, list_plan_meta, list_plans, list_runs, load_run, pause_run, preview_plan,
    resume_run_async, retry_task_async, start_rework_from_run, start_run_async,
    start_run_from_plan, stop_run, stop_task as services_stop_task, PlanMeta, PlanPreview,
    ReworkStartResponse, RunSummary, StartRunRequest,
};
use crate::state::{RunState, RunStatus};

/// Map domain final status to wire `RunStatus` (cco-run/v1).
pub fn wire_final_status(any_stopped: bool, has_failed: bool, on_failure_pause: bool) -> RunStatus {
    match resolve_final_run_status(any_stopped, has_failed, on_failure_pause) {
        FinalRunStatus::Aborted => RunStatus::Aborted,
        FinalRunStatus::Paused => RunStatus::Paused,
        FinalRunStatus::Failed => RunStatus::Failed,
        FinalRunStatus::Completed => RunStatus::Completed,
    }
}

/// Expose domain retry classify for tests / future app policies.
pub fn retry_decision(
    reason_code: &str,
    attempt: u32,
    budget: u32,
    failover_used: bool,
    failover_enabled: bool,
) -> RetryKind {
    classify_retry(
        reason_code,
        attempt,
        budget,
        failover_used,
        failover_enabled,
    )
}

/// Stop a whole run (freeze Pending included). A0-R2.
pub fn stop(config: &Config, run_id: &str) -> Result<()> {
    stop_run(config, run_id)
}

/// Pause a running run.
pub fn pause(config: &Config, run_id: &str) -> Result<()> {
    pause_run(config, run_id)
}

/// Stop one task (`Some`) or live-helper whole-run path (`None`, freezes Pending).
///
/// Prefer [`stop`] for explicit whole-run abort (CLI `cco stop`, desktop `stop_run_cmd`).
pub fn stop_task(config: &Config, run_id: &str, task_id: Option<&str>) -> Result<()> {
    services_stop_task(config, run_id, task_id)
}

/// Resume a paused/aborted run in background (desktop) or via CLI scheduler loop.
pub fn resume(config: Config, run_id: &str) -> Result<()> {
    resume_run_async(config, run_id)
}

/// Manual re-run of **one** failed/stopped/timeout task (same run dir).
///
/// Not a new Mode B open-run and not re-split. Done tasks stay Done.
pub fn retry_task(config: Config, run_id: &str, task_id: &str) -> Result<()> {
    retry_task_async(config, run_id, task_id)
}

/// List recent runs (newest first).
pub fn list(config: &Config) -> Result<Vec<RunSummary>> {
    list_runs(config)
}

/// Load one run state from disk.
pub fn load(config: &Config, run_id: &str) -> Result<RunState> {
    load_run(config, run_id)
}

/// Load run state from an already-resolved run directory (TUI observer · A5-3).
///
/// Prefer [`load`] when Presentation only has `run_id`; use this when the shell
/// already resolved `run_dir` (CLI `cco tui`, attach-during-run).
pub fn load_by_dir(run_dir: &Path) -> Result<RunState> {
    RunState::load(run_dir)
}

/// Load `plan.resolved.json` beside a run (TUI Graph · A5-3).
///
/// Presentation must **not** hard-code the filename — only this query does.
/// Returns `None` when missing or unreadable (observer degrades to task list).
pub fn load_resolved_plan(run_dir: &Path) -> Option<PlanIR> {
    let text = std::fs::read_to_string(run_dir.join("plan.resolved.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// List plan relative paths under a project.
pub fn plans(project: &Path) -> Result<Vec<String>> {
    list_plans(project)
}

/// Plan chooser rows with ever_completed / last_run_* (H2).
pub fn plan_meta(config: &Config, project: &Path) -> Result<Vec<PlanMeta>> {
    list_plan_meta(config, project)
}

/// Lightweight plan preview (no worker).
pub fn plan_preview(project: &Path, plan_rel: &Path, config: &Config) -> Result<PlanPreview> {
    preview_plan(project, plan_rel, config)
}

/// Legacy ParseOnly / direct start from disk plan (desktop `start_run` IPC).
///
/// **Not** Mode B open-run. Mode B must use [`super::split::confirm`]. Soft-fill
/// policy is inside the request's provider wipe today (historical); new callers
/// should prefer plan job + confirm.
pub fn start_from_request(config: Config, req: StartRunRequest) -> Result<String> {
    start_run_async(config, req)
}

/// Start scheduler from an already-built PlanIR (used by split confirm + rework).
///
/// Presentation must **not** call this to open Mode B runs — only
/// [`super::split::confirm`] and internal rework.
pub fn start_from_plan(config: Config, project: PathBuf, ir: &PlanIR) -> Result<String> {
    start_run_from_plan(config, project, ir)
}

/// P-loop: build rework wave from inspect ISSUES and start a new run.
pub fn start_rework(config: Config, source_run_id: &str) -> Result<ReworkStartResponse> {
    start_rework_from_run(config, source_run_id)
}

/// User accepts residual open risks.
/// Also best-effort writes project last_summary (P2-2 · does not fail accept).
pub fn accept_residual(config: &Config, run_id: &str, note: Option<&str>) -> Result<()> {
    accept_run_residual(config, run_id, note)?;
    super::memory::try_writeback_from_run(config, run_id, note);
    Ok(())
}

/// Result-desk「完成并回写」: rule-template last_summary for the project (P2-2).
/// Best-effort — never blocks ending the round.
pub fn writeback_memory(
    config: &Config,
    run_id: &str,
) -> Result<Option<crate::state::ProjectLastSummary>> {
    super::memory::writeback_from_run(config, run_id, None)
}

/// Handoff file paths for status / observe (no Handoff type leak to CLI).
pub fn handoff_paths(run_dir: &Path) -> (PathBuf, PathBuf) {
    (run_dir.join("handoff.md"), run_dir.join("handoff.json"))
}

/// Provider rollup text for `cco status` (report helper; not a second strategy).
pub fn format_status_by_provider(st: &RunState) -> String {
    report::format_status_by_provider(&st.tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{OnFailure, TaskIR};

    #[test]
    fn stop_is_aborted_not_completed() {
        assert_eq!(wire_final_status(true, true, true), RunStatus::Aborted);
    }

    #[test]
    fn user_stop_reason_never_retries() {
        assert_eq!(
            retry_decision("stopped", 1, 5, false, true),
            RetryKind::Permanent
        );
    }

    fn mixed_plan() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "mixed".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("mixed.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![
                task("t1", "claude"),
                task("t2", "codex"),
                task("t3", "default"),
            ],
        }
    }

    fn task(id: &str, provider: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: provider.into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn soft_provider_keeps_explicit_codex() {
        let mut ir = mixed_plan();
        let report = apply_provider_override(&mut ir, Some("fake".into()), None);
        let msg = report.as_ref().map(|r| r.summary_line()).unwrap();
        assert!(msg.contains("filled 2"));
        assert_eq!(ir.tasks[1].provider, "codex");
        assert_eq!(ir.tasks[0].provider, "fake");
        assert_eq!(report.as_ref().unwrap().kept_ids, vec!["t2".to_string()]);
    }

    #[test]
    fn force_provider_wins_over_soft() {
        let mut ir = mixed_plan();
        let report = apply_provider_override(&mut ir, Some("claude".into()), Some("fake".into()));
        let msg = report.as_ref().map(|r| r.summary_line()).unwrap();
        assert!(msg.contains("force-provider"));
        assert!(ir.tasks.iter().all(|t| t.provider == "fake"));
        assert_eq!(
            report.as_ref().unwrap().mode,
            crate::domain::worker::RouteFillMode::Force
        );
    }
}
