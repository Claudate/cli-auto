//! Runtime: scheduler, providers, logs, worktree, acceptance, handoff.
//!
//! [INPUT]: 无；子模块 re-export
//! [OUTPUT]: Scheduler · LogEvent · ProviderRegistry · handoff 等
//! [POS]: 执行内核入口
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md

pub mod acceptance;
pub mod handoff;
pub mod log_events;
pub mod provider;
pub mod scheduler;
pub mod worktree;

pub use provider::{
    Capabilities, ProviderRegistry, StartCtx, TaskResult, TaskStatus, WorkerHandle,
    WorkerPort, WorkerProvider, WorkerStatus,
};
pub use log_events::{events_to_plain, parse_worker_logs, LogEvent};
pub use scheduler::Scheduler;
