//! Preflight checks.
//!
//! [INPUT]: 依赖 config::Config 与 runtime::provider::ProviderRegistry
//! [OUTPUT]: 对外提供 DoctorReport / CheckLine / run_doctor / print_report
//! [POS]: doctor 模块入口，桌面与 CLI 共用的环境门禁
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/doctor/CLAUDE.md

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::runtime::provider::shell_print::provider_docs_url;
use crate::runtime::provider::ProviderRegistry;

mod provider_probe;
pub use provider_probe::{probe_provider, ProbeResult};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub lines: Vec<CheckLine>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckLine {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    /// Optional official docs / download page (desktop 「官网下载」).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help_url: Option<String>,
    /// Check category: "binary" | "auth" | "info" (default "info", backward compatible).
    #[serde(default)]
    pub kind: String,
    /// Provider probe result (auth/balance). None for legacy info lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeResult>,
}

impl CheckLine {
    fn ok_line(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ok: true,
            detail: detail.into(),
            help_url: None,
            kind: "info".into(),
            probe: None,
        }
    }

    fn fail_line(
        name: impl Into<String>,
        detail: impl Into<String>,
        help_url: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            ok: false,
            detail: detail.into(),
            help_url,
            kind: "info".into(),
            probe: None,
        }
    }

    fn binary_line(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ok,
            detail: detail.into(),
            help_url: None,
            kind: "binary".into(),
            probe: None,
        }
    }

    fn auth_line(name: impl Into<String>, ok: bool, detail: impl Into<String>, probe: ProbeResult) -> Self {
        Self {
            name: name.into(),
            ok,
            detail: detail.into(),
            help_url: None,
            kind: "auth".into(),
            probe: Some(probe),
        }
    }
}

fn provider_help_url(name: &str) -> Option<String> {
    provider_docs_url(name).map(|s| s.to_string())
}

pub async fn run_doctor(config: &Config, project_root: Option<&Path>) -> Result<DoctorReport> {
    let mut lines = Vec::new();
    let mut ok = true;

    // state root
    let state = &config.state_root;
    match std::fs::create_dir_all(state) {
        Ok(()) => lines.push(CheckLine::ok_line(
            "state_root",
            state.display().to_string(),
        )),
        Err(e) => {
            ok = false;
            lines.push(CheckLine::fail_line(
                "state_root",
                format!("{} ({e})", state.display()),
                None,
            ));
        }
    }

    // project
    if let Some(p) = project_root {
        if p.is_dir() {
            lines.push(CheckLine::ok_line("project_root", p.display().to_string()));
            if p.join(".git").exists() {
                lines.push(CheckLine::ok_line("git", "repository detected"));
            } else {
                lines.push(CheckLine::ok_line("git", "not a git repo (warning only)"));
            }
        } else {
            ok = false;
            lines.push(CheckLine::fail_line(
                "project_root",
                format!("not a directory: {}", p.display()),
                None,
            ));
        }
    }

    // API key (for claude bare)
    match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => lines.push(CheckLine::ok_line(
            "ANTHROPIC_API_KEY",
            format!("set ({}…)", k.chars().take(8).collect::<String>()),
        )),
        _ => lines.push(CheckLine::ok_line(
            "ANTHROPIC_API_KEY",
            "not set (required for claude --bare print mode)",
        )),
    }

    // providers
    let registry = ProviderRegistry::from_config(config)?;
    let mut any_provider_ok = false;
    let default_name = config.default.default_provider.as_str();
    let preflight_results = registry.preflight_all().await;
    for (name, res) in &preflight_results {
        let binary_name = format!("provider:{name}:binary");
        let is_default = name.as_str() == default_name;
        match res {
            Ok(()) => {
                any_provider_ok = true;
                lines.push(CheckLine::binary_line(binary_name, true, "ok"));
            }
            Err(e) => {
                let help = provider_help_url(name);
                let detail = if let Some(url) = help.as_deref() {
                    format!("{e:#} · 下载: {url}")
                } else {
                    format!("{e:#}")
                };
                // Non-default missing CLI = soft tip (line.ok=true so it won't
                // pollute the workspace warn bar). Default binary fail stays hard.
                lines.push(CheckLine {
                    name: binary_name,
                    ok: !is_default,
                    detail,
                    help_url: help,
                    kind: "binary".into(),
                    probe: None,
                });
            }
        }
    }

    // Auth/balance probe per enabled provider (A: doctor enhancement).
    // Default provider: only hard auth fails (invalid key / no balance / dead
    // endpoint) mark the line failed. Missing API Key + CLI login
    // (`not_supported`) must NOT fail — Claude subscription users have no
    // ANTHROPIC_API_KEY and still work. Non-default auth never fails the line.
    //
    // Each probe is a network round-trip with a 6s timeout; run them
    // concurrently, then fold in registration order for a stable report.
    let cfg_arc = std::sync::Arc::new(config.clone());
    let probe_handles: Vec<_> = preflight_results
        .iter()
        .map(|(name, _)| name.clone())
        // fake is a drill/demo stub — always ok, no key to probe.
        .filter(|name| name != "fake")
        .map(|name| {
            let cfg = std::sync::Arc::clone(&cfg_arc);
            tokio::spawn(async move {
                let probe = probe_provider(&name, &cfg).await;
                (name, probe)
            })
        })
        .collect();
    for h in probe_handles {
        let Ok((name, probe)) = h.await else { continue };
        let auth_name = format!("provider:{name}:auth");
        let is_default = name.as_str() == default_name;
        let line_ok = !is_default || !probe.is_blocking_auth_fail();
        let detail = probe.detail_line();
        lines.push(CheckLine::auth_line(auth_name, line_ok, detail, probe));
    }

    // 汇总：默认 provider binary 不可用则整体失败；否则通过
    let default_binary_ok = preflight_results
        .iter()
        .find(|(n, _)| n == default_name)
        .map(|(_, r)| r.is_ok())
        .unwrap_or(any_provider_ok);
    if !default_binary_ok {
        ok = false;
    }
    // 默认 provider 硬 auth 失败才拉红（Key 无效/余额耗尽/接口挂）
    let default_auth_ok = lines
        .iter()
        .find(|l| l.name == format!("provider:{default_name}:auth"))
        .map(|l| l.ok)
        .unwrap_or(true);
    if !default_auth_ok {
        ok = false;
    }
    // git binary (worktree)
    match which::which("git") {
        Ok(p) => lines.push(CheckLine::ok_line("git_bin", p.display().to_string())),
        Err(_) => lines.push(CheckLine::ok_line(
            "git_bin",
            "git not in PATH (worktree disabled until available)",
        )),
    }

    // P1-7: mixed-plan tip (info only; never fails doctor)
    lines.push(CheckLine::ok_line(
        "run_provider_flags",
        "cco run --provider fills defaults only; --force-provider wipes all tasks",
    ));

    // Browser MCP (W1): never fails overall doctor when disabled; when enabled+broken, warn only.
    let (browser_line_ok, browser_detail, browser_help) =
        crate::runtime::browser_mcp::doctor_browser_line(&config.browser);
    lines.push(CheckLine {
        name: "browser_automation".into(),
        // Soft: missing chrome with enabled=true does not fail whole doctor.
        ok: true,
        detail: if browser_line_ok {
            browser_detail
        } else {
            format!("{browser_detail} (提示，不挡无浏览器计划)")
        },
        help_url: browser_help,
        kind: "info".into(),
        probe: None,
    });

    Ok(DoctorReport { lines, ok })
}

pub fn print_report(report: &DoctorReport) {
    for line in &report.lines {
        let mark = if line.ok { "ok" } else { "FAIL" };
        println!("  [{mark}] {:<22} {}", line.name, line.detail);
        if let Some(url) = &line.help_url {
            println!("           download: {url}");
        }
    }
    if report.ok {
        println!("\ndoctor: all critical checks passed");
    } else {
        println!("\ndoctor: some checks failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_help_urls_known() {
        assert!(provider_help_url("gemini").unwrap().contains("gemini-cli"));
        assert!(provider_help_url("claude").unwrap().contains("claude-code"));
        assert!(provider_help_url("codebuddy").is_some());
        assert!(provider_help_url("unknown-x").is_none());
    }
}
