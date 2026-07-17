//! WorkerProvider trait and registry. Host never assembles CLI flags here.
//!
//! [INPUT]: 依赖 config::Config、plan::TaskIR、which/dirs 解析 bin
//! [OUTPUT]: ProviderRegistry / WorkerProvider / bin 解析
//! [POS]: runtime/provider 总线，被 scheduler 与 doctor 消费
//! [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

pub mod claude;
pub mod codex;
pub mod fake;

// re-export parse helper for tests
pub use claude::parse_agent_id;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::plan::TaskIR;

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

#[async_trait]
pub trait WorkerProvider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn preflight(&self) -> Result<()>;
    fn validate_task(&self, task: &TaskIR) -> Result<()>;
    async fn start(&self, task: &TaskIR, ctx: &StartCtx) -> Result<WorkerHandle>;
    async fn poll(&self, handle: &WorkerHandle) -> Result<WorkerStatus>;
    async fn stop(&self, handle: &WorkerHandle) -> Result<()>;
    async fn collect(&self, handle: &WorkerHandle) -> Result<TaskResult>;
}

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn WorkerProvider>>,
}

impl ProviderRegistry {
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut providers: Vec<Arc<dyn WorkerProvider>> = Vec::new();

        if let Some(pc) = config.provider("claude") {
            if pc.enabled {
                let bin = resolve_bin_override(&pc.bin, "CCO_CLAUDE_BIN");
                providers.push(Arc::new(claude::ClaudeProvider::new(bin, pc.extra_args.clone())));
            }
        }
        // Always register fake for tests / dry runs when enabled
        if let Some(pc) = config.provider("fake") {
            if pc.enabled {
                let bin = std::env::var("CCO_FAKE_BIN")
                    .or_else(|_| std::env::var("CCO_CLAUDE_BIN"))
                    .unwrap_or_else(|_| pc.bin.clone());
                providers.push(Arc::new(fake::FakeProvider::new(bin)));
            }
        }
        if let Some(pc) = config.provider("codex") {
            if pc.enabled {
                let bin = resolve_bin_override(&pc.bin, "CCO_CODEX_BIN");
                providers.push(Arc::new(codex::CodexProvider::new(bin, pc.extra_args.clone())));
            }
        }

        if providers.is_empty() {
            // fallback: claude default
            providers.push(Arc::new(claude::ClaudeProvider::new(
                resolve_bin_override("claude", "CCO_CLAUDE_BIN"),
                vec![],
            )));
        }

        Ok(Self { providers })
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn WorkerProvider>> {
        self.providers
            .iter()
            .find(|p| p.name() == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("provider not registered: {name}"))
    }

    pub fn list(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    pub async fn preflight_all(&self) -> Vec<(String, Result<()>)> {
        let mut out = Vec::new();
        for p in &self.providers {
            out.push((p.name().to_string(), p.preflight().await));
        }
        out
    }
}

fn resolve_bin_override(default: &str, env_key: &str) -> String {
    if let Ok(v) = std::env::var(env_key) {
        if !v.trim().is_empty() {
            return v;
        }
    }
    // GUI apps (Tauri/.app) often lack interactive-shell PATH.
    // Prefer a real binary on disk over a bare command name.
    if let Some(found) = resolve_bin_on_disk(default) {
        return found;
    }
    default.to_string()
}

/// Look up a binary in PATH, then common user/Homebrew locations.
fn resolve_bin_on_disk(name_or_path: &str) -> Option<String> {
    let p = std::path::Path::new(name_or_path);
    if p.is_absolute() && p.is_file() {
        return Some(name_or_path.to_string());
    }
    if let Ok(found) = which::which(name_or_path) {
        return Some(found.display().to_string());
    }
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".local/bin").join(name_or_path),
        home.join(".claude/local/bin").join(name_or_path),
        home.join("bin").join(name_or_path),
        std::path::PathBuf::from("/opt/homebrew/bin").join(name_or_path),
        std::path::PathBuf::from("/usr/local/bin").join(name_or_path),
        std::path::PathBuf::from("/opt/local/bin").join(name_or_path),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c.display().to_string());
        }
    }
    None
}

pub fn ensure_done_marker(stdout: &str) -> bool {
    stdout.lines().any(|l| {
        let t = l.trim();
        t.starts_with("CCO_DONE") || t.starts_with("ORCH_DONE")
    })
}

pub fn parse_claude_result_json(text: &str) -> Result<serde_json::Value> {
    // claude -p --output-format json may emit a single JSON object or NDJSON; take last object
    let trimmed = text.trim();
    if trimmed.is_empty() {
        bail!("empty stdout from worker");
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }
    // try last non-empty line
    for line in trimmed.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            return Ok(v);
        }
    }
    // try find outermost {…}
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    bail!("could not parse worker JSON result");
}
