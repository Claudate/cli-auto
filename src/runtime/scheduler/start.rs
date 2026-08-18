//! Spawn a single task: worktree · handoff prefix · WorkerPort.start · terminal open.
//!
//! [INPUT]: TaskIR · registry · state paths
//! [OUTPUT]: (WorkerPort, handle, work_dir)
//! [POS]: runtime/scheduler IO adapter slice
//! [PROTOCOL]: 变更时更新 scheduler/mod.rs 头部；隔离策略经 domain/worker

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing::{info, warn};

use super::super::provider::{StartCtx, TaskStatus, WorkerHandle, WorkerPort};
use super::super::worktree;
use super::Scheduler;
use crate::domain::run::provider_slot_open;
use crate::plan::TaskIR;
use crate::runtime::handoff;

impl Scheduler {
    pub(super) fn maybe_open_terminal(
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

    pub(super) async fn start_task(
        &self,
        task: &TaskIR,
    ) -> Result<(Arc<dyn WorkerPort>, WorkerHandle, PathBuf)> {
        let provider = self.registry.get(&task.provider)?;
        provider.validate_task(task)?;
        let task_dir = self.state.task_dir(&task.id);
        std::fs::create_dir_all(&task_dir)?;

        let want_wt = task.worktree.unwrap_or(self.plan.worktree);
        // A1-4: isolation policy from domain/worker (mix → FailClosed).
        let on_fail =
            worktree::on_fail_for_providers(self.plan.tasks.iter().map(|t| t.provider.as_str()));
        // per_task 下每个任务独立 worktree；从「已提交的依赖分支」fork，而不是裸 main HEAD，
        // 否则 t2 看不到 t1 产物 → codex 空转「无法执行」却 exit 0 被记成 done（假成功）。
        let fork_base = self.fork_base_for(task);
        let (work_dir, wt_info) = worktree::resolve_work_dir(
            &self.state.project_root,
            &self.state.run_id,
            &task.id,
            want_wt,
            on_fail,
            fork_base.as_deref(),
        )?;

        // Resolve fork base to a concrete SHA so the noop guard can later ask
        // "did the worker add anything on top of what it started with?"
        let fork_base_sha = fork_base
            .as_deref()
            .and_then(|ref_name| {
                std::process::Command::new("git")
                    .args(["-C"])
                    .arg(&work_dir)
                    .args(["rev-parse", ref_name])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            });

        let meta = serde_json::json!({
            "work_dir": work_dir,
            "worktree_branch": wt_info.as_ref().map(|w| &w.branch),
            "worktree_path": wt_info.as_ref().map(|w| &w.path),
            "fork_base": fork_base,
            "fork_base_sha": fork_base_sha,
        });
        std::fs::write(
            task_dir.join("work_dir.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        if let Some(ref info) = wt_info {
            info!(
                task = %task.id,
                path = %info.path.display(),
                branch = %info.branch,
                "using worktree"
            );
        }

        // P1-5: host injects latest handoff summary as prompt prefix (once, scheduler-side).
        let mut task_for_start = task.clone();
        task_for_start.prompt =
            handoff::with_handoff_prefix(&task.prompt, task, &self.state.run_dir);

        // Browser MCP (optional): tag `browser` + config.browser.enabled → mcp-config + env.
        // ui-verify + require_preview without URL → Err (task Failed, not silent PASS).
        let preview_url = crate::services::preview_status(&self.state.project_root)
            .ok()
            .and_then(|s| s.url);
        let env_extra = crate::runtime::browser_mcp::prepare_task_browser(
            &self.browser,
            &mut task_for_start,
            &self.state.project_root,
            &task_dir,
            preview_url.as_deref(),
        )?;
        let ctx = StartCtx {
            run_id: self.state.run_id.clone(),
            project_root: self.state.project_root.clone(),
            work_dir: work_dir.clone(),
            task_dir,
            env_extra,
        };

        let handle = provider.start(&task_for_start, &ctx).await?;
        Ok((provider, handle, work_dir))
    }

    /// Choose the git ref a new worktree should branch from so the worker sees
    /// earlier tasks' committed artifacts.
    ///
    /// Takes the **newest committed dependency branch** (per_task mode records the
    /// branch on TaskState after `auto_commit_task`). In a serial chain t1→t2→t3
    /// the newest commit transitively contains all earlier artifacts. Falls back to
    /// None (main `HEAD`) when no dependency has committed a branch yet.
    pub(super) fn fork_base_for(&self, task: &TaskIR) -> Option<String> {
        let mut best: Option<(chrono::DateTime<chrono::Utc>, String)> = None;
        for dep in &task.depends_on {
            let Some(ts) = self.state.tasks.get(dep) else {
                continue;
            };
            if !matches!(ts.status, TaskStatus::Done | TaskStatus::Stopped) {
                continue;
            }
            let Some(branch) = ts.worktree_branch.clone() else {
                continue;
            };
            let at = ts.finished_at.unwrap_or_else(chrono::Utc::now);
            if best.as_ref().map(|(t, _)| at > *t).unwrap_or(true) {
                best = Some((at, branch));
            }
        }
        best.map(|(_, b)| b)
    }

    pub(super) fn provider_slot_available(
        &self,
        running: &HashMap<String, (Arc<dyn WorkerPort>, WorkerHandle, PathBuf)>,
        provider: &str,
    ) -> bool {
        let cap = self.provider_max_parallel.get(provider).copied();
        let used = running
            .values()
            .filter(|(p, _, _)| p.name() == provider)
            .count();
        provider_slot_open(used, cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::TaskStatus;
    use crate::state::RunState;

    fn plan_with_deps() -> crate::plan::PlanIR {
        let task = |id: &str, deps: &[&str]| crate::plan::TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            group: None,
            provider: "fake".into(),
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
            wait_for: vec![],
        };
        crate::plan::PlanIR {
            schema: "cco-plan/v1".into(),
            name: "chain".into(),
            adapter: "cco-plan/v1".into(),
            source_path: std::path::PathBuf::from("p.cco.yaml"),
            max_parallel: 2,
            on_failure: crate::plan::OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![
                task("t1", &[]),
                task("t2", &["t1"]),
                task("t3", &["t1", "t2"]),
            ],
        }
    }

    #[test]
    fn fork_base_picks_newest_committed_dependency() {
        let ir = plan_with_deps();
        let run_dir = std::env::temp_dir().join(format!("cco-fork-test-{}", std::process::id()));
        let state = RunState::new("r1".into(), run_dir.clone(), &ir, run_dir.join("run"));
        {
            // t1 done with branch; t2 still running (no branch).
            let mut t1 = state.tasks.get("t1").unwrap().clone();
            t1.status = TaskStatus::Done;
            t1.worktree_branch = Some("cco/r1/t1".into());
            t1.finished_at = Some(chrono::Utc::now());
            // t2 later-finished branch should win over t1.
            let mut t2 = state.tasks.get("t2").unwrap().clone();
            t2.status = TaskStatus::Done;
            t2.worktree_branch = Some("cco/r1/t2".into());
            t2.finished_at = Some(chrono::Utc::now() + chrono::Duration::seconds(5));
            let mut s = state.clone();
            s.tasks.insert("t1".into(), t1);
            s.tasks.insert("t2".into(), t2);

            let scheduler = crate::runtime::Scheduler {
                max_parallel: 1,
                plan: ir.clone(),
                state: s,
                registry: crate::runtime::provider::ProviderRegistry::from_providers(vec![
                    Arc::new(crate::runtime::provider::fake::FakeProvider::new("fake".into())),
                ])
                .expect("registry"),
                poll_interval: std::time::Duration::from_millis(5),
                yes: true,
                only: None,
                from_task: None,
                dry_run: false,
                mirror_state: None,
                auto_open_terminal: false,
                terminal_kind: crate::SessionKind::Embedded,
                terminal_manager: None,
                run_max_budget_usd: None,
                provider_max_parallel: Default::default(),
                retry_max: 0,
                stall_secs: 600,
                failover_enabled: false,
                fallback_extra_attempts: 1,
                failover_order: vec![],
                cost_escalate_enabled: false,
                browser: crate::config::BrowserConfig::default(),
                provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
                event_emitter: None,
            };
            // t3 depends on [t1, t2] → newest committed branch t2 wins.
            let base = scheduler.fork_base_for(&ir.tasks[2]);
            assert_eq!(base.as_deref(), Some("cco/r1/t2"));
            // t2 depends on [t1] → t1 wins.
            let base2 = scheduler.fork_base_for(&ir.tasks[1]);
            assert_eq!(base2.as_deref(), Some("cco/r1/t1"));
        }
        let _ = std::fs::remove_dir_all(run_dir);
    }

    #[test]
    fn fork_base_none_when_no_deps_committed() {
        let ir = plan_with_deps();
        let run_dir = std::env::temp_dir().join(format!("cco-fork-none-{}", std::process::id()));
        let state = RunState::new("r1".into(), run_dir.clone(), &ir, run_dir.join("run"));
        let scheduler = crate::runtime::Scheduler {
            max_parallel: 1,
            plan: ir.clone(),
            state,
            registry: crate::runtime::provider::ProviderRegistry::from_providers(vec![Arc::new(
                crate::runtime::provider::fake::FakeProvider::new("fake".into()),
            )])
            .expect("registry"),
            poll_interval: std::time::Duration::from_millis(5),
            yes: true,
            only: None,
            from_task: None,
            dry_run: false,
            mirror_state: None,
            auto_open_terminal: false,
            terminal_kind: crate::SessionKind::Embedded,
            terminal_manager: None,
            run_max_budget_usd: None,
            provider_max_parallel: Default::default(),
            retry_max: 0,
            stall_secs: 600,
            failover_enabled: false,
            fallback_extra_attempts: 1,
            failover_order: vec![],
            cost_escalate_enabled: false,
            browser: crate::config::BrowserConfig::default(),
            provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
                event_emitter: None,
        };
        // No dependency done yet → fall back to main HEAD (None).
        assert_eq!(scheduler.fork_base_for(&ir.tasks[1]), None);
        let _ = std::fs::remove_dir_all(run_dir);
    }
}
