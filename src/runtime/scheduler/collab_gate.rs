//! Collab-bus glue for the scheduler loop: output publishing + wait_for gate.
//!
//! [INPUT]: task stdout files · TaskIR.wait_for · run state task statuses
//! [OUTPUT]: TaskEvent publications · spawn gate verdict (proceed/defer/fail)
//! [POS]: runtime/scheduler private — keeps tick.rs thin (hard rule 18)
//! [PROTOCOL]: 变更时更新 scheduler/mod.rs 头部
//!
//! Design note: wait_for conditions are evaluated **non-blocking** against the
//! bus history. The scheduler loop itself publishes the awaited events during
//! reap, so awaiting a condition inline inside spawn_ready would deadlock the
//! whole run until the wait timeout (observed as a 3600s hang in e2e).

use std::collections::HashMap;
use std::path::Path;

use tracing::debug;

use super::super::collab::{CollabBus, TaskEvent};
use super::types::ProgressWatch;
use super::Scheduler;
use crate::plan::TaskIR;

/// Verdict for a task's wait_for conditions at spawn time.
pub(super) enum WaitGate {
    /// All conditions satisfied (or no bus / no conditions) — spawn now.
    Proceed,
    /// Unmet condition but the awaited task may still emit events — retry next tick.
    Defer,
    /// Unmet condition and the awaited task is terminal/unknown — can never satisfy.
    Fail(String),
}

impl Scheduler {
    /// Publish new stdout lines (and CCO_STEP markers) to the collab bus.
    ///
    /// Uses `ProgressWatch.collab_pos` as the publish cursor so content already
    /// present at spawn time (e.g. inline fake providers pre-write everything)
    /// is published too — `last_bytes` snapshots it away for stall patrol.
    pub(super) fn publish_collab_output(
        &self,
        id: &str,
        stdout_path: &Path,
        progress: &mut HashMap<String, ProgressWatch>,
    ) {
        let Some(bus) = &self.collab_bus else { return };
        let Ok(content) = std::fs::read_to_string(stdout_path) else {
            return;
        };
        let pos = progress.get(id).map(|p| p.collab_pos as usize).unwrap_or(0);
        // get() guards against a rewritten (shrunk) file / non-boundary cursor.
        let Some(tail) = content.get(pos..) else { return };
        if tail.is_empty() {
            return;
        }
        for line in tail.lines() {
            if let Some((step, status)) = CollabBus::parse_step_marker(line) {
                bus.publish(TaskEvent::Step {
                    task_id: id.to_string(),
                    step,
                    status,
                });
            }
            bus.publish(TaskEvent::Output {
                task_id: id.to_string(),
                line: line.to_string(),
            });
        }
        if let Some(p) = progress.get_mut(id) {
            p.collab_pos = content.len() as u64;
        }
    }

    /// Evaluate a task's non-Complete wait_for conditions without blocking.
    pub(super) fn collab_wait_gate(&self, task: &TaskIR) -> WaitGate {
        let Some(bus) = &self.collab_bus else {
            return WaitGate::Proceed;
        };
        for wait in &task.wait_for {
            // Complete conditions are handled by the depends_on graph.
            if matches!(wait.condition, crate::domain::plan::WaitType::Complete) {
                continue;
            }
            if bus.condition_met(wait) {
                continue;
            }
            let dep_alive = self
                .state
                .tasks
                .get(&wait.task_id)
                .map(|ts| !ts.status.is_terminal())
                .unwrap_or(false);
            if dep_alive {
                debug!(
                    task = %task.id,
                    dep = %wait.task_id,
                    condition = ?wait.condition,
                    pattern = ?wait.pattern,
                    "wait_for not yet satisfied; deferring spawn to next tick"
                );
                return WaitGate::Defer;
            }
            return WaitGate::Fail(format!(
                "wait_for unsatisfied: task {} reached terminal state without matching {:?} {:?}",
                wait.task_id, wait.condition, wait.pattern
            ));
        }
        WaitGate::Proceed
    }
}
