//! Stall patrol, budget, retry budget, H4 failover target resolve (preflight IO).
//!
//! [INPUT]: stdout path · budgets · registry
//! [OUTPUT]: StallAction · retry numbers · optional failover provider name
//! [POS]: runtime/scheduler
//! [PROTOCOL]: 变更时更新 scheduler/mod.rs 头部

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use tracing::warn;

use super::super::provider::WorkerHandle;
use super::types::{stdout_len, ProgressWatch, StallAction};
use super::Scheduler;
use crate::domain::run::{budget_exceeded, effective_retry_max, stall_triggered};
use crate::domain::worker::FailoverPolicy;

impl Scheduler {
    pub(super) fn budget_exceeded(&self) -> Result<bool> {
        let Some(cap) = self.run_max_budget_usd else {
            return Ok(false);
        };
        let spent = self.state.total_cost_usd();
        if budget_exceeded(spent, cap) {
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
    pub(super) fn effective_retry_max(&self) -> u32 {
        effective_retry_max(self.plan.retry_max, self.retry_max)
    }

    /// Pure failover policy object (A1-4); preflight stays here (IO).
    pub(super) fn failover_policy(&self) -> FailoverPolicy {
        FailoverPolicy::with_order(
            self.failover_enabled,
            self.fallback_extra_attempts,
            self.failover_order.clone(),
        )
    }

    /// Resolve a live fallback provider when H4 failover is armed and preflight passes.
    /// Walks order; skips candidates that are not registered or fail preflight.
    pub(super) async fn resolve_failover_provider(
        &self,
        current: &str,
        already_tried: &[String],
    ) -> Option<String> {
        let policy = self.failover_policy();
        let mut tried = already_tried.to_vec();
        loop {
            let target = policy.target_for(current, &tried)?;
            let provider = match self.registry.get(&target) {
                Ok(p) => p,
                Err(_) => {
                    warn!(
                        from = %current,
                        to = %target,
                        "failover target not registered — try next"
                    );
                    tried.push(target);
                    continue;
                }
            };
            if provider.preflight().await.is_err() {
                warn!(
                    from = %current,
                    to = %target,
                    "failover preflight failed — try next"
                );
                tried.push(target);
                continue;
            }
            return Some(target);
        }
    }

    /// Attempt budget for this task: after failover, only `fallback_extra_attempts`.
    pub(super) fn attempt_budget_for(&self, id: &str, same_provider_budget: u32) -> u32 {
        let used = self
            .state
            .tasks
            .get(id)
            .map(|t| t.failover_used)
            .unwrap_or(false);
        self.failover_policy()
            .attempt_budget(used, same_provider_budget)
    }

    /// Update progress fingerprint; return Some(action) when stalled long enough.
    pub(super) async fn patrol_stall(
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
        if !stall_triggered(idle, stall_for) {
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
}
