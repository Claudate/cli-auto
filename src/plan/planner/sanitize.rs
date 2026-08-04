//! Confirm-screen sanitize deps — CcoSplit SoT first (P3-4).
//!
//! [INPUT]: Config · job_id
//! [OUTPUT]: SanitizeDepsResult (removed count + desk view)
//! [POS]: planner — 从 view 抽出，禁止再堆 view.rs
//! [PROTOCOL]: 优先改 cco_split depends；无 SoT 时回落 PlanIR digest 规则

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Serialize;

use crate::config::Config;
use crate::plan::planner::job::{append_log, job_dir, PlanJob, PlanJobStatus};
use crate::plan::planner::view::{job_view, load_proposed, write_proposed, PlanJobView};
use crate::plan::PlanIR;

/// Drop unmotivated depends_on edges (confirm-screen action).
pub fn sanitize_proposed_deps(config: &Config, job_id: &str) -> Result<SanitizeDepsResult> {
    let mut job = PlanJob::load(config, job_id)?;
    if !matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        bail!(
            "计划任务状态为 {}，仅 planned/confirmed 可清理依赖",
            job.status.as_str()
        );
    }

    // P3-4: operate on CcoSplit SoT when present.
    if let Ok(Some(mut doc)) = crate::state::cco_split_store::load_cco_split(config, job_id) {
        let removed = crate::plan::sanitize_cco_split_deps(&mut doc);
        doc.updated_at = Utc::now().to_rfc3339();
        crate::state::cco_split_store::try_save_cco_split(config, &doc);
        // Export PlanIR snapshot for legacy consumers.
        let mut ir = crate::plan::to_plan_ir(&doc, &job.provider, &job.exec_mode);
        crate::domain::worker::apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
        let _ = crate::plan::soften_plan_for_accept(&mut ir);
        ir.validate()?;
        write_proposed(config, job_id, &ir)?;
        return finish_sanitize(config, &mut job, &ir, removed);
    }

    // Legacy: PlanIR-only jobs (pre-SoT).
    let mut ir = load_proposed(config, job_id).or_else(|_| {
        let path = job_dir(config, job_id).join("plan.resolved.json");
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let ir: PlanIR =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        Ok::<PlanIR, anyhow::Error>(ir)
    })?;

    let before: usize = ir.tasks.iter().map(|t| t.depends_on.len()).sum();
    super::digest::sanitize_task_deps(&mut ir.tasks);
    let after: usize = ir.tasks.iter().map(|t| t.depends_on.len()).sum();
    let removed = before.saturating_sub(after);

    ir.validate()?;
    write_proposed(config, job_id, &ir)?;
    finish_sanitize(config, &mut job, &ir, removed)
}

fn finish_sanitize(
    config: &Config,
    job: &mut PlanJob,
    ir: &PlanIR,
    removed: usize,
) -> Result<SanitizeDepsResult> {
    job.plan_name = Some(ir.name.clone());
    job.task_count = Some(ir.tasks.len());
    job.max_parallel = Some(ir.max_parallel);
    job.critic_summary = Some(if removed > 0 {
        format!("拆分校对：手动清理 · 去掉 {removed} 条可疑依赖")
    } else {
        "拆分校对：手动清理 · 未发现可疑依赖".into()
    });
    job.critic_edges_removed = Some(removed);
    job.critic_titles_rewritten = job.critic_titles_rewritten.or(Some(0));
    job.critic_prompts_tagged = job.critic_prompts_tagged.or(Some(0));
    job.updated_at = Utc::now();
    if matches!(job.status, PlanJobStatus::Confirmed) {
        job.status = PlanJobStatus::Planned;
        job.run_id = None;
    }
    job.save(config)?;
    append_log(
        config,
        &job.job_id,
        &format!("sanitize deps: removed {removed} edge(s) (cco-split SoT preferred)"),
    );
    let view = job_view(config, job, 48_000)?;
    Ok(SanitizeDepsResult { removed, view })
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizeDepsResult {
    /// Number of depends_on edges dropped.
    pub removed: usize,
    pub view: PlanJobView,
}
