//! Plan job views, proposed plan IO, confirm helpers.
//!
//! [INPUT]: PlanJob · PlanIR · Config
//! [OUTPUT]: PlanJobView · load_proposed · update_proposed_task · remove_proposed_task ·
//!           sanitize_proposed_deps · user_edits(P2-1/P2-2) · mark_confirmed · load_proposed_for_exec
//! [POS]: planner 子模块；桌面/CLI 确认屏消费
//! note: sanitize_proposed_deps 复用 digest::sanitize_task_deps；确认屏可删任务/改依赖；
//!       replan 经 preserve_from_job_id 应用 plan.user_edits.json
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::graph::topo_layers;
use crate::plan::{PlanIR, TaskIR};
use crate::runtime::log_events::{self, LogEvent};

use super::job::{append_log, apply_worker_defaults, job_dir, read_log_tail, PlanJob, PlanJobStatus};
use super::llm::read_planner_cost;

/// Scope paths exposed on the confirm DTO (S-role).
#[derive(Debug, Clone, Serialize)]
pub struct TaskScopeView {
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readonly: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbid: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanTaskView {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub group: Option<String>,
    /// Worker engine for this task (confirm screen may override; H4).
    pub provider: String,
    /// Collaboration role wire name (`scout`|`implement`|…); absent = unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Path contract for advanced fold (S-role).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<TaskScopeView>,
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
    /// Document mode: regression | greenfield | audit | mixed (from plan digest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_mode: Option<String>,
    /// Critic one-liner for confirm hygiene strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_edges_removed: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_titles_rewritten: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_prompts_tagged: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub critic_notes: Vec<String>,
    /// Optional LLM second-pass was invoked for this split.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critic_llm_ms: Option<u64>,
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
    let scope = t.scope.as_ref().map(|s| TaskScopeView {
        paths: s.paths.clone(),
        readonly: s.readonly.clone(),
        forbid: s.forbid.clone(),
    });
    PlanTaskView {
        id: t.id.clone(),
        title: t.title.clone(),
        depends_on: t.depends_on.clone(),
        group: t.group.clone(),
        provider: t.provider.clone(),
        role: super::task_edit::role_wire(t.role),
        scope,
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
        digest_mode: job.digest_mode.clone(),
        critic_summary: job.critic_summary.clone(),
        critic_edges_removed: job.critic_edges_removed,
        critic_titles_rewritten: job.critic_titles_rewritten,
        critic_prompts_tagged: job.critic_prompts_tagged,
        critic_notes: job.critic_notes.clone(),
        critic_llm_used: job.critic_llm_used,
        critic_llm_cost_usd: job.critic_llm_cost_usd,
        critic_llm_ms: job.critic_llm_ms,
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

/// Normalize a task title for user-edit matching across replan (id 会变).
pub fn normalize_task_title_key(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Per-task manual edits captured on the confirm screen (P2-1 / P2-2 / S-role).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskUserEdit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<bool>,
    /// When set, deps were explicitly edited; values are normalized titles of dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on_titles: Option<Vec<String>>,
    /// Wire role name, or empty string meaning "cleared".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Writable scope paths when user edited advanced fold (S-role).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_paths: Option<Vec<String>>,
}

/// Sidecar next to plan.proposed.json so replan can re-apply human patches by title.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanUserEdits {
    /// Key = normalize_task_title_key of the task title *at the time of the last edit*.
    #[serde(default)]
    pub by_title: BTreeMap<String, TaskUserEdit>,
    /// Titles the user deleted on the confirm screen (normalized).
    #[serde(default)]
    pub removed_titles: Vec<String>,
}

fn user_edits_path(config: &Config, job_id: &str) -> std::path::PathBuf {
    job_dir(config, job_id).join("plan.user_edits.json")
}

pub fn load_user_edits(config: &Config, job_id: &str) -> PlanUserEdits {
    let path = user_edits_path(config, job_id);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn write_user_edits(config: &Config, job_id: &str, edits: &PlanUserEdits) -> Result<()> {
    let path = user_edits_path(config, job_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(edits)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Copy user-edits sidecar from an older job (replan preserve).
pub fn copy_user_edits(config: &Config, from_job_id: &str, to_job_id: &str) -> Result<()> {
    let edits = load_user_edits(config, from_job_id);
    if edits.by_title.is_empty() && edits.removed_titles.is_empty() {
        return Ok(());
    }
    write_user_edits(config, to_job_id, &edits)
}

/// `match_title_key` is the title key *before* this edit (planner/original title).
/// It stays stable across renames so replan can still match the freshly split task.
fn record_task_edit(
    edits: &mut PlanUserEdits,
    match_title_key: &str,
    task: &TaskIR,
    ir: &PlanIR,
    patch: TaskUserEdit,
) {
    let key = match_title_key.to_string();
    // Never treat a removed title as still present.
    edits.removed_titles.retain(|t| t != &key);

    // Resolve dep match-keys *before* mutably borrowing `by_title`.
    let dep_titles = if patch.depends_on_titles.is_some() {
        Some(
            task.depends_on
                .iter()
                .filter_map(|id| ir.tasks.iter().find(|t| &t.id == id))
                .map(|t| {
                    let cur = normalize_task_title_key(&t.title);
                    edits
                        .by_title
                        .iter()
                        .find(|(k, e)| {
                            e.title
                                .as_ref()
                                .map(|tt| normalize_task_title_key(tt) == cur)
                                .unwrap_or(false)
                                || *k == &cur
                        })
                        .map(|(k, _)| k.clone())
                        .unwrap_or(cur)
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let entry = edits.by_title.entry(key).or_default();
    if patch.title.is_some() {
        // Store the *new* display title; map key remains the original match key.
        entry.title = Some(task.title.clone());
    }
    if patch.prompt.is_some() {
        entry.prompt = Some(task.prompt.clone());
    }
    if patch.provider.is_some() {
        entry.provider = Some(task.provider.clone());
    }
    if patch.include.is_some() {
        entry.include = Some(if task.optional { task.include } else { true });
    }
    if patch.role.is_some() {
        // Empty string = user cleared role (preserve as clear on replan).
        entry.role = Some(
            task.role
                .map(|r| r.as_str().to_string())
                .unwrap_or_default(),
        );
    }
    if patch.scope_paths.is_some() {
        entry.scope_paths = Some(
            task.scope
                .as_ref()
                .map(|s| s.paths.clone())
                .unwrap_or_default(),
        );
    }
    if let Some(deps) = dep_titles {
        entry.depends_on_titles = Some(deps);
    }
}

/// Apply preserved user edits onto a freshly planned IR (P2-2).
/// Matches by normalized title; rebuilds depends_on from dep titles → new ids.
/// Returns (applied_field_count, removed_task_count).
pub fn apply_user_edits_to_ir(ir: &mut PlanIR, edits: &PlanUserEdits) -> (usize, usize) {
    if edits.by_title.is_empty() && edits.removed_titles.is_empty() {
        return (0, 0);
    }
    let removed: std::collections::HashSet<String> = edits.removed_titles.iter().cloned().collect();
    let before_len = ir.tasks.len();
    // Collect ids of tasks we are about to drop so remaining edges can be rewritten.
    let removed_ids: std::collections::HashSet<String> = ir
        .tasks
        .iter()
        .filter(|t| removed.contains(&normalize_task_title_key(&t.title)))
        .map(|t| t.id.clone())
        .collect();
    ir.tasks
        .retain(|t| !removed.contains(&normalize_task_title_key(&t.title)));
    let removed_n = before_len.saturating_sub(ir.tasks.len());
    if !removed_ids.is_empty() {
        for t in ir.tasks.iter_mut() {
            t.depends_on.retain(|d| !removed_ids.contains(d));
        }
    }

    // Build title-key → id map after removals.
    let mut title_to_id: BTreeMap<String, String> = BTreeMap::new();
    for t in &ir.tasks {
        title_to_id.insert(normalize_task_title_key(&t.title), t.id.clone());
    }

    let mut applied = 0usize;
    for t in ir.tasks.iter_mut() {
        let key = normalize_task_title_key(&t.title);
        let Some(edit) = edits.by_title.get(&key) else {
            continue;
        };
        if let Some(ref title) = edit.title {
            let title = title.trim();
            if !title.is_empty() && title != t.title {
                t.title = if t.optional {
                    crate::plan::normalize_optional_title(title, true)
                } else {
                    title.to_string()
                };
                applied += 1;
            }
        }
        if let Some(ref prompt) = edit.prompt {
            if !prompt.trim().is_empty() && prompt != &t.prompt {
                t.prompt = prompt.clone();
                applied += 1;
            }
        }
        if let Some(ref provider) = edit.provider {
            let p = provider.trim().to_ascii_lowercase();
            if !p.is_empty() && p != t.provider {
                t.provider = p;
                applied += 1;
            }
        }
        if let Some(inc) = edit.include {
            if t.optional && t.include != inc {
                t.include = inc;
                applied += 1;
            }
        }
        if let Some(ref role_raw) = edit.role {
            if let Ok(changed) =
                super::task_edit::apply_role_patch(t, Some(role_raw.clone()))
            {
                if changed {
                    applied += 1;
                }
            }
        }
        if let Some(ref paths) = edit.scope_paths {
            if let Ok(changed) =
                super::task_edit::apply_scope_paths_patch(t, Some(paths.clone()))
            {
                if changed {
                    applied += 1;
                }
            }
        }
        if let Some(ref dep_titles) = edit.depends_on_titles {
            let mut deps: Vec<String> = Vec::new();
            for dt in dep_titles {
                if let Some(id) = title_to_id.get(dt) {
                    if id != &t.id && !deps.contains(id) {
                        deps.push(id.clone());
                    }
                }
            }
            if deps != t.depends_on {
                t.depends_on = deps;
                applied += 1;
            }
        }
    }

    // After title renames, refresh title_to_id and re-apply deps that might have missed.
    // (deps already applied against pre-rename titles of *this* task's deps; dep targets use
    // their planned titles which is correct — user edit stores dep titles as they were.)
    let _ = title_to_id;
    (applied, removed_n)
}

fn load_proposed_or_resolved(config: &Config, job_id: &str) -> Result<PlanIR> {
    load_proposed(config, job_id).or_else(|_| {
        let path = job_dir(config, job_id).join("plan.resolved.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let ir: PlanIR = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok::<PlanIR, anyhow::Error>(ir)
    })
}

fn touch_job_after_edit(job: &mut PlanJob, ir: &PlanIR) {
    job.plan_name = Some(ir.name.clone());
    job.task_count = Some(ir.tasks.len());
    job.max_parallel = Some(ir.max_parallel);
    job.updated_at = Utc::now();
    if matches!(job.status, PlanJobStatus::Confirmed) {
        job.status = PlanJobStatus::Planned;
        job.run_id = None;
    }
}

/// Patch one task in the proposed plan (title/prompt/include/provider/depends_on/role/scope)
/// while still planned/confirmed. P2-1: depends_on is optional explicit edge list.
/// S-role: `role` / `scope_paths` optional; empty role clears; empty paths clears writable scope.
pub fn update_proposed_task(
    config: &Config,
    job_id: &str,
    task_id: &str,
    title: Option<String>,
    prompt: Option<String>,
    include: Option<bool>,
    provider: Option<String>,
    depends_on: Option<Vec<String>>,
    role: Option<String>,
    scope_paths: Option<Vec<String>>,
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
    let mut ir = load_proposed_or_resolved(config, job_id)?;
    let Some(task_idx) = ir.tasks.iter().position(|t| t.id == task_id) else {
        bail!("任务不存在: {task_id}");
    };
    let prev_title_key = normalize_task_title_key(&ir.tasks[task_idx].title);
    let mut patch = TaskUserEdit::default();

    if let Some(t) = title {
        let t = t.trim().to_string();
        if t.is_empty() {
            bail!("标题不能为空");
        }
        let task = &mut ir.tasks[task_idx];
        task.title = if task.optional {
            crate::plan::normalize_optional_title(&t, true)
        } else {
            t
        };
        patch.title = Some(task.title.clone());
    }
    if let Some(p) = prompt {
        let p = p.trim_end().to_string();
        if p.trim().is_empty() {
            bail!("任务说明不能为空");
        }
        ir.tasks[task_idx].prompt = p;
        patch.prompt = Some(ir.tasks[task_idx].prompt.clone());
    }
    if let Some(inc) = include {
        let task = &mut ir.tasks[task_idx];
        if !task.optional {
            if !inc {
                bail!("必选任务不能取消勾选");
            }
            task.include = true;
        } else {
            task.include = inc;
        }
        patch.include = Some(task.include);
    }
    if let Some(p) = provider {
        let p = super::task_edit::validate_provider_name(&p)?;
        ir.tasks[task_idx].provider = p;
        patch.provider = Some(ir.tasks[task_idx].provider.clone());
    }
    if role.is_some() {
        let changed = super::task_edit::apply_role_patch(&mut ir.tasks[task_idx], role)?;
        if changed {
            // Marker so record_task_edit stores role (including clear → "").
            patch.role = Some(
                ir.tasks[task_idx]
                    .role
                    .map(|r| r.as_str().to_string())
                    .unwrap_or_default(),
            );
        }
    }
    if scope_paths.is_some() {
        let changed =
            super::task_edit::apply_scope_paths_patch(&mut ir.tasks[task_idx], scope_paths)?;
        if changed {
            patch.scope_paths = Some(
                ir.tasks[task_idx]
                    .scope
                    .as_ref()
                    .map(|s| s.paths.clone())
                    .unwrap_or_default(),
            );
        }
    }
    // Role=inspect defaults (tools/scope/prompt) — idempotent; only affects inspect.
    crate::plan::materialize_role_defaults(&mut ir);
    if let Some(deps) = depends_on {
        let ids: std::collections::HashSet<_> =
            ir.tasks.iter().map(|t| t.id.as_str()).collect();
        let mut clean: Vec<String> = Vec::new();
        for d in deps {
            let d = d.trim().to_string();
            if d.is_empty() {
                continue;
            }
            if d == task_id {
                bail!("任务不能依赖自己: {task_id}");
            }
            if !ids.contains(d.as_str()) {
                bail!("依赖不存在: {d}");
            }
            if !clean.contains(&d) {
                clean.push(d);
            }
        }
        ir.tasks[task_idx].depends_on = clean;
        // Marker so record_task_edit stores depends_on_titles.
        patch.depends_on_titles = Some(Vec::new());
    }

    ir.validate()?;
    write_proposed(config, job_id, &ir)?;

    // Record human patch for replan preserve (P2-2).
    // Key stays the title *before this edit* so renames still match on next split.
    let mut edits = load_user_edits(config, job_id);
    // If this task was already edited under an older key, keep that key.
    let match_key = {
        let cur = normalize_task_title_key(&ir.tasks[task_idx].title);
        edits
            .by_title
            .iter()
            .find(|(k, e)| {
                **k == prev_title_key
                    || e.title
                        .as_ref()
                        .map(|tt| normalize_task_title_key(tt) == prev_title_key)
                        .unwrap_or(false)
                    || **k == cur
            })
            .map(|(k, _)| k.clone())
            .unwrap_or(prev_title_key.clone())
    };
    record_task_edit(
        &mut edits,
        &match_key,
        &ir.tasks[task_idx],
        &ir,
        patch,
    );
    let _ = write_user_edits(config, job_id, &edits);

    touch_job_after_edit(&mut job, &ir);
    job.save(config)?;
    let provider_note = ir.tasks[task_idx].provider.as_str();
    let role_note = ir.tasks[task_idx]
        .role
        .map(|r| r.as_str())
        .unwrap_or("-");
    let deps_n = ir.tasks[task_idx].depends_on.len();
    let scope_n = ir.tasks[task_idx]
        .scope
        .as_ref()
        .map(|s| s.paths.len())
        .unwrap_or(0);
    append_log(
        config,
        job_id,
        &format!(
            "updated task {task_id} (title/prompt/include/provider={provider_note}/role={role_note}/scope_paths={scope_n}/deps={deps_n})",
        ),
    );
    job_view(config, &job, 48_000)
}

/// Remove a task from the proposed plan (P2-1). Rewrites other tasks' depends_on.
/// Refuses if it would leave the plan empty.
pub fn remove_proposed_task(
    config: &Config,
    job_id: &str,
    task_id: &str,
) -> Result<PlanJobView> {
    let mut job = PlanJob::load(config, job_id)?;
    if !matches!(
        job.status,
        PlanJobStatus::Planned | PlanJobStatus::Confirmed
    ) {
        bail!(
            "计划任务状态为 {}，仅 planned/confirmed 可删任务",
            job.status.as_str()
        );
    }
    let mut ir = load_proposed_or_resolved(config, job_id)?;
    let Some(pos) = ir.tasks.iter().position(|t| t.id == task_id) else {
        bail!("任务不存在: {task_id}");
    };
    if ir.tasks.len() <= 1 {
        bail!("至少保留一个任务，不能删光");
    }
    let removed_title = ir.tasks[pos].title.clone();
    let removed_key = normalize_task_title_key(&removed_title);
    ir.tasks.remove(pos);
    for t in ir.tasks.iter_mut() {
        t.depends_on.retain(|d| d != task_id);
    }
    ir.validate()?;
    write_proposed(config, job_id, &ir)?;

    let mut edits = load_user_edits(config, job_id);
    edits.by_title.remove(&removed_key);
    if !edits.removed_titles.iter().any(|t| t == &removed_key) {
        edits.removed_titles.push(removed_key);
    }
    let _ = write_user_edits(config, job_id, &edits);

    touch_job_after_edit(&mut job, &ir);
    job.save(config)?;
    append_log(
        config,
        job_id,
        &format!("removed task {task_id} ({removed_title})"),
    );
    job_view(config, &job, 48_000)
}

/// Drop unmotivated depends_on edges on the proposed plan (confirm-screen action).
/// Same rules as planner post-process: edge kept only if the dependent prompt
/// mentions the dep id/title or an explicit depend-reason line.
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
    let mut ir = load_proposed(config, job_id).or_else(|_| {
        let path = job_dir(config, job_id).join("plan.resolved.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let ir: PlanIR = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        Ok::<PlanIR, anyhow::Error>(ir)
    })?;

    let before: usize = ir.tasks.iter().map(|t| t.depends_on.len()).sum();
    super::digest::sanitize_task_deps(&mut ir.tasks);
    let after: usize = ir.tasks.iter().map(|t| t.depends_on.len()).sum();
    let removed = before.saturating_sub(after);

    ir.validate()?;
    write_proposed(config, job_id, &ir)?;
    job.plan_name = Some(ir.name.clone());
    job.task_count = Some(ir.tasks.len());
    job.max_parallel = Some(ir.max_parallel);
    // Refresh hygiene strip so confirm UI shows latest cleanup result.
    job.critic_summary = Some(if removed > 0 {
        format!("拆分校对：手动清理 · 去掉 {removed} 条可疑依赖")
    } else {
        "拆分校对：手动清理 · 未发现可疑依赖".into()
    });
    job.critic_edges_removed = Some(removed);
    // Manual sanitize does not rewrite titles/prompts.
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
        job_id,
        &format!("sanitize deps: removed {removed} edge(s) (before={before} after={after})"),
    );
    let view = job_view(config, &job, 48_000)?;
    Ok(SanitizeDepsResult { removed, view })
}

#[derive(Debug, Clone, Serialize)]
pub struct SanitizeDepsResult {
    /// Number of depends_on edges dropped.
    pub removed: usize,
    pub view: PlanJobView,
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
