//! Plan job types, paths, start/get/latest lifecycle.
//!
//! [INPUT]: StartPlanJobRequest · Config
//! [OUTPUT]: PlanJob · start_plan_job · get_plan_job · latest_plan_job_for_project
//! [POS]: planner 子模块；Mode B job IO
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::topo_layers;
use crate::plan::{load_plan, PlanIR};
use crate::runtime::log_events;

use super::heuristic::{build_fake_plan, build_heuristic_ai_plan};
use super::llm::build_llm_plan;
use super::view::{
    apply_user_edits_to_ir, copy_user_edits, job_view, load_user_edits, write_proposed, PlanJobView,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanJobStatus {
    Planning,
    Planned,
    PlanFailed,
    Confirmed,
    Cancelled,
}

impl PlanJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Planned => "planned",
            Self::PlanFailed => "plan_failed",
            Self::Confirmed => "confirmed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanJob {
    pub job_id: String,
    pub status: PlanJobStatus,
    pub project: PathBuf,
    pub plan_path: PathBuf,
    /// parse | fake | ai
    pub plan_mode: String,
    /// worker provider after confirm
    pub provider: String,
    /// worker mode after confirm
    pub exec_mode: String,
    pub error: Option<String>,
    pub run_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Set when planned
    pub plan_name: Option<String>,
    pub task_count: Option<usize>,
    pub max_parallel: Option<usize>,
    pub adapter: Option<String>,
    /// Planner LLM spend (USD), separate from worker run cost (P1-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_cost_usd: Option<f64>,
    /// Document mode from digest: regression | greenfield | audit | mixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_mode: Option<String>,
    /// One-line critic report for confirm UI (deps cleaned / titles rewritten / notes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_summary: Option<String>,
    /// Structured critic counters (confirm strip chips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_edges_removed: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_titles_rewritten: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_prompts_tagged: Option<usize>,
    /// Free-form critic notes (e.g. missing inspect tail).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub critic_notes: Vec<String>,
    /// True when optional LLM second-pass critic was actually invoked this split.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_used: Option<bool>,
    /// USD spend of optional LLM critic (if reported by provider).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_cost_usd: Option<f64>,
    /// Wall time of optional LLM critic in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_ms: Option<u64>,
    /// W4: grain line forwarded to ModelSplitAgent (empty = omit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grain_hint: Option<String>,
    /// Clarify depth forwarded to ModelSplitAgent (none/soft1/soft2/full_opt; empty = omit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarify_depth: Option<String>,
    /// Free-text replan feedback for ModelSplitAgent (empty = omit). Not an open-run gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_notes: Option<String>,
    /// Per-split reasoning depth (`low`…`max`|`ultracode`); omit → config default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartPlanJobRequest {
    pub project: PathBuf,
    pub plan: PathBuf,
    /// parse | fake | ai  (default: parse)
    pub plan_mode: Option<String>,
    pub provider: Option<String>,
    pub mode: Option<String>,
    /// Scheduler concurrency chosen at split time (1–32). Defaults to config.
    pub max_parallel: Option<usize>,
    /// P2-2: previous plan job id whose `plan.user_edits.json` should be re-applied
    /// onto the fresh split (match by title; rebuild depends_on by dep titles).
    #[serde(default)]
    pub preserve_from_job_id: Option<String>,
    /// W4: optional grain line for ModelSplitAgent user prompt (偏粗/偏细); never forces fast.
    #[serde(default)]
    pub grain_hint: Option<String>,
    /// Optional clarify depth for ModelSplitAgent user prompt (none/soft1/soft2/full_opt).
    #[serde(default)]
    pub clarify_depth: Option<String>,
    /// Optional free-text "why re-split / what to change" for ModelSplitAgent.
    #[serde(default)]
    pub revision_notes: Option<String>,
    /// Optional per-split reasoning depth (`low`…`max`|`ultracode`); else config default.
    #[serde(default)]
    pub effort: Option<String>,
}

pub fn plan_jobs_dir(config: &Config) -> PathBuf {
    config.state_root.join("plan_jobs")
}

pub fn job_dir(config: &Config, job_id: &str) -> PathBuf {
    plan_jobs_dir(config).join(job_id)
}

fn new_job_id() -> String {
    // Distinct from run ids for clarity in UI/logs.
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let suffix: u32 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        % 900_000)
        + 100_000;
    format!("plan-{ts}-{suffix}")
}

impl PlanJob {
    pub fn save(&self, config: &Config) -> Result<()> {
        let dir = job_dir(config, &self.job_id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("job.json");
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("write {}", path.display()))?;
        // Dual-write index for UI/query (best-effort; JSON remains source of truth).
        crate::state::sqlite::try_upsert_plan_job(config, self);
        Ok(())
    }

    pub fn load(config: &Config, job_id: &str) -> Result<Self> {
        let path = job_dir(config, job_id).join("job.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("load plan job {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Planner job log (also used by split_agent / llm paths).
pub(crate) fn append_log(config: &Config, job_id: &str, line: &str) {
    let path = job_dir(config, job_id).join("planner.log");
    let _ = std::fs::create_dir_all(job_dir(config, job_id));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{}] {}", Utc::now().to_rfc3339(), line);
    }
}

pub(super) fn read_log_tail(config: &Config, job_id: &str, max_bytes: usize) -> String {
    let path = job_dir(config, job_id).join("planner.log");
    // 行边界 tail，避免半截行（与 worker log_events 同构）
    log_events::read_text_tail(&path, max_bytes).0
}

/// Create job and run planner synchronously (parse/fake are fast; ai heuristic too).
pub fn start_plan_job(config: &Config, req: StartPlanJobRequest) -> Result<PlanJobView> {
    if !req.project.is_dir() {
        bail!("项目路径不是目录: {}", req.project.display());
    }
    let plan_mode = req
        .plan_mode
        .as_deref()
        .unwrap_or("parse")
        .trim()
        .to_ascii_lowercase();
    if !matches!(
        plan_mode.as_str(),
        "parse" | "fake" | "ai" | "fast" | "heuristic" | "direct"
    ) {
        bail!("未知 plan_mode: {plan_mode}（支持 parse|fake|ai|fast|direct）");
    }
    let provider = req
        .provider
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.default.default_provider.clone());
    let exec_mode = req
        .mode
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.default.default_mode.clone());
    // Chosen at split time so the confirm UI and DAG reflect user intent.
    // direct = whole plan as one task → always serial (finish also stamps job.max_parallel onto IR).
    let max_parallel = if plan_mode == "direct" {
        1
    } else {
        req.max_parallel
            .unwrap_or(config.default.max_parallel)
            .clamp(1, 32)
    };

    let job_id = new_job_id();
    let project = req
        .project
        .canonicalize()
        .with_context(|| format!("canonicalize {}", req.project.display()))?;
    let now = Utc::now();
    let preserve_from = req
        .preserve_from_job_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let grain_hint = req
        .grain_hint
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let clarify_depth = req
        .clarify_depth
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    // Cap so a pasted essay cannot blow the split prompt.
    let revision_notes = req
        .revision_notes
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| {
            const MAX: usize = 2000;
            if s.chars().count() > MAX {
                let truncated: String = s.chars().take(MAX).collect();
                format!("{truncated}…")
            } else {
                s
            }
        });
    let effort = req
        .effort
        .as_ref()
        .and_then(|s| crate::config::normalize_effort(s));
    let mut job = PlanJob {
        job_id: job_id.clone(),
        status: PlanJobStatus::Planning,
        project: project.clone(),
        plan_path: req.plan.clone(),
        plan_mode: plan_mode.clone(),
        provider: provider.clone(),
        exec_mode: exec_mode.clone(),
        error: None,
        run_id: None,
        created_at: now,
        updated_at: now,
        plan_name: None,
        task_count: None,
        max_parallel: Some(max_parallel),
        adapter: None,
        planner_cost_usd: None,
        digest_mode: None,
        critic_summary: None,
        critic_edges_removed: None,
        critic_titles_rewritten: None,
        critic_prompts_tagged: None,
        critic_notes: vec![],
        critic_llm_used: None,
        critic_llm_cost_usd: None,
        critic_llm_ms: None,
        grain_hint,
        clarify_depth,
        revision_notes,
        effort,
    };
    std::fs::create_dir_all(job_dir(config, &job_id))?;
    // P2-2: copy user-edits sidecar before planner finishes so async path can apply it.
    if let Some(ref from) = preserve_from {
        if let Err(e) = copy_user_edits(config, from, &job_id) {
            append_log(
                config,
                &job_id,
                &format!("preserve user_edits from {from} failed: {e:#}"),
            );
        } else {
            append_log(config, &job_id, &format!("preserve user_edits from {from}"));
        }
    }
    job.save(config)?;
    append_log(
        config,
        &job_id,
        &format!(
            "plan job started mode={plan_mode} max_parallel={max_parallel} effort={} project={} plan={}",
            job.effort.as_deref().unwrap_or("(default)"),
            project.display(),
            req.plan.display()
        ),
    );
    // 同项目 + 同 plan_path 的旧 planning 标 cancelled（W2-4：不误杀其它计划的 planned/planning）
    supersede_planning_jobs(config, &project, &req.plan, &job_id);

    // `ai` may call Claude CLI (print) and take minutes — background + UI poll.
    // 若本机解析不到 claude bin，则同步跑启发式，避免 UI 空等异步轮询。
    let async_ai = plan_mode == "ai" && planner_should_try_llm(config, &provider);
    if async_ai {
        append_log(
            config,
            &job_id,
            "async planner: will invoke Claude CLI (print / stream-json)",
        );
        let cfg = config.clone();
        let jid = job_id.clone();
        std::thread::spawn(move || {
            let mut job = match PlanJob::load(&cfg, &jid) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(error = %e, "plan job load failed in worker");
                    return;
                }
            };
            finish_plan_job(&cfg, &mut job);
        });
        // Return immediately while planning
        let job = PlanJob::load(config, &job_id)?;
        return job_view(config, &job, 48_000);
    }

    if plan_mode == "ai" {
        append_log(
            config,
            &job_id,
            "claude CLI not resolvable in this process; running heuristic splitter synchronously",
        );
    }
    finish_plan_job(config, &mut job);
    let job = PlanJob::load(config, &job_id)?;
    job_view(config, &job, 48_000)
}

fn planner_should_try_llm(config: &Config, provider_name: &str) -> bool {
    if provider_name == "fake" {
        return false;
    }
    if std::env::var("CCO_PLANNER_HEURISTIC")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return false;
    }
    // Fixture / Messages-capable split agent can run without a local claude bin.
    if split_agent_can_run_without_cli() {
        return true;
    }
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let p = std::path::Path::new(&bin);
    p.is_file() || which::which(&bin).is_ok()
}

fn split_agent_can_run_without_cli() -> bool {
    for key in ["CCO_SPLIT_AGENT_JSON", "CCO_SPLIT_AGENT_FIXTURE"] {
        if std::env::var(key)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            return true;
        }
    }
    // Messages HTTP path when API key is present.
    crate::runtime::provider::sdk_http::resolve_api_key().is_some()
}

/// Mark other in-flight **planning** jobs for the same project **and same plan
/// document** as cancelled; kill planner PID (C6).
///
/// W2-4: supersede is per `plan_path`, not whole-project — re-splitting plan A
/// must not cancel plan B's still-planning job.
fn supersede_planning_jobs(config: &Config, project: &Path, plan_path: &Path, keep_job_id: &str) {
    let root = plan_jobs_dir(config);
    if !root.is_dir() {
        return;
    }
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let want_plan = crate::state::sqlite::plan_path_key(&plan_path.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let job_path = entry.path().join("job.json");
        if !job_path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&job_path) else {
            continue;
        };
        let Ok(mut other) = serde_json::from_str::<PlanJob>(&text) else {
            continue;
        };
        if other.job_id == keep_job_id {
            continue;
        }
        if !matches!(other.status, PlanJobStatus::Planning) {
            continue;
        }
        let jp = other
            .project
            .canonicalize()
            .unwrap_or_else(|_| other.project.clone());
        if jp != project {
            continue;
        }
        // Same plan document only (normalized key; allow suffix match like loaders).
        if !want_plan.is_empty() {
            let other_plan =
                crate::state::sqlite::plan_path_key(&other.plan_path.to_string_lossy());
            let same = other_plan == want_plan
                || (!other_plan.is_empty()
                    && (other_plan.ends_with(&want_plan) || want_plan.ends_with(&other_plan)));
            if !same {
                continue;
            }
        }
        // Kill leftover planner CLI so it cannot keep spinning after cancel.
        kill_planner_pid(config, &other.job_id);
        other.status = PlanJobStatus::Cancelled;
        other.error = Some("superseded by newer plan job for same plan_path".into());
        other.updated_at = Utc::now();
        let _ = other.save(config);
        append_log(
            config,
            &other.job_id,
            &format!(
                "cancelled: superseded by {keep_job_id} (same plan_path; planner pid kill attempted)"
            ),
        );
    }
}

fn job_marked_cancelled(config: &Config, job_id: &str) -> bool {
    PlanJob::load(config, job_id)
        .map(|j| matches!(j.status, PlanJobStatus::Cancelled))
        .unwrap_or(false)
}

/// Cancelled **or** reaped to plan_failed while a long critic/worker was in-flight.
fn job_finish_aborted(config: &Config, job_id: &str) -> bool {
    PlanJob::load(config, job_id)
        .map(|j| {
            matches!(
                j.status,
                PlanJobStatus::Cancelled | PlanJobStatus::PlanFailed
            )
        })
        .unwrap_or(false)
}

pub(super) fn finish_plan_job(config: &Config, job: &mut PlanJob) {
    let job_id = job.job_id.clone();
    if matches!(job.status, PlanJobStatus::Cancelled) || job_marked_cancelled(config, &job_id) {
        append_log(config, &job_id, "finish skipped: job cancelled/superseded");
        return;
    }
    match run_planner(config, job) {
        Ok(mut ir) => {
            // UI poll may falsely reap when CLI pid exits *before* we write proposed.
            // If we still hold a valid IR, recover from process-gone plan_failed and continue.
            if !finish_may_continue_with_ir(config, &job_id, job) {
                append_log(
                    config,
                    &job_id,
                    "finish aborted after planner: job cancelled/superseded/reaped",
                );
                return;
            }
            // Split-time concurrency wins over planner defaults / document values.
            if let Some(n) = job.max_parallel {
                ir.max_parallel = n.clamp(1, 32);
            }
            // Rule critic (no second LLM): sanitize deps + regression title/prompt hygiene.
            // Ensure digest_mode early so critic mode matches UI badge.
            if job.digest_mode.is_none() {
                if let Ok(abs) = crate::plan::resolve_plan_path(&job.project, &job.plan_path) {
                    if let Ok(text) = std::fs::read_to_string(&abs) {
                        let d = super::digest::build_plan_digest(&text);
                        job.digest_mode = Some(d.mode.as_str().to_string());
                    }
                }
            }
            let mode = job
                .digest_mode
                .as_deref()
                .map(super::digest::mode_from_str)
                .unwrap_or(super::digest::PlanModeKind::Greenfield);
            let mut critic = super::digest::critic_plan_tasks(&mut ir.tasks, mode);
            // Optional second-pass LLM critic (settings / CCO_PLANNER_CRITIC). Soft-fail.
            // Skipped for plan_mode=fast|heuristic|parse (local path must not call Claude).
            let llm_out = super::llm::run_optional_llm_critic(config, job, &mut ir, mode);
            if job_finish_aborted(config, &job_id) {
                append_log(
                    config,
                    &job_id,
                    "finish aborted after critic: job cancelled/superseded/reaped",
                );
                return;
            }
            critic.edges_removed += llm_out.report.edges_removed;
            critic.titles_rewritten += llm_out.report.titles_rewritten;
            critic.prompts_tagged += llm_out.report.prompts_tagged;
            for n in llm_out.report.notes {
                if !critic.notes.iter().any(|x| x == &n) {
                    critic.notes.push(n);
                }
            }
            let mut critic_line = critic.summary_line();
            if llm_out.used {
                if critic_line.contains("无需改动") {
                    critic_line = "拆分校对：规则 + 智能第二跳（无额外改动）".into();
                } else if !critic_line.contains("智能") {
                    critic_line = format!("{critic_line} · 含智能第二跳");
                }
                if let Some(ms) = llm_out.duration_ms {
                    if ms >= 1000 {
                        critic_line = format!("{critic_line} · {:.1}s", ms as f64 / 1000.0);
                    } else {
                        critic_line = format!("{critic_line} · {ms}ms");
                    }
                }
                if let Some(c) = llm_out.cost_usd {
                    critic_line = format!("{critic_line} · ${c:.3}");
                }
            }
            append_log(config, &job_id, &critic_line);
            job.critic_summary = Some(critic_line);
            job.critic_edges_removed = Some(critic.edges_removed);
            job.critic_titles_rewritten = Some(critic.titles_rewritten);
            job.critic_prompts_tagged = Some(critic.prompts_tagged);
            job.critic_notes = critic.notes.clone();
            job.critic_llm_used = Some(llm_out.used);
            job.critic_llm_cost_usd = llm_out.cost_usd;
            job.critic_llm_ms = llm_out.duration_ms;
            // Persist critic before any later disk refresh can wipe in-memory fields.
            job.updated_at = Utc::now();
            let _ = job.save(config);
            // System post-tasks（巡检 / git push）：不参与拆解，按设置注入可选尾任务
            crate::plan::inject_system_post_tasks(&mut ir, config);
            // P2-2: re-apply human confirm-screen patches (title match) before write.
            let edits = load_user_edits(config, &job_id);
            let (applied, removed_n) = apply_user_edits_to_ir(&mut ir, &edits);
            if applied > 0 || removed_n > 0 {
                append_log(
                    config,
                    &job_id,
                    &format!("preserve user edits: applied={applied} removed_tasks={removed_n}"),
                );
                // Ensure DAG still valid after preserve (strip unknown deps as last resort).
                if let Err(e) = ir.validate() {
                    let ids: std::collections::HashSet<_> =
                        ir.tasks.iter().map(|t| t.id.clone()).collect();
                    for t in ir.tasks.iter_mut() {
                        t.depends_on.retain(|d| ids.contains(d) && d != &t.id);
                    }
                    if let Err(e2) = ir.validate() {
                        append_log(
                            config,
                            &job_id,
                            &format!(
                                "preserve user edits left invalid plan: {e:#} → still {e2:#}; writing anyway"
                            ),
                        );
                    } else {
                        append_log(
                            config,
                            &job_id,
                            &format!("preserve user edits repaired after: {e:#}"),
                        );
                    }
                }
            }
            if !finish_may_continue_with_ir(config, &job_id, job) {
                append_log(
                    config,
                    &job_id,
                    "finish aborted before write: job cancelled/superseded/reaped",
                );
                return;
            }
            if let Err(e) = write_proposed(config, &job_id, &ir) {
                job.status = PlanJobStatus::PlanFailed;
                job.error = Some(e.to_string());
                job.updated_at = Utc::now();
                let _ = job.save(config);
                append_log(config, &job_id, &format!("write proposed failed: {e:#}"));
                return;
            }
            if !finish_may_continue_with_ir(config, &job_id, job) {
                // proposed already on disk — if false reap, still promote to planned below
                if !is_false_zombie_reap(config, &job_id) {
                    append_log(
                        config,
                        &job_id,
                        "finish aborted after write: job cancelled/superseded/reaped (proposed left on disk)",
                    );
                    return;
                }
                append_log(
                    config,
                    &job_id,
                    "finish continues after write despite false zombie reap (proposed on disk)",
                );
                let _ = refresh_job_from_disk(config, &job_id, job);
            }
            job.status = PlanJobStatus::Planned;
            job.plan_name = Some(ir.name.clone());
            job.task_count = Some(ir.tasks.len());
            job.max_parallel = Some(ir.max_parallel);
            job.adapter = Some(ir.adapter.clone());
            // Planner cost from LLM path (if any); parse/fake leave None.
            if job.planner_cost_usd.is_none() {
                job.planner_cost_usd = super::llm::read_planner_cost(config, &job_id);
            }
            // Ensure digest_mode is set even for heuristic/parse paths.
            if job.digest_mode.is_none() {
                if let Ok(abs) = crate::plan::resolve_plan_path(&job.project, &job.plan_path) {
                    if let Ok(text) = std::fs::read_to_string(&abs) {
                        let d = super::digest::build_plan_digest(&text);
                        job.digest_mode = Some(d.mode.as_str().to_string());
                    }
                }
            }
            job.error = None;
            job.updated_at = Utc::now();
            let _ = job.save(config);
            append_log(
                config,
                &job_id,
                &format!(
                    "planned ok name={} tasks={} max_parallel={} layers={} planner_cost={:?}",
                    ir.name,
                    ir.tasks.len(),
                    ir.max_parallel,
                    topo_layers(&ir).len(),
                    job.planner_cost_usd
                ),
            );
            for (i, layer) in topo_layers(&ir).iter().enumerate() {
                append_log(
                    config,
                    &job_id,
                    &format!("  wave {}: {}", i + 1, layer.join(", ")),
                );
            }
        }
        Err(e) => {
            job.status = PlanJobStatus::PlanFailed;
            job.error = Some(e.to_string());
            job.updated_at = Utc::now();
            let _ = job.save(config);
            append_log(config, &job_id, &format!("plan failed: {e:#}"));
        }
    }
}

pub fn get_plan_job(config: &Config, job_id: &str) -> Result<PlanJobView> {
    let mut job = PlanJob::load(config, job_id)?;
    if let Some(reaped) = try_reap_zombie_planning(config, &mut job) {
        job = reaped;
    }
    // Recover desks that already hit the false-reap race (split artifact ready, no proposed).
    if try_salvage_plan_job(config, &mut job) {
        // reloaded inside salvage
    }
    job_view(config, &job, 96_000)
}

/// Latest restorable plan job for a **plan document path** (not whole-project latest).
///
/// Prefer SQLite `plan_jobs` index (dual-write), then fall back to scanning plan_jobs dirs
/// so older installs without a fresh dual-write still work. Skips cancelled / failed
/// without salvageable artifact.
pub fn latest_plan_job_for_plan_path(
    config: &Config,
    project: &Path,
    plan_path: &str,
) -> Result<Option<PlanJobView>> {
    // 1) SQLite index (fast, written on every job.save)
    if let Ok(Some(job_id)) =
        crate::state::sqlite::latest_job_id_for_plan_path(config, project, plan_path)
    {
        match get_plan_job(config, &job_id) {
            Ok(view) => {
                let st = view.status.to_ascii_lowercase();
                if matches!(st.as_str(), "planning" | "planned" | "confirmed") {
                    return Ok(Some(view));
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    job_id = %job_id,
                    "latest_plan_job_for_plan_path: sqlite id load failed, scan disk"
                );
            }
        }
    }

    // 2) Disk scan (same project + plan_path match)
    let root = plan_jobs_dir(config);
    if !root.is_dir() {
        return Ok(None);
    }
    let project_c = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let want = crate::state::sqlite::plan_path_key(plan_path);
    if want.is_empty() {
        return Ok(None);
    }

    let mut best: Option<PlanJob> = None;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let job_path = entry.path().join("job.json");
        if !job_path.is_file() {
            continue;
        }
        let mut job: PlanJob = match std::fs::read_to_string(&job_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(j) => j,
            None => continue,
        };
        let jp = job
            .project
            .canonicalize()
            .unwrap_or_else(|_| job.project.clone());
        if jp != project_c {
            continue;
        }
        let job_plan = job.plan_path.to_string_lossy();
        let same_path = crate::state::sqlite::plan_path_key(&job_plan) == want
            || crate::state::sqlite::plan_path_key(&job_plan).ends_with(&want)
            || want.ends_with(&crate::state::sqlite::plan_path_key(&job_plan));
        if !same_path {
            continue;
        }
        match job.status {
            PlanJobStatus::Planning | PlanJobStatus::Planned | PlanJobStatus::Confirmed => {}
            PlanJobStatus::PlanFailed => {
                if !try_salvage_plan_job(config, &mut job) {
                    continue;
                }
            }
            PlanJobStatus::Cancelled => continue,
        }
        if matches!(job.status, PlanJobStatus::Planning) {
            if try_salvage_plan_job(config, &mut job) {
                // became planned
            } else if try_reap_zombie_planning(config, &mut job).is_some() {
                continue;
            }
        }
        if matches!(
            job.status,
            PlanJobStatus::Confirmed | PlanJobStatus::Planned
        ) {
            let dir = entry.path();
            if !dir.join("plan.proposed.json").is_file()
                && !dir.join("plan.resolved.json").is_file()
                && !dir.join("cco_split_agent.json").is_file()
            {
                continue;
            }
        }
        let replace = match &best {
            None => true,
            Some(b) => {
                // Quality first (multi-step AI ≫ direct 1-step), then status, then time.
                // Incomplete re-splits must not hide a better prior graph; confirmed
                // direct must not hide a planned multi-step AI desk.
                use crate::state::sqlite::{cmp_split_restore, split_graph_quality};
                let qj = split_graph_quality(
                    Some(job.plan_mode.as_str()),
                    job.adapter.as_deref(),
                    job.task_count.map(|n| n as u32),
                );
                let qb = split_graph_quality(
                    Some(b.plan_mode.as_str()),
                    b.adapter.as_deref(),
                    b.task_count.map(|n| n as u32),
                );
                let sj = match job.status {
                    PlanJobStatus::Confirmed => "confirmed",
                    PlanJobStatus::Planned => "planned",
                    PlanJobStatus::Planning => "planning",
                    _ => "other",
                };
                let sb = match b.status {
                    PlanJobStatus::Confirmed => "confirmed",
                    PlanJobStatus::Planned => "planned",
                    PlanJobStatus::Planning => "planning",
                    _ => "other",
                };
                let uj = job.updated_at.to_rfc3339();
                let ub = b.updated_at.to_rfc3339();
                cmp_split_restore(qj, sj, &uj, &job.job_id, qb, sb, &ub, &b.job_id).is_gt()
            }
        };
        if replace {
            best = Some(job);
        }
    }

    match best {
        Some(job) => Ok(Some(job_view(config, &job, 96_000)?)),
        None => Ok(None),
    }
}

/// Absolute wall clock since job created — cap so desk does not fake-spin for 10+ min (C6).
/// LLM worker may still use ~600s internally; we fail the job status sooner for UI.
const PLANNING_HARD_TIMEOUT_SECS: i64 = 5 * 60;
/// No planner.log growth / no alive pid after this → fail (UI stop spinning).
const PLANNING_DEAD_PID_GRACE_SECS: i64 = 30;
/// 卡住的 planning 超过此时长：latest 不恢复；reap 标 plan_failed。
const STALE_PLANNING_SECS: i64 = 6 * 60;

/// If `planning` but worker process is gone / timed out → `plan_failed` + log.
/// Returns updated job when reaped; `None` if still live or not planning.
///
/// **Do not** treat a normal CLI exit as zombie while finish still holds the IR:
/// ModelSplitAgent / LLM writes `.done` + `exit_code` then parent converts → write_proposed.
/// UI poll used to see dead pid and flip `plan_failed` in that window → desk "共 0 步".
pub(super) fn try_reap_zombie_planning(config: &Config, job: &mut PlanJob) -> Option<PlanJob> {
    if !matches!(job.status, PlanJobStatus::Planning) {
        return None;
    }
    let now = Utc::now();
    let age_created = now.signed_duration_since(job.created_at).num_seconds();
    let age_updated = now.signed_duration_since(job.updated_at).num_seconds();

    let dir = job_dir(config, &job.job_id);
    let meta_pid = read_planner_meta_pid(&dir);
    let pid_dead = match meta_pid {
        Some(pid) => !process_alive(pid),
        None => false, // no pid yet (not started) or fake — don't reap on pid alone
    };
    let log_stale = planner_log_stale(&dir, PLANNING_DEAD_PID_GRACE_SECS);

    // Worker finished successfully (or split artifact already on disk) → finish thread
    // is still materializing; never reap as zombie.
    if llm_work_finished_successfully(&dir) || has_recoverable_split_artifact(&dir) {
        if pid_dead && age_created > PLANNING_DEAD_PID_GRACE_SECS {
            append_log(
                config,
                &job.job_id,
                "skip zombie reap: worker finished or split artifact present (finish in progress)",
            );
        }
        return None;
    }

    let reason = if age_created > PLANNING_HARD_TIMEOUT_SECS {
        Some(format!(
            "planning hard timeout ({}s since create; planner worker did not finish)",
            age_created
        ))
    } else if meta_pid.is_some() && pid_dead && age_created > PLANNING_DEAD_PID_GRACE_SECS {
        Some(format!(
            "planner process gone (pid={:?}); job left in planning",
            meta_pid
        ))
    } else if log_stale && age_updated > STALE_PLANNING_SECS {
        Some(format!(
            "planning stale (no progress {}s; log quiet)",
            age_updated
        ))
    } else if age_updated > STALE_PLANNING_SECS && age_created > STALE_PLANNING_SECS {
        // No heartbeat ever updated job.json (old builds) — still reap
        Some(format!(
            "planning stale ({}s without status update)",
            age_updated
        ))
    } else {
        None
    };

    let Some(reason) = reason else {
        return None;
    };

    // C6: kill planner process so CLI cannot zombie after status flip.
    kill_planner_pid(config, &job.job_id);

    job.status = PlanJobStatus::PlanFailed;
    job.error = Some(reason.clone());
    job.updated_at = now;
    let _ = job.save(config);
    append_log(
        config,
        &job.job_id,
        &format!("reaped zombie planning → plan_failed: {reason}"),
    );
    Some(job.clone())
}

/// True when any `llm_work/tasks/*` wrote `.done` and (if present) `exit_code == 0`.
fn llm_work_finished_successfully(job_dir: &std::path::Path) -> bool {
    let tasks = job_dir.join("llm_work").join("tasks");
    let Ok(entries) = std::fs::read_dir(&tasks) else {
        return false;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let dir = entry.path();
        if !dir.join(".done").is_file() {
            continue;
        }
        let meta_path = dir.join("meta.json");
        let Ok(text) = std::fs::read_to_string(&meta_path) else {
            // .done without meta → treat as finished
            return true;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return true;
        };
        match v.get("exit_code").and_then(|c| c.as_i64()) {
            Some(0) => return true,
            Some(_) => continue, // failed worker exit
            None => return true, // .done, no exit field yet / legacy
        }
    }
    false
}

/// Split agent / proposed graph already on disk — finish or salvage can complete.
fn has_recoverable_split_artifact(job_dir: &std::path::Path) -> bool {
    if job_dir.join("plan.proposed.json").is_file() {
        return true;
    }
    let agent = job_dir.join("cco_split_agent.json");
    if !agent.is_file() {
        return false;
    }
    std::fs::read_to_string(&agent)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("tasks")
                .and_then(|t| t.as_array())
                .map(|a| !a.is_empty())
        })
        .unwrap_or(false)
}

fn error_is_false_zombie_reap(err: Option<&str>) -> bool {
    err.map(|e| e.contains("process gone") || e.contains("job left in planning"))
        .unwrap_or(false)
}

fn is_false_zombie_reap(config: &Config, job_id: &str) -> bool {
    PlanJob::load(config, job_id)
        .map(|j| {
            matches!(j.status, PlanJobStatus::PlanFailed)
                && error_is_false_zombie_reap(j.error.as_deref())
        })
        .unwrap_or(false)
}

fn refresh_job_from_disk(config: &Config, job_id: &str, job: &mut PlanJob) -> Result<()> {
    *job = PlanJob::load(config, job_id)?;
    Ok(())
}

/// After run_planner produced IR: continue unless truly cancelled / hard-failed.
/// Clears false `plan_failed` from dead-pid reap so write_proposed can promote to planned.
fn finish_may_continue_with_ir(config: &Config, job_id: &str, job: &mut PlanJob) -> bool {
    if matches!(job.status, PlanJobStatus::Cancelled) || job_marked_cancelled(config, job_id) {
        return false;
    }
    let Ok(disk) = PlanJob::load(config, job_id) else {
        return !job_finish_aborted(config, job_id);
    };
    match disk.status {
        PlanJobStatus::Cancelled => false,
        PlanJobStatus::Confirmed | PlanJobStatus::Planned => {
            // Already finalized elsewhere — don't double-write status; still ok to no-op finish.
            *job = disk;
            false
        }
        PlanJobStatus::PlanFailed => {
            if error_is_false_zombie_reap(disk.error.as_deref()) {
                append_log(
                    config,
                    job_id,
                    "finish recovers from false zombie reap (holding IR)",
                );
                *job = disk;
                job.status = PlanJobStatus::Planning;
                job.error = None;
                job.updated_at = Utc::now();
                let _ = job.save(config);
                true
            } else {
                *job = disk;
                false
            }
        }
        PlanJobStatus::Planning => {
            *job = disk;
            true
        }
    }
}

/// If job was plan_failed by false reap but `cco_split_agent.json` is ready → planned + proposed.
/// Also salvages when proposed exists but status never flipped.
fn try_salvage_plan_job(config: &Config, job: &mut PlanJob) -> bool {
    if matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        return false;
    }
    let dir = job_dir(config, &job.job_id);
    // Only salvage false-reap / still-planning-with-artifact (not user cancel).
    let salvageable = match job.status {
        PlanJobStatus::PlanFailed => error_is_false_zombie_reap(job.error.as_deref()),
        PlanJobStatus::Planning => {
            llm_work_finished_successfully(&dir) || has_recoverable_split_artifact(&dir)
        }
        _ => false,
    };
    if !salvageable {
        return false;
    }

    // Prefer proposed already on disk (finish aborted after write).
    if dir.join("plan.proposed.json").is_file() {
        if let Ok(ir) = super::view::load_proposed(config, &job.job_id) {
            if !ir.tasks.is_empty() {
                job.status = PlanJobStatus::Planned;
                job.plan_name = Some(ir.name.clone());
                job.task_count = Some(ir.tasks.len());
                job.max_parallel = Some(ir.max_parallel);
                job.adapter = Some(ir.adapter.clone());
                job.error = None;
                job.updated_at = Utc::now();
                let _ = job.save(config);
                append_log(
                    config,
                    &job.job_id,
                    &format!(
                        "salvaged planned from plan.proposed.json ({} tasks)",
                        ir.tasks.len()
                    ),
                );
                return true;
            }
        }
    }

    let agent_path = dir.join("cco_split_agent.json");
    let Ok(text) = std::fs::read_to_string(&agent_path) else {
        return false;
    };
    let Ok(doc) = serde_json::from_str::<crate::domain::plan::CcoSplitJob>(&text) else {
        return false;
    };
    if doc.tasks.is_empty() {
        return false;
    }
    let Ok(ir) = crate::plan::split_agent::cco_split_to_plan_ir(&doc, job) else {
        append_log(
            config,
            &job.job_id,
            "salvage: cco_split_agent.json present but convert to PlanIR failed",
        );
        return false;
    };
    if let Err(e) = write_proposed(config, &job.job_id, &ir) {
        append_log(
            config,
            &job.job_id,
            &format!("salvage write_proposed failed: {e:#}"),
        );
        return false;
    }
    job.status = PlanJobStatus::Planned;
    job.plan_name = Some(ir.name.clone());
    job.task_count = Some(ir.tasks.len());
    job.max_parallel = Some(ir.max_parallel);
    job.adapter = Some(ir.adapter.clone());
    job.error = None;
    job.updated_at = Utc::now();
    let _ = job.save(config);
    append_log(
        config,
        &job.job_id,
        &format!(
            "salvaged planned from cco_split_agent.json ({} tasks) after false zombie reap",
            ir.tasks.len()
        ),
    );
    true
}

/// Best-effort SIGTERM (then SIGKILL) of **all** live pids under
/// `llm_work/tasks/*/meta.json` (`__planner__`, `__critic__`, …).
fn kill_planner_pid(config: &Config, job_id: &str) {
    let dir = job_dir(config, job_id);
    let pids = collect_llm_work_pids(&dir);
    if pids.is_empty() {
        return;
    }
    for pid in pids {
        if kill_pid_best_effort(pid) {
            append_log(config, job_id, &format!("killed planner/worker pid={pid}"));
        }
    }
}

/// Public for planner LLM short-calls (critic timeout) that already hold a pid.
pub(super) fn kill_pid_best_effort(pid: u32) -> bool {
    if pid == 0 || !process_alive(pid) {
        return false;
    }
    #[cfg(unix)]
    {
        unsafe {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let _ = kill(pid as i32, 15); // SIGTERM
            std::thread::sleep(std::time::Duration::from_millis(200));
            if process_alive(pid) {
                let _ = kill(pid as i32, 9); // SIGKILL
            }
        }
        true
    }
    #[cfg(not(unix))]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
        true
    }
}

/// First live (or any known) pid among planner task metas — used for dead-pid reap heuristics.
fn read_planner_meta_pid(job_dir: &std::path::Path) -> Option<u32> {
    let pids = collect_llm_work_pids(job_dir);
    // Prefer a still-alive pid (critic hang) so dead-pid / kill logic sees the real worker.
    pids.iter()
        .copied()
        .find(|p| process_alive(*p))
        .or_else(|| pids.into_iter().next())
}

/// Scan `llm_work/tasks/*/meta.json` for pids (planner + critic + future short workers).
fn collect_llm_work_pids(job_dir: &std::path::Path) -> Vec<u32> {
    let tasks = job_dir.join("llm_work").join("tasks");
    let Ok(entries) = std::fs::read_dir(&tasks) else {
        return Vec::new();
    };
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let meta = entry.path().join("meta.json");
        let Ok(text) = std::fs::read_to_string(&meta) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(pid) = v
            .get("pid")
            .and_then(|p| p.as_u64())
            .map(|p| p as u32)
            .filter(|p| *p > 0)
        {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

fn planner_log_stale(job_dir: &std::path::Path, quiet_secs: i64) -> bool {
    let path = job_dir.join("planner.log");
    let Ok(meta) = std::fs::metadata(&path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = modified.elapsed() else {
        return false;
    };
    elapsed.as_secs() as i64 > quiet_secs
}

fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        unsafe { kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true // unknown — don't reap on pid alone
    }
}

/// 查找项目最近可恢复的规划会话（planning / planned / confirmed 且有任务图）。
/// 用于进项目时接上「上次拆分结果」，避免每次重拆。
/// 排序：仅 `updated_at` 最新；**跳过**超时仍 planning 的僵尸 job。
pub fn latest_plan_job_for_project(config: &Config, project: &Path) -> Result<Option<PlanJobView>> {
    let root = plan_jobs_dir(config);
    if !root.is_dir() {
        return Ok(None);
    }
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let now = Utc::now();

    let mut best: Option<PlanJob> = None;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let job_path = entry.path().join("job.json");
        if !job_path.is_file() {
            continue;
        }
        let mut job: PlanJob = match std::fs::read_to_string(&job_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(j) => j,
            None => continue,
        };
        let jp = job
            .project
            .canonicalize()
            .unwrap_or_else(|_| job.project.clone());
        if jp != project {
            continue;
        }
        // 只恢复仍有价值的状态；plan_failed 若可从 cco_split_agent 抢救则继续
        match job.status {
            PlanJobStatus::Planning | PlanJobStatus::Planned | PlanJobStatus::Confirmed => {}
            PlanJobStatus::PlanFailed => {
                if !try_salvage_plan_job(config, &mut job) {
                    continue;
                }
            }
            PlanJobStatus::Cancelled => continue,
        }
        // Reap or skip zombie planning (process gone / timeout)
        if matches!(job.status, PlanJobStatus::Planning) {
            if try_salvage_plan_job(config, &mut job) {
                // became planned from artifact
            } else if try_reap_zombie_planning(config, &mut job).is_some() {
                continue; // now plan_failed
            } else {
                let age = now.signed_duration_since(job.updated_at).num_seconds();
                if age > STALE_PLANNING_SECS {
                    continue;
                }
            }
        }
        // confirmed/planned 必须仍有图文件
        if matches!(
            job.status,
            PlanJobStatus::Confirmed | PlanJobStatus::Planned
        ) {
            let dir = entry.path();
            if !dir.join("plan.proposed.json").is_file()
                && !dir.join("plan.resolved.json").is_file()
            {
                continue;
            }
        }
        let replace = match &best {
            None => true,
            Some(b) => {
                // Prefer confirmed > planned > planning; then newer.
                // Residual planned must not hide an older confirmed success.
                fn rank(s: &PlanJobStatus) -> u8 {
                    match s {
                        PlanJobStatus::Confirmed => 3,
                        PlanJobStatus::Planned => 2,
                        PlanJobStatus::Planning => 1,
                        _ => 0,
                    }
                }
                let rj = rank(&job.status);
                let rb = rank(&b.status);
                rj > rb || (rj == rb && job.updated_at > b.updated_at)
            }
        };
        if replace {
            best = Some(job);
        }
    }

    match best {
        Some(job) => Ok(Some(job_view(config, &job, 96_000)?)),
        None => Ok(None),
    }
}

fn run_planner(config: &Config, job: &mut PlanJob) -> Result<PlanIR> {
    match job.plan_mode.as_str() {
        // Chat/plan-card「直接执行」: whole document = one worker task; still Mode B
        // (plan job → proposed → confirm_start). Never bypass confirm.
        "direct" => {
            append_log(
                config,
                &job.job_id,
                "using direct single-task (whole plan, no multi-step split)",
            );
            let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
            let text = std::fs::read_to_string(&abs)
                .map_err(|e| anyhow::anyhow!("读计划失败 {}: {e}", abs.display()))?;
            let mut ir = crate::plan::adapters::raw_single::parse(&abs, &text, config)?;
            // Force serial single slot so desk/run never claim multi-parallel.
            ir.max_parallel = 1;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        "parse" => {
            append_log(config, &job.job_id, "using adapter parse (load_plan)");
            let mut ir = load_plan(&job.project, &job.plan_path, None, config)?;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        "fake" => {
            append_log(config, &job.job_id, "using fake multi-task demo DAG");
            Ok(build_fake_plan(config, job)?)
        }
        // C6 fast path: local heuristic only — no Claude CLI wait.
        "fast" | "heuristic" => {
            append_log(
                config,
                &job.job_id,
                "using fast local splitter (heuristic; no LLM)",
            );
            let mut ir = build_heuristic_ai_plan(config, job)?;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        "ai" => {
            // Prefer structured parse when the document already has a real work graph.
            // Spec / contract MD often falsely matches serial-prompts (### Board, ### P0…)
            // — treat meta-heavy / product-spec graphs as prose and re-split.
            let source_text = crate::plan::resolve_plan_path(&job.project, &job.plan_path)
                .ok()
                .and_then(|p| std::fs::read_to_string(p).ok());
            let is_spec = source_text
                .as_deref()
                .map(super::heuristic::looks_like_spec_document)
                .unwrap_or(false);

            match load_plan(&job.project, &job.plan_path, None, config) {
                Ok(mut ir) if ir.adapter != "raw-single" && !ir.tasks.is_empty() => {
                    let meta_n = ir
                        .tasks
                        .iter()
                        .filter(|t| {
                            crate::plan::title_is_meta_heading(&t.id)
                                || crate::plan::title_is_meta_heading(&t.title)
                        })
                        .count();
                    let meta_heavy = meta_n * 2 >= ir.tasks.len().max(1);
                    // serial-prompts without fenced worker prompts is almost always a false graph
                    let unfenced = ir
                        .tasks
                        .iter()
                        .filter(|t| !t.prompt.contains("```"))
                        .count();
                    let looks_like_false_graph = ir.adapter == "serial-prompts/v0"
                        && (is_spec || unfenced * 2 >= ir.tasks.len().max(1) || meta_heavy);

                    if is_spec || meta_heavy || looks_like_false_graph {
                        append_log(
                            config,
                            &job.job_id,
                            &format!(
                                "adapter={} tasks={} meta={} spec={} unfenced={} → re-planning as work orders",
                                ir.adapter,
                                ir.tasks.len(),
                                meta_n,
                                is_spec,
                                unfenced
                            ),
                        );
                    } else {
                        append_log(
                            config,
                            &job.job_id,
                            &format!(
                                "document already structured (adapter={}), skipping LLM",
                                ir.adapter
                            ),
                        );
                        apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
                        return Ok(ir);
                    }
                }
                _ => {}
            }

            // 1) ModelSplitAgent → cco-split/v1 → SQLite SoT (OpenHands Plan Mode)
            //    then legacy PlanIR LLM, then heuristic.
            let force_heuristic = std::env::var("CCO_PLANNER_HEURISTIC")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
                || job.provider == "fake";
            let skip_split_agent = std::env::var("CCO_SPLIT_AGENT")
                .map(|v| {
                    v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")
                })
                .unwrap_or(false);
            if !force_heuristic && !skip_split_agent {
                match crate::plan::split_agent::build_split_agent_plan(config, job) {
                    Ok(ir) => {
                        append_log(
                            config,
                            &job.job_id,
                            &format!("ModelSplitAgent path ok (adapter={})", ir.adapter),
                        );
                        return Ok(ir);
                    }
                    Err(e) => {
                        append_log(
                            config,
                            &job.job_id,
                            &format!(
                                "ModelSplitAgent failed ({e:#}); trying legacy LLM PlanIR planner"
                            ),
                        );
                    }
                }
            } else if skip_split_agent {
                append_log(
                    config,
                    &job.job_id,
                    "skipping ModelSplitAgent (CCO_SPLIT_AGENT=off)",
                );
            }
            if !force_heuristic {
                match build_llm_plan(config, job) {
                    Ok(mut ir) => {
                        apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
                        return Ok(ir);
                    }
                    Err(e) => {
                        append_log(config, &job.job_id, &format!("LLM planner failed ({e:#})"));
                    }
                }
            } else {
                append_log(
                    config,
                    &job.job_id,
                    "skipping LLM planner (fake provider or CCO_PLANNER_HEURISTIC)",
                );
            }

            // Product rule: only a **complete** split may become the desk graph.
            // plan_mode=ai must not silently publish heuristic residual (5 C-headings, etc.)
            // — that covered prior success and showed incomplete graphs as if planned.
            // Explicit local path: force_heuristic (fake / CCO_PLANNER_HEURISTIC) or
            // CCO_PLANNER_ALLOW_HEURISTIC_FALLBACK=1, or plan_mode=fast (handled above).
            let allow_heuristic_fallback = force_heuristic
                || std::env::var("CCO_PLANNER_ALLOW_HEURISTIC_FALLBACK")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

            if !allow_heuristic_fallback {
                if let Some(prior) =
                    find_prior_successful_split(config, &job.project, &job.plan_path, &job.job_id)
                {
                    let n = prior.task_count.unwrap_or(0);
                    append_log(
                        config,
                        &job.job_id,
                        &format!(
                            "ai incomplete: plan_failed; keep prior {} ({} tasks); no residual desk",
                            prior.job_id, n
                        ),
                    );
                    bail!(
                        "智能拆分未完整完成，已保留上次成功的拆分（{} 步 · {}）。\
失败结果不展示、不覆盖。可「再拆一次」或更多选项显式「本地规则拆分」。",
                        n,
                        prior.job_id
                    );
                }
                append_log(
                    config,
                    &job.job_id,
                    "ai incomplete: plan_failed; no prior success; nothing to show on desk",
                );
                bail!(
                    "智能拆分未完整完成，没有可展示的完整拆分结果。\
（不会用残图充数。）可「再拆一次」，或在更多选项显式选「本地规则拆分」。"
                );
            }

            // Intentional local path only (tests / explicit env / fake).
            if let Some(prior) =
                find_prior_successful_split(config, &job.project, &job.plan_path, &job.job_id)
            {
                let n = prior.task_count.unwrap_or(0);
                append_log(
                    config,
                    &job.job_id,
                    &format!(
                        "refuse heuristic cover of prior {} ({} tasks) even with allow-fallback",
                        prior.job_id, n
                    ),
                );
                bail!(
                    "智能拆分未完整完成，已保留上次成功的拆分（{} 步）。未用本地残图覆盖。",
                    n
                );
            }

            append_log(
                config,
                &job.job_id,
                "using ai heuristic splitter (heading/paragraph; explicit fallback allowed)",
            );
            let mut ir = build_heuristic_ai_plan(config, job)?;
            apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
            Ok(ir)
        }
        other => bail!("unsupported plan_mode {other}"),
    }
}

/// Prior planned/confirmed job for the same plan path (not this job).
/// Used so incomplete AI fallback cannot become the restorable desk graph.
fn find_prior_successful_split(
    config: &Config,
    project: &Path,
    plan_path: &Path,
    exclude_job_id: &str,
) -> Option<PlanJob> {
    let want = crate::state::sqlite::plan_path_key(&plan_path.to_string_lossy());
    if want.is_empty() {
        return None;
    }
    let project_c = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    let root = plan_jobs_dir(config);
    if !root.is_dir() {
        return None;
    }
    let mut best: Option<PlanJob> = None;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return None;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if id == exclude_job_id {
            continue;
        }
        let job_path = entry.path().join("job.json");
        let Ok(text) = std::fs::read_to_string(&job_path) else {
            continue;
        };
        let Ok(other) = serde_json::from_str::<PlanJob>(&text) else {
            continue;
        };
        if !matches!(
            other.status,
            PlanJobStatus::Planned | PlanJobStatus::Confirmed
        ) {
            continue;
        }
        // Must have a real proposed graph (complete enough to restore).
        if other.task_count.unwrap_or(0) == 0 {
            continue;
        }
        if !entry.path().join("plan.proposed.json").is_file() {
            continue;
        }
        let jp = other
            .project
            .canonicalize()
            .unwrap_or_else(|_| other.project.clone());
        if jp != project_c {
            continue;
        }
        let other_plan = crate::state::sqlite::plan_path_key(&other.plan_path.to_string_lossy());
        if !crate::state::sqlite::plan_paths_match(&other_plan, &want) {
            continue;
        }
        let replace = match &best {
            None => true,
            Some(b) => {
                // Same product rank as latest_job restore: multi-step AI ≫ direct 1-step,
                // then status, then updated_at. Confirmed raw-single must not win over
                // a planned 8-step AI graph when re-split fails.
                use crate::state::sqlite::{cmp_split_restore, split_graph_quality};
                let qo = split_graph_quality(
                    Some(other.plan_mode.as_str()),
                    other.adapter.as_deref(),
                    other.task_count.map(|n| n as u32),
                );
                let qb = split_graph_quality(
                    Some(b.plan_mode.as_str()),
                    b.adapter.as_deref(),
                    b.task_count.map(|n| n as u32),
                );
                let so = match other.status {
                    PlanJobStatus::Confirmed => "confirmed",
                    PlanJobStatus::Planned => "planned",
                    PlanJobStatus::Planning => "planning",
                    _ => "other",
                };
                let sb = match b.status {
                    PlanJobStatus::Confirmed => "confirmed",
                    PlanJobStatus::Planned => "planned",
                    PlanJobStatus::Planning => "planning",
                    _ => "other",
                };
                let uo = other.updated_at.to_rfc3339();
                let ub = b.updated_at.to_rfc3339();
                cmp_split_restore(qo, so, &uo, &other.job_id, qb, sb, &ub, &b.job_id).is_gt()
            }
        };
        if replace {
            best = Some(other);
        }
    }
    best
}

/// Soft-fill job defaults onto tasks (H4 / Q6 / A1-4).
///
/// Delegates pure route fill to [`crate::domain::worker::apply_worker_defaults`].
/// Soft: never overwrites explicit non-default engines. Aligns with CLI `--provider`.
/// Report is discarded here (plan-job path); confirm stamps provenance at materialize.
pub(super) fn apply_worker_defaults(ir: &mut PlanIR, provider: &str, exec_mode: &str) {
    let _ = crate::domain::worker::apply_worker_defaults(ir, provider, exec_mode);
}

#[cfg(test)]
mod apply_worker_defaults_tests {
    use super::apply_worker_defaults;
    use crate::plan::{OnFailure, PlanIR, TaskIR};
    use std::path::PathBuf;

    fn task(id: &str, provider: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: provider.into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec![],
        }
    }

    fn mixed_ir() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "mixed".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("mixed.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![
                task("t1", "claude"),
                task("t2", "codex"),
                task("t3", "default"),
                task("t4", ""),
            ],
        }
    }

    #[test]
    fn soft_fill_keeps_explicit_provider() {
        let mut ir = mixed_ir();
        apply_worker_defaults(&mut ir, "fake", "bg");
        assert_eq!(ir.default_provider, "fake");
        assert_eq!(ir.default_mode, "bg");
        assert_eq!(ir.tasks[0].provider, "fake"); // was plan default
        assert_eq!(ir.tasks[1].provider, "codex"); // user/plan explicit — kept
        assert_eq!(ir.tasks[2].provider, "fake"); // placeholder
        assert_eq!(ir.tasks[3].provider, "fake"); // empty
        assert!(ir.tasks.iter().all(|t| t.mode == "bg"));
    }

    #[test]
    fn soft_fill_same_provider_is_noop_on_tasks() {
        let mut ir = mixed_ir();
        apply_worker_defaults(&mut ir, "claude", "print");
        assert_eq!(ir.tasks[0].provider, "claude");
        assert_eq!(ir.tasks[1].provider, "codex");
    }
}

#[cfg(test)]
mod reap_pid_scan_tests {
    use super::{collect_llm_work_pids, get_plan_job, job_dir, PlanJob, PlanJobStatus};
    use crate::config::Config;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn collect_llm_work_pids_reads_critic_and_planner_meta() {
        let dir = tempdir().unwrap();
        let job = dir.path().join("job");
        for (name, pid) in [("__planner__", 111u32), ("__critic__", 222u32)] {
            let t = job.join("llm_work/tasks").join(name);
            std::fs::create_dir_all(&t).unwrap();
            std::fs::write(
                t.join("meta.json"),
                format!(r#"{{"pid": {pid}, "opaque_id": "pid:{pid}"}}"#),
            )
            .unwrap();
        }
        let pids = collect_llm_work_pids(&job);
        assert!(pids.contains(&111), "pids={pids:?}");
        assert!(pids.contains(&222), "pids={pids:?}");
    }

    #[test]
    fn get_plan_job_reaps_zombie_when_only_critic_meta_exists() {
        // Repro: fast path hung on __critic__ only; old reaper looked solely at __planner__.
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let zombie_id = "plan-reap-critic-only-test";
        let zombie_dir = job_dir(&cfg, zombie_id);
        std::fs::create_dir_all(zombie_dir.join("llm_work/tasks/__critic__")).unwrap();
        std::fs::write(
            zombie_dir.join("llm_work/tasks/__critic__/meta.json"),
            r#"{"pid": 999999, "opaque_id": "pid:999999"}"#,
        )
        .unwrap();
        std::fs::write(
            zombie_dir.join("planner.log"),
            "using fast local splitter\n",
        )
        .unwrap();
        let zombie = PlanJob {
            job_id: zombie_id.into(),
            status: PlanJobStatus::Planning,
            project: project.clone(),
            plan_path: PathBuf::from("idea.md"),
            plan_mode: "fast".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now() - Duration::minutes(6),
            updated_at: Utc::now() - Duration::minutes(6),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        };
        zombie.save(&cfg).unwrap();

        let view = get_plan_job(&cfg, zombie_id).unwrap();
        assert_eq!(view.status, "plan_failed", "err={:?}", view.error);
        assert!(
            view.error
                .as_deref()
                .map(|e| e.contains("process gone") || e.contains("timeout") || e.contains("stale"))
                .unwrap_or(false),
            "expected reap reason, got {:?}",
            view.error
        );
    }

    /// Repro: ModelSplitAgent CLI exits (pid dead + .done) while finish still converting.
    /// Old reaper flipped plan_failed → desk "共 0 步". Must NOT reap.
    #[test]
    fn try_reap_skips_when_worker_done_successfully() {
        use super::{llm_work_finished_successfully, try_reap_zombie_planning};

        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let job_id = "plan-reap-skip-done-test";
        let jdir = job_dir(&cfg, job_id);
        let agent = jdir.join("llm_work/tasks/__split_agent__");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("meta.json"),
            r#"{"pid": 999998, "exit_code": 0, "mode": "print"}"#,
        )
        .unwrap();
        std::fs::write(agent.join(".done"), b"1").unwrap();
        std::fs::write(jdir.join("planner.log"), "ModelSplitAgent ok\n").unwrap();
        assert!(llm_work_finished_successfully(&jdir));

        let mut job = PlanJob {
            job_id: job_id.into(),
            status: PlanJobStatus::Planning,
            project: project.clone(),
            plan_path: PathBuf::from("docs/x.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now() - Duration::minutes(3),
            updated_at: Utc::now() - Duration::minutes(3),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        };
        job.save(&cfg).unwrap();

        assert!(
            try_reap_zombie_planning(&cfg, &mut job).is_none(),
            "must not reap successful worker exit"
        );
        let reloaded = PlanJob::load(&cfg, job_id).unwrap();
        assert_eq!(reloaded.status, PlanJobStatus::Planning);
    }

    /// Desk poll after false reap: salvage from cco_split_agent.json → planned + tasks.
    #[test]
    fn get_plan_job_salvages_false_reap_from_cco_split_agent() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_path_buf();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let job_id = "plan-salvage-split-agent-test";
        let jdir = job_dir(&cfg, job_id);
        std::fs::create_dir_all(jdir.join("llm_work/tasks/__split_agent__")).unwrap();
        std::fs::write(
            jdir.join("llm_work/tasks/__split_agent__/meta.json"),
            r#"{"pid": 999997, "exit_code": 0}"#,
        )
        .unwrap();
        std::fs::write(jdir.join("llm_work/tasks/__split_agent__/.done"), b"1").unwrap();

        let agent_json = format!(
            r#"{{
  "job_id": "{job_id}",
  "project": {},
  "plan_path": "docs/x.md",
  "status": "ready",
  "title": "salvage demo",
  "max_parallel": 2,
  "source": "llm",
  "created_at": "2026-07-22T00:00:00Z",
  "updated_at": "2026-07-22T00:01:00Z",
  "tasks": [
    {{
      "task_id": "t1",
      "ord": 0,
      "title": "补齐模板五节",
      "summary": "新建计划有五节",
      "body": "做模板",
      "depends_on": [],
      "wave": 0,
      "enabled": true,
      "optional": false,
      "done_when": "可见五节",
      "plan_ref": "D0-1",
      "kind": "do",
      "status": "pending",
      "scope_paths": []
    }},
    {{
      "task_id": "t2",
      "ord": 1,
      "title": "黄条不拦确认",
      "summary": "空心有提醒",
      "body": "黄条",
      "depends_on": ["t1"],
      "wave": 1,
      "enabled": true,
      "optional": false,
      "done_when": "有黄条",
      "plan_ref": "D0-2",
      "kind": "do",
      "status": "pending",
      "scope_paths": []
    }}
  ]
}}"#,
            serde_json::to_string(project.to_str().unwrap_or(".")).unwrap()
        );
        std::fs::write(jdir.join("cco_split_agent.json"), agent_json).unwrap();

        let failed = PlanJob {
            job_id: job_id.into(),
            status: PlanJobStatus::PlanFailed,
            project: project.clone(),
            plan_path: PathBuf::from("docs/x.md"),
            plan_mode: "ai".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: Some("planner process gone (pid=Some(999997)); job left in planning".into()),
            run_id: None,
            created_at: Utc::now() - Duration::minutes(3),
            updated_at: Utc::now() - Duration::minutes(1),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        };
        failed.save(&cfg).unwrap();

        let view = get_plan_job(&cfg, job_id).unwrap();
        assert_eq!(
            view.status, "planned",
            "err={:?} tasks={:?}",
            view.error, view.task_count
        );
        assert!(
            view.task_count.unwrap_or(0) >= 2,
            "expected salvaged tasks, got {:?}",
            view.task_count
        );
        assert!(
            jdir.join("plan.proposed.json").is_file(),
            "salvage must write plan.proposed.json"
        );
    }

    /// Live salvage of the user's failed smart-split (needs ~/.cco job on disk).
    /// Run: `CCO_SALVAGE_JOB=plan-… cargo test -p cco salvage_real_failed -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn salvage_real_failed_plan_job_from_env() {
        let id = std::env::var("CCO_SALVAGE_JOB").expect("set CCO_SALVAGE_JOB");
        let cfg = Config::load().expect("Config::load");
        let before = PlanJob::load(&cfg, &id).expect("load job");
        println!("before status={:?} err={:?}", before.status, before.error);
        let view = get_plan_job(&cfg, &id).expect("get_plan_job");
        println!(
            "after status={} tasks={:?} err={:?}",
            view.status, view.task_count, view.error
        );
        assert_eq!(view.status, "planned");
        assert!(view.task_count.unwrap_or(0) >= 1);
        assert!(
            job_dir(&cfg, &id).join("plan.proposed.json").is_file(),
            "proposed missing after salvage"
        );
    }

    /// W2-4: re-split plan A must not cancel plan B's in-flight planning job.
    #[test]
    fn supersede_planning_is_per_plan_path() {
        use super::{plan_jobs_dir, supersede_planning_jobs};

        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        std::fs::create_dir_all(plan_jobs_dir(&cfg)).unwrap();

        let mk = |id: &str, plan: &str| {
            let j = PlanJob {
                job_id: id.into(),
                status: PlanJobStatus::Planning,
                project: project.clone(),
                plan_path: PathBuf::from(plan),
                plan_mode: "fast".into(),
                provider: "fake".into(),
                exec_mode: "print".into(),
                error: None,
                run_id: None,
                created_at: Utc::now() - Duration::minutes(1),
                updated_at: Utc::now() - Duration::minutes(1),
                plan_name: None,
                task_count: None,
                max_parallel: Some(2),
                adapter: None,
                planner_cost_usd: None,
                digest_mode: None,
                critic_summary: None,
                critic_edges_removed: None,
                critic_titles_rewritten: None,
                critic_prompts_tagged: None,
                critic_notes: vec![],
                critic_llm_used: None,
                critic_llm_cost_usd: None,
                critic_llm_ms: None,
                grain_hint: None,
                clarify_depth: None,
                revision_notes: None,
                effort: None,
            };
            std::fs::create_dir_all(job_dir(&cfg, id)).unwrap();
            j.save(&cfg).unwrap();
        };

        mk("job-a-old", "plans/a.md");
        mk("job-b", "plans/b.md");
        mk("job-a-new", "plans/a.md");

        supersede_planning_jobs(
            &cfg,
            &project,
            PathBuf::from("plans/a.md").as_path(),
            "job-a-new",
        );

        let a_old = PlanJob::load(&cfg, "job-a-old").unwrap();
        let b = PlanJob::load(&cfg, "job-b").unwrap();
        let a_new = PlanJob::load(&cfg, "job-a-new").unwrap();
        assert!(
            matches!(a_old.status, PlanJobStatus::Cancelled),
            "same plan_path old planning must cancel, got {:?}",
            a_old.status
        );
        assert!(
            matches!(b.status, PlanJobStatus::Planning),
            "other plan_path must survive, got {:?}",
            b.status
        );
        assert!(
            matches!(a_new.status, PlanJobStatus::Planning),
            "keep job must stay planning"
        );
    }
}
