//! CLI subcommand handler.
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；D4 自 mod.rs 抽出；P1-8 per-provider 分栏 + handoff 路径
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::config::Config;
use crate::report;
use crate::runtime::handoff::Handoff;
use crate::state::{self, RunState};

pub fn run(config: &Config, run_id: Option<String>) -> Result<i32> {
    let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
    let st = RunState::load(&dir)?;
    println!("run_id: {}", st.run_id);
    println!("status: {:?}", st.status);
    println!("project: {}", st.project_root.display());
    println!("plan: {}", st.plan_path.display());
    println!("dir: {}", st.run_dir.display());
    // P1-8: per-provider rollup (running / done / failed / cost).
    print!("{}", report::format_status_by_provider(&st.tasks));
    let handoff_md = Handoff::path_md(&st.run_dir);
    let handoff_json = Handoff::path_json(&st.run_dir);
    println!(
        "handoff.md: {} ({})",
        handoff_md.display(),
        if handoff_md.exists() {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "handoff.json: {} ({})",
        handoff_json.display(),
        if handoff_json.exists() {
            "present"
        } else {
            "missing"
        }
    );
    println!("tasks:");
    let mut ids: Vec<_> = st.tasks.keys().cloned().collect();
    ids.sort();
    for id in ids {
        let t = &st.tasks[&id];
        let wd = t
            .work_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".into());
        println!(
            "  {id}: {:?} provider={} cost={:?} work_dir={wd} terms={}",
            t.status,
            t.provider,
            t.cost_usd,
            t.terminals.len()
        );
    }
    Ok(0)
}
