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

use super::super::provider::{StartCtx, WorkerHandle, WorkerPort};
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
        let (work_dir, wt_info) = worktree::resolve_work_dir(
            &self.state.project_root,
            &self.state.run_id,
            &task.id,
            want_wt,
            on_fail,
        )?;

        let meta = serde_json::json!({
            "work_dir": work_dir,
            "worktree_branch": wt_info.as_ref().map(|w| &w.branch),
            "worktree_path": wt_info.as_ref().map(|w| &w.path),
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
