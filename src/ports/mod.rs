//! Ports (traits) (A1 · P2-17).
//!
//! [INPUT]: domain models for method args (TaskIR, …)
//! [OUTPUT]: trait 定义 only；实现落在 adapters（现 `runtime/provider` · `runtime/handoff`）
//! [POS]: Domain/App 依赖 ports；Adapters 实现 ports
//! [PROTOCOL]: 禁止再发明 XxxManager；组合在 app 用例内
//!
//! Target (architecture-redesign 附录 B):
//! WorkerPort ✅ A1-4 · HandoffStore ✅ A1-5 · SplitAgentPort ✅ (openhands landing) ·
//! PlanJobStore · RunStore · ChatStore · PlannerPort · ProcessPort · WorktreePort · Clock

/// Multi-CLI worker bus (start/poll/stop/collect/preflight/capabilities).
pub mod worker;

/// Host-owned handoff ledger (board · fragments · task/run end).
pub mod handoff;

/// Dedicated plan-split agent (cco-split/v1 · Plan Mode).
pub mod split_agent;

pub use handoff::HandoffStore;
pub use split_agent::{SplitAgentPort, SplitRequest};
pub use worker::{
    Capabilities, StartCtx, TaskResult, TaskStatus, WorkerHandle, WorkerPort, WorkerStatus,
};

/// A0 baseline marker.
pub const A0_BASELINE: &str = "ports-a0";

#[cfg(test)]
mod tests {
    #[test]
    fn a0_ports_skeleton_loads() {
        assert_eq!(super::A0_BASELINE, "ports-a0");
    }

    #[test]
    fn worker_port_task_status_terminal() {
        use super::TaskStatus;
        assert!(TaskStatus::Done.is_terminal());
        assert!(TaskStatus::Done.is_success());
        assert!(!TaskStatus::Running.is_terminal());
    }
}
