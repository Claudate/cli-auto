//! Desktop/CLI settings subset of Config.
//!
//! [INPUT]: Config · SettingsUpdate
//! [OUTPUT]: get_settings · set_settings · SettingsView
//! [POS]: services 子模块
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Subset of config exposed to the desktop UI for reading.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub poll_interval_secs: u64,
    pub default_provider: String,
    pub default_mode: String,
    pub max_parallel: usize,
    pub ui_refresh_secs: u64,
    /// Extra auto-retries after first try (0–10).
    pub retry_max: u32,
    /// Stall threshold seconds (no log growth → stop + retry).
    pub stall_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsUpdate {
    pub poll_interval_secs: Option<u64>,
    pub default_provider: Option<String>,
    pub default_mode: Option<u32>,
    pub max_parallel: Option<usize>,
    pub retry_max: Option<u32>,
    pub stall_secs: Option<u64>,
}

pub fn get_settings(config: &Config) -> SettingsView {
    SettingsView {
        poll_interval_secs: config.default.poll_interval_secs,
        default_provider: config.default.default_provider.clone(),
        default_mode: config.default.default_mode.clone(),
        max_parallel: config.default.max_parallel,
        ui_refresh_secs: 2, // UI hardcoded; could become configurable later
        retry_max: config.default.retry_max,
        stall_secs: config.default.stall_secs,
    }
}

/// Apply partial update to config and persist.
pub fn set_settings(config: &mut Config, update: SettingsUpdate) -> Result<()> {
    if let Some(v) = update.poll_interval_secs {
        config.default.poll_interval_secs = v.clamp(1, 60);
    }
    if let Some(p) = update.default_provider {
        if !p.is_empty() {
            config.default.default_provider = p;
        }
    }
    if let Some(m) = update.default_mode {
        match m {
            0 => config.default.default_mode = "print".to_string(),
            1 => config.default.default_mode = "bg".to_string(),
            2 => config.default.default_mode = "auto".to_string(),
            _ => {}
        }
    }
    if let Some(v) = update.max_parallel {
        config.default.max_parallel = v.clamp(1, 32);
    }
    if let Some(v) = update.retry_max {
        config.default.retry_max = v.min(10);
    }
    if let Some(v) = update.stall_secs {
        // 60s–2h production range; allow lower only via config.toml for tests.
        config.default.stall_secs = v.clamp(30, 7200);
    }
    config.save()
}
