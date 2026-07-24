//! CLI `cco status` — observe via [`crate::app::run`] (A5-1).
//!
//! [INPUT]: Config · clap fields
//! [OUTPUT]: exit code
//! [POS]: cli/commands；load + handoff_paths；**禁止**碰 handoff 内部类型
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/cli/CLAUDE.md

use anyhow::Result;

use crate::app::run as run_uc;
use crate::config::Config;
use crate::state;

pub fn run(config: &Config, run_id: Option<String>) -> Result<i32> {
    let dir = state::resolve_run_dir(&config.runs_dir(), run_id.as_deref())?;
    // Prefer app query; dir already resolved for "latest" default.
    let st = run_uc::load_by_dir(&dir)?;
    // H0-5 / H1-4: first line shared StatusOneLiner; machine fields follow.
    println!("{}", crate::app::run::from_run_state(&st).text);
    println!("run_id: {}", st.run_id);
    println!("status: {:?}", st.status);
    println!("project: {}", st.project_root.display());
    println!("plan: {}", st.plan_path.display());
    println!("dir: {}", st.run_dir.display());
    print!("{}", run_uc::format_status_by_provider(&st));
    let (handoff_md, handoff_json) = run_uc::handoff_paths(&st.run_dir);
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
