//! Worker adapters implementing [`crate::ports::WorkerPort`] (A1-4).
//! Host never assembles CLI flags here.
//!
//! [INPUT]: config::Config · domain::plan::TaskIR · which/dirs 解析 bin
//! [OUTPUT]: ProviderRegistry · WorkerPort 实现（claude/codex/fake/sdk）
//! [POS]: runtime/provider 适配器；被 scheduler 与 doctor 消费
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//!
//! ## Pure vs IO
//! | Pure (domain/worker) | IO (this module) |
//! |----------------------|------------------|
//! | route soft/force fill | preflight · spawn · poll · stop · collect |
//! | failover target name | live registry get + preflight gate |
//! | isolation FailClosed | worktree path create (worktree.rs) |
//! note: `sdk` = 非 CLI 路径（P2-7 S0）；默认 config 不注册

pub mod claude;
pub mod codex;
pub mod fake;
pub mod sdk;

// re-export parse helper for tests
pub use claude::parse_agent_id;
pub use sdk::{InlineSdkBackend, SdkBackend, SdkProvider};

use std::sync::Arc;

use anyhow::{bail, Result};

use crate::config::Config;

// ── Port DTO + trait re-exports (compat: historical `runtime::provider::*` paths) ──
pub use crate::ports::worker::{
    Capabilities, StartCtx, TaskResult, TaskStatus, WorkerHandle, WorkerPort, WorkerStatus,
};

/// Historical name for [`WorkerPort`]. Prefer `WorkerPort` / `ports::WorkerPort`.
pub use crate::ports::worker::WorkerPort as WorkerProvider;

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn WorkerPort>>,
}

impl ProviderRegistry {
    pub fn from_config(config: &Config) -> Result<Self> {
        let mut providers: Vec<Arc<dyn WorkerPort>> = Vec::new();

        if let Some(pc) = config.provider("claude") {
            if pc.enabled {
                let bin = resolve_provider_bin(&pc.bin, "CCO_CLAUDE_BIN");
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
                let bin = resolve_provider_bin(&pc.bin, "CCO_CODEX_BIN");
                providers.push(Arc::new(codex::CodexProvider::new(bin, pc.extra_args.clone())));
            }
        }
        // P2-7 S0: non-CLI sdk path — opt-in only (default enabled=false).
        if let Some(pc) = config.provider("sdk") {
            if pc.enabled {
                providers.push(Arc::new(sdk::SdkProvider::new()));
            }
        }

        if providers.is_empty() {
            // fallback: claude default
            providers.push(Arc::new(claude::ClaudeProvider::new(
                resolve_provider_bin("claude", "CCO_CLAUDE_BIN"),
                vec![],
            )));
        }

        Ok(Self { providers })
    }

    /// Build a registry from pre-constructed providers (integration tests / mock aliases).
    pub fn from_providers(providers: Vec<Arc<dyn WorkerPort>>) -> Result<Self> {
        if providers.is_empty() {
            bail!("ProviderRegistry::from_providers: empty provider list");
        }
        Ok(Self { providers })
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn WorkerPort>> {
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

pub fn resolve_provider_bin(default: &str, env_key: &str) -> String {
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
pub fn resolve_bin_on_disk(name_or_path: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig};

    #[test]
    fn registry_omits_sdk_when_disabled_by_default() {
        let cfg = Config::default();
        let reg = ProviderRegistry::from_config(&cfg).unwrap();
        let names = reg.list();
        assert!(
            !names.contains(&"sdk"),
            "sdk must stay off by default: {names:?}"
        );
        assert!(names.contains(&"claude") || names.contains(&"fake") || names.contains(&"codex"));
    }

    #[test]
    fn registry_includes_sdk_when_enabled() {
        let mut cfg = Config::default();
        cfg.providers.insert(
            "sdk".into(),
            ProviderConfig {
                enabled: true,
                bin: "inline".into(),
                extra_args: vec![],
                max_parallel: None,
            },
        );
        let reg = ProviderRegistry::from_config(&cfg).unwrap();
        assert!(reg.list().contains(&"sdk"));
        assert_eq!(reg.get("sdk").unwrap().name(), "sdk");
    }
}
