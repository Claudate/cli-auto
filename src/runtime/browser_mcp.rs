//! Browser MCP helpers for worker start (W1 · docs/browser-automation-cco.md).
//!
//! [INPUT]: BrowserConfig · TaskIR tags · project_root · task_id · task_dir
//! [OUTPUT]: mcp-browser.json path · env pairs · whether to inject
//! [POS]: runtime adapter — no Domain browser crates; Claude spawn reads opts
//! [PROTOCOL]: 变更时更新 runtime/CLAUDE.md；引擎默认 kitewright

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use crate::config::BrowserConfig;
use crate::domain::plan::task_has_browser_tag;
use crate::plan::TaskIR;

/// Relative path written under task_dir for Claude `--mcp-config`.
pub const MCP_BROWSER_FILE: &str = "mcp-browser.json";

/// Provider opt key: absolute path to mcp config JSON (Claude spawn).
pub const OPT_MCP_CONFIG: &str = "mcp_config";
/// Provider opt key: pass `--strict-mcp-config` when true.
pub const OPT_MCP_STRICT: &str = "mcp_strict";

/// True when this task should receive browser MCP (config on + tag).
pub fn should_inject_browser_mcp(cfg: &BrowserConfig, task: &TaskIR) -> bool {
    cfg.is_enabled() && task_has_browser_tag(&task.tags)
}

/// Evidence directory: `{project}/{out_dir}/{task_id}` (absolute).
pub fn browser_out_dir(cfg: &BrowserConfig, project_root: &Path, task_id: &str) -> PathBuf {
    let rel = cfg.out_dir.trim().trim_start_matches('/');
    let base = if rel.is_empty() {
        PathBuf::from(".cco-out/browser")
    } else {
        PathBuf::from(rel)
    };
    project_root.join(base).join(task_id)
}

/// Build Claude Code MCP servers JSON (`mcpServers` map).
pub fn build_mcp_servers_json(cfg: &BrowserConfig) -> serde_json::Value {
    let engine = cfg.effective_engine();
    let (command, args) = match engine.to_ascii_lowercase().as_str() {
        "playwright" | "playwright_mcp" | "playwright-mcp" => {
            let args_look_pw = cfg.args.iter().any(|a| a.contains("playwright"));
            if args_look_pw {
                (cfg.command.clone(), cfg.args.clone())
            } else {
                (
                    "npx".to_string(),
                    vec!["-y".into(), "@playwright/mcp".into()],
                )
            }
        }
        _ => {
            // kitewright (default) — honor command/args from config
            (cfg.command.clone(), cfg.args.clone())
        }
    };
    serde_json::json!({
        "mcpServers": {
            "cco-browser": {
                "command": command,
                "args": args,
            }
        }
    })
}

/// Write `task_dir/mcp-browser.json`; return absolute path.
pub fn write_mcp_config(cfg: &BrowserConfig, task_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(task_dir)
        .with_context(|| format!("mkdir task_dir {}", task_dir.display()))?;
    let path = task_dir.join(MCP_BROWSER_FILE);
    let body = build_mcp_servers_json(cfg);
    let text = serde_json::to_string_pretty(&body).context("serialize mcp-browser.json")?;
    std::fs::write(&path, text)
        .with_context(|| format!("write {}", path.display()))?;
    info!(path = %path.display(), engine = %cfg.effective_engine(), "browser mcp config written");
    Ok(path)
}

/// Env pairs for the worker process.
pub fn browser_env_pairs(
    cfg: &BrowserConfig,
    project_root: &Path,
    task_id: &str,
    preview_url: Option<&str>,
) -> Vec<(String, String)> {
    let out = browser_out_dir(cfg, project_root, task_id);
    let _ = std::fs::create_dir_all(&out);
    let mut env = vec![
        (
            "CCO_BROWSER_OUT".into(),
            out.to_string_lossy().into_owned(),
        ),
        (
            "CCO_BROWSER_ENGINE".into(),
            cfg.effective_engine(),
        ),
    ];
    if let Some(url) = preview_url.map(str::trim).filter(|s| !s.is_empty()) {
        env.push(("CCO_PREVIEW_URL".into(), url.to_string()));
    }
    env
}

/// Apply browser MCP into task provider_opts + collect env (scheduler start).
///
/// No-op when disabled or task lacks `browser` tag.
pub fn prepare_task_browser(
    cfg: &BrowserConfig,
    task: &mut TaskIR,
    project_root: &Path,
    task_dir: &Path,
    preview_url: Option<&str>,
) -> Result<Vec<(String, String)>> {
    if !should_inject_browser_mcp(cfg, task) {
        return Ok(vec![]);
    }
    let mcp_path = write_mcp_config(cfg, task_dir)?;
    if let Some(obj) = task.provider_opts.as_object_mut() {
        obj.insert(
            OPT_MCP_CONFIG.into(),
            serde_json::json!(mcp_path.to_string_lossy()),
        );
        obj.insert(OPT_MCP_STRICT.into(), serde_json::json!(cfg.strict_mcp));
    } else {
        task.provider_opts = serde_json::json!({
            OPT_MCP_CONFIG: mcp_path.to_string_lossy(),
            OPT_MCP_STRICT: cfg.strict_mcp,
        });
    }
    // Ensure browser system prompt if materialize ran without tags (defensive).
    inject_browser_opts_prompt(&mut task.provider_opts);
    Ok(browser_env_pairs(
        cfg,
        project_root,
        &task.id,
        preview_url,
    ))
}

fn inject_browser_opts_prompt(opts: &mut serde_json::Value) {
    use crate::domain::plan::{BROWSER_SYSTEM_PROMPT, BROWSER_SYSTEM_PROMPT_MARKER};
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(BROWSER_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let merged = if existing.trim().is_empty() {
        BROWSER_SYSTEM_PROMPT.to_string()
    } else {
        format!("{existing}\n\n{BROWSER_SYSTEM_PROMPT}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

/// Doctor-style readiness (never hard-fails the whole doctor alone).
pub fn doctor_browser_line(cfg: &BrowserConfig) -> (bool, String, Option<String>) {
    if !cfg.is_enabled() {
        return (
            true,
            "网页自动化：默认关（config.browser.enabled / CCO_BROWSER_ENABLED）".into(),
            Some("https://github.com/kitewright/kitewright".into()),
        );
    }
    let engine = cfg.effective_engine();
    let chrome_ok = chrome_on_path();
    let launcher = which::which(&cfg.command).is_ok()
        || cfg.command == "npx" && which::which("npx").is_ok();
    let detail = format!(
        "enabled engine={engine} command={} chrome={}",
        cfg.command,
        if chrome_ok { "ok" } else { "missing?" }
    );
    let help = match engine.to_ascii_lowercase().as_str() {
        "playwright" | "playwright_mcp" | "playwright-mcp" => {
            Some("https://github.com/microsoft/playwright-mcp".into())
        }
        _ => Some("https://github.com/kitewright/kitewright".into()),
    };
    // Soft: enabled but missing chrome/launcher → ok=false line but doctor overall may still pass
    // if we mark as non-critical (name browser_* and ok=false only when enabled+broken).
    let line_ok = launcher && chrome_ok;
    (
        line_ok,
        if line_ok {
            format!("网页自动化：已就绪（{detail}）")
        } else {
            format!("网页自动化：未就绪（{detail}）— 装 Chrome 与 MCP 启动器后重试")
        },
        help,
    )
}

fn chrome_on_path() -> bool {
    const CANDIDATES: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ];
    if CANDIDATES.iter().any(|c| which::which(c).is_ok()) {
        return true;
    }
    // macOS app bundle
    Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome").is_file()
        || Path::new("/Applications/Chromium.app/Contents/MacOS/Chromium").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BrowserConfig;

    #[test]
    fn mcp_json_kitewright_default() {
        let cfg = BrowserConfig::default();
        let v = build_mcp_servers_json(&cfg);
        let server = &v["mcpServers"]["cco-browser"];
        assert_eq!(server["command"], "npx");
        let args = server["args"].as_array().unwrap();
        assert!(args.iter().any(|a| a.as_str() == Some("@kitewright/mcp")));
    }

    #[test]
    fn should_inject_requires_tag_and_enabled() {
        let mut cfg = BrowserConfig::default();
        let mut task = TaskIR {
            id: "t1".into(),
            title: "x".into(),
            depends_on: vec![],
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: true,
            include: false,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec!["browser".into(), "ui-verify".into()],
        };
        assert!(!should_inject_browser_mcp(&cfg, &task));
        cfg.enabled = true;
        assert!(should_inject_browser_mcp(&cfg, &task));
        task.tags.clear();
        assert!(!should_inject_browser_mcp(&cfg, &task));
    }

    #[test]
    fn write_mcp_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = BrowserConfig {
            enabled: true,
            ..BrowserConfig::default()
        };
        let path = write_mcp_config(&cfg, dir.path()).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(raw.contains("cco-browser"));
        assert!(raw.contains("@kitewright/mcp"));
    }
}
