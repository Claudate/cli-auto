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
use crate::domain::plan::{task_has_browser_tag, task_has_ui_verify_tag};
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
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
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
        ("CCO_BROWSER_OUT".into(), out.to_string_lossy().into_owned()),
        ("CCO_BROWSER_ENGINE".into(), cfg.effective_engine()),
    ];
    if let Some(url) = preview_url.map(str::trim).filter(|s| !s.is_empty()) {
        env.push(("CCO_PREVIEW_URL".into(), url.to_string()));
    }
    env
}

/// Soft-fail ui-verify when preview is required but missing (honest, not silent PASS).
///
/// Returns human error when the task should **not** spawn a worker.
pub fn preview_required_missing(
    cfg: &BrowserConfig,
    task: &TaskIR,
    preview_url: Option<&str>,
) -> Option<String> {
    if !cfg.is_enabled() || !cfg.require_preview {
        return None;
    }
    if !task_has_browser_tag(&task.tags) || !task_has_ui_verify_tag(&task.tags) {
        return None;
    }
    let has = preview_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    if has {
        return None;
    }
    Some(
        "网页验收需要本机预览地址（CCO_PREVIEW_URL），当前没有可用预览。\
请先启动预览，或在任务说明里写明 URL；不要假装验收通过。\
（config.browser.require_preview=true；可关 require_preview 仅靠 prompt 自报）"
            .into(),
    )
}

/// Providers that receive host-injected `--mcp-config` (Claude print path).
pub fn provider_supports_host_mcp(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "claude" | "fake"
    )
}

/// When browser is enabled for a non-Claude worker: inject env + prompt only
/// (no `--mcp-config`). Caller should still prepare out dir.
pub fn non_claude_browser_hint(provider: &str) -> String {
    format!(
        "CCO browser note: provider `{provider}` does not get host MCP inject \
(Claude-only). Env CCO_PREVIEW_URL / CCO_BROWSER_OUT are set; configure browser \
MCP on this CLI yourself, or route this step to claude. Evidence still under \
CCO_BROWSER_OUT."
    )
}

/// Apply browser MCP into task provider_opts + collect env (scheduler start).
///
/// - Claude/fake + browser tag + enabled → mcp-config + env + system prompt.
/// - Other providers + browser tag + enabled → env + system prompt + honest note
///   (no `--mcp-config`; shell CLIs have no host MCP inject path).
/// - Errors when ui-verify + require_preview and no preview URL.
pub fn prepare_task_browser(
    cfg: &BrowserConfig,
    task: &mut TaskIR,
    project_root: &Path,
    task_dir: &Path,
    preview_url: Option<&str>,
) -> Result<Vec<(String, String)>> {
    if let Some(msg) = preview_required_missing(cfg, task, preview_url) {
        anyhow::bail!("{msg}");
    }
    if !cfg.is_enabled() || !task_has_browser_tag(&task.tags) {
        return Ok(vec![]);
    }

    let env = browser_env_pairs(cfg, project_root, &task.id, preview_url);
    inject_browser_opts_prompt(&mut task.provider_opts);

    if provider_supports_host_mcp(&task.provider) {
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
            inject_browser_opts_prompt(&mut task.provider_opts);
        }
    } else {
        // Soft support: do not fail the run; surface capability gap in system prompt.
        let hint = non_claude_browser_hint(&task.provider);
        let existing = task
            .provider_opts
            .get("append_system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !existing.contains("does not get host MCP inject") {
            let merged = if existing.trim().is_empty() {
                hint
            } else {
                format!("{existing}\n\n{hint}")
            };
            task.provider_opts["append_system_prompt"] = serde_json::json!(merged);
        }
        info!(
            task = %task.id,
            provider = %task.provider,
            "browser enabled for non-Claude provider: env only, no mcp-config"
        );
    }

    Ok(env)
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
    let launcher =
        which::which(&cfg.command).is_ok() || cfg.command == "npx" && which::which("npx").is_ok();
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

// ── W3: evidence for result desk ─────────────────────────────────────

/// One browser artifact under `.cco-out/browser/<task>/` for desktop result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrowserEvidenceItem {
    pub task_id: String,
    /// `shot` | `report` | `smoke` | `raw` | `other`
    pub kind: String,
    /// Path relative to project root (posix-ish display).
    pub rel_path: String,
    /// Absolute path (open in Finder / debug).
    pub abs_path: String,
    /// Small PNG as `data:image/png;base64,…` when kind=shot and file is small enough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_data_url: Option<String>,
    /// First ~400 chars of report/smoke/raw markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

const MAX_SHOT_BYTES: u64 = 280_000;
const MAX_EVIDENCE_ITEMS: usize = 12;
const EXCERPT_CHARS: usize = 400;

/// Scan `project/{out_dir}/**` for shot.png / report.md / smoke.md / raw.md.
pub fn collect_browser_evidence(
    project_root: &Path,
    out_dir_rel: &str,
) -> Vec<BrowserEvidenceItem> {
    let rel = out_dir_rel.trim().trim_start_matches('/');
    let base = if rel.is_empty() {
        project_root.join(".cco-out/browser")
    } else {
        project_root.join(rel)
    };
    if !base.is_dir() {
        return vec![];
    }
    let mut items = Vec::new();
    let Ok(task_dirs) = std::fs::read_dir(&base) else {
        return vec![];
    };
    let mut dirs: Vec<_> = task_dirs.filter_map(|e| e.ok()).collect();
    dirs.sort_by_key(|e| e.file_name());
    for ent in dirs {
        if !ent.path().is_dir() {
            continue;
        }
        let task_id = ent.file_name().to_string_lossy().into_owned();
        if task_id.starts_with('.') {
            continue;
        }
        for (name, kind) in [
            ("shot.png", "shot"),
            ("shot.jpg", "shot"),
            ("screenshot.png", "shot"),
            ("report.md", "report"),
            ("smoke.md", "smoke"),
            ("raw.md", "raw"),
        ] {
            let path = ent.path().join(name);
            if !path.is_file() {
                continue;
            }
            let rel_path = path
                .strip_prefix(project_root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let mut item = BrowserEvidenceItem {
                task_id: task_id.clone(),
                kind: kind.into(),
                rel_path: rel_path.replace('\\', "/"),
                abs_path: path.display().to_string(),
                preview_data_url: None,
                excerpt: None,
            };
            if kind == "shot" {
                item.preview_data_url = shot_data_url(&path);
            } else {
                item.excerpt = text_excerpt(&path, EXCERPT_CHARS);
            }
            items.push(item);
            if items.len() >= MAX_EVIDENCE_ITEMS {
                return items;
            }
        }
    }
    items
}

fn shot_data_url(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() == 0 || meta.len() > MAX_SHOT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let b64 = encode_base64_std(&bytes);
    let mime = if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false)
    {
        "image/jpeg"
    } else {
        "image/png"
    };
    Some(format!("data:{mime};base64,{b64}"))
}

/// Standard base64 (no external crate).
fn encode_base64_std(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let n = (data[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn text_excerpt(path: &Path, max_chars: usize) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let s: String = t.chars().take(max_chars).collect();
    if t.chars().count() > max_chars {
        Some(format!("{s}…"))
    } else {
        Some(s)
    }
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
            wait_for: vec![],
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
    #[test]
    fn preview_required_blocks_ui_verify() {
        let mut cfg = BrowserConfig {
            enabled: true,
            require_preview: true,
            ..BrowserConfig::default()
        };
        let mut task = TaskIR {
            id: "ui".into(),
            title: "shot".into(),
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
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec!["browser".into(), "ui-verify".into()],
            wait_for: vec![],
        };
        assert!(preview_required_missing(&cfg, &task, None).is_some());
        assert!(preview_required_missing(&cfg, &task, Some("http://127.0.0.1:5173/")).is_none());
        cfg.require_preview = false;
        assert!(preview_required_missing(&cfg, &task, None).is_none());
        task.tags = vec!["browser".into(), "ui-smoke".into()];
        cfg.require_preview = true;
        assert!(preview_required_missing(&cfg, &task, None).is_none());
    }

    #[test]
    fn collect_finds_shot_and_report() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let task_dir = root.join(".cco-out/browser/ui-shot");
        std::fs::create_dir_all(&task_dir).unwrap();
        // minimal 1x1 PNG
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xfe,
            0xd4, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        std::fs::write(task_dir.join("shot.png"), png).unwrap();
        std::fs::write(task_dir.join("report.md"), "# ok\n主 CTA 可见\n").unwrap();
        let items = collect_browser_evidence(root, ".cco-out/browser");
        assert!(items
            .iter()
            .any(|i| i.kind == "shot" && i.preview_data_url.is_some()));
        assert!(items
            .iter()
            .any(|i| i.kind == "report" && i.excerpt.as_deref().unwrap_or("").contains("主 CTA")));
    }

    #[test]
    fn base64_roundtrip_len() {
        let s = encode_base64_std(b"hi");
        assert_eq!(s, "aGk=");
    }

    #[test]
    fn non_claude_gets_env_without_mcp_config() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        let task_dir = project.join("tasks/sc1");
        std::fs::create_dir_all(&task_dir).unwrap();
        let cfg = BrowserConfig {
            enabled: true,
            require_preview: false,
            ..BrowserConfig::default()
        };
        let mut task = TaskIR {
            id: "sc1".into(),
            title: "s".into(),
            depends_on: vec![],
            group: None,
            provider: "codex".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: true,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec!["browser".into(), "ui-smoke".into()],
            wait_for: vec![],
        };
        let env = prepare_task_browser(
            &cfg,
            &mut task,
            project,
            &task_dir,
            Some("http://127.0.0.1:5173/"),
        )
        .unwrap();
        assert!(env.iter().any(|(k, _)| k == "CCO_BROWSER_OUT"));
        assert!(task.provider_opts.get("mcp_config").is_none());
        let sys = task
            .provider_opts
            .get("append_system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(sys.contains("does not get host MCP inject"), "{sys}");
    }

    #[test]
    fn provider_supports_host_mcp_claude_only() {
        assert!(provider_supports_host_mcp("claude"));
        assert!(provider_supports_host_mcp("fake"));
        assert!(!provider_supports_host_mcp("codex"));
    }
}
