//! HandoffStore — host-owned run ledger port (A1-5 · P2-17).
//!
//! ## Pure vs IO
//! | Pure (domain/inspect) | IO (this port · runtime/handoff adapter) |
//! |-----------------------|------------------------------------------|
//! | VERDICT/ISSUES parse  | load/save board · fragments · timeline   |
//! | gate fail reasons     | on_task_start / on_task_end / on_run_end |
//! | candidate path lists  | write_shell · prompt prefix injection   |
//!
//! [INPUT]: PlanIR · RunState · TaskResult · task id
//! [OUTPUT]: board/fragments persistence
//! [POS]: trait only；实现落在 `runtime/handoff`（FsHandoffStore）
//! [PROTOCOL]: **勿**静默改 handoff.json schema `cco-handoff/v1`；scheduler 只经 facade/port，禁止 VERDICT 正文解析

use std::path::Path;

use anyhow::Result;

use crate::domain::plan::{PlanIR, TaskIR};
use crate::ports::worker::TaskResult;
use crate::state::{RunState, RunStatus};

/// Host-owned handoff ledger (board · timeline · fragments).
///
/// Implementations live under `runtime/handoff` (fs adapter). Scheduler and
/// services call the free-function facade or this trait — never parse VERDICT
/// text themselves.
pub trait HandoffStore: Send + Sync {
    /// Create empty board shell if missing.
    fn write_shell(&self, plan: &PlanIR, state: &RunState) -> Result<()>;

    /// Board row → running when a task spawns.
    fn on_task_start(&self, plan: &PlanIR, state: &RunState, task_id: &str) -> Result<()>;

    /// Merge fragment after task terminal; update Board / Timeline / Open risks.
    fn on_task_end(
        &self,
        plan: &PlanIR,
        state: &RunState,
        task: &TaskIR,
        result: &TaskResult,
        work_dir: Option<&Path>,
    ) -> Result<()>;

    /// Final run status stamp on handoff.
    fn on_run_end(&self, plan: &PlanIR, state: &RunState, status: RunStatus) -> Result<()>;
}
