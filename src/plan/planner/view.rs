//! Plan job views, proposed plan IO, confirm helpers.
//!
//! [INPUT]: PlanJob · PlanIR · Config
//! [OUTPUT]: PlanJobView · load_proposed · update_proposed_task · mark_confirmed · load_proposed_for_exec
//! [POS]: planner 子模块；桌面/CLI 确认屏消费
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Serialize;

use crate::config::Config;
use crate::graph::topo_layers;
use crate::plan::{PlanIR, TaskIR};
use crate::runtime::log_events::{self, LogEvent};

use super::job::{append_log, apply_worker_defaults, job_dir, read_log_tail, PlanJob, PlanJobStatus};
use super::llm::read_planner_cost;

#[derive(Debug, Clone, Serialize)]
pub struct PlanTaskView {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub group: Option<String>,
    /// Full worker prompt (confirm screen needs complete text).
    pub prompt: String,
    /// Short one-line summary for lists / tooltips.
    pub prompt_preview: String,
    /// Optional tasks are user-selectable on the confirm screen.
    pub optional: bool,
    /// Whether this task will run (optional defaults false until checked).
    pub include: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanJobView {
    pub job_id: String,
    pub status: String,
    pub project: String,
    pub plan_path: String,
    pub plan_mode: String,
    pub provider: String,
    pub exec_mode: String,
    pub error: Option<String>,
    pub run_id: Option<String>,
    pub plan_name: Option<String>,
    pub task_count: Option<usize>,
    pub max_parallel: Option<usize>,
    pub adapter: Option<String>,
    /// Planner LLM spend (USD); None for parse/fake or unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_cost_usd: Option<f64>,
    pub layers: Vec<Vec<String>>,
    pub tasks: Vec<PlanTaskView>,
    pub planner_log_tail: String,
    /// Structured planner log events for LogConsole (P1-3).
    #[serde(default)]
    pub planner_log_events: Vec<LogEvent>,
    pub created_at: String,
    pub updated_at: String,
}

fn task_view(t: &TaskIR) -> PlanTaskView {
    let preview: String = t.prompt.chars().take(120).collect();
    let preview = if t.prompt.chars().count() > 120 {
        format!("{preview}…")
    } else {
        preview
    };
    PlanTaskView {
        id: t.id.clone(),
        title: t.title.clone(),
        depends_on: t.depends_on.clone(),
        group: t.group.clone(),
        prompt: t.prompt.clone(),
        prompt_preview: preview,
        optional: t.optional,
        include: if t.optional { t.include } else { true },
    }
}

pub fn job_view(config: &Config, job: &PlanJob, log_max: usize) -> Result<PlanJobView> {
    let mut layers = Vec::new();
    let mut tasks = Vec::new();
    if matches!(job.status, PlanJobStatus::Planned | PlanJobStatus::Confirmed) {
        let ir_loaded = load_proposed(config, &job.job_id).or_else(|_| {
            let path = job_dir(config, &job.job_id).join("plan.resolved.json");
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let ir: PlanIR = serde_json::from_str(&text)
                .with_context(|| format!("parse {}", path.display()))?;
            Ok::<PlanIR, anyhow::Error>(ir)
        });
        if let Ok(ir) = ir_loaded {
            layers = topo_layers(&ir);
            tasks = ir.tasks.iter().map(task_view).collect();
        }
    }
    // Single read: structured events + compact raw for IPC (P1-1 / P1-3)
    // compact_text_tail floors to UTF-8 char boundaries (CJK-safe; same class of
    // bug as services::compact_log_tail_for_live / CCO-2026-07-18 crashes).
    let full = read_log_tail(config, &job.job_id, log_max.max(48_000));
    let planner_log_events = log_events::parse_worker_logs(&full, "", 200);
    let planner_log_tail = if full.len() > 8_000 {
        log_events::compact_text_tail(&full, 6_000, "… (compact)\n")
    } else {
        full
    };
    Ok(PlanJobView {
        job_id: job.job_id.clone(),
        status: job.status.as_str().to_string(),
        project: job.project.display().to_string(),
        plan_path: job.plan_path.display().to_string(),
        plan_mode: job.plan_mode.clone(),
        provider: job.provider.clone(),
        exec_mode: job.exec_mode.clone(),
        error: job.error.clone(),
        run_id: job.run_id.clone(),
        plan_name: job.plan_name.clone(),
        task_count: job.task_count,
        max_parallel: job.max_parallel,
        adapter: job.adapter.clone(),
        planner_cost_usd: job.planner_cost_usd,
        layers,
        tasks,
        planner_log_tail,
        planner_log_events,
        created_at: job.created_at.to_rfc3339(),
        updated_at: job.updated_at.to_rfc3339(),
    })
}

pub fn load_proposed(config: &Config, job_id: &str) -> Result<PlanIR> {
    let path = job_dir(config, job_id).join("plan.proposed.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("missing plan.proposed.json for {job_id}"))?;
    let ir: PlanIR = serde_json::from_str(&text)?;
    ir.validate()?;
    Ok(ir)
}

pub(super) fn write_proposed(config: &Config, job_id: &str, ir: &PlanIR) -> Result<()> {
    let path = job_dir(config, job_id).join("plan.proposed.json");
    std::fs::write(&path, serde_json::to_string_pretty(ir)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Patch one task in the proposed plan (title/prompt/include) while still planned/confirmed.
pub fn update_proposed_task(
    config: &Config,
    job_id: &str,
    task_id: &str,
    title: Option<String>,
    prompt: Option<String>,
    include: Option<bool>,
) -> Result<PlanJobView> {
    let mut job = PlanJob::load(config, job_id)?;
    if !matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        bail!(
            "计划任务状态为 {}，仅 planned/confirmed 可编辑",
            job.status.as_str()
        );
    }
    let mut ir = load_proposed(config, job_id).or_else(|_| {
        let path = job_dir(config, job_id).join("plan.resolved.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let ir: PlanIR = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok::<PlanIR, anyhow::Error>(ir)
    })?;
    let Some(task) = ir.tasks.iter_mut().find(|t| t.id == task_id) else {
        bail!("任务不存在: {task_id}");
    };
    if let Some(t) = title {
        let t = t.trim().to_string();
        if t.is_empty() {
            bail!("标题不能为空");
        }
        // Keep optional marker visible if the task is optional.
        task.title = if task.optional {
            crate::plan::normalize_optional_title(&t, true)
        } else {
            t
        };
    }
    if let Some(p) = prompt {
        let p = p.trim_end().to_string();
        if p.trim().is_empty() {
            bail!("任务说明不能为空");
        }
        task.prompt = p;
    }
    if let Some(inc) = include {
        if !task.optional {
            if !inc {
                bail!("必选任务不能取消勾选");
            }
            task.include = true;
        } else {
            task.include = inc;
        }
    }
    ir.validate()?;
    write_proposed(config, job_id, &ir)?;
    job.plan_name = Some(ir.name.clone());
    job.task_count = Some(ir.tasks.len());
    job.max_parallel = Some(ir.max_parallel);
    job.updated_at = Utc::now();
    // Editing a confirmed job returns it to planned so re-run uses the patch.
    if matches!(job.status, PlanJobStatus::Confirmed) {
        job.status = PlanJobStatus::Planned;
        job.run_id = None;
    }
    job.save(config)?;
    append_log(
        config,
        job_id,
        &format!(
            "updated task {task_id} (title/prompt/include={})",
            ir.tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| t.include.to_string())
                .unwrap_or_else(|| "?".into())
        ),
    );
    job_view(config, &job, 48_000)
}

/// Mark job confirmed after exec run was spawned (called from services).
pub fn mark_confirmed(config: &Config, job_id: &str, run_id: &str, ir: &PlanIR) -> Result<()> {
    let mut job = PlanJob::load(config, job_id)?;
    if !matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        bail!(
            "计划任务状态为 {}，无法绑定 run",
            job.status.as_str()
        );
    }
    job.status = PlanJobStatus::Confirmed;
    job.run_id = Some(run_id.to_string());
    job.updated_at = Utc::now();
    job.save(config)?;
    append_log(config, job_id, &format!("confirmed run_id={run_id}"));
    let _ = std::fs::write(
        job_dir(config, job_id).join("plan.resolved.json"),
        serde_json::to_string_pretty(ir)?,
    );
    // Attach planner cost to the exec run dir for report / live split (P1-5).
    if let Some(c) = job.planner_cost_usd.or_else(|| read_planner_cost(config, job_id)) {
        if let Ok(run_dir) = crate::state::resolve_run_dir(&config.runs_dir(), Some(run_id)) {
            let _ = std::fs::write(
                run_dir.join("planner_cost.json"),
                serde_json::to_string_pretty(&serde_json::json!({
                    "cost_usd": c,
                    "plan_job_id": job_id,
                }))?,
            );
        }
    }
    Ok(())
}

/// Planner cost recorded on an exec run (if plan-job linked).
pub fn planner_cost_for_run(run_dir: &Path) -> Option<f64> {
    let path = run_dir.join("planner_cost.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("cost_usd").and_then(|x| x.as_f64())
}

/// Load proposed plan and apply job's provider/mode defaults.
pub fn load_proposed_for_exec(config: &Config, job_id: &str) -> Result<(PlanJob, PlanIR)> {
    let job = PlanJob::load(config, job_id)?;
    // planned：首次确认；confirmed：允许用同一份拆分结果再次开跑（不必重拆）
    if !matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        bail!(
            "计划任务状态为 {}，只有「待确认/已确认」才能开始运行",
            job.status.as_str()
        );
    }
    let mut ir = load_proposed(config, job_id).or_else(|_| {
        // 回落 resolved（确认后写的冻结图）
        let path = job_dir(config, job_id).join("plan.resolved.json");
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!("missing plan.proposed/resolved for {job_id}")
        })?;
        let ir: PlanIR = serde_json::from_str(&text)
            .with_context(|| format!("parse plan.resolved.json for {job_id}"))?;
        Ok::<PlanIR, anyhow::Error>(ir)
    })?;
    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
    // Drop unselected optional tasks before validate / spawn.
    let before = ir.tasks.len();
    ir = crate::plan::materialize_selected_tasks(ir)?;
    let skipped = before.saturating_sub(ir.tasks.len());
    if skipped > 0 {
        append_log(
            config,
            job_id,
            &format!("confirm_start → skipped {skipped} unselected optional task(s)"),
        );
    }
    append_log(
        config,
        job_id,
        &format!("confirm_start → spawning run with {} tasks", ir.tasks.len()),
    );
    Ok((job, ir))
}
