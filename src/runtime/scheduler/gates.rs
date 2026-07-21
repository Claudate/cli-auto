//! Post-task gates: outputs · inspect VERDICT (handoff/domain API only) · handoff end.
//!
//! VERDICT **text parse stays in domain::inspect** (via handoff facade). This file only
//! isolates the call sites so the orchestrator loop does not grow more gate logic.
//!
//! [INPUT]: TaskIR · work_dir · TaskResult
//! [OUTPUT]: may flip result to Failed
//! [POS]: runtime/scheduler
//! [PROTOCOL]: 禁止在此重写 VERDICT 解析；变更时更新 scheduler/mod.rs

use tracing::warn;

use super::super::provider::TaskResult;
use super::Scheduler;
use crate::plan::TaskIR;
use crate::runtime::handoff;

impl Scheduler {
    /// P1-4: if TaskIR.outputs non-empty and any missing → Failed.
    pub(super) fn enforce_outputs(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut TaskResult,
    ) {
        if result.status != super::super::provider::TaskStatus::Done {
            return;
        }
        let missing = handoff::missing_outputs(task, work_dir, &self.state.project_root);
        if missing.is_empty() {
            return;
        }
        result.status = super::super::provider::TaskStatus::Failed;
        result.error = Some(format!("missing outputs: {}", missing.join(", ")));
        warn!(
            task = %task.id,
            missing = ?missing,
            "task failed: required outputs missing"
        );
    }

    /// P2-3 + P-loop: inspect VERDICT gate via handoff/domain API only (no parse here).
    pub(super) fn enforce_inspect_verdict(
        &self,
        task: &TaskIR,
        work_dir: &std::path::Path,
        result: &mut TaskResult,
    ) {
        use super::super::provider::TaskStatus;
        if result.status != TaskStatus::Done {
            return;
        }
        if !handoff::task_has_verdict_gate(task) {
            return;
        }
        let verdict = handoff::read_inspect_verdict(task, work_dir, &self.state.project_root);
        let issues = handoff::collect_inspect_issues(task, work_dir, &self.state.project_root);
        let (blocked, blocking_n) =
            handoff::inspect_pass_blocked_by_issues(task, work_dir, &self.state.project_root);
        let _ = blocked; // folded into domain fail_reason via blocking_n
        let treat_unknown_as_fail = self.plan.require_inspect
            || task.role == Some(crate::plan::TaskRole::Inspect);

        let fail_reason = handoff::inspect_gate_fail_reason(
            verdict,
            blocking_n,
            issues.len(),
            treat_unknown_as_fail,
            &task.id,
        );

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
    pub(super) fn handoff_task_end(
        &self,
        id: &str,
        result: &TaskResult,
        work_dir: Option<&std::path::Path>,
    ) {
        let Some(task) = self.plan.task(id) else {
            return;
        };
        if let Err(e) = handoff::on_task_end(&self.plan, &self.state, task, result, work_dir) {
            warn!(task = %id, err = %e, "handoff task_end failed");
        }
    }
}
