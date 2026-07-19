//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::{bail, Context, Result};

use crate::cli::commands::common::{
    make_terminal_manager, poll_interval, preflight_providers, provider_parallel_caps, run_scheduler,
};
use crate::cli::interactive;
use crate::config::Config;
use crate::graph::format_graph;
use crate::plan::PlanIR;
use crate::runtime::provider::{ProviderRegistry, TaskStatus};
use crate::runtime::Scheduler;
use crate::state::{self, RunState, RunStatus};
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
    let mut st = RunState::load(&dir)?;
    if matches!(st.status, RunStatus::Running) {
        bail!("run {} is still marked running; stop it first", st.run_id);
    }
    let plan_path = dir.join("plan.resolved.json");
    if !plan_path.exists() {
        bail!("missing plan.resolved.json in {}", dir.display());
    }
    let ir: PlanIR = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)
        .context("parse plan.resolved.json")?;
    let n = st.prepare_for_resume();
    // clear .done for non-success so provider can re-run
    for (id, ts) in &st.tasks {
        if matches!(ts.status, TaskStatus::Pending) {
            let done = dir.join("tasks").join(id).join(".done");
            let _ = std::fs::remove_file(done);
        }
    }
    st.save()?;
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

    let registry = ProviderRegistry::from_config(&config)?;
    preflight_providers(&registry, &ir).await?;
    let tm = make_terminal_manager(&dir, &config);
    let poll = poll_interval(&config);
    let budget = max_budget.or(config.default.run_max_budget_usd);
    let provider_caps = provider_parallel_caps(&config);
    let run_id = st.run_id.clone();

    let sched = Scheduler {
        max_parallel: max_parallel.unwrap_or(ir.max_parallel),
        plan: ir,
        state: st,
        registry,
        poll_interval: poll,
        yes,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
        terminal_manager: Some(tm),
        run_max_budget_usd: budget,
        provider_max_parallel: provider_caps,
        retry_max: config.default.retry_max,
        stall_secs: config.default.stall_secs,
    };
    run_scheduler(sched, &config, &run_id, tui).await
}
