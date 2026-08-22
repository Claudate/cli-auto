//! Provider-agnostic scheduler (A1-3/A1-4 · thin multi-file orchestrator).
//!
//! Pure rules: [`crate::domain::run`] + [`crate::domain::worker`]; workers via [`WorkerPort`].
//! VERDICT parse stays in handoff (A1-5).
//!
//! [INPUT]: PlanIR · RunState · ProviderRegistry · TerminalManager 可选 · 预算/并行 · retry/stall · failover
//! [OUTPUT]: 推进任务状态 · events.jsonl · plan.resolved.json · handoff · 终态 RunStatus
//! [POS]: runtime 编排循环；CLI run 与 services.start_run_* 共用；行为契约不变
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
//! note: per_task worktree 从已提交依赖分支 fork（fork_base_for），后置任务可见前置产物；
//!       implement 任务 Done 前过 noop guard（零产出 → Failed，防 codex 空转假成功）。
//!
//! ## Pure vs IO (A1-3/A1-4 map)
//! | Pure (domain) | IO (this module) |
//! |---------------|------------------|
//! | final status / external-stop labels | save run.json · events |
//! | retry/failover classify · FailoverPolicy | WorkerPort start/poll/stop/collect |
//! | isolation FailClosed on multi-provider | worktree path create |
//! | active set --only/--from | terminal open |
//! | stall idle threshold · slot caps | handoff write · VERDICT port call |
//! | soft-fill route (domain/worker) | acceptance shell · mirror_run |

mod active;
mod collab_gate;
mod finish;
mod gates;
mod memory;
mod patrol;
mod start;
mod tick;
mod types;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::{info, warn};

use super::collab::CollabBus;
use super::handoff;
use super::provider::{ProviderRegistry, TaskStatus, WorkerHandle, WorkerPort};
use crate::domain::run::{resolve_final_run_status, FinalRunStatus};
use crate::graph::ready_tasks;
use crate::plan::{OnFailure, PlanIR};
use crate::state::{RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

use types::{mirror_run, ProgressWatch};

pub struct Scheduler {
    pub plan: PlanIR,
    pub state: RunState,
    pub registry: ProviderRegistry,
    pub max_parallel: usize,
    pub poll_interval: Duration,
    pub yes: bool,
    pub only: Option<HashSet<String>>,
    pub from_task: Option<String>,
    pub dry_run: bool,
    pub mirror_state: Option<PathBuf>,
    /// Auto-open terminal on task start (embedded or external).
    pub auto_open_terminal: bool,
    pub terminal_kind: SessionKind,
    pub terminal_manager: Option<TerminalManager>,
    /// Optional run-level total USD budget across all tasks.
    pub run_max_budget_usd: Option<f64>,
    /// Optional per-provider parallel caps.
    pub provider_max_parallel: HashMap<String, usize>,
    /// Extra attempts after the first try (0 = no auto-retry). Effective: max(plan.retry_max, this).
    pub retry_max: u32,
    /// No stdout growth for this long → stall → stop + retry (or pause if exhausted).
    pub stall_secs: u64,
    /// H4: after same-provider retries exhaust, walk failover_order and re-try.
    /// Manual stop never triggers failover. Default true (config.default.failover_enabled).
    pub failover_enabled: bool,
    /// Extra attempts allowed on the fallback provider after a switch (default 1).
    pub fallback_extra_attempts: u32,
    /// Production failover walk order (default claude,codex). Empty → policy default.
    pub failover_order: Vec<String>,
    /// P1: prefer higher-cost tier before walking [`failover_order`].
    pub cost_escalate_enabled: bool,
    /// Browser MCP for tagged tasks (default off). See `docs/browser-automation-cco.md`.
    pub browser: crate::config::BrowserConfig,
    /// Providers that failed preflight this run (skip in cost escalate / budget picks).
    pub provider_unhealthy: Vec<String>,
    /// Optional collaboration bus for runtime task coordination (wait_for conditions).
    pub collab_bus: Option<Arc<CollabBus>>,
    /// P3 memory pilot: outcome recording + preventive failover (None = disabled).
    pub memory: Option<Arc<dyn crate::ports::MemoryPort>>,
    /// B1: optional frontend event emitter (None on CLI/TUI → emit silently skipped).
    pub event_emitter: Option<Arc<dyn crate::ports::EventEmitter>>,
}

impl Scheduler {
    pub async fn run(mut self) -> Result<RunStatus> {
        self.state.status = RunStatus::Validated;
        self.state.save()?;
        let run_start_payload = serde_json::json!({
            "run_id": self.state.run_id,
            "project": self.state.project_root,
            "plan": self.state.plan_path,
        });
        self.state.event("run_start", run_start_payload.clone())?;
        self.emit_event("run_start", run_start_payload);

        let resolved = self.state.run_dir.join("plan.resolved.json");
        std::fs::write(&resolved, serde_json::to_string_pretty(&self.plan)?)?;

        // P1-4: host-owned handoff shell (Board = all pending). Skip overwrite on resume.
        if !handoff::Handoff::path_json(&self.state.run_dir).exists() {
            if let Err(e) = handoff::write_shell(&self.plan, &self.state) {
                warn!(err = %e, "handoff write_shell failed");
            }
        }

        if self.dry_run {
            info!("dry-run: not starting workers");
            self.state.status = RunStatus::Completed;
            self.state.finished_at = Some(chrono::Utc::now());
            self.state.save()?;
            let _ = handoff::on_run_end(&self.plan, &self.state, RunStatus::Completed);
            return Ok(RunStatus::Completed);
        }

        let active_ids = self.active_task_ids()?;
        for t in &self.plan.tasks {
            if !active_ids.contains(&t.id) {
                if let Some(ts) = self.state.tasks.get_mut(&t.id) {
                    ts.status = TaskStatus::Skipped;
                }
            }
        }
        self.state.save()?;

        self.state.status = RunStatus::Running;
        self.state.save()?;

        let mut done: HashSet<String> = self
            .state
            .tasks
            .iter()
            .filter(|(_, t)| t.status == TaskStatus::Done || t.status == TaskStatus::Skipped)
            .map(|(k, _)| k.clone())
            .collect();
        let mut failed: HashSet<String> = HashSet::new();
        let mut running: HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)> =
            HashMap::new();
        // Tasks that have been *accepted* as started at least once this run (not cleared on retry).
        let mut started: HashSet<String> = HashSet::new();
        let mut progress: HashMap<String, ProgressWatch> = HashMap::new();
        let retry_budget = self.effective_retry_max();
        let stall_for = Duration::from_secs(self.stall_secs.max(1));
        info!(
            retry_max = retry_budget,
            stall_secs = stall_for.as_secs(),
            failover = self.failover_enabled,
            fallback_extra = self.fallback_extra_attempts,
            "scheduler patrol armed"
        );

        loop {
            if let Some(status) = self
                .handle_external_stop(
                    &mut running,
                    &mut progress,
                    &mut done,
                    &mut failed,
                    &mut started,
                    retry_budget,
                )
                .await?
            {
                return Ok(status);
            }

            self.reap_finished(
                &mut running,
                &mut progress,
                &mut done,
                &mut failed,
                &mut started,
                retry_budget,
                stall_for,
            )
            .await?;

            if failed.contains("__budget__") && running.is_empty() {
                self.state.status = RunStatus::Paused;
                self.state.save()?;
                let budget_payload = serde_json::json!({"status": "paused", "reason": "budget_exceeded"});
                self.state.event("run_end", budget_payload.clone())?;
                self.emit_event("run_end", budget_payload);
                let _ = handoff::on_run_end(&self.plan, &self.state, RunStatus::Paused);
                return Ok(RunStatus::Paused);
            }

            if !failed.is_empty() && matches!(self.plan.on_failure, OnFailure::Pause) {
                if running.is_empty() {
                    self.state.status = RunStatus::Paused;
                    self.state.save()?;
                    let pause_payload = serde_json::json!({"status": "paused", "failed": failed});
                    self.state.event("run_end", pause_payload.clone())?;
                    self.emit_event("run_end", pause_payload);
                    let _ = handoff::on_run_end(&self.plan, &self.state, RunStatus::Paused);
                    return Ok(RunStatus::Paused);
                }
            }

            // LX1: one pure tick decision drives spawn (borrowed from LoopX should-run).
            // Ready set + budget + slots collapse into a single domain enum; the loop
            // only executes its side effects (hard rule 8: thin orchestrator).
            let mut ready = ready_tasks(&self.plan, &done, &started);
            ready.retain(|id| active_ids.contains(id));
            let snapshot = crate::domain::run::RunTickSnapshot {
                spent: self.state.total_cost_usd(),
                cap: self.run_max_budget_usd,
                ready_ids: ready,
                running: running.len(),
                slot_cap: Some(self.max_parallel),
                any_stalled: false,
            };
            let spawn_allowed = (failed.is_empty()
                || matches!(self.plan.on_failure, OnFailure::Continue))
                && !failed.contains("__budget__");
            match crate::domain::run::decide_tick(&snapshot) {
                crate::domain::run::TickDecision::Spawn(mut ids) if spawn_allowed => {
                    self.spawn_ready(
                        &active_ids,
                        &mut ids,
                        &mut running,
                        &mut progress,
                        &mut done,
                        &mut failed,
                        &mut started,
                        retry_budget,
                    )
                    .await?;
                }
                // Halt (over budget) / Wait (slots full or nothing ready) / Spawn while
                // paused-on-failure → spawn nothing this tick (quiet skip, no spend).
                _ => {}
            }

            // Single budget catch (was scattered across reap / spawn fast-path): mark
            // __budget__ once spend crosses the cap so the pause path fires next tick.
            if self.budget_exceeded()? {
                failed.insert("__budget__".into());
            }

            if self.should_exit_loop(&active_ids, &done, &failed, &started, &running) {
                break;
            }

            tokio::time::sleep(self.poll_interval).await;
        }

        // User stop → Aborted, not Completed. Stopped sits in `done` so mid-graph
        // on_failure Pause is not tripped, but the whole run must not look successful.
        let any_stopped = self
            .state
            .tasks
            .values()
            .any(|t| t.status == TaskStatus::Stopped);
        let status = match resolve_final_run_status(
            any_stopped,
            !failed.is_empty(),
            matches!(self.plan.on_failure, OnFailure::Pause),
        ) {
            FinalRunStatus::Aborted => RunStatus::Aborted,
            FinalRunStatus::Paused => RunStatus::Paused,
            FinalRunStatus::Failed => RunStatus::Failed,
            FinalRunStatus::Completed => RunStatus::Completed,
        };
        self.state.status = status;
        self.state.finished_at = Some(chrono::Utc::now());
        self.auto_commit_plan(status);
        self.state.save()?;
        let end_payload = serde_json::json!({"status": status});
        self.state.event("run_end", end_payload.clone())?;
        self.emit_event("run_end", end_payload);
        let _ = handoff::on_run_end(&self.plan, &self.state, status);

        if let Some(mirror) = &self.mirror_state {
            let _ = mirror_run(&self.state.run_dir, mirror);
        }

        Ok(status)
    }

    /// B1-0: Bridge disk events to optional frontend emitter.
    fn emit_event(&self, type_name: &str, payload: serde_json::Value) {
        if let Some(emitter) = &self.event_emitter {
            let _ = emitter.emit_run_event(&self.state.run_id, type_name, payload);
        }
    }
}
