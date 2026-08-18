//! CLI `cco resume` — thin presentation over [`crate::app::run`] (A5-1).
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；prepare_resume + prepare_scheduler；**禁止**手搓 Scheduler 字段策略
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::app::run as run_uc;
use crate::cli::commands::common::{run_scheduler_loop, HeadlessMode};
use crate::cli::interactive;
use crate::config::Config;
use crate::graph::format_graph;
use crate::state;
use crate::terminal::SessionKind;

pub async fn run(
    config: &Config,
    run_id: Option<String>,
    yes: bool,
    max_parallel: Option<usize>,
    tui: bool,
    max_budget: Option<f64>,
) -> Result<i32> {
    let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
    let rid = state::RunState::load(&dir)?.run_id;

    let (ir, st, n) = run_uc::prepare_resume(config, &rid)?;
    println!(
        "resume {} · reset {n} unfinished task(s) · project={}",
        st.run_id,
        st.project_root.display()
    );
    print!("{}", format_graph(&ir));
    if !yes && !interactive::confirm("resume?", true)? {
        println!("aborted");
        return Ok(1);
    }

    let opts = run_uc::ForegroundOpts {
        max_parallel,
        yes,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
        max_budget,
    };
    let sched = run_uc::prepare_scheduler(config, ir, st, opts, None)?;
    run_uc::preflight_plan(&sched.registry, &sched.plan).await?;
    run_scheduler_loop(sched, config, &rid, tui, HeadlessMode::Off).await
}
