//! One scheduler tick: external stop · reap · spawn · exit predicate.
//!
//! [INPUT]: running map · done/failed/started · active ids
//! [OUTPUT]: optional early RunStatus · side effects on state
//! [POS]: runtime/scheduler loop body (keeps mod.rs thin)
//! [PROTOCOL]: stop freezes Pending; never re-spawn terminal tasks

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::{error, info, warn};

use super::super::acceptance::run_acceptance_soft;
use super::super::handoff;
use super::super::provider::{TaskStatus, WorkerHandle, WorkerPort, WorkerStatus};
use super::types::{stdout_len, ProgressWatch};
use super::Scheduler;
use crate::domain::worker::{
    may_budget_downgrade, role_default_tier, suggest_budget_downgrade,
};
use crate::graph::ready_tasks;
use crate::plan::OnFailure;
use crate::state::{RouteSource, RunState, RunStatus};

impl Scheduler {
    /// Reload disk stop signal; kill workers; freeze live tasks. Returns Some(status) to exit.
    pub(super) async fn handle_external_stop(
        &mut self,
        running: &mut HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)>,
        progress: &mut HashMap<String, ProgressWatch>,
        done: &mut HashSet<String>,
        failed: &mut HashSet<String>,
        started: &mut HashSet<String>,
        retry_budget: u32,
    ) -> Result<Option<RunStatus>> {
        let Ok(disk) = RunState::load(&self.state.run_dir) else {
            return Ok(None);
        };
        let external_stop = matches!(disk.status, RunStatus::Aborted | RunStatus::Paused);

        for (id, dts) in &disk.tasks {
            if !dts.status.is_terminal() {
                continue;
            }
            if let Some(mts) = self.state.tasks.get_mut(id) {
                // stop_run wins over kill race: disk Stopped must override in-memory
                // Failed from stream exit -1 (SIGKILL), even if already terminal.
                let stop_overrides_fail = matches!(dts.status, TaskStatus::Stopped)
                    && matches!(mts.status, TaskStatus::Failed | TaskStatus::Timeout);
                if !mts.status.is_terminal() || stop_overrides_fail {
                    mts.status = dts.status;
                    mts.finished_at = dts.finished_at.or(mts.finished_at);
                    mts.pid = None;
                    if stop_overrides_fail {
                        mts.error = None;
                        mts.exit_code = Some(130);
                    }
                }
            }
            match dts.status {
                TaskStatus::Stopped | TaskStatus::Skipped | TaskStatus::Done => {
                    done.insert(id.clone());
                    failed.remove(id);
                    started.insert(id.clone());
                }
                TaskStatus::Failed | TaskStatus::Timeout => {
                    failed.insert(id.clone());
                    started.insert(id.clone());
                }
                _ => {}
            }
        }

        if !external_stop {
            return Ok(None);
        }

        let live: Vec<String> = running.keys().cloned().collect();
        for id in live {
            if let Some((provider, handle, work_dir)) = running.remove(&id) {
                progress.remove(&id);
                if let Err(e) = provider.stop(&handle).await {
                    warn!(task = %id, err = %e, "external-stop provider.stop failed");
                }
                let mut result = provider.collect(&handle).await.unwrap_or_else(|e| {
                    super::super::provider::TaskResult {
                        status: TaskStatus::Stopped,
                        exit_code: Some(130),
                        stdout_path: Some(handle.stdout_path.clone()),
                        session_id: None,
                        agent_id: None,
                        cost_usd: None,
                        raw: serde_json::json!({}),
                        error: Some(format!("external stop collect: {e:#}")),
                    }
                });
                result.status = TaskStatus::Stopped;
                if result.exit_code.is_none() {
                    result.exit_code = Some(130);
                }
                let _ = self
                    .finish_or_retry(
                        &id,
                        result,
                        "stopped",
                        done,
                        failed,
                        started,
                        retry_budget,
                        Some(&work_dir),
                    )
                    .await;
            }
        }

        // Freeze remaining pending so end-of-loop does not claim success.
        for (id, ts) in self.state.tasks.iter_mut() {
            if matches!(
                ts.status,
                TaskStatus::Pending
                    | TaskStatus::Queued
                    | TaskStatus::Starting
                    | TaskStatus::Running
            ) {
                ts.status = TaskStatus::Stopped;
                ts.finished_at = Some(chrono::Utc::now());
                ts.pid = None;
                done.insert(id.clone());
                started.insert(id.clone());
            }
        }
        self.state.status = disk.status;
        if self.state.finished_at.is_none() {
            self.state.finished_at = Some(chrono::Utc::now());
        }
        self.state.save()?;
        self.state.event(
            "run_end",
            serde_json::json!({
                "status": match disk.status {
                    RunStatus::Aborted => "aborted",
                    RunStatus::Paused => "paused",
                    _ => "stopped",
                },
                "via": "external_stop",
            }),
        )?;
        let _ = handoff::on_run_end(&self.plan, &self.state, disk.status);
        Ok(Some(disk.status))
    }

    pub(super) async fn reap_finished(
        &mut self,
        running: &mut HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)>,
        progress: &mut HashMap<String, ProgressWatch>,
        done: &mut HashSet<String>,
        failed: &mut HashSet<String>,
        started: &mut HashSet<String>,
        retry_budget: u32,
        stall_for: Duration,
    ) -> Result<()> {
        let ids: Vec<String> = running.keys().cloned().collect();
        for id in ids {
            let (provider, handle, _) = running.get(&id).unwrap();
            let st = provider.poll(handle).await?;
            match st {
                WorkerStatus::Running => {
                    if let Some(action) = self
                        .patrol_stall(&id, handle, progress, stall_for)
                        .await?
                    {
                        let (provider, handle, work_dir) = running.remove(&id).unwrap();
                        progress.remove(&id);
                        if let Err(e) = provider.stop(&handle).await {
                            warn!(task = %id, err = %e, "stall stop failed");
                        }
                        let mut result = provider.collect(&handle).await.unwrap_or_else(|e| {
                            super::super::provider::TaskResult {
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
                        let _ = self
                            .finish_or_retry(
                                &id,
                                result,
                                &action.reason_code,
                                done,
                                failed,
                                started,
                                retry_budget,
                                Some(&work_dir),
                            )
                            .await?;
                        if self.budget_exceeded()? {
                            failed.insert("__budget__".into());
                        }
                    }
                }
                other => {
                    let (provider, handle, work_dir) = running.remove(&id).unwrap();
                    progress.remove(&id);
                    let mut result = provider.collect(&handle).await?;

                    if result.status == TaskStatus::Done {
                        if let Some(task) = self.plan.task(&id) {
                            self.apply_post_done_gates(task, &work_dir, &mut result)
                                .await;
                        }
                    }

                    // inspect VERDICT gate FAIL is semantic (needs rework), not a crash:
                    // permanent — do not SameProvider-retry / provider-failover storm.
                    let reason_code = match other {
                        WorkerStatus::Timeout => "timeout",
                        WorkerStatus::Stopped => "stopped",
                        WorkerStatus::Failed => "fail",
                        WorkerStatus::Done if result.status != TaskStatus::Done => {
                            if crate::domain::run::is_inspect_gate_error(result.error.as_deref()) {
                                "inspect_fail"
                            } else {
                                "fail"
                            }
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
                                done,
                                failed,
                                started,
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
        Ok(())
    }

    async fn apply_post_done_gates(
        &self,
        task: &crate::plan::TaskIR,
        work_dir: &std::path::Path,
        result: &mut super::super::provider::TaskResult,
    ) {
        // H2: only effective verify_cmd (verify_cmd | runnable legacy acceptance).
        if let Some(cmd) = task.effective_verify_cmd() {
            let acc = run_acceptance_soft(work_dir, cmd, Duration::from_secs(300)).await;
            let acc_path = self.state.task_dir(&task.id).join("acceptance.json");
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
        } else if let Some(raw) = task
            .acceptance
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            // Legacy human acceptance present but not shell — record skip, never fail.
            tracing::info!(
                task_id = %task.id,
                acceptance = %raw.chars().take(120).collect::<String>(),
                "acceptance skipped (not shell); continue outputs/inspect gates"
            );
            let acc_path = self.state.task_dir(&task.id).join("acceptance.json");
            let _ = std::fs::write(
                &acc_path,
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": null,
                    "skipped": true,
                    "reason": "skipped_not_shell",
                    "command": raw,
                    "passed": false,
                }))
                .unwrap_or_default(),
            );
        }
        if result.status == TaskStatus::Done {
            self.enforce_outputs(task, work_dir, result);
        }
        if result.status == TaskStatus::Done {
            self.enforce_inspect_verdict(task, work_dir, result);
        }
    }

    pub(super) async fn spawn_ready(
        &mut self,
        active_ids: &HashSet<String>,
        ready_seed: &mut Vec<String>,
        running: &mut HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)>,
        progress: &mut HashMap<String, ProgressWatch>,
        done: &mut HashSet<String>,
        failed: &mut HashSet<String>,
        started: &mut HashSet<String>,
        retry_budget: u32,
    ) -> Result<()> {
        let mut ready = ready_seed.clone();
        ready.retain(|id| {
            self.state
                .tasks
                .get(id)
                .map(|t| !t.status.is_terminal())
                .unwrap_or(true)
        });
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
            let mut pick: Option<usize> = None;
            for (i, id) in ready.iter().enumerate() {
                if let Some(task) = self.plan.task(id) {
                    if self.provider_slot_available(running, &task.provider) {
                        pick = Some(i);
                        break;
                    }
                }
            }
            let Some(idx) = pick else {
                break;
            };
            let id = ready.remove(idx);
            // P2: before spawn, optionally downgrade still-auto routes under budget ceiling.
            self.maybe_budget_downgrade_task(&id);

            let task = self
                .plan
                .task(&id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing task {id}"))?;

            if let Err(reason) =
                handoff::system_push_inspect_gate(&self.plan, &task, &self.state.project_root)
            {
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
                    if let Err(e) = handoff::on_task_start(&self.plan, &self.state, &id) {
                        warn!(task = %id, err = %e, "handoff task_start failed");
                    }

                    let st = provider.poll(&handle).await?;
                    if !matches!(st, WorkerStatus::Running) {
                        progress.remove(&id);
                        let mut result = provider.collect(&handle).await?;
                        if result.status == TaskStatus::Done {
                            self.apply_post_done_gates(&task, &work_dir, &mut result)
                                .await;
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
                            let reason = match result.status {
                                TaskStatus::Timeout => "timeout",
                                TaskStatus::Stopped => "stopped",
                                _ if crate::domain::run::is_inspect_gate_error(
                                    result.error.as_deref(),
                                ) =>
                                {
                                    "inspect_fail"
                                }
                                _ => "fail",
                            };
                            let _ = self
                                .finish_or_retry(
                                    &id,
                                    result,
                                    reason,
                                    done,
                                    failed,
                                    started,
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
                    if let Some(ts) = self.state.tasks.get_mut(&id) {
                        ts.attempt = ts.attempt.saturating_add(1);
                    }
                    started.insert(id.clone());
                    let result = super::super::provider::TaskResult {
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
                            done,
                            failed,
                            started,
                            retry_budget,
                            None,
                        )
                        .await?;
                    if !retried && matches!(self.plan.on_failure, OnFailure::Pause) {
                        break;
                    }
                }
            }
        }
        *ready_seed = ready;
        let _ = active_ids;
        Ok(())
    }

    pub(super) fn should_exit_loop(
        &self,
        active_ids: &HashSet<String>,
        done: &HashSet<String>,
        failed: &HashSet<String>,
        started: &HashSet<String>,
        running: &HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)>,
    ) -> bool {
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
            return true;
        }
        if running.is_empty()
            && ready_tasks(&self.plan, done, started)
                .into_iter()
                .all(|id| !active_ids.contains(&id))
        {
            if !failed.is_empty() {
                return true;
            }
            if started.len() >= active_ids.len() {
                return true;
            }
        }
        false
    }

    /// P2: if run spend crossed budget thresholds, shrink auto routes before spawn.
    ///
    /// Only touches soft_fill / cost_auto (and unset). Never explicit / tag / force /
    /// escalate / failover. Updates plan + TaskState + plan.resolved.json.
    fn maybe_budget_downgrade_task(&mut self, id: &str) {
        let Some(cap) = self.run_max_budget_usd else {
            return;
        };
        let spent = self.state.total_cost_usd();
        let src_wire = self
            .state
            .tasks
            .get(id)
            .and_then(|ts| ts.route_source)
            .map(|s| match s {
                RouteSource::Explicit => "explicit",
                RouteSource::SoftFill => "soft_fill",
                RouteSource::TagRouting => "tag_routing",
                RouteSource::Force => "force",
                RouteSource::Failover => "failover",
                RouteSource::CostAuto => "cost_auto",
                RouteSource::CostEscalate => "cost_escalate",
                RouteSource::CostBudget => "cost_budget",
            });
        // Allow re-tightening cost_budget when spend climbs further (mid → cheap).
        let src_for_policy = match src_wire {
            Some("cost_budget") => Some("cost_auto"),
            other => other,
        };
        if !may_budget_downgrade(src_for_policy) {
            return;
        }
        let (current, role) = match self.plan.task(id) {
            Some(t) => (t.provider.clone(), t.role),
            None => return,
        };
        let available: Vec<String> = self
            .registry
            .list()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let Some(pick) = suggest_budget_downgrade(
            &current,
            role_default_tier(role),
            spent,
            Some(cap),
            &available,
            &[],
        ) else {
            return;
        };
        let previous = current;
        if let Some(t) = self.plan.tasks.iter_mut().find(|t| t.id == id) {
            t.provider = pick.provider.clone();
        }
        if let Some(ts) = self.state.tasks.get_mut(id) {
            ts.provider = pick.provider.clone();
            ts.route_source = Some(RouteSource::CostBudget);
            ts.route_previous = Some(previous.clone());
            ts.route_note = Some(format!(
                "预算收紧·spend≈{spent:.2}/{cap:.2}→{}",
                pick.tier.as_str()
            ));
        }
        let resolved = self.state.run_dir.join("plan.resolved.json");
        if let Ok(text) = serde_json::to_string_pretty(&self.plan) {
            let _ = std::fs::write(&resolved, text);
        }
        let _ = self.state.event(
            "cost_budget",
            serde_json::json!({
                "task_id": id,
                "from": previous,
                "to": pick.provider,
                "spent": spent,
                "cap": cap,
                "tier": pick.tier.as_str(),
            }),
        );
        info!(
            task = %id,
            from = %previous,
            to = %pick.provider,
            spent,
            cap,
            "budget tier downgrade before spawn"
        );
    }
}
