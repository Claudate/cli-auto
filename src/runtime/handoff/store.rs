//! FsHandoffStore — ports::HandoffStore filesystem adapter (A1-5).
//!
//! [INPUT]: PlanIR · RunState · TaskResult
//! [OUTPUT]: handoff board lifecycle via free functions
//! [POS]: runtime/handoff implements ports::HandoffStore
//! [PROTOCOL]: trait 形状变更同步 ports/handoff.rs

use std::path::Path;

use anyhow::Result;

use crate::domain::plan::{PlanIR, TaskIR};
use crate::ports::worker::TaskResult;
use crate::ports::HandoffStore;
use crate::state::{RunState, RunStatus};

use super::lifecycle;

/// Default filesystem-backed handoff store.
#[derive(Debug, Default, Clone, Copy)]
pub struct FsHandoffStore;

impl HandoffStore for FsHandoffStore {
    fn write_shell(&self, plan: &PlanIR, state: &RunState) -> Result<()> {
        lifecycle::write_shell(plan, state)
    }

    fn on_task_start(&self, plan: &PlanIR, state: &RunState, task_id: &str) -> Result<()> {
        lifecycle::on_task_start(plan, state, task_id)
    }

    fn on_task_end(
        &self,
        plan: &PlanIR,
        state: &RunState,
        task: &TaskIR,
        result: &TaskResult,
        work_dir: Option<&Path>,
    ) -> Result<()> {
        lifecycle::on_task_end(plan, state, task, result, work_dir)
    }

    fn on_run_end(&self, plan: &PlanIR, state: &RunState, status: RunStatus) -> Result<()> {
        lifecycle::on_run_end(plan, state, status)
    }
}
