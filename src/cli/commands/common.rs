//! Shared helpers for CLI command handlers (A5-1 thin).
//!
//! [INPUT]: Config · plan path · Scheduler
//! [OUTPUT]: plan_then_load_ir · run_scheduler_loop · term helpers
//! [POS]: cli/commands 内部共用；调度装配在 `app::run`，本文件只 poll + 打印
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::PlanIR;
use crate::runtime::Scheduler;
use crate::terminal::SessionKind;

/// B2: completion output format for headless mode.
#[derive(Clone, Copy)]
pub(crate) enum OutputKind {
    Human,
    Json,
}

/// B2: headless routing for `run_scheduler_loop`.
/// `Off` = interactive/normal; `On(kind)` = no TUI, route machine lines to stderr.
#[derive(Clone, Copy)]
pub(crate) enum HeadlessMode {
    Off,
    On(OutputKind),
}

/// Mode B: plan job → poll → load proposed PlanIR (CLI `run` prose path, display only).
///
/// Open-run is **not** here — caller uses [`crate::app::split::confirm_materialize`].
pub(crate) fn plan_then_load_ir(
    config: &Config,
    project: &std::path::Path,
    plan: &std::path::Path,
    plan_mode: &str,
    provider: Option<String>,
    mode: Option<String>,
    max_parallel: Option<usize>,
) -> Result<(PlanIR, String)> {
    use crate::app::split as split_uc;
    use crate::plan::planner::StartPlanJobRequest;

    let view = split_uc::start_job(
        config,
        StartPlanJobRequest {
            project: project.to_path_buf(),
            plan: plan.to_path_buf(),
            plan_mode: Some(plan_mode.to_string()),
            provider,
            mode,
            max_parallel,
            preserve_from_job_id: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        },
    )?;
    let mut view = view;
    if view.status == "planning" {
        println!("planning… (job {})", view.job_id);
        for _ in 0..600 {
            std::thread::sleep(Duration::from_millis(500));
            view = split_uc::get_job(config, &view.job_id)?;
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
    let ir = split_uc::load_proposed_plan(config, &view.job_id)
        .with_context(|| format!("load proposed for job {}", view.job_id))?;
    Ok((ir, view.job_id))
}

pub(crate) fn task_paths(
    st: &crate::state::RunState,
    task: &str,
) -> Result<(PathBuf, PathBuf, PathBuf)> {
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
    if !stdout.exists() {
        let _ = std::fs::write(&stdout, "");
    }
    if !stderr.exists() {
        let _ = std::fs::write(&stderr, "");
    }
    Ok((cwd, stdout, stderr))
}

pub(crate) fn resolve_term_kind(
    arg: Option<super::super::TermKindArg>,
    config: &Config,
) -> SessionKind {
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

/// Run scheduler foreground (optional TUI) and finish with reports via app.
///
/// B2: `headless` routes the completion output — JSON / human summary to stdout,
/// run_id / run_dir / report path to stderr (so `--output json` stdout is parseable).
pub(crate) async fn run_scheduler_loop(
    sched: Scheduler,
    config: &Config,
    run_id: &str,
    tui: bool,
    headless: HeadlessMode,
) -> Result<i32> {
    use crate::app::run as run_uc;

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
        print_run_finish(config, run_id, status, headless);
        let code = run_uc::finish_with_reports(config, run_id, status)?;
        report_path_line(config, run_id, headless);
        return Ok(code);
    }

    let status = sched.run().await?;
    print_run_finish(config, run_id, status, headless);
    let code = run_uc::finish_with_reports(config, run_id, status)?;
    report_path_line(config, run_id, headless);
    Ok(code)
}

/// Emit the completion summary. Headless → stdout (JSON or human one-liner);
/// interactive → stdout (human block). Matches §U-B2.
fn print_run_finish(
    config: &Config,
    run_id: &str,
    status: crate::state::RunStatus,
    headless: HeadlessMode,
) {
    let run_dir = config.runs_dir().join(run_id);
    let st = crate::state::RunState::load(&run_dir).ok();
    match headless {
        HeadlessMode::On(OutputKind::Json) => {
            // JSON is the UX: stdout gets one JSON object (rule 23: summary first).
            let plan = st.as_ref().and_then(|s| load_plan_resolved_opt(&s.run_dir));
            let st = match &st {
                Some(s) => s,
                None => {
                    println!(
                        "{{\"summary\":\"本轮状态：{:?}\",\"status\":\"failed\",\"tasks\":[],\"failed_tasks\":[],\"exit_code\":1}}",
                        status
                    );
                    return;
                }
            };
            let result = crate::report::headless_result(st, plan.as_ref());
            match serde_json::to_string(&result) {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("headless json error: {e:#}"),
            }
        }
        HeadlessMode::On(OutputKind::Human) => {
            // Human one-liner to stdout; machine enum to stderr (keeps stdout clean).
            if let Some(s) = &st {
                println!("{}", crate::report::report_summary_line(s).replace("**", "").trim());
            } else {
                println!("本轮状态：{:?}", status);
            }
            eprintln!("status: {:?}", status);
        }
        HeadlessMode::Off => {
            if let Some(s) = &st {
                println!("\n{}", crate::app::run::from_run_state(s).text);
            } else {
                println!("\n本轮状态：{:?}", status);
            }
            println!("status: {:?}", status);
        }
    }
}

/// `report:` path line. Headless → stderr; interactive → stdout.
fn report_path_line(config: &Config, run_id: &str, headless: HeadlessMode) {
    let path = config
        .runs_dir()
        .join(run_id)
        .join("report.md")
        .display()
        .to_string();
    match headless {
        HeadlessMode::On(_) => eprintln!("report: {path}"),
        HeadlessMode::Off => println!("report: {path}"),
    }
}

/// Thin wrapper so `print_run_finish` doesn't re-import the fallback module path.
fn load_plan_resolved_opt(run_dir: &std::path::Path) -> Option<PlanIR> {
    crate::report::load_plan_resolved(run_dir)
}
