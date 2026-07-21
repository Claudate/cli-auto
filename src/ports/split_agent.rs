//! SplitAgentPort — dedicated plan-split agent (OpenHands Plan Mode style).
//!
//! [INPUT]: plan markdown + job context
//! [OUTPUT]: CcoSplitJob (cco-split/v1 shape)
//! [POS]: ports — Domain/App 依赖 trait；实现在 `plan/split_agent`
//! [PROTOCOL]: 拆分 ≠ 执行 Worker；禁止 UI/clap 依赖；实现不写业务代码到仓库
//!
//! Product: ModelSplitAgent produces structured tasks; human confirms; Worker executes.

use std::path::PathBuf;

use anyhow::Result;

use crate::domain::plan::CcoSplitJob;

/// Context for one split invocation (no UI, no run spawn).
#[derive(Debug, Clone)]
pub struct SplitRequest {
    pub job_id: String,
    pub project: PathBuf,
    pub plan_path: PathBuf,
    /// Absolute or project-relative resolved path optional; agent may re-resolve.
    pub plan_abs: PathBuf,
    /// Full or truncated plan markdown.
    pub plan_md: String,
    pub max_parallel: usize,
    pub created_at: String,
    pub updated_at: String,
}

/// Port: plan document → cco-native split graph.
///
/// Implementations must **not** start Scheduler/Workers. Confirm stays the only open-run gate.
pub trait SplitAgentPort: Send + Sync {
    fn split(&self, req: &SplitRequest) -> Result<CcoSplitJob>;
}
