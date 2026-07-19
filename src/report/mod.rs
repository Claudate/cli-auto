//! Markdown + JSON reports.
//!
//! [INPUT]: RunState · optional run_dir/planner_cost.json
//! [OUTPUT]: report.md · report.json · print_report_md（规划/执行预算分栏 · per-provider · handoff 路径）
//! [POS]: run 结束后产物（不替代 handoff 事中更新）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/report/CLAUDE.md

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::plan::planner::planner_cost_for_run;
use crate::runtime::handoff::Handoff;
use crate::runtime::provider::TaskStatus;
use crate::state::{RunState, TaskState};

/// Per-provider rollup for report/status (P1-8).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ProviderSummary {
    pub provider: String,
    pub tasks: usize,
    pub pending: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub other: usize,
    /// Sum of task cost_usd when present; None if no task reported cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Aggregate tasks by provider (sorted by provider name).
pub fn summarize_providers<'a, I>(tasks: I) -> Vec<ProviderSummary>
where
    I: IntoIterator<Item = &'a TaskState>,
{
    let mut map: BTreeMap<String, ProviderSummary> = BTreeMap::new();
    for t in tasks {
        let e = map
            .entry(t.provider.clone())
            .or_insert_with(|| ProviderSummary {
                provider: t.provider.clone(),
                ..Default::default()
            });
        e.tasks += 1;
        match t.status {
            TaskStatus::Pending | TaskStatus::Queued => e.pending += 1,
            TaskStatus::Starting | TaskStatus::Running => e.running += 1,
            TaskStatus::Done => e.done += 1,
            TaskStatus::Failed | TaskStatus::Timeout => e.failed += 1,
            TaskStatus::Stopped | TaskStatus::Skipped => e.other += 1,
        }
        if let Some(c) = t.cost_usd {
            e.cost_usd = Some(e.cost_usd.unwrap_or(0.0) + c);
        }
    }
    map.into_values().collect()
}

/// Absolute + relative (to run_dir) handoff paths for report linking.
#[derive(Debug, Clone, Serialize)]
pub struct HandoffPaths {
    pub md: String,
    pub json: String,
    pub md_rel: String,
    pub json_rel: String,
    pub exists_md: bool,
    pub exists_json: bool,
}

pub fn handoff_paths(run_dir: &Path) -> HandoffPaths {
    let md = Handoff::path_md(run_dir);
    let json = Handoff::path_json(run_dir);
    HandoffPaths {
        md: md.display().to_string(),
        json: json.display().to_string(),
        md_rel: "handoff.md".into(),
        json_rel: "handoff.json".into(),
        exists_md: md.exists(),
        exists_json: json.exists(),
    }
}

pub fn write_reports(state: &RunState) -> Result<()> {
    let json_path = state.run_dir.join("report.json");
    let md_path = state.run_dir.join("report.md");

    // Worker / exec cost (business tasks only).
    let mut exec_cost = 0.0;
    let mut has_exec = false;
    for t in state.tasks.values() {
        if let Some(c) = t.cost_usd {
            exec_cost += c;
            has_exec = true;
        }
    }
    // Planner cost (Mode B plan job), stored beside run when confirmed.
    let planner_cost = planner_cost_for_run(&state.run_dir);
    let has_plan = planner_cost.is_some();
    let total_cost = match (has_plan, has_exec) {
        (true, true) => Some(planner_cost.unwrap_or(0.0) + exec_cost),
        (true, false) => planner_cost,
        (false, true) => Some(exec_cost),
        (false, false) => None,
    };

    let by_provider = summarize_providers(state.tasks.values());
    let handoff = handoff_paths(&state.run_dir);

    let summary = serde_json::json!({
        "run_id": state.run_id,
        "status": state.status,
        "project_root": state.project_root,
        "plan_path": state.plan_path,
        "adapter": state.adapter,
        "started_at": state.started_at,
        "finished_at": state.finished_at,
        "planner_cost_usd": planner_cost,
        "exec_cost_usd": if has_exec { Some(exec_cost) } else { None },
        "total_cost_usd": total_cost,
        "by_provider": by_provider,
        "handoff": {
            "md": handoff.md,
            "json": handoff.json,
            "md_rel": handoff.md_rel,
            "json_rel": handoff.json_rel,
            "exists_md": handoff.exists_md,
            "exists_json": handoff.exists_json,
        },
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
    // P1-5: budget columns — 规划 vs 执行
    if has_plan || has_exec {
        md.push_str("\n## Budget\n\n");
        md.push_str(&format!(
            "- **规划 (planner)**: {}\n",
            planner_cost
                .map(|c| format!("${c:.4}"))
                .unwrap_or_else(|| "—".into())
        ));
        md.push_str(&format!(
            "- **执行 (workers)**: {}\n",
            if has_exec {
                format!("${exec_cost:.4}")
            } else {
                "—".into()
            }
        ));
        if let Some(t) = total_cost {
            md.push_str(&format!("- **合计**: ${t:.4}\n"));
        }
        md.push('\n');
    }

    // P1-8: per-provider columns (task counts / cost; not a perfect budget align).
    md.push_str("\n## By provider\n\n");
    md.push_str("| provider | tasks | pending | running | done | failed | other | cost |\n");
    md.push_str("|----------|------:|--------:|--------:|-----:|-------:|------:|------|\n");
    if by_provider.is_empty() {
        md.push_str("| — | 0 | 0 | 0 | 0 | 0 | 0 | — |\n");
    } else {
        for p in &by_provider {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                p.provider,
                p.tasks,
                p.pending,
                p.running,
                p.done,
                p.failed,
                p.other,
                p.cost_usd
                    .map(|c| format!("${c:.4}"))
                    .unwrap_or_else(|| "—".into()),
            ));
        }
    }
    md.push('\n');

    md.push_str("## Tasks\n\n");
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
    // P1-8: link handoff (host mid-run ledger; report is terminal snapshot only).
    md.push_str(&format!(
        "- handoff.md: [`{}`]({}) · abs `{}`\n",
        handoff.md_rel, handoff.md_rel, handoff.md
    ));
    md.push_str(&format!(
        "- handoff.json: [`{}`]({}) · abs `{}`\n",
        handoff.json_rel, handoff.json_rel, handoff.json
    ));
    if !handoff.exists_md && !handoff.exists_json {
        md.push_str("- handoff note: not written yet (mid-run ledger is host-owned; see runtime/handoff)\n");
    }

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

/// Format a compact multi-line status block for CLI `cco status` (P1-8).
pub fn format_status_by_provider(tasks: &std::collections::HashMap<String, TaskState>) -> String {
    let rows = summarize_providers(tasks.values());
    let mut out = String::from("by_provider:\n");
    if rows.is_empty() {
        out.push_str("  (none)\n");
        return out;
    }
    for p in rows {
        let cost = p
            .cost_usd
            .map(|c| format!("${c:.4}"))
            .unwrap_or_else(|| "—".into());
        out.push_str(&format!(
            "  {}: tasks={} pending={} running={} done={} failed={} other={} cost={}\n",
            p.provider, p.tasks, p.pending, p.running, p.done, p.failed, p.other, cost
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::TaskStatus;
    use crate::state::{RunState, RunStatus, TaskState};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn task(provider: &str, status: TaskStatus, cost: Option<f64>) -> TaskState {
        let mut t = TaskState::pending(provider, "print");
        t.status = status;
        t.cost_usd = cost;
        t
    }

    #[test]
    fn summarize_providers_counts_and_cost() {
        let tasks = vec![
            task("claude", TaskStatus::Done, Some(0.10)),
            task("claude", TaskStatus::Running, None),
            task("claude", TaskStatus::Failed, Some(0.05)),
            task("codex", TaskStatus::Done, Some(1.25)),
            task("codex", TaskStatus::Pending, None),
            task("fake", TaskStatus::Skipped, None),
        ];
        let rows = summarize_providers(tasks.iter());
        assert_eq!(rows.len(), 3);
        let claude = rows.iter().find(|r| r.provider == "claude").unwrap();
        assert_eq!(claude.tasks, 3);
        assert_eq!(claude.running, 1);
        assert_eq!(claude.done, 1);
        assert_eq!(claude.failed, 1);
        assert!((claude.cost_usd.unwrap() - 0.15).abs() < 1e-9);
        let codex = rows.iter().find(|r| r.provider == "codex").unwrap();
        assert_eq!(codex.tasks, 2);
        assert_eq!(codex.done, 1);
        assert_eq!(codex.pending, 1);
        assert!((codex.cost_usd.unwrap() - 1.25).abs() < 1e-9);
        let fake = rows.iter().find(|r| r.provider == "fake").unwrap();
        assert_eq!(fake.other, 1);
        assert!(fake.cost_usd.is_none());
    }

    #[test]
    fn write_reports_includes_by_provider_and_handoff_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = tmp.path().join("run-p1-8");
        std::fs::create_dir_all(run_dir.join("tasks")).unwrap();
        // Pretend mid-run handoff already exists (host ledger).
        std::fs::write(run_dir.join("handoff.md"), "# handoff\n").unwrap();
        std::fs::write(run_dir.join("handoff.json"), r#"{"schema":"cco-handoff/v1"}"#).unwrap();

        let mut tasks = HashMap::new();
        tasks.insert("a".into(), task("claude", TaskStatus::Done, Some(0.2)));
        tasks.insert("b".into(), task("codex", TaskStatus::Failed, Some(0.01)));
        tasks.insert("c".into(), task("claude", TaskStatus::Running, None));

        let state = RunState {
            schema: "cco-run/v1".into(),
            run_id: "run-p1-8".into(),
            project_root: tmp.path().join("proj"),
            plan_path: tmp.path().join("plan.yaml"),
            adapter: "cco-plan/v1".into(),
            started_at: Utc::now(),
            finished_at: None,
            status: RunStatus::Running,
            tasks,
            run_dir: run_dir.clone(),
        };
        write_reports(&state).unwrap();

        let md = std::fs::read_to_string(run_dir.join("report.md")).unwrap();
        assert!(md.contains("## By provider"), "missing by-provider section:\n{md}");
        assert!(md.contains("| claude |"), "missing claude row:\n{md}");
        assert!(md.contains("| codex |"), "missing codex row:\n{md}");
        assert!(md.contains("handoff.md"), "missing handoff.md link:\n{md}");
        assert!(md.contains("handoff.json"), "missing handoff.json link:\n{md}");

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
                .unwrap();
        let by = json["by_provider"].as_array().expect("by_provider array");
        assert_eq!(by.len(), 2);
        let claude = by.iter().find(|r| r["provider"] == "claude").unwrap();
        assert_eq!(claude["tasks"], 2);
        assert_eq!(claude["running"], 1);
        assert_eq!(claude["done"], 1);
        assert!((claude["cost_usd"].as_f64().unwrap() - 0.2).abs() < 1e-9);
        let codex = by.iter().find(|r| r["provider"] == "codex").unwrap();
        assert_eq!(codex["failed"], 1);
        assert_eq!(json["handoff"]["md_rel"], "handoff.md");
        assert_eq!(json["handoff"]["json_rel"], "handoff.json");
        assert_eq!(json["handoff"]["exists_md"], true);
        assert!(
            json["handoff"]["md"]
                .as_str()
                .unwrap()
                .ends_with("handoff.md")
        );

        let status_txt = format_status_by_provider(&state.tasks);
        assert!(status_txt.contains("claude:"));
        assert!(status_txt.contains("running=1"));
        assert!(status_txt.contains("codex:"));
    }

    #[test]
    fn handoff_paths_relative_and_abs() {
        let p = PathBuf::from("/tmp/fake-run");
        let h = handoff_paths(&p);
        assert_eq!(h.md_rel, "handoff.md");
        assert_eq!(h.json_rel, "handoff.json");
        assert!(h.md.ends_with("handoff.md"));
        assert!(h.json.ends_with("handoff.json"));
    }
}
