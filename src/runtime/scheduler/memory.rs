//! P3 memory pilot: task outcome recording + memory-informed pre-spawn failover.
//!
//! [INPUT]: finished task outcomes · pre-spawn task route · MemoryPort handle
//! [OUTPUT]: semantic memory entries · preventive provider switch (Failover route)
//! [POS]: runtime/scheduler — IO glue only; decision rule in domain/worker/memory_route
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
//!
//! Everything here is best-effort behind `Scheduler.memory` (None by default):
//! recording failures never fail the run; Explicit routes are never rewritten
//! (hard rule 13 — history is advisory, not force).

use tracing::{debug, info};

use crate::domain::worker::{memory_failover_target, MemoryOutcomeStats};
use crate::plan::TaskIR;
use crate::ports::memory::Metadata;
use crate::state::RouteSource;

use super::Scheduler;

impl Scheduler {
    /// Record a finished task outcome ("success" | "timeout" | "failed" | "stopped")
    /// into semantic memory (best-effort · no-op when memory is disabled).
    pub(super) async fn record_task_outcome(&self, id: &str, outcome: &str) {
        let Some(mem) = &self.memory else { return };
        let Some(task) = self.plan.task(id) else { return };
        // State provider is authoritative (includes failover switches mid-run).
        let provider = self
            .state
            .tasks
            .get(id)
            .map(|t| t.provider.clone())
            .unwrap_or_else(|| task.provider.clone());
        let role = task.role.map(|r| r.as_str()).unwrap_or("implement");
        // Leading tokens align with the retrieval query in `maybe_memory_failover`.
        let content = format!("outcome {provider} {role} {outcome} 任务 {}", task.title);
        let metadata = Metadata {
            project_id: Some(
                self.state
                    .project_root
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_string(),
            ),
            task_role: Some(role.into()),
            provider: Some(provider),
            outcome: Some(outcome.into()),
            tags: vec!["task-outcome".into()],
            ..Default::default()
        };
        let key = format!(
            "outcome-{}-{}-{}",
            self.state.run_id,
            id,
            chrono::Utc::now().timestamp_millis()
        );
        if let Err(e) = mem.store(&key, &content, metadata).await {
            debug!(error = %e, task = %id, "memory outcome record failed (best-effort)");
        }
    }

    /// Preventive failover before spawn when history shows a high failure rate
    /// for this (provider, role) pair. Explicit routes are never touched.
    pub(super) async fn maybe_memory_failover(&mut self, id: &str, task: &mut TaskIR) {
        let Some(mem) = self.memory.clone() else { return };
        // Hard rule 13: user-pinned engines stay (H4 failover after real failures still applies).
        if self
            .state
            .tasks
            .get(id)
            .and_then(|t| t.route_source)
            == Some(RouteSource::Explicit)
        {
            return;
        }
        let role = task.role.map(|r| r.as_str()).unwrap_or("implement");
        let query = format!("outcome {} {}", task.provider, role);
        let hits = match mem.search(&query, 20).await {
            Ok(h) => h,
            Err(e) => {
                debug!(error = %e, task = %id, "memory history search failed (skipping)");
                return;
            }
        };
        let mut stats = MemoryOutcomeStats::default();
        for h in &hits {
            if h.metadata.provider.as_deref() == Some(task.provider.as_str())
                && h.metadata.task_role.as_deref() == Some(role)
            {
                if let Some(outcome) = h.metadata.outcome.as_deref() {
                    stats.add_outcome(outcome);
                }
            }
        }
        // Candidates: failover order minus current / unhealthy / unregistered.
        let candidates: Vec<String> = self
            .failover_order
            .iter()
            .filter(|p| p.as_str() != task.provider)
            .filter(|p| !self.provider_unhealthy.contains(*p))
            .filter(|p| self.registry.get(p).is_ok())
            .cloned()
            .collect();
        let Some((target, reason)) = memory_failover_target(&stats, &task.provider, &candidates)
        else {
            return;
        };

        let from = task.provider.clone();
        info!(
            task = %id,
            from = %from,
            to = %target,
            "[MEMORY] {reason}"
        );
        if let Some(t) = self.plan.tasks.iter_mut().find(|t| t.id == id) {
            t.provider = target.clone();
        }
        if let Some(ts) = self.state.tasks.get_mut(id) {
            ts.provider = target.clone();
            ts.route_previous = Some(from.clone());
            ts.route_source = Some(RouteSource::Failover);
            ts.route_note = Some(format!("memory:{reason}"));
        }
        let _ = self.state.event(
            "provider_switched",
            serde_json::json!({
                "task": id,
                "provider": target,
                "from_provider": from,
                "why": "memory_history",
                "reason": reason,
            }),
        );
        task.provider = target;
        let _ = self.state.save();
    }
}
