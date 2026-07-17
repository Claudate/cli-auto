//! Single-file whole prompt → one task.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::plan::{OnFailure, PlanIR, TaskIR};

pub fn parse(path: &Path, text: &str, config: &Config) -> Result<PlanIR> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("raw")
        .to_string();

    let provider = config.default.default_provider.clone();
    let opts = default_provider_opts(config, &provider);

    Ok(PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "raw-single".into(),
        source_path: path.to_path_buf(),
        max_parallel: 1,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: provider.clone(),
        default_mode: config.default.default_mode.clone(),
        worktree: false,
        tasks: vec![TaskIR {
            id: "t1".into(),
            title: "raw prompt".into(),
            depends_on: vec![],
            group: Some("G1".into()),
            provider,
            mode: config.default.default_mode.clone(),
            prompt: text.to_string(),
            acceptance: None,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: opts,
        }],
    })
}

pub(crate) fn default_provider_opts(config: &Config, provider: &str) -> serde_json::Value {
    if provider == "claude" || provider == "fake" {
        serde_json::json!({
            "max_turns": config.default.max_turns,
            "max_budget_usd": config.default.max_budget_usd,
            "permission_mode": config.default.permission_mode,
            "allowed_tools": config.default.allowed_tools,
        })
    } else {
        serde_json::json!({})
    }
}
