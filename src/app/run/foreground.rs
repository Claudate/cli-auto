//! Foreground scheduler build + resume prep + post-run reports (A5-1 · S-run extract).
//!
//! [INPUT]: Config · PlanIR · RunState · ForegroundOpts · run_id
//! [OUTPUT]: Scheduler · (PlanIR, RunState, reset_count) · exit code
//! [POS]: app::run sub-module; policy stays in domain/config; loop stays runtime/scheduler
//! [PROTOCOL]: does not open Mode B runs; CLI uses after confirm_materialize / materialize_parse_only

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::PlanIR;
use crate::report;
use crate::runtime::provider::{ProviderRegistry, TaskStatus};
use crate::runtime::Scheduler;
use crate::state::{self, RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

/// CLI / desktop flags for building a foreground (or blocking) Scheduler.
#[derive(Debug, Clone)]
pub struct ForegroundOpts {
    pub max_parallel: Option<usize>,
    pub yes: bool,
    pub only: Option<HashSet<String>>,
    pub from_task: Option<String>,
    pub dry_run: bool,
    pub mirror_state: Option<std::path::PathBuf>,
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
        failover_order: config.default.failover_order.clone(),
        cost_escalate_enabled: config.default.cost_escalate_enabled,
        browser: config.browser.clone(),
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
/// [`prepare_scheduler`] + foreground loop; desktop uses [`super::resume`].
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

/// After scheduler finishes: write reports, maybe Ensure auto-rework, map exit code.
pub fn finish_with_reports(config: &Config, run_id: &str, status: RunStatus) -> Result<i32> {
    let run_dir = config.runs_dir().join(run_id);
    let st = RunState::load(&run_dir)?;
    report::write_reports(&st)?;
    // Ensure E3: docs-closeout FAIL → auto rework (best-effort; never fails finish).
    if matches!(status, RunStatus::Failed | RunStatus::Paused) {
        if let Some(resp) = super::ensure_loop::maybe_auto_rework_quiet(config, run_id) {
            tracing::info!(
                %run_id,
                new_run = %resp.run_id,
                round = resp.round,
                "ensure auto_rework started"
            );
        }
    }
    Ok(match status {
        RunStatus::Completed => 0,
        RunStatus::Paused => 2,
        _ => 1,
    })
}
