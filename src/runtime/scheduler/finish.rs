//! finish_or_retry · apply_result · attempt log archive (domain RetryKind).
//!
//! [INPUT]: TaskResult · reason_code · retry budget
//! [OUTPUT]: re-queued (true) or permanent terminal (false)
//! [POS]: runtime/scheduler
//! [PROTOCOL]: stop never retries; A1-4 may move provider switch to Worker policy

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use tracing::{info, warn};

use super::super::provider::{TaskResult, TaskStatus};
use super::Scheduler;
use crate::domain::run::RetryKind;

impl Scheduler {
    /// Apply terminal failure. If attempts remain, reset to Pending and clear `started`
    /// so the ready set can pick it up again. When same-provider budget is exhausted and
    /// H4 failover is armed, switch `task.provider` (run-state + in-memory plan) and re-queue.
    /// User-initiated stop (`reason_code == "stopped"`) never retries and never failovers.
    /// Returns true when a retry (same house or switched) was scheduled.
    pub(super) async fn finish_or_retry(
        &mut self,
        id: &str,
        result: TaskResult,
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
        let failover_used = self
            .state
            .tasks
            .get(id)
            .map(|t| t.failover_used)
            .unwrap_or(false);
        // A1-4: FailoverPolicy object (pure classify); stop never failovers.
        let kind = self.failover_policy().classify(
            reason_code,
            attempt,
            budget,
            failover_used,
        );

        if kind == RetryKind::SameProvider {
            self.archive_attempt_logs(id, attempt);
            self.clear_done_flag(id);

            if let Some(ts) = self.state.tasks.get_mut(id) {
                ts.status = TaskStatus::Pending;
                ts.error = result
                    .error
                    .clone()
                    .or_else(|| Some(format!("{reason_code} on attempt {attempt}")));
                ts.finished_at = None;
                ts.started_at = None;
                ts.pid = None;
                ts.last_retry_reason = Some(reason_code.to_string());
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

        if kind == RetryKind::TryFailover {
            let current = self
                .plan
                .task(id)
                .map(|t| t.provider.clone())
                .or_else(|| self.state.tasks.get(id).map(|t| t.provider.clone()))
                .unwrap_or_default();
            if let Some(fallback) = self.resolve_failover_provider(&current).await {
                self.archive_attempt_logs(id, attempt);
                self.clear_done_flag(id);

                if let Some(t) = self.plan.tasks.iter_mut().find(|t| t.id == id) {
                    t.provider = fallback.clone();
                }
                let resolved = self.state.run_dir.join("plan.resolved.json");
                if let Ok(text) = serde_json::to_string_pretty(&self.plan) {
                    let _ = std::fs::write(&resolved, text);
                }

                if let Some(ts) = self.state.tasks.get_mut(id) {
                    ts.provider = fallback.clone();
                    ts.failover_used = true;
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
                    // P1-2: persist route provenance (failover + previous provider).
                    ts.route_source = Some(crate::state::RouteSource::Failover);
                    ts.route_previous = Some(current.clone());
                    ts.route_note = Some(reason_code.to_string());
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
            // preflight miss → fall through to permanent
        }

        // Exhausted or non-retryable → permanent fail / timeout / user stop.
        self.apply_result(id, &result)?;
        self.handoff_task_end(id, &result, _work_dir);
        if let Some(ts) = self.state.tasks.get_mut(id) {
            ts.last_retry_reason = Some(reason_code.to_string());
            if reason_code != "stopped" {
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
        }
        if result.status == TaskStatus::Done || result.status == TaskStatus::Stopped {
            done.insert(id.to_string());
            if result.status == TaskStatus::Stopped {
                info!(task = %id, "task stopped by user / external");
            }
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

    pub(super) fn archive_attempt_logs(&self, id: &str, attempt: u32) {
        let dir = self.state.task_dir(id);
        let stamp = format!("attempt-{attempt}");
        for name in ["stdout.json", "stderr.log", "meta.json", "status.json"] {
            let src = dir.join(name);
            if src.exists() {
                let dst = dir.join(format!("{stamp}.{name}"));
                let _ = std::fs::rename(&src, &dst);
            }
        }
        let _ = std::fs::write(dir.join("stdout.json"), "");
    }

    pub(super) fn clear_done_flag(&self, id: &str) {
        let dir = self.state.task_dir(id);
        let _ = std::fs::remove_file(dir.join(".done"));
    }

    pub(super) fn apply_result(&mut self, id: &str, result: &TaskResult) -> Result<()> {
        let wd_meta = self.state.task_dir(id).join("work_dir.json");
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
