//! Provider-agnostic scheduler.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};

use super::acceptance::run_acceptance_soft;
use super::provider::{
    ProviderRegistry, StartCtx, TaskStatus, WorkerHandle, WorkerProvider, WorkerStatus,
};
use super::worktree;
use crate::graph::ready_tasks;
use crate::plan::{OnFailure, PlanIR, TaskIR};
use crate::state::{RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

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

        if self.dry_run {
            info!("dry-run: not starting workers");
            self.state.status = RunStatus::Completed;
            self.state.finished_at = Some(chrono::Utc::now());
            self.state.save()?;
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
        let mut started: HashSet<String> = HashSet::new();

        loop {
            // Reap finished
            let ids: Vec<String> = running.keys().cloned().collect();
            for id in ids {
                let (provider, handle, _) = running.get(&id).unwrap();
                let st = provider.poll(handle).await?;
                match st {
                    WorkerStatus::Running => {}
                    other => {
                        let (provider, handle, work_dir) = running.remove(&id).unwrap();
                        let mut result = provider.collect(&handle).await?;

                        // acceptance gate
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
                            }
                        }

                        self.apply_result(&id, &result)?;
                        match other {
                            WorkerStatus::Done if result.status == TaskStatus::Done => {
                                done.insert(id.clone());
                                info!(task = %id, cost = ?result.cost_usd, "task done");
                            }
                            WorkerStatus::Timeout => {
                                failed.insert(id.clone());
                                warn!(task = %id, "task timeout");
                            }
                            _ => {
                                if result.status == TaskStatus::Done {
                                    done.insert(id.clone());
                                } else {
                                    failed.insert(id.clone());
                                    warn!(task = %id, err = ?result.error, "task failed");
                                }
                            }
                        }
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
                    match self.start_task(&task).await {
                        Ok((provider, handle, work_dir)) => {
                            started.insert(id.clone());
                            if let Some(ts) = self.state.tasks.get_mut(&id) {
                                ts.status = TaskStatus::Running;
                                ts.started_at = Some(chrono::Utc::now());
                                ts.work_dir = Some(work_dir.clone());
                                ts.pid = handle.pid;
                            }
                            self.maybe_open_terminal(&id, &work_dir, &handle)?;
                            self.state.event(
                                "task_start",
                                serde_json::json!({
                                    "task_id": id,
                                    "provider": task.provider,
                                    "mode": task.mode,
                                    "pid": handle.pid,
                                    "work_dir": work_dir,
                                }),
                            )?;
                            self.state.save()?;

                            let st = provider.poll(&handle).await?;
                            if !matches!(st, WorkerStatus::Running) {
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
                                }
                                self.apply_result(&id, &result)?;
                                if result.status == TaskStatus::Done {
                                    done.insert(id.clone());
                                } else {
                                    failed.insert(id.clone());
                                }
                                self.state.event(
                                    "task_end",
                                    serde_json::json!({
                                        "task_id": id,
                                        "status": result.status,
                                        "cost_usd": result.cost_usd,
                                    }),
                                )?;
                                self.state.save()?;
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
                                ts.status = TaskStatus::Failed;
                                ts.error = Some(format!("{e:#}"));
                                ts.finished_at = Some(chrono::Utc::now());
                            }
                            failed.insert(id.clone());
                            started.insert(id.clone());
                            self.state.event(
                                "task_end",
                                serde_json::json!({
                                    "task_id": id,
                                    "status": "failed",
                                    "error": format!("{e:#}"),
                                }),
                            )?;
                            self.state.save()?;
                            if matches!(self.plan.on_failure, OnFailure::Pause) {
                                break;
                            }
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
        let (work_dir, wt_info) =
            worktree::resolve_work_dir(&self.state.project_root, &self.state.run_id, &task.id, want_wt)?;

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
        let handle = provider.start(task, &ctx).await?;

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
