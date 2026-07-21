//! Run use case surface (A1-3 · A1-7 · **A5-1** CLI 1:1).
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
//! ## Presentation map (A1-7 + A5-1 + **A5-3 TUI**)
//! | CLI / Tauri / TUI | app::run |
//! |------------------|---------|
//! | `stop` / `stop_run_cmd` / TUI `s` | [`stop`] / [`stop_task`] |
//! | `resume` / `resume_run_cmd` | [`prepare_resume`] + [`prepare_scheduler`] / [`resume`] |
//! | rework / residual | [`start_rework`] / [`accept_residual`] |
//! | `get_runs` / `get_run` / `cco status` / TUI reload | [`list`] / [`load`] / [`load_by_dir`] / [`handoff_paths`] |
//! | TUI Graph plan | [`load_resolved_plan`] |
//! | legacy ParseOnly `start_run` IPC | [`start_from_request`] (not Mode B) |
//! | `cco run` ParseOnly / `--skip-plan` | [`materialize_parse_only`] + [`prepare_scheduler`] |
//! | `cco run --provider` | [`apply_provider_override`] → domain soft/force |
//! | CLI foreground loop | [`prepare_scheduler`] · [`finish_with_reports`] |

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::domain::run::{
    classify_retry, resolve_final_run_status, FinalRunStatus, RetryKind,
};
use crate::domain::worker::{apply_route_fill, RouteFillMode};
use crate::plan::{load_plan, PlanIR};
use crate::report;
use crate::runtime::provider::{ProviderRegistry, TaskStatus};
use crate::runtime::Scheduler;
use crate::services::{
    accept_run_residual, list_plan_meta, list_plans, list_runs, load_run, preview_plan,
    resume_run_async, start_rework_from_run, start_run_async, start_run_from_plan, stop_run,
    stop_task as services_stop_task, PlanMeta, PlanPreview, ReworkStartResponse, RunSummary,
    StartRunRequest,
};
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

/// Map domain final status to wire `RunStatus` (cco-run/v1).
pub fn wire_final_status(
    any_stopped: bool,
    has_failed: bool,
    on_failure_pause: bool,
) -> RunStatus {
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
pub fn accept_residual(config: &Config, run_id: &str, note: Option<&str>) -> Result<()> {
    accept_run_residual(config, run_id, note)
}

/// CLI `cco run` provider override: soft-fill vs force wipe (A0-R3 / A1-4).
///
/// | flag | behavior |
/// |------|----------|
/// | none | no change |
/// | `--provider P` | soft-fill via [`RouteFillMode::Soft`] |
/// | `--force-provider P` | force wipe via [`RouteFillMode::Force`] |
///
/// When both are set, force wins. Returns a short log line when applied.
pub fn apply_provider_override(
    ir: &mut PlanIR,
    provider: Option<String>,
    force_provider: Option<String>,
) -> Option<String> {
    if let Some(p) = force_provider {
        return apply_route_fill(ir, &p, RouteFillMode::Force).map(|r| r.summary_line());
    }
    if let Some(p) = provider {
        return apply_route_fill(ir, &p, RouteFillMode::Soft).map(|r| r.summary_line());
    }
    None
}

// ── A5-1: materialize + foreground scheduler (CLI 1:1; no second open-run) ──

/// Disk materialization of a new run (run_id + run.json + plan.resolved.json).
///
/// Does **not** spawn the scheduler. Mode B callers must obtain `ir` only via
/// [`super::split::confirm_materialize`] (optional drop + soft defaults).
/// ParseOnly / `--skip-plan` use [`materialize_parse_only`].
pub fn materialize_run(
    config: &Config,
    project: PathBuf,
    ir: &PlanIR,
) -> Result<(String, RunState)> {
    if !project.is_dir() {
        bail!("项目路径不是目录: {}", project.display());
    }
    ir.validate()?;
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id)?;
    let project = project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", project.display()))?;
    let run_state = RunState::new(run_id.clone(), project, ir, run_dir.clone());
    run_state.save()?;
    let resolved = run_dir.join("plan.resolved.json");
    std::fs::write(&resolved, serde_json::to_string_pretty(ir)?)?;
    Ok((run_id, run_state))
}

/// ParseOnly / structured / `--skip-plan`: load plan from disk and materialize.
///
/// **Not** Mode B. Documented ParseOnly path — does not create a plan job.
/// Soft-fill / force-provider must be applied by the caller via
/// [`apply_provider_override`] before this call (or pass already-patched IR via
/// [`materialize_run`] after load).
pub fn materialize_parse_only(
    config: &Config,
    project: PathBuf,
    plan: &Path,
    adapter: Option<&str>,
) -> Result<(String, RunState, PlanIR)> {
    let ir = load_plan(&project, plan, adapter, config)?;
    let (run_id, st) = materialize_run(config, project, &ir)?;
    Ok((run_id, st, ir))
}

/// CLI / desktop flags for building a foreground (or blocking) Scheduler.
#[derive(Debug, Clone)]
pub struct ForegroundOpts {
    pub max_parallel: Option<usize>,
    pub yes: bool,
    pub only: Option<HashSet<String>>,
    pub from_task: Option<String>,
    pub dry_run: bool,
    pub mirror_state: Option<PathBuf>,
    pub auto_open_terminal: bool,
    pub terminal_kind: SessionKind,
    pub max_budget: Option<f64>,
}

impl Default for ForegroundOpts {
    fn default() -> Self {
        Self {
            max_parallel: None,
            yes: true,
            only: None,
            from_task: None,
            dry_run: false,
            mirror_state: None,
            auto_open_terminal: false,
            terminal_kind: SessionKind::Embedded,
            max_budget: None,
        }
    }
}

/// Build a Scheduler for foreground CLI exec (or tests). Policy stays in domain/config.
pub fn prepare_scheduler(
    config: &Config,
    ir: PlanIR,
    state: RunState,
    opts: ForegroundOpts,
) -> Result<Scheduler> {
    let registry = ProviderRegistry::from_config(config)?;
    let max_parallel = opts.max_parallel.unwrap_or(ir.max_parallel);
    let tm = TerminalManager::for_run(
        &state.run_dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    )
    .with_limits(config.terminal.max_embedded, config.terminal.max_external);
    let provider_caps: HashMap<String, usize> = config
        .providers
        .iter()
        .filter_map(|(n, pc)| pc.max_parallel.map(|m| (n.clone(), m)))
        .collect();
    let budget = opts.max_budget.or(config.default.run_max_budget_usd);
    let poll = if std::env::var("CCO_FAST_POLL").is_ok() {
        Duration::from_millis(50)
    } else {
        Duration::from_millis((config.default.poll_interval_secs.max(1) * 1000).min(5_000))
    };
    Ok(Scheduler {
        max_parallel,
        plan: ir,
        state,
        registry,
        poll_interval: poll,
        yes: opts.yes,
        only: opts.only,
        from_task: opts.from_task,
        dry_run: opts.dry_run,
        mirror_state: opts.mirror_state,
        auto_open_terminal: opts.auto_open_terminal,
        terminal_kind: opts.terminal_kind,
        terminal_manager: Some(tm),
        run_max_budget_usd: budget,
        provider_max_parallel: provider_caps,
        retry_max: config.default.retry_max,
        stall_secs: config.default.stall_secs,
        failover_enabled: config.default.failover_enabled,
        fallback_extra_attempts: config.default.fallback_extra_attempts,
    })
}

/// Preflight every provider used by the plan (CLI shared with desktop path).
pub async fn preflight_plan(registry: &ProviderRegistry, ir: &PlanIR) -> Result<()> {
    let used: HashSet<_> = ir.tasks.iter().map(|t| t.provider.clone()).collect();
    for name in &used {
        let p = registry.get(name)?;
        if let Err(e) = p.preflight().await {
            bail!("provider {name} preflight failed: {e:#}");
        }
    }
    Ok(())
}

/// Prepare a paused/aborted run for resume (reset unfinished → Pending, clear .done).
///
/// Returns (PlanIR, RunState, reset_count). Does **not** spawn; CLI uses
/// [`prepare_scheduler`] + foreground loop; desktop uses [`resume`].
pub fn prepare_resume(config: &Config, run_id: &str) -> Result<(PlanIR, RunState, usize)> {
    let dir = state::resolve_run_dir(&config.runs_dir(), Some(run_id))?;
    let mut rs = RunState::load(&dir)?;
    if matches!(rs.status, RunStatus::Running) {
        bail!("run {} is still marked running; stop it first", rs.run_id);
    }
    let plan_path = dir.join("plan.resolved.json");
    if !plan_path.exists() {
        bail!("missing plan.resolved.json in {}", dir.display());
    }
    let ir: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)
        .context("parse plan.resolved.json")?;
    let n = rs.prepare_for_resume();
    for (id, ts) in &rs.tasks {
        if matches!(ts.status, TaskStatus::Pending) {
            let _ = std::fs::remove_file(dir.join("tasks").join(id).join(".done"));
        }
    }
    rs.save()?;
    Ok((ir, rs, n))
}

/// After scheduler finishes: write reports and map status → process exit code.
pub fn finish_with_reports(config: &Config, run_id: &str, status: RunStatus) -> Result<i32> {
    let run_dir = config.runs_dir().join(run_id);
    let st = RunState::load(&run_dir)?;
    report::write_reports(&st)?;
    Ok(match status {
        RunStatus::Completed => 0,
        RunStatus::Paused => 2,
        _ => 1,
    })
}

/// Handoff file paths for status / observe (no Handoff type leak to CLI).
pub fn handoff_paths(run_dir: &Path) -> (PathBuf, PathBuf) {
    (
        run_dir.join("handoff.md"),
        run_dir.join("handoff.json"),
    )
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
        assert_eq!(
            wire_final_status(true, true, true),
            RunStatus::Aborted
        );
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
        let msg = apply_provider_override(&mut ir, Some("fake".into()), None);
        assert!(msg.as_deref().unwrap().contains("filled 2"));
        assert_eq!(ir.tasks[1].provider, "codex");
        assert_eq!(ir.tasks[0].provider, "fake");
    }

    #[test]
    fn force_provider_wins_over_soft() {
        let mut ir = mixed_plan();
        let msg = apply_provider_override(
            &mut ir,
            Some("claude".into()),
            Some("fake".into()),
        );
        assert!(msg.as_deref().unwrap().contains("force-provider"));
        assert!(ir.tasks.iter().all(|t| t.provider == "fake"));
    }
}
