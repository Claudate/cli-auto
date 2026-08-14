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
    /// Platform completed signal (process finished / result object present). 契约层 T1.
    #[serde(default)]
    pub done_marker: bool,
    /// Whether output shows evidence of actual execution (tool_use / command / result object). 契约层 T1.
    #[serde(default)]
    pub execution_evidence: bool,
}

/// Per-decoder outcome for shell-print CLIs — the single "how to judge success" answer.
///
/// 契约层 T1：每个 CLI 用一个 decoder 回答三件事——「怎么判成功」「有没有执行动作证据」
/// 「提取 session/cost/人话错误」。`log_events` / `stream_parse` 的散落硬编码收编于此。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutcome {
    pub status: TaskStatus,
    /// Platform completed signal (done_marker / result object present).
    pub done_marker: bool,
    /// Whether the output shows evidence of actual execution (tool_use / command / result object).
    pub execution_evidence: bool,
    pub empty_stdout: bool,
    /// Human-readable hint when a platform "spun" (no execution / empty output).
    pub error_hint: Option<String>,
    pub session_id: Option<String>,
    pub cost_usd: Option<f64>,
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
    /// Default permission tier this provider declares (Harness-aligned · A3bis).
    ///
    /// Override per provider when it natively supports a stricter tier. The
    /// default `FullAccess` preserves the existing `bypassPermissions` soft-fill
    /// behavior — this is a *declaration*, not a runtime override: `apply_permission_mode`
    /// remains the authority on what actually spawns (rule 13 provider routing
    /// unchanged).
    fn default_permission_tier(&self) -> crate::domain::worker::PermissionTier {
        crate::domain::worker::PermissionTier::FullAccess
    }

    /// Decode provider output into a structured success judgement (契约层 T1).
    ///
    /// Default = exit-status mapping for mature providers that keep their own collect
    /// (claude / fake / sdk). shell-print providers override per [`ResultKind`][] and
    /// answer 「怎么判成功 / 有没有执行动作证据 / 人话错误」 from stdout alone.
    ///
    /// [`ResultKind`]: crate::runtime::provider::shell_print::profiles::ResultKind
    fn decode_result(
        &self,
        stdout: &str,
        _stderr: &str,
        _meta: &serde_json::Value,
        exit: Option<i32>,
    ) -> WorkerOutcome {
        let status = default_status_from_exit(exit);
        let completed = exit.is_some();
        WorkerOutcome {
            status,
            done_marker: completed,
            // Mature providers: a clean completion IS evidence they did the work.
            execution_evidence: status == TaskStatus::Done,
            empty_stdout: stdout.trim().is_empty(),
            error_hint: None,
            session_id: None,
            cost_usd: None,
        }
    }
}

/// Exit-code → TaskStatus mapping (shared by the default decoder; mirrors
/// `runtime/provider/exit_status` semantics without crossing the ports boundary).
fn default_status_from_exit(code: Option<i32>) -> TaskStatus {
    match code {
        Some(0) => TaskStatus::Done,
        Some(124) => TaskStatus::Timeout,
        Some(130) | Some(-1) => TaskStatus::Stopped,
        Some(_) => TaskStatus::Failed,
        None => TaskStatus::Failed,
    }
}
