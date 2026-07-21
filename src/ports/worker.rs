//! WorkerPort — unified multi-CLI worker bus (A1-4 · P2-17).
//!
//! ## Pure strategy vs process IO
//! | Pure (domain/worker · domain/run) | IO (adapters: runtime/provider/*) |
//! |-----------------------------------|-----------------------------------|
//! | ProviderId · Route · Capability flags | spawn CLI · poll pid/agent · kill |
//! | soft-fill / force route fill | preflight `which` / version |
//! | failover target name (claude↔codex) | preflight-gated live switch |
//! | isolation FailClosed on multi-provider | git worktree path create |
//! | retry classify / budgets | start/poll/stop/collect |
//!
//! [INPUT]: TaskIR (domain/plan) · StartCtx paths
//! [OUTPUT]: WorkerHandle · WorkerStatus · TaskResult
//! [POS]: Domain/App depend on this trait; `runtime/provider` implements it
//! [PROTOCOL]: 禁止第二总线；DTO 形状变更须同步 run-dir 契约；实现只在 adapter

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::plan::TaskIR;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Queued,
    Starting,
    Running,
    Done,
    Failed,
    Stopped,
    Skipped,
    Timeout,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Stopped | Self::Skipped | Self::Timeout
        )
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Done)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub print: bool,
    pub background: bool,
    pub stop: bool,
    pub cost: bool,
    pub session_resume: bool,
    pub interactive_pty: bool,
}

#[derive(Debug, Clone)]
pub struct StartCtx {
    pub run_id: String,
    pub project_root: PathBuf,
    pub work_dir: PathBuf,
    pub task_dir: PathBuf,
    pub env_extra: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct WorkerHandle {
    pub provider: String,
    pub task_id: String,
    pub mode: String,
    /// Provider-private opaque id (pid string, agent id, …)
    pub opaque_id: String,
    pub pid: Option<u32>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub stdout_path: PathBuf,
    pub meta_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Running,
    Done,
    Failed,
    Stopped,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub exit_code: Option<i32>,
    pub stdout_path: Option<PathBuf>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub cost_usd: Option<f64>,
    pub raw: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Unified worker port (附录 B). Adapters: claude / codex / fake.
#[async_trait]
pub trait WorkerPort: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn preflight(&self) -> Result<()>;
    fn validate_task(&self, task: &TaskIR) -> Result<()>;
    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle>;
    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus>;
    async fn stop(&self, handle: &WorkerHandle) -> Result<()>;
    async fn collect(&self, handle: &WorkerHandle) -> Result<TaskResult>;
}
