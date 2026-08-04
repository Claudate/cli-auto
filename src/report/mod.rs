//! Markdown + JSON reports.
//!
//! [INPUT]: RunState · optional run_dir/planner_cost.json · plan.resolved + inspect DTO
//! [OUTPUT]: report.md · report.json · print_report_md
//!   骨架：摘要 · **对照计划**（P2-1 含原计划验收清单副栏）· 步骤结果 · 花费与用时 · 后续 · 备注
//!   （人话 H1 · plan-compare fallback 不伪造 PASS · 备注下沉 run_id）
//! [POS]: run 结束后产物（不替代 handoff 事中更新）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/report/CLAUDE.md

mod fallback;

pub use fallback::{
    build_plan_compare, fill_plan_compare, follow_up_lines, format_elapsed_human,
    load_plan_resolved, render_plan_compare_md, PlanCompareKind, PlanCompareSection,
};

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::plan::planner::planner_cost_for_run;
use crate::runtime::handoff::Handoff;
use crate::runtime::provider::TaskStatus;
use crate::state::{RunState, RunStatus, TaskState};

/// Plan short name from `plan_path` file name (stem preferred; empty → 未命名计划).
pub fn plan_short_name(plan_path: &Path) -> String {
    plan_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            plan_path
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "未命名计划".into())
}

/// Human-readable run status for report summary line.
fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Init => "初始化",
        RunStatus::Validated => "已校验",
        RunStatus::Running => "进行中",
        RunStatus::Paused => "已暂停",
        RunStatus::Completed => "已完成",
        RunStatus::Failed => "失败",
        RunStatus::Aborted => "已中止",
    }
}

/// Count tasks in Done status.
fn done_task_count<'a, I>(tasks: I) -> usize
where
    I: IntoIterator<Item = &'a TaskState>,
{
    tasks
        .into_iter()
        .filter(|t| t.status == TaskStatus::Done)
        .count()
}

/// H1 + headline: `本轮结果 · 《计划短名》`
pub fn report_headline(plan_path: &Path) -> String {
    format!("本轮结果 · 《{}》", plan_short_name(plan_path))
}

/// One-line human summary: status + done/total tasks.
pub fn report_summary_line(state: &RunState) -> String {
    let total = state.tasks.len();
    let done = done_task_count(state.tasks.values());
    format!(
        "本轮状态：**{}** · 完成 {}/{} 项任务",
        status_label(state.status),
        done,
        total
    )
}

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
    let headline = report_headline(&state.plan_path);
    let summary_line = report_summary_line(state);
    let done = done_task_count(state.tasks.values());
    let total_tasks = state.tasks.len();
    let elapsed = format_elapsed_human(state.started_at, state.finished_at);

    // P0-3: always build plan-compare (fallback when no inspect / no VERDICT).
    let plan_compare = build_plan_compare(state);
    let follow_ups = follow_up_lines(&plan_compare);

    // P2-1: top-level verification mirror of plan_compare.verification (live DTO shape).
    let verification = plan_compare.verification.clone();

    // Old fields kept; `headline` + `plan_compare` additive (P0-2 / P0-3).
    // Clone plan_compare so md render + Notes can still borrow it after json! moves a copy.
    let summary = serde_json::json!({
        "run_id": state.run_id,
        "status": state.status,
        "headline": headline,
        "project_root": state.project_root,
        "plan_path": state.plan_path,
        "adapter": state.adapter,
        "started_at": state.started_at,
        "finished_at": state.finished_at,
        "elapsed_human": elapsed,
        "planner_cost_usd": planner_cost,
        "exec_cost_usd": if has_exec { Some(exec_cost) } else { None },
        "total_cost_usd": total_cost,
        "tasks_done": done,
        "tasks_total": total_tasks,
        "by_provider": by_provider,
        "plan_compare": &plan_compare,
        "verification": verification,
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
    // --- 摘要 (P0-2) ---
    md.push_str(&format!("# {headline}\n\n"));
    md.push_str(&format!("{summary_line}\n"));

    // --- 对照计划 (P0-3 · always present; never invent PASS; P2-1 plan checklist sidebar) ---
    md.push_str("\n## 对照计划\n\n");
    md.push_str(&render_plan_compare_md(&plan_compare));

    // --- 步骤结果 (tasks + by-provider) ---
    md.push_str("\n## 步骤结果\n\n");
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

    // P1-8: per-provider under 步骤结果 (compat heading kept as subsection label in body).
    md.push_str("\n### By provider\n\n");
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

    // --- 花费与用时 (was ## Budget; always emit so skeleton is complete) ---
    md.push_str("## 花费与用时\n\n");
    md.push_str(&format!("- **用时**: {elapsed}\n"));
    md.push_str(&format!(
        "- **规划 (planner)**: {}\n",
        planner_cost
            .map(|c| format!("${c:.4}"))
            .unwrap_or_else(|| "未汇总".into())
    ));
    md.push_str(&format!(
        "- **执行 (workers)**: {}\n",
        if has_exec {
            format!("${exec_cost:.4}")
        } else {
            "未汇总".into()
        }
    ));
    if let Some(t) = total_cost {
        md.push_str(&format!("- **合计**: ${t:.4}\n"));
    } else if !has_plan && !has_exec {
        md.push_str("- **合计**: 费用未汇总\n");
    }
    md.push('\n');

    // --- 后续 ---
    md.push_str("## 后续\n\n");
    for line in &follow_ups {
        md.push_str(&format!("- {line}\n"));
    }

    // --- 备注 (P0-2 metadata sink + P0-3 fallback reason) ---
    md.push_str("\n## 备注\n\n");
    if let Some(reason) = &plan_compare.fallback_reason {
        md.push_str(&format!("- **fallback**: {reason}\n"));
    }
    if plan_compare.is_fallback {
        md.push_str(&format!(
            "- **plan_compare**: 占位（kind={:?}）— 未伪造通过\n",
            plan_compare.kind
        ));
    } else {
        md.push_str(&format!(
            "- **plan_compare**: 实检（kind={:?} · blocking={} · residual={}）\n",
            plan_compare.kind, plan_compare.blocking_count, plan_compare.residual_count
        ));
    }
    md.push_str(&format!("- **run_id**: `{}`\n", state.run_id));
    md.push_str(&format!("- **status**: {:?}\n", state.status));
    md.push_str(&format!("- **adapter**: {}\n", state.adapter));
    md.push_str(&format!(
        "- **project**: `{}`\n",
        state.project_root.display()
    ));
    md.push_str(&format!("- **plan**: `{}`\n", state.plan_path.display()));
    md.push_str(&format!(
        "- **started**: {}\n",
        state.started_at.to_rfc3339()
    ));
    if let Some(f) = state.finished_at {
        md.push_str(&format!("- **finished**: {}\n", f.to_rfc3339()));
    }
    md.push_str(&format!("- **run dir**: `{}`\n", state.run_dir.display()));
    md.push_str(&format!(
        "- **events**: `{}`\n",
        state.events_path().display()
    ));
    md.push_str(&format!(
        "- handoff.md: [`{}`]({})\n",
        handoff.md_rel, handoff.md_rel
    ));
    md.push_str(&format!(
        "- handoff.json: [`{}`]({})\n",
        handoff.json_rel, handoff.json_rel
    ));
    if !handoff.exists_md && !handoff.exists_json {
        md.push_str(
            "- handoff note: not written yet (mid-run ledger is host-owned; see runtime/handoff)\n",
        );
    }
    md.push_str(&format!("- **handoff.md (abs)**: `{}`\n", handoff.md));
    md.push_str(&format!("- **handoff.json (abs)**: `{}`\n", handoff.json));

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
#[path = "write_tests.rs"]
mod write_tests;
