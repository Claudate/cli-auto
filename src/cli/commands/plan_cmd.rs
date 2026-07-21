//! CLI `cco plan` — thin presentation over [`crate::app::split`].
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；A1-7 只规划，开跑经桌面 confirm / split::confirm
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use crate::app::split as split_uc;
use crate::cli::interactive;
use crate::config::Config;
use crate::plan::planner::StartPlanJobRequest;

pub fn run(
    config: &Config,
    project: Option<PathBuf>,
    plan: Option<PathBuf>,
    plan_mode: String,
    provider: Option<String>,
    mode: Option<String>,
    json: bool,
) -> Result<i32> {
    let project = interactive::resolve_project(project, true)?;
    interactive::ensure_project_dir(&project)?;
    let plan = interactive::resolve_plan(&project, plan, true)?;
    let view = split_uc::start_job(
        config,
        StartPlanJobRequest {
            project: project.clone(),
            plan: plan.clone(),
            plan_mode: Some(plan_mode),
            provider,
            mode,
            max_parallel: None,
            preserve_from_job_id: None,
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
        eprintln!("plan failed: {}", view.error.as_deref().unwrap_or("?"));
        if !view.planner_log_tail.is_empty() {
            eprintln!("--- planner log ---\n{}", view.planner_log_tail);
        }
        return Ok(1);
    }
    println!("job_id: {}", view.job_id);
    println!(
        "status: {}  name: {}  tasks: {}  max_parallel: {}  mode: {}",
        view.status,
        view.plan_name.as_deref().unwrap_or("—"),
        view.task_count.unwrap_or(0),
        view.max_parallel.unwrap_or(0),
        view.plan_mode
    );
    for (i, layer) in view.layers.iter().enumerate() {
        println!("wave {}: {}", i + 1, layer.join(", "));
    }
    for t in &view.tasks {
        let deps = if t.depends_on.is_empty() {
            "—".into()
        } else {
            t.depends_on.join(",")
        };
        println!("  - {}  \"{}\"  depends=[{deps}]", t.id, t.title);
    }
    let proposed = crate::plan::planner::job_dir(config, &view.job_id).join("plan.proposed.json");
    println!("proposed: {}", proposed.display());
    if json {
        if let Ok(body) = std::fs::read_to_string(&proposed) {
            println!("{body}");
        }
    }
    println!("next: open desktop App to confirm, or use proposed JSON as plan source");
    Ok(0)
}
