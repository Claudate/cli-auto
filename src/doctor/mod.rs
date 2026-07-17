//! Preflight checks.
//!
//! [INPUT]: 依赖 config::Config 与 runtime::provider::ProviderRegistry
//! [OUTPUT]: 对外提供 DoctorReport / CheckLine / run_doctor / print_report
//! [POS]: doctor 模块入口，桌面与 CLI 共用的环境门禁
//! [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::runtime::provider::ProviderRegistry;

pub struct DoctorReport {
    pub lines: Vec<CheckLine>,
    pub ok: bool,
}

pub struct CheckLine {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub async fn run_doctor(config: &Config, project_root: Option<&Path>) -> Result<DoctorReport> {
    let mut lines = Vec::new();
    let mut ok = true;

    // state root
    let state = &config.state_root;
    match std::fs::create_dir_all(state) {
        Ok(()) => lines.push(CheckLine {
            name: "state_root".into(),
            ok: true,
            detail: state.display().to_string(),
        }),
        Err(e) => {
            ok = false;
            lines.push(CheckLine {
                name: "state_root".into(),
                ok: false,
                detail: format!("{} ({e})", state.display()),
            });
        }
    }

    // project
    if let Some(p) = project_root {
        if p.is_dir() {
            lines.push(CheckLine {
                name: "project_root".into(),
                ok: true,
                detail: p.display().to_string(),
            });
            if p.join(".git").exists() {
                lines.push(CheckLine {
                    name: "git".into(),
                    ok: true,
                    detail: "repository detected".into(),
                });
            } else {
                lines.push(CheckLine {
                    name: "git".into(),
                    ok: true,
                    detail: "not a git repo (warning only)".into(),
                });
            }
        } else {
            ok = false;
            lines.push(CheckLine {
                name: "project_root".into(),
                ok: false,
                detail: format!("not a directory: {}", p.display()),
            });
        }
    }

    // API key (for claude bare)
    match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => lines.push(CheckLine {
            name: "ANTHROPIC_API_KEY".into(),
            ok: true,
            detail: format!("set ({}…)", k.chars().take(8).collect::<String>()),
        }),
        _ => lines.push(CheckLine {
            name: "ANTHROPIC_API_KEY".into(),
            ok: true,
            detail: "not set (required for claude --bare print mode)".into(),
        }),
    }

    // providers
    let registry = ProviderRegistry::from_config(config)?;
    let mut any_provider_ok = false;
    for (name, res) in registry.preflight_all().await {
        match res {
            Ok(()) => {
                any_provider_ok = true;
                lines.push(CheckLine {
                    name: format!("provider:{name}"),
                    ok: true,
                    detail: "ok".into(),
                });
            }
            Err(e) => {
                // 非默认 provider 的失败降为提示；默认 provider 失败才拉红
                let is_default = name == config.default.default_provider;
                lines.push(CheckLine {
                    name: format!("provider:{name}"),
                    ok: !is_default,
                    detail: format!("{e:#}"),
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
        Ok(p) => lines.push(CheckLine {
            name: "git_bin".into(),
            ok: true,
            detail: p.display().to_string(),
        }),
        Err(_) => lines.push(CheckLine {
            name: "git_bin".into(),
            ok: true,
            detail: "git not in PATH (worktree disabled until available)".into(),
        }),
    }

    Ok(DoctorReport { lines, ok })
}

pub fn print_report(report: &DoctorReport) {
    for line in &report.lines {
        let mark = if line.ok { "ok" } else { "FAIL" };
        println!("  [{mark}] {:<22} {}", line.name, line.detail);
    }
    if report.ok {
        println!("\ndoctor: all critical checks passed");
    } else {
        println!("\ndoctor: some checks failed");
    }
}
