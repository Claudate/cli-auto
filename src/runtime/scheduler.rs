//! Provider-agnostic scheduler.
//!
//! [INPUT]: PlanIR · RunState · ProviderRegistry · TerminalManager 可选 · 预算/并行上限 · retry/stall 巡检 · failover
//! [OUTPUT]: 推进任务状态 · events.jsonl · plan.resolved.json · handoff · [CCO_HANDOFF] ·
//!           inspect VERDICT 门禁(P2-3+P-loop) · sys-post-git-push 先巡检 PASS 硬门禁 ·
//!           task_retry / provider_switched(H4) · 终态 RunStatus
//! [POS]: runtime 调度核心；CLI run 与 services.start_run_* 共用
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};

use super::acceptance::run_acceptance_soft;
use super::handoff;
use super::provider::{
    ProviderRegistry, StartCtx, TaskStatus, WorkerHandle, WorkerProvider, WorkerStatus,
};
use super::worktree;
use crate::graph::ready_tasks;
use crate::plan::{OnFailure, PlanIR, TaskIR};
use crate::state::{RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

/// In-memory progress fingerprint for stall patrol (not persisted).
struct ProgressWatch {
    last_bytes: u64,
    last_change: chrono::DateTime<chrono::Utc>,
}

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
    /// H4: after same-provider retries exhaust, switch claude↔codex and re-try.
    /// Manual stop never triggers failover. Default true (config.default.failover_enabled).
    pub failover_enabled: bool,
    /// Extra attempts allowed on the fallback provider after a switch (default 1).
    pub fallback_extra_attempts: u32,
}

impl Scheduler {
    pub async fn run(mut self) -> Result<RunStatus> {
        self.state.status = RunStatus::Validated;
        self.state.save()?;
        self.state.event(
            "run_start",
            serde_json::json!({
                "run_id": self.state.run_id,
                "project": self.state.project_root,
                "plan": self.state.plan_path,
            }),
        )?;

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
        let mut running: HashMap<String, (Arc<dyn WorkerProvider>, WorkerHandle, PathBuf)> =
            HashMap::new();
        // Tasks that have been *accepted* as started at least once this run (not cleared on retry).
        let mut started: HashSet<String> = HashSet::new();
        // In-memory stall patrol: stdout size fingerprint per running task.
        let mut progress: HashMap<String, ProgressWatch> = HashMap::new();
        let retry_budget = self.effective_retry_max();
        // Floor 1s (tests); production default is 180. Config UI can clamp higher.
        let stall_for = Duration::from_secs(self.stall_secs.max(1));
        info!(
            retry_max = retry_budget,
            stall_secs = stall_for.as_secs(),
            failover = self.failover_enabled,
            fallback_extra = self.fallback_extra_attempts,
            "scheduler patrol armed"
        );

        loop {
            // ── Reap finished workers ─────────────────────────────────
            let ids: Vec<String> = running.keys().cloned().collect();
            for id in ids {
                let (provider, handle, _) = running.get(&id).unwrap();
                let st = provider.poll(handle).await?;
                match st {
                    WorkerStatus::Running => {
                        // Stall patrol: no stdout growth for stall_secs → stop + retry/pause.
                        if let Some(action) = self
                            .patrol_stall(&id, handle, &mut progress, stall_for)
                            .await?
                        {
                            let (provider, handle, work_dir) = running.remove(&id).unwrap();
                            progress.remove(&id);
                            // Force-stop the hung worker before collect.
                            if let Err(e) = provider.stop(&handle).await {
                                warn!(task = %id, err = %e, "stall stop failed");
                            }
                            let mut result = provider.collect(&handle).await.unwrap_or_else(|e| {
                                super::provider::TaskResult {
                                    status: TaskStatus::Timeout,
                                    exit_code: Some(124),
                                    stdout_path: Some(handle.stdout_path.clone()),
                                    session_id: None,
                                    agent_id: None,
                                    cost_usd: None,
                                    raw: serde_json::json!({}),
                                    error: Some(format!("stall collect: {e:#}")),
                                }
                            });
                            result.status = TaskStatus::Timeout;
                            if result.error.is_none() {
                                result.error = Some(action.reason.clone());
                            }
                            let retried = self
                                .finish_or_retry(
                                    &id,
                                    result,
                                    &action.reason_code,
                                    &mut done,
                                    &mut failed,
                                    &mut started,
                                    retry_budget,
                                    Some(&work_dir),
                                )
                                .await?;
                            if retried {
                                // Allowed back into ready set (started bit cleared for this id).
                            }
                            if self.budget_exceeded()? {
                                failed.insert("__budget__".into());
                            }
                        }
                    }
                    other => {
                        let (provider, handle, work_dir) = running.remove(&id).unwrap();
                        progress.remove(&id);
                        let mut result = provider.collect(&handle).await?;

                        // acceptance gate + outputs check (P1-4)
                        if result.status == TaskStatus::Done {
                            if let Some(task) = self.plan.task(&id) {
                                if let Some(cmd) = &task.acceptance {
                                    let acc = run_acceptance_soft(
                                        &work_dir,
                                        cmd,
                                        Duration::from_secs(300),
                                    )
                                    .await;
                                    let acc_path = self.state.task_dir(&id).join("acceptance.json");
                                    let _ = std::fs::write(
                                        &acc_path,
                                        serde_json::to_string_pretty(&serde_json::json!({
                                            "ok": acc.ok,
                                            "exit_code": acc.exit_code,
                                            "stdout": acc.stdout.chars().take(2000).collect::<String>(),
                                            "stderr": acc.stderr.chars().take(2000).collect::<String>(),
                                            "command": cmd,
                                        }))
                                        .unwrap_or_default(),
                                    );
                                    if !acc.ok {
                                        result.status = TaskStatus::Failed;
                                        result.error = Some(format!(
                                            "acceptance failed: {}",
                                            acc.stderr.chars().take(300).collect::<String>()
                                        ));
                                    }
                                }
                                if result.status == TaskStatus::Done {
                                    self.enforce_outputs(task, &work_dir, &mut result);
                                }
                                // P2-3: after outputs exist, VERDICT=FAIL → Failed (pause path).
                                if result.status == TaskStatus::Done {
                                    self.enforce_inspect_verdict(task, &work_dir, &mut result);
                                }
                            }
                        }

                        let reason_code = match other {
                            WorkerStatus::Timeout => "timeout",
                            WorkerStatus::Stopped => "stopped",
                            WorkerStatus::Failed => "fail",
                            WorkerStatus::Done
                                if result.status != TaskStatus::Done =>
                            {
                                "fail"
                            }
                            _ => "ok",
                        };
                        if reason_code == "ok" && result.status == TaskStatus::Done {
                            self.apply_result(&id, &result)?;
                            done.insert(id.clone());
                            info!(task = %id, cost = ?result.cost_usd, "task done");
                            self.state.event(
                                "task_end",
                                serde_json::json!({
                                    "task_id": id,
                                    "status": result.status,
                                    "cost_usd": result.cost_usd,
                                    "error": result.error,
                                }),
                            )?;
                            self.state.save()?;
                            self.handoff_task_end(&id, &result, Some(&work_dir));
                        } else {
                            let _ = self
                                .finish_or_retry(
                                    &id,
                                    result,
                                    reason_code,
                                    &mut done,
                                    &mut failed,
                                    &mut started,
                                    retry_budget,
                                    Some(&work_dir),
                                )
                                .await?;
                        }

                        if self.budget_exceeded()? {
                            failed.insert("__budget__".into());
                        }
                    }
                }
            }

            if failed.contains("__budget__") && running.is_empty() {
                self.state.status = RunStatus::Paused;
                self.state.save()?;
                self.state.event(
                    "run_end",
                    serde_json::json!({"status": "paused", "reason": "budget_exceeded"}),
                )?;
                let _ = handoff::on_run_end(&self.plan, &self.state, RunStatus::Paused);
                return Ok(RunStatus::Paused);
            }

            if !failed.is_empty() && matches!(self.plan.on_failure, OnFailure::Pause) {
                if running.is_empty() {
                    self.state.status = RunStatus::Paused;
                    self.state.save()?;
                    self.state.event(
                        "run_end",
                        serde_json::json!({"status": "paused", "failed": failed}),
                    )?;
                    let _ = handoff::on_run_end(&self.plan, &self.state, RunStatus::Paused);
                    return Ok(RunStatus::Paused);
                }
            }

            if (failed.is_empty() || matches!(self.plan.on_failure, OnFailure::Continue))
                && !failed.contains("__budget__")
            {
                let mut ready = ready_tasks(&self.plan, &done, &started);
                ready.retain(|id| active_ids.contains(id));
                if matches!(self.plan.on_failure, OnFailure::Continue) {
                    for id in ready.clone() {
                        if let Some(t) = self.plan.task(&id) {
                            if t.depends_on.iter().any(|d| failed.contains(d)) {
                                ready.retain(|x| x != &id);
                                if let Some(ts) = self.state.tasks.get_mut(&id) {
                                    ts.status = TaskStatus::Skipped;
                                }
                                done.insert(id);
                            }
                        }
                    }
                }

                while running.len() < self.max_parallel {
                    // find first ready task that respects per-provider cap
                    let mut pick: Option<usize> = None;
                    for (i, id) in ready.iter().enumerate() {
                        if let Some(task) = self.plan.task(id) {
                            if self.provider_slot_available(&running, &task.provider) {
                                pick = Some(i);
                                break;
                            }
                        }
                    }
                    let Some(idx) = pick else {
                        break;
                    };
                    let id = ready.remove(idx);
                    let task = self
                        .plan
                        .task(&id)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("missing task {id}"))?;
                    // Host hard-gate: sys-post-git-push only after inspect VERDICT=PASS
                    if let Err(reason) = handoff::system_push_inspect_gate(
                        &self.plan,
                        &task,
                        &self.state.project_root,
                    ) {
                        warn!(task = %id, %reason, "skip system push: inspect gate");
                        if let Some(ts) = self.state.tasks.get_mut(&id) {
                            ts.status = TaskStatus::Skipped;
                            ts.error = Some(reason.clone());
                            ts.finished_at = Some(chrono::Utc::now());
                        }
                        done.insert(id.clone());
                        started.insert(id.clone());
                        let _ = self.state.event(
                            "task_end",
                            serde_json::json!({
                                "task_id": id,
                                "status": "skipped",
                                "error": reason,
                                "gate": "system_push_inspect",
                            }),
                        );
                        let _ = self.state.save();
                        continue;
                    }
                    match self.start_task(&task).await {
                        Ok((provider, handle, work_dir)) => {
                            started.insert(id.clone());
                            let attempt = if let Some(ts) = self.state.tasks.get_mut(&id) {
                                ts.attempt = ts.attempt.saturating_add(1);
                                ts.status = TaskStatus::Running;
                                ts.started_at = Some(chrono::Utc::now());
                                ts.finished_at = None;
                                ts.error = None;
                                ts.work_dir = Some(work_dir.clone());
                                ts.pid = handle.pid;
                                ts.attempt
                            } else {
                                1
                            };
                            // Seed stall watch from current stdout size (retry may keep file).
                            let bytes = stdout_len(&handle.stdout_path);
                            progress.insert(
                                id.clone(),
                                ProgressWatch {
                                    last_bytes: bytes,
                                    last_change: chrono::Utc::now(),
                                },
                            );
                            self.maybe_open_terminal(&id, &work_dir, &handle)?;
                            self.state.event(
                                "task_start",
                                serde_json::json!({
                                    "task_id": id,
                                    "provider": task.provider,
                                    "mode": task.mode,
                                    "pid": handle.pid,
                                    "work_dir": work_dir,
                                    "attempt": attempt,
                                }),
                            )?;
                            self.state.save()?;
                            // P1-4: Board → running
                            if let Err(e) = handoff::on_task_start(&self.plan, &self.state, &id) {
                                warn!(task = %id, err = %e, "handoff task_start failed");
                            }

                            let st = provider.poll(&handle).await?;
                            if !matches!(st, WorkerStatus::Running) {
                                progress.remove(&id);
                                let mut result = provider.collect(&handle).await?;
                                if result.status == TaskStatus::Done {
                                    if let Some(cmd) = &task.acceptance {
                                        let acc = run_acceptance_soft(
                                            &work_dir,
                                            cmd,
                                            Duration::from_secs(300),
                                        )
                                        .await;
                                        if !acc.ok {
                                            result.status = TaskStatus::Failed;
                                            result.error = Some(format!(
                                                "acceptance failed: {}",
                                                acc.stderr.chars().take(300).collect::<String>()
                                            ));
                                        }
                                    }
                                    if result.status == TaskStatus::Done {
                                        self.enforce_outputs(&task, &work_dir, &mut result);
                                    }
                                    if result.status == TaskStatus::Done {
                                        self.enforce_inspect_verdict(
                                            &task,
                                            &work_dir,
                                            &mut result,
                                        );
                                    }
                                }
                                if result.status == TaskStatus::Done {
                                    self.apply_result(&id, &result)?;
                                    done.insert(id.clone());
                                    self.state.event(
                                        "task_end",
                                        serde_json::json!({
                                            "task_id": id,
                                            "status": result.status,
                                            "cost_usd": result.cost_usd,
                                            "attempt": attempt,
                                        }),
                                    )?;
                                    self.state.save()?;
                                    self.handoff_task_end(&id, &result, Some(&work_dir));
                                } else {
                                    // Align with reap-loop reason codes so stop never retries/failovers.
                                    let reason = match result.status {
                                        TaskStatus::Timeout => "timeout",
                                        TaskStatus::Stopped => "stopped",
                                        _ => "fail",
                                    };
                                    let _ = self
                                        .finish_or_retry(
                                            &id,
                                            result,
                                            reason,
                                            &mut done,
                                            &mut failed,
                                            &mut started,
                                            retry_budget,
                                            Some(&work_dir),
                                        )
                                        .await?;
                                }
                                if self.budget_exceeded()? {
                                    failed.insert("__budget__".into());
                                }
                            } else {
                                running.insert(id, (provider, handle, work_dir));
                            }
                        }
                        Err(e) => {
                            error!(task = %id, err = %e, "failed to start");
                            let attempt = if let Some(ts) = self.state.tasks.get_mut(&id) {
                                ts.attempt = ts.attempt.saturating_add(1);
                                ts.attempt
                            } else {
                                1
                            };
                            started.insert(id.clone());
                            let result = super::provider::TaskResult {
                                status: TaskStatus::Failed,
                                exit_code: None,
                                stdout_path: None,
                                session_id: None,
                                agent_id: None,
                                cost_usd: None,
                                raw: serde_json::json!({}),
                                error: Some(format!("{e:#}")),
                            };
                            let retried = self
                                .finish_or_retry(
                                    &id,
                                    result,
                                    "start_fail",
                                    &mut done,
                                    &mut failed,
                                    &mut started,
                                    retry_budget,
                                    None,
                                )
                                .await?;
                            if !retried && matches!(self.plan.on_failure, OnFailure::Pause) {
                                break;
                            }
                            let _ = attempt;
                        }
                    }
                }
            }

            let all_terminal = self.plan.tasks.iter().all(|t| {
                !active_ids.contains(&t.id)
                    || done.contains(&t.id)
                    || failed.contains(&t.id)
                    || self
                        .state
                        .tasks
                        .get(&t.id)
                        .map(|s| s.status.is_terminal())
                        .unwrap_or(false)
            });

            if running.is_empty() && all_terminal {
                break;
            }
            if running.is_empty()
                && ready_tasks(&self.plan, &done, &started)
                    .into_iter()
                    .all(|id| !active_ids.contains(&id))
            {
                if !failed.is_empty() {
                    break;
                }
                if started.len() >= active_ids.len() {
                    break;
                }
            }

            tokio::time::sleep(self.poll_interval).await;
        }

        let status = if !failed.is_empty() {
            if matches!(self.plan.on_failure, OnFailure::Pause) {
                RunStatus::Paused
            } else {
                RunStatus::Failed
            }
        } else {
            RunStatus::Completed
        };
        self.state.status = status;
        self.state.finished_at = Some(chrono::Utc::now());
        self.state.save()?;
        self.state.event(
            "run_end",
            serde_json::json!({
                "status": status,
            }),
        )?;
        let _ = handoff::on_run_end(&self.plan, &self.state, status);

        if let Some(mirror) = &self.mirror_state {
            let _ = mirror_run(&self.state.run_dir, mirror);
        }

        Ok(status)
    }

    fn maybe_open_terminal(
        &mut self,
        task_id: &str,
        work_dir: &std::path::Path,
        handle: &WorkerHandle,
    ) -> Result<()> {
        if !self.auto_open_terminal {
            return Ok(());
        }
        let Some(tm) = &self.terminal_manager else {
            return Ok(());
        };
        let stderr = handle
            .stdout_path
            .parent()
            .map(|p| p.join("stderr.log"))
            .unwrap_or_else(|| work_dir.join("stderr.log"));
        match tm.open_follow_logs(
            task_id,
            work_dir,
            &handle.stdout_path,
            &stderr,
            self.terminal_kind,
        ) {
            Ok(session) => {
                if let Some(ts) = self.state.tasks.get_mut(task_id) {
                    ts.terminals.push(session.id.clone());
                }
                let _ = self.state.event(
                    "terminal_open",
                    serde_json::json!({
                        "task_id": task_id,
                        "kind": session.kind,
                        "session_id": session.id,
                    }),
                );
            }
            Err(e) => warn!(task = %task_id, err = %e, "auto-open terminal failed"),
        }
        Ok(())
    }

    fn active_task_ids(&self) -> Result<HashSet<String>> {
        let all: HashSet<String> = self.plan.tasks.iter().map(|t| t.id.clone()).collect();
        if let Some(only) = &self.only {
            for id in only {
                if !all.contains(id) {
                    bail!("--only unknown task: {id}");
                }
            }
            return Ok(only.clone());
        }
        if let Some(from) = &self.from_task {
            if !all.contains(from) {
                bail!("--from-task unknown: {from}");
            }
            let mut include = HashSet::new();
            include.insert(from.clone());
            let mut changed = true;
            while changed {
                changed = false;
                for t in &self.plan.tasks {
                    if t.depends_on.iter().any(|d| include.contains(d)) && !include.contains(&t.id)
                    {
                        include.insert(t.id.clone());
                        changed = true;
                    }
                }
            }
            return Ok(include);
        }
        Ok(all)
    }

    async fn start_task(
        &self,
        task: &TaskIR,
    ) -> Result<(Arc<dyn WorkerProvider>, WorkerHandle, PathBuf)> {
        let provider = self.registry.get(&task.provider)?;
        provider.validate_task(task)?;
        let task_dir = self.state.task_dir(&task.id);
        std::fs::create_dir_all(&task_dir)?;

        let want_wt = task.worktree.unwrap_or(self.plan.worktree);
        // P1-3: multi-provider mix-run must not silent-fallback to shared project_root.
        let on_fail = if worktree::is_multi_provider(
            self.plan.tasks.iter().map(|t| t.provider.as_str()),
        ) {
            worktree::WorktreeOnFail::FailClosed
        } else {
            worktree::WorktreeOnFail::FallbackProjectRoot
        };
        let (work_dir, wt_info) = worktree::resolve_work_dir(
            &self.state.project_root,
            &self.state.run_id,
            &task.id,
            want_wt,
            on_fail,
        )?;

        // Persist work dir for term open later
        let meta = serde_json::json!({
            "work_dir": work_dir,
            "worktree_branch": wt_info.as_ref().map(|w| &w.branch),
            "worktree_path": wt_info.as_ref().map(|w| &w.path),
        });
        std::fs::write(task_dir.join("work_dir.json"), serde_json::to_string_pretty(&meta)?)?;

        if let Some(ref info) = wt_info {
            // also stamp on state if already inserted
            info!(
                task = %task.id,
                path = %info.path.display(),
                branch = %info.branch,
                "using worktree"
            );
        }

        let ctx = StartCtx {
            run_id: self.state.run_id.clone(),
            project_root: self.state.project_root.clone(),
            work_dir: work_dir.clone(),
            task_dir,
            env_extra: vec![],
        };

        // P1-5: host injects latest handoff summary as prompt prefix (once, scheduler-side).
        // Providers see the wrapped prompt; plan.resolved.json keeps the original business prompt.
        let mut task_for_start = task.clone();
        task_for_start.prompt =
            handoff::with_handoff_prefix(&task.prompt, task, &self.state.run_dir);

        let handle = provider.start(&task_for_start, &ctx).await?;

        // update branch on task state if present — caller sets running; we need branch
        // write branch into a side file already done

        Ok((provider, handle, work_dir))
    }

    fn provider_slot_available(
        &self,
        running: &HashMap<String, (Arc<dyn WorkerProvider>, WorkerHandle, PathBuf)>,
        provider: &str,
    ) -> bool {
        let Some(&cap) = self.provider_max_parallel.get(provider) else {
            return true;
        };
        let used = running
            .values()
            .filter(|(p, _, _)| p.name() == provider)
            .count();
        used < cap
    }

    fn budget_exceeded(&self) -> Result<bool> {
        let Some(cap) = self.run_max_budget_usd else {
            return Ok(false);
        };
        let spent = self.state.total_cost_usd();
        if spent > cap + f64::EPSILON {
            warn!(spent, cap, "run budget exceeded");
            self.state.event(
                "budget_exceeded",
                serde_json::json!({"spent": spent, "cap": cap}),
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    /// plan.retry_max wins if higher; otherwise scheduler/config default. Cap 10.
    fn effective_retry_max(&self) -> u32 {
        self.plan.retry_max.max(self.retry_max).min(10)
    }

    /// Production failover target: claude↔codex only. `fake` and others → None.
    fn production_failover_target(current: &str) -> Option<&'static str> {
        match current {
            "claude" => Some("codex"),
            "codex" => Some("claude"),
            _ => None,
        }
    }

    /// Resolve a live fallback provider when H4 failover is armed and preflight passes.
    async fn resolve_failover_provider(&self, current: &str) -> Option<String> {
        if !self.failover_enabled {
            return None;
        }
        let target = Self::production_failover_target(current)?;
        let provider = match self.registry.get(target) {
            Ok(p) => p,
            Err(_) => return None,
        };
        if provider.preflight().await.is_err() {
            warn!(
                from = %current,
                to = %target,
                "failover preflight failed — skipping switch"
            );
            return None;
        }
        Some(target.to_string())
    }

    /// Attempt budget for this task: after failover, only `fallback_extra_attempts`.
    fn attempt_budget_for(&self, id: &str, same_provider_budget: u32) -> u32 {
        let used = self
            .state
            .tasks
            .get(id)
            .map(|t| t.failover_used)
            .unwrap_or(false);
        if used {
            self.fallback_extra_attempts.min(10)
        } else {
            same_provider_budget
        }
    }

    /// Update progress fingerprint; return Some(action) when stalled long enough.
    async fn patrol_stall(
        &self,
        id: &str,
        handle: &WorkerHandle,
        progress: &mut HashMap<String, ProgressWatch>,
        stall_for: Duration,
    ) -> Result<Option<StallAction>> {
        let bytes = stdout_len(&handle.stdout_path);
        let now = chrono::Utc::now();
        let entry = progress.entry(id.to_string()).or_insert_with(|| ProgressWatch {
            last_bytes: bytes,
            last_change: now,
        });
        if bytes > entry.last_bytes {
            entry.last_bytes = bytes;
            entry.last_change = now;
            return Ok(None);
        }
        let idle = now
            .signed_duration_since(entry.last_change)
            .to_std()
            .unwrap_or_default();
        if idle < stall_for {
            return Ok(None);
        }
        let secs = idle.as_secs();
        warn!(
            task = %id,
            idle_secs = secs,
            log_bytes = bytes,
            "stall detected — no log growth"
        );
        Ok(Some(StallAction {
            reason_code: "stall".into(),
            reason: format!(
                "CLI 卡死：日志 {secs}s 无增长（阈值 {}s，stdout {bytes} bytes）",
                stall_for.as_secs()
            ),
        }))
    }

    /// Apply terminal failure. If attempts remain, reset to Pending and clear `started`
    /// so the ready set can pick it up again. When same-provider budget is exhausted and
    /// H4 failover is armed, switch `task.provider` (run-state + in-memory plan) and re-queue.
    /// User-initiated stop (`reason_code == "stopped"`) never retries and never failovers.
    /// Returns true when a retry (same house or switched) was scheduled.
    async fn finish_or_retry(
        &mut self,
        id: &str,
        result: super::provider::TaskResult,
        reason_code: &str,
        done: &mut HashSet<String>,
        failed: &mut HashSet<String>,
        started: &mut HashSet<String>,
        retry_budget: u32,
        _work_dir: Option<&std::path::Path>,
    ) -> Result<bool> {
        let attempt = self
            .state
            .tasks
            .get(id)
            .map(|t| t.attempt.max(1))
            .unwrap_or(1);
        let budget = self.attempt_budget_for(id, retry_budget);
        // User-initiated stop: never auto-retry, never failover.
        let non_retryable = reason_code == "stopped" || reason_code == "ok";
        let can_same_retry = !non_retryable && attempt <= budget;

        if can_same_retry {
            // Archive this attempt's logs so the next try starts clean.
            self.archive_attempt_logs(id, attempt);
            self.clear_done_flag(id);

            if let Some(ts) = self.state.tasks.get_mut(id) {
                ts.status = TaskStatus::Pending;
                ts.error = result.error.clone().or_else(|| {
                    Some(format!("{reason_code} on attempt {attempt}"))
                });
                ts.finished_at = None;
                ts.started_at = None;
                ts.pid = None;
                ts.last_retry_reason = Some(reason_code.to_string());
                // Keep cost_usd accumulated if any? Prefer last non-null; leave as-is.
            }
            started.remove(id);
            self.state.event(
                "task_retry",
                serde_json::json!({
                    "task_id": id,
                    "attempt": attempt,
                    "next_attempt": attempt + 1,
                    "retry_max": budget,
                    "reason": reason_code,
                    "error": result.error,
                }),
            )?;
            self.state.save()?;
            info!(
                task = %id,
                attempt,
                next = attempt + 1,
                reason = reason_code,
                "auto-retry scheduled"
            );
            return Ok(true);
        }

        // Same-provider budget exhausted → H4 provider failover (once).
        if !non_retryable {
            let already_failed_over = self
                .state
                .tasks
                .get(id)
                .map(|t| t.failover_used)
                .unwrap_or(false);
            if !already_failed_over {
                let current = self
                    .plan
                    .task(id)
                    .map(|t| t.provider.clone())
                    .or_else(|| {
                        self.state
                            .tasks
                            .get(id)
                            .map(|t| t.provider.clone())
                    })
                    .unwrap_or_default();
                if let Some(fallback) = self.resolve_failover_provider(&current).await {
                    self.archive_attempt_logs(id, attempt);
                    self.clear_done_flag(id);

                    // Run-state override: mutate in-memory plan + task state (not source plan file).
                    if let Some(t) = self.plan.tasks.iter_mut().find(|t| t.id == id) {
                        t.provider = fallback.clone();
                    }
                    // Keep plan.resolved.json in sync with run-time overrides.
                    let resolved = self.state.run_dir.join("plan.resolved.json");
                    if let Ok(text) = serde_json::to_string_pretty(&self.plan) {
                        let _ = std::fs::write(&resolved, text);
                    }

                    if let Some(ts) = self.state.tasks.get_mut(id) {
                        ts.provider = fallback.clone();
                        ts.failover_used = true;
                        // Reset attempt so fallback house starts at attempt 1.
                        ts.attempt = 0;
                        ts.status = TaskStatus::Pending;
                        ts.error = result.error.clone().or_else(|| {
                            Some(format!(
                                "{reason_code} after {attempt} attempt(s); failover {current}→{fallback}"
                            ))
                        });
                        ts.finished_at = None;
                        ts.started_at = None;
                        ts.pid = None;
                        ts.last_retry_reason = Some(format!("failover:{reason_code}"));
                    }
                    started.remove(id);

                    self.state.event(
                        "provider_switched",
                        serde_json::json!({
                            "task_id": id,
                            "from": current,
                            "to": fallback,
                            "reason": reason_code,
                            "attempt": attempt,
                            "fallback_extra_attempts": self.fallback_extra_attempts.min(10),
                        }),
                    )?;
                    self.state.event(
                        "task_retry",
                        serde_json::json!({
                            "task_id": id,
                            "attempt": attempt,
                            "next_attempt": 1,
                            "retry_max": self.fallback_extra_attempts.min(10),
                            "reason": format!("failover:{reason_code}"),
                            "error": result.error,
                            "provider": fallback,
                            "from_provider": current,
                        }),
                    )?;
                    self.state.save()?;
                    info!(
                        task = %id,
                        from = %current,
                        to = %fallback,
                        reason = reason_code,
                        "provider failover scheduled"
                    );
                    return Ok(true);
                }
            }
        }

        // Exhausted or non-retryable → permanent fail / timeout.
        self.apply_result(id, &result)?;
        self.handoff_task_end(id, &result, _work_dir);
        if let Some(ts) = self.state.tasks.get_mut(id) {
            ts.last_retry_reason = Some(reason_code.to_string());
            if ts.error.is_none() {
                ts.error = Some(format!(
                    "{reason_code} after {attempt} attempt(s) (retry_max={budget})"
                ));
            } else if attempt > 1 {
                let prev = ts.error.clone().unwrap_or_default();
                ts.error = Some(format!(
                    "{prev} · 已重试 {}/{} 次仍失败",
                    attempt.saturating_sub(1),
                    budget
                ));
            }
        }
        if result.status == TaskStatus::Done {
            done.insert(id.to_string());
        } else {
            failed.insert(id.to_string());
            warn!(
                task = %id,
                attempt,
                reason = reason_code,
                err = ?result.error,
                "task failed (retries exhausted or non-retryable)"
            );
        }
        self.state.event(
            "task_end",
            serde_json::json!({
                "task_id": id,
                "status": result.status,
                "cost_usd": result.cost_usd,
                "error": result.error,
                "attempt": attempt,
                "reason": reason_code,
                "retries_exhausted": attempt > 1,
            }),
        )?;
        self.state.save()?;
        Ok(false)
    }

    fn archive_attempt_logs(&self, id: &str, attempt: u32) {
        let dir = self.state.task_dir(id);
        let stamp = format!("attempt-{attempt}");
        for name in ["stdout.json", "stderr.log", "meta.json", "status.json"] {
            let src = dir.join(name);
            if src.exists() {
                let dst = dir.join(format!("{stamp}.{name}"));
                let _ = std::fs::rename(&src, &dst);
            }
        }
        // Truncate / recreate empty stdout so stall watch starts from 0 on retry.
        let _ = std::fs::write(dir.join("stdout.json"), "");
    }

    fn clear_done_flag(&self, id: &str) {
        let dir = self.state.task_dir(id);
        let _ = std::fs::remove_file(dir.join(".done"));
    }

    /// P1-4: if TaskIR.outputs non-empty and any missing → Failed.
    fn enforce_outputs(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut super::provider::TaskResult,
    ) {
        if result.status != TaskStatus::Done {
            return;
        }
        let missing = handoff::missing_outputs(task, work_dir, &self.state.project_root);
        if missing.is_empty() {
            return;
        }
        result.status = TaskStatus::Failed;
        result.error = Some(format!(
            "missing outputs: {}",
            missing.join(", ")
        ));
        warn!(
            task = %task.id,
            missing = ?missing,
            "task failed: required outputs missing"
        );
    }

    /// P2-3 + P-loop: inspect VERDICT gate.
    /// - FAIL → task Failed
    /// - UNKNOWN when `require_inspect` or role=inspect → Failed (Unknown ≡ FAIL)
    /// - PASS but blocking/map ISSUES remain → Failed (no silent residual PASS)
    /// Does **not** auto-merge / open PR; `on_failure: pause` applies. ISSUES → handoff.
    fn enforce_inspect_verdict(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut super::provider::TaskResult,
    ) {
        if result.status != TaskStatus::Done {
            return;
        }
        if !handoff::task_has_verdict_gate(task) {
            return;
        }
        let verdict =
            handoff::read_inspect_verdict(task, work_dir, &self.state.project_root);
        let issues =
            handoff::collect_inspect_issues(task, work_dir, &self.state.project_root);
        let (blocked, blocking_n) = handoff::inspect_pass_blocked_by_issues(
            task,
            work_dir,
            &self.state.project_root,
        );
        let treat_unknown_as_fail = self.plan.require_inspect
            || task.role == Some(crate::plan::TaskRole::Inspect);

        let fail_reason = match verdict {
            handoff::InspectVerdict::Fail => {
                let issues_hint = if issues.is_empty() {
                    format!("see {}", handoff::INSPECT_ISSUES_REL)
                } else {
                    format!(
                        "{} ISSUES line(s) for rework (Open risks ISSUES[{}])",
                        issues.len(),
                        task.id
                    )
                };
                Some(format!("inspect VERDICT=FAIL ({issues_hint})"))
            }
            handoff::InspectVerdict::Unknown if treat_unknown_as_fail => Some(format!(
                "inspect VERDICT=UNKNOWN (require_inspect/role=inspect treats Unknown as FAIL; expected {})",
                handoff::INSPECT_VERDICT_REL
            )),
            handoff::InspectVerdict::Pass if blocked => Some(format!(
                "inspect VERDICT=PASS but {blocking_n} blocking/map ISSUE(s) remain — cannot close plan loop (P-loop R-inspect)"
            )),
            _ => None,
        };

        let Some(reason) = fail_reason else {
            return;
        };
        result.status = TaskStatus::Failed;
        result.error = Some(reason.clone());
        warn!(
            task = %task.id,
            issues = issues.len(),
            blocking = blocking_n,
            ?verdict,
            "task failed: inspect gate ({reason})"
        );
    }

    /// P1-4: host merges fragment into global handoff (never written by worker).
    fn handoff_task_end(
        &self,
        id: &str,
        result: &super::provider::TaskResult,
        work_dir: Option<&std::path::Path>,
    ) {
        let Some(task) = self.plan.task(id) else {
            return;
        };
        if let Err(e) = handoff::on_task_end(&self.plan, &self.state, task, result, work_dir) {
            warn!(task = %id, err = %e, "handoff task_end failed");
        }
    }

    fn apply_result(
        &mut self,
        id: &str,
        result: &super::provider::TaskResult,
    ) -> Result<()> {
        // load worktree branch from work_dir.json if any
        let wd_meta = self
            .state
            .task_dir(id)
            .join("work_dir.json");
        let (work_dir, branch) = if wd_meta.exists() {
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&wd_meta).unwrap_or_default())
                    .unwrap_or_default();
            (
                v.get("work_dir")
                    .and_then(|x| x.as_str())
                    .map(PathBuf::from),
                v.get("worktree_branch")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
            )
        } else {
            (None, None)
        };

        if let Some(ts) = self.state.tasks.get_mut(id) {
            ts.status = result.status;
            ts.session_id = result.session_id.clone();
            ts.agent_id = result.agent_id.clone();
            ts.cost_usd = result.cost_usd;
            ts.exit_code = result.exit_code;
            ts.error = result.error.clone();
            ts.finished_at = Some(chrono::Utc::now());
            if ts.work_dir.is_none() {
                ts.work_dir = work_dir;
            }
            if ts.worktree_branch.is_none() {
                ts.worktree_branch = branch;
            }
        }
        let dir = self.state.task_dir(id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("status.json"),
            serde_json::to_string_pretty(result)?,
        )?;
        Ok(())
    }
}

struct StallAction {
    reason_code: String,
    reason: String,
}

fn stdout_len(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn mirror_run(src: &std::path::Path, dst_root: &std::path::Path) -> Result<()> {
    let name = src.file_name().context("run dir name")?;
    let dst = dst_root.join(name);
    copy_dir_all(src, &dst)?;
    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for ent in std::fs::read_dir(src)? {
        let ent = ent?;
        let ty = ent.file_type()?;
        let to = dst.join(ent.file_name());
        if ty.is_dir() {
            copy_dir_all(&ent.path(), &to)?;
        } else {
            std::fs::copy(ent.path(), to)?;
        }
    }
    Ok(())
}
