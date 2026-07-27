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
}

impl CheckLine {
    fn ok_line(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ok: true,
            detail: detail.into(),
            help_url: None,
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
        Ok(()) => lines.push(CheckLine::ok_line("state_root", state.display().to_string())),
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
    for (name, res) in registry.preflight_all().await {
        match res {
            Ok(()) => {
                any_provider_ok = true;
                lines.push(CheckLine::ok_line(format!("provider:{name}"), "ok"));
            }
            Err(e) => {
                // 非默认 provider 的失败降为提示；默认 provider 失败才拉红
                let is_default = name == config.default.default_provider;
                let help = provider_help_url(&name);
                let detail = if let Some(url) = help.as_deref() {
                    format!("{e:#} · 下载: {url}")
                } else {
                    format!("{e:#}")
                };
                lines.push(CheckLine {
                    name: format!("provider:{name}"),
                    ok: !is_default,
                    detail,
                    help_url: help,
                });
            }
        }
    }
    // 汇总：默认 provider 不可用则整体失败；否则通过
    let default_name = config.default.default_provider.as_str();
    let default_line_ok = lines
        .iter()
        .find(|l| l.name == format!("provider:{default_name}"))
        .map(|l| l.ok)
        .unwrap_or(any_provider_ok);
    if !default_line_ok {
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
        assert!(provider_help_url("gemini")
            .unwrap()
            .contains("gemini-cli"));
        assert!(provider_help_url("claude")
            .unwrap()
            .contains("claude-code"));
        assert!(provider_help_url("codebuddy").is_some());
        assert!(provider_help_url("unknown-x").is_none());
    }
}
