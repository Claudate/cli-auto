//! Markdown + JSON reports.

use std::path::Path;

use anyhow::Result;

use crate::state::RunState;

pub fn write_reports(state: &RunState) -> Result<()> {
    let json_path = state.run_dir.join("report.json");
    let md_path = state.run_dir.join("report.md");

    let mut total_cost = 0.0;
    let mut has_cost = false;
    for t in state.tasks.values() {
        if let Some(c) = t.cost_usd {
            total_cost += c;
            has_cost = true;
        }
    }

    let summary = serde_json::json!({
        "run_id": state.run_id,
        "status": state.status,
        "project_root": state.project_root,
        "plan_path": state.plan_path,
        "adapter": state.adapter,
        "started_at": state.started_at,
        "finished_at": state.finished_at,
        "total_cost_usd": if has_cost { Some(total_cost) } else { None },
        "tasks": state.tasks,
    });
    std::fs::write(&json_path, serde_json::to_string_pretty(&summary)?)?;

    let mut md = String::new();
    md.push_str(&format!("# cco report · {}\n\n", state.run_id));
    md.push_str(&format!("- **status**: {:?}\n", state.status));
    md.push_str(&format!("- **project**: `{}`\n", state.project_root.display()));
    md.push_str(&format!("- **plan**: `{}`\n", state.plan_path.display()));
    md.push_str(&format!("- **adapter**: {}\n", state.adapter));
    md.push_str(&format!("- **started**: {}\n", state.started_at.to_rfc3339()));
    if let Some(f) = state.finished_at {
        md.push_str(&format!("- **finished**: {}\n", f.to_rfc3339()));
    }
    if has_cost {
        md.push_str(&format!("- **total_cost_usd**: {total_cost:.4}\n"));
    }
    md.push_str("\n## Tasks\n\n");
    md.push_str("| id | status | provider | cost | session | worktree | terms |\n");
    md.push_str("|----|--------|----------|------|---------|----------|-------|\n");
    let mut ids: Vec<_> = state.tasks.keys().cloned().collect();
    ids.sort();
    for id in ids {
        let t = &state.tasks[&id];
        md.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {} | {} |\n",
            id,
            t.status,
            t.provider,
            t.cost_usd
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "—".into()),
            t.session_id.as_deref().unwrap_or("—"),
            t.worktree_branch.as_deref().unwrap_or("—"),
            t.terminals.len(),
        ));
        if let Some(err) = &t.error {
            md.push_str(&format!("| | error | `{err}` | | | | |\n"));
        }
        if let Some(wd) = &t.work_dir {
            md.push_str(&format!("| | work_dir | `{}` | | | | |\n", wd.display()));
        }
    }
    md.push_str("\n## Paths\n\n");
    md.push_str(&format!("- run dir: `{}`\n", state.run_dir.display()));
    md.push_str(&format!("- events: `{}`\n", state.events_path().display()));

    std::fs::write(&md_path, md)?;
    Ok(())
}

pub fn print_report_md(run_dir: &Path) -> Result<()> {
    let md = run_dir.join("report.md");
    if md.exists() {
        print!("{}", std::fs::read_to_string(md)?);
    } else {
        let state = RunState::load(run_dir)?;
        write_reports(&state)?;
        print!("{}", std::fs::read_to_string(run_dir.join("report.md"))?);
    }
    Ok(())
}
