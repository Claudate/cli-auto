//! Shared helpers for CLI command handlers.
//!
//! [INPUT]: Config · RunState · PlanIR · ProviderRegistry
//! [OUTPUT]: plan_then_load_ir · run_scheduler · term/poll helpers
//! [POS]: cli/commands 内部共用；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::PlanIR;
use crate::report;
use crate::runtime::provider::ProviderRegistry;
use crate::runtime::Scheduler;
use crate::state::{RunState, RunStatus};
use crate::terminal::{SessionKind, TerminalManager};

/// Mode B: plan job → poll → load proposed PlanIR (CLI `run` prose path).
pub(crate) fn plan_then_load_ir(
    config: &Config,
    project: &std::path::Path,
    plan: &std::path::Path,
    plan_mode: &str,
    provider: Option<String>,
    mode: Option<String>,
    max_parallel: Option<usize>,
) -> Result<(PlanIR, String)> {
    let view = crate::services::start_plan_job(
        config,
        crate::services::StartPlanJobRequest {
            project: project.to_path_buf(),
            plan: plan.to_path_buf(),
            plan_mode: Some(plan_mode.to_string()),
            provider,
            mode,
            max_parallel,
        },
    )?;
    let mut view = view;
    if view.status == "planning" {
        println!("planning… (job {})", view.job_id);
        for _ in 0..600 {
            std::thread::sleep(Duration::from_millis(500));
            view = crate::services::get_plan_job(config, &view.job_id)?;
            if view.status != "planning" {
                break;
            }
            if !view.planner_log_tail.is_empty() {
                if let Some(last) = view.planner_log_tail.lines().last() {
                    eprint!("\r{}", last.chars().take(100).collect::<String>());
                }
            }
        }
        eprintln!();
    }
    if view.status == "plan_failed" {
        bail!(
            "plan failed: {}",
            view.error.as_deref().unwrap_or("unknown")
        );
    }
    if view.status != "planned" && view.status != "confirmed" {
        bail!("unexpected plan job status: {}", view.status);
    }
    println!(
        "job_id: {}  tasks: {}  max_parallel: {}",
        view.job_id,
        view.task_count.unwrap_or(0),
        view.max_parallel.unwrap_or(0)
    );
    for (i, layer) in view.layers.iter().enumerate() {
        println!("wave {}: {}", i + 1, layer.join(", "));
    }
    let ir = crate::plan::planner::load_proposed(config, &view.job_id)
        .with_context(|| format!("load proposed for job {}", view.job_id))?;
    Ok((ir, view.job_id))
}

pub(crate) fn task_paths(st: &RunState, task: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let task_dir = st.task_dir(task);
    let mut cwd = st.project_root.clone();
    let wd = task_dir.join("work_dir.json");
    if wd.exists() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(wd)?) {
            if let Some(p) = v.get("work_dir").and_then(|x| x.as_str()) {
                cwd = PathBuf::from(p);
            }
        }
    } else if let Some(ts) = st.tasks.get(task) {
        if let Some(p) = &ts.work_dir {
            cwd = p.clone();
        }
    }
    let stdout = task_dir.join("stdout.json");
    let stderr = task_dir.join("stderr.log");
    // ensure log files exist so tail -f works
    if !stdout.exists() {
        let _ = std::fs::write(&stdout, "");
    }
    if !stderr.exists() {
        let _ = std::fs::write(&stderr, "");
    }
    Ok((cwd, stdout, stderr))
}

pub(crate) fn poll_interval(config: &Config) -> Duration {
    if std::env::var("CCO_FAST_POLL").is_ok() {
        Duration::from_millis(50)
    } else {
        Duration::from_millis((config.default.poll_interval_secs.max(1) * 1000).min(5_000))
    }
}

pub(crate) fn resolve_term_kind(arg: Option<super::super::TermKindArg>, config: &Config) -> SessionKind {
    match arg {
        Some(super::super::TermKindArg::Embedded) => SessionKind::Embedded,
        Some(super::super::TermKindArg::External) => SessionKind::External,
        None => {
            if config.terminal.default_kind == "external" {
                SessionKind::External
            } else {
                SessionKind::Embedded
            }
        }
    }
}

pub(crate) fn make_terminal_manager(run_dir: &std::path::Path, config: &Config) -> TerminalManager {
    TerminalManager::for_run(
        run_dir,
        &config.terminal.external_launcher,
        config.terminal.external_command.clone(),
    )
    .with_limits(config.terminal.max_embedded, config.terminal.max_external)
}

pub(crate) fn provider_parallel_caps(config: &Config) -> HashMap<String, usize> {
    config
        .providers
        .iter()
        .filter_map(|(name, pc)| pc.max_parallel.map(|n| (name.clone(), n)))
        .collect()
}

pub(crate) async fn preflight_providers(registry: &ProviderRegistry, ir: &PlanIR) -> Result<()> {
    let used: HashSet<_> = ir.tasks.iter().map(|t| t.provider.clone()).collect();
    for name in &used {
        let p = registry.get(name)?;
        if let Err(e) = p.preflight().await {
            bail!("provider {name} preflight failed: {e:#}");
        }
    }
    Ok(())
}

pub(crate) async fn run_scheduler(
    sched: Scheduler,
    config: &Config,
    run_id: &str,
    tui: bool,
) -> Result<i32> {
    if tui {
        let run_dir_tui = config.runs_dir().join(run_id);
        let config_tui = config.clone();
        let join = tokio::spawn(async move { sched.run().await });
        let opts = crate::tui::options_from_config(run_dir_tui, &config_tui);
        let tui_result = tokio::task::spawn_blocking(move || crate::tui::run_tui(opts)).await;
        match tui_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("tui error: {e:#}"),
            Err(e) => eprintln!("tui join error: {e:#}"),
        }
        let status = join
            .await
            .map_err(|e| anyhow::anyhow!("scheduler join: {e}"))??;
        let run_dir = config.runs_dir().join(run_id);
        let st = RunState::load(&run_dir)?;
        report::write_reports(&st)?;
        println!("\nstatus: {:?}", status);
        println!("report: {}", run_dir.join("report.md").display());
        return Ok(match status {
            RunStatus::Completed => 0,
            RunStatus::Paused => 2,
            _ => 1,
        });
    }

    let status = sched.run().await?;
    let run_dir = config.runs_dir().join(run_id);
    let st = RunState::load(&run_dir)?;
    report::write_reports(&st)?;
    println!("\nstatus: {:?}", status);
    println!("report: {}", run_dir.join("report.md").display());
    Ok(match status {
        RunStatus::Completed => 0,
        RunStatus::Paused => 2,
        _ => 1,
    })
}

pub(crate) fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15);
            let _ = kill(pid as i32, 9);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}

