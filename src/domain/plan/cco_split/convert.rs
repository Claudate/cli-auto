//! PlanIR ↔ CcoSplit conversion (pure).
//!
//! [INPUT]: PlanIR · CcoSplitJob
//! [OUTPUT]: from_plan_ir · to_plan_ir
//! [POS]: domain/plan/cco_split
//! [PROTOCOL]: 变更时更新此头部

use std::path::PathBuf;

use super::accept::{first_line_summary, soft_accept_split};
use super::types::{
    CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask, CcoTaskKind, CcoTaskStatus,
};
use crate::domain::plan::types::{OnFailure, PlanIR, TaskIR, TaskRole, TaskScope};

/// Build CcoSplit from PlanIR (producer / import path).
pub fn from_plan_ir(
    job_id: &str,
    project: PathBuf,
    plan_path: PathBuf,
    ir: &PlanIR,
    source: CcoSplitSource,
    status: CcoSplitStatus,
    created_at: &str,
    updated_at: &str,
) -> CcoSplitJob {
    let mut doc = CcoSplitJob {
        job_id: job_id.to_string(),
        project,
        plan_path,
        status,
        title: ir.name.clone(),
        max_parallel: ir.max_parallel.max(1),
        source,
        error: None,
        run_id: None,
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
        tasks: ir
            .tasks
            .iter()
            .enumerate()
            .map(|(i, t)| task_from_ir(i as i32, t))
            .collect(),
    };
    let _ = soft_accept_split(&mut doc);
    doc
}

fn task_from_ir(ord: i32, t: &TaskIR) -> CcoSplitTask {
    let kind = if t.id.starts_with("sys-post-") {
        CcoTaskKind::System
    } else if t.role == Some(TaskRole::Inspect) {
        CcoTaskKind::Check
    } else {
        CcoTaskKind::Do
    };
    // H2: split human vs shell. Runnable acceptance/verify → verify_cmd;
    // non-runnable acceptance → done_when; parse body for more human criteria.
    let verify_cmd = t
        .verify_cmd
        .clone()
        .filter(|s| crate::domain::plan::is_runnable_verify(s))
        .or_else(|| {
            t.acceptance
                .clone()
                .filter(|s| crate::domain::plan::is_runnable_verify(s))
        });
    let done_when = t
        .acceptance
        .as_ref()
        .filter(|s| !crate::domain::plan::is_runnable_verify(s))
        .cloned()
        .or_else(|| super::humanize::parse_done_when(&t.prompt));
    let summary = super::humanize::human_summary(
        &t.title,
        &t.prompt,
        done_when.as_deref().or(t.acceptance.as_deref()),
    );
    let summary = if summary.is_empty() {
        first_line_summary(&t.prompt)
    } else {
        summary
    };
    let scope_paths = t
        .scope
        .as_ref()
        .map(|s| s.paths.clone())
        .unwrap_or_default();
    let role = t.role.map(|r| r.as_str().to_string());
    let mut meta = serde_json::Map::new();
    if let Some(g) = &t.group {
        meta.insert("group".into(), serde_json::json!(g));
    }
    if !t.tags.is_empty() {
        meta.insert("tags".into(), serde_json::json!(t.tags));
    }
    if !t.mode.is_empty() {
        meta.insert("mode".into(), serde_json::json!(t.mode));
    }
    if !t.outputs.is_empty() {
        meta.insert("outputs".into(), serde_json::json!(t.outputs));
    }
    if t.worktree.is_some() {
        meta.insert("worktree".into(), serde_json::json!(t.worktree));
    }
    if t.provider_opts != serde_json::json!({}) && !t.provider_opts.is_null() {
        meta.insert("provider_opts".into(), t.provider_opts.clone());
    }
    if let Some(s) = &t.scope {
        if !s.readonly.is_empty() {
            meta.insert("scope_readonly".into(), serde_json::json!(s.readonly));
        }
        if !s.forbid.is_empty() {
            meta.insert("scope_forbid".into(), serde_json::json!(s.forbid));
        }
    }
    CcoSplitTask {
        task_id: t.id.clone(),
        ord,
        title: super::humanize::display_title(&t.title),
        summary,
        body: t.prompt.clone(),
        depends_on: t.depends_on.clone(),
        wave: 0,
        enabled: if t.optional { t.include } else { true },
        optional: t.optional,
        done_when,
        verify_cmd,
        plan_ref: t.group.clone(),
        kind,
        status: CcoTaskStatus::Pending,
        provider: if t.provider.is_empty() {
            None
        } else {
            Some(t.provider.clone())
        },
        role,
        scope_paths,
        meta_json: if meta.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(meta))
        },
    }
}

/// Materialize PlanIR for Worker/Scheduler at confirm (hard path).
pub fn to_plan_ir(doc: &CcoSplitJob, default_provider: &str, default_mode: &str) -> PlanIR {
    let tasks: Vec<TaskIR> = doc
        .tasks
        .iter()
        .map(|t| task_to_ir(t, default_provider, default_mode))
        .collect();
    PlanIR {
        schema: "cco-plan/v1".into(),
        name: doc.title.clone(),
        adapter: format!("cco-split/{}", doc.source.as_str()),
        source_path: doc.plan_path.clone(),
        max_parallel: doc.max_parallel.max(1),
        on_failure: OnFailure::Pause,
        retry_max: 1,
        default_provider: default_provider.to_string(),
        default_mode: default_mode.to_string(),
        worktree: false,
        require_inspect: false,
        tasks,
    }
}

fn task_to_ir(t: &CcoSplitTask, default_provider: &str, default_mode: &str) -> TaskIR {
    let meta = t.meta_json.as_ref();
    let group = meta
        .and_then(|m| m.get("group"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| t.plan_ref.clone());
    let tags: Vec<String> = meta
        .and_then(|m| m.get("tags"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mode = meta
        .and_then(|m| m.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or(default_mode)
        .to_string();
    let outputs: Vec<String> = meta
        .and_then(|m| m.get("outputs"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let worktree = meta
        .and_then(|m| m.get("worktree"))
        .and_then(|v| v.as_bool());
    let provider_opts = meta
        .and_then(|m| m.get("provider_opts"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let readonly: Vec<String> = meta
        .and_then(|m| m.get("scope_readonly"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let forbid: Vec<String> = meta
        .and_then(|m| m.get("scope_forbid"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let scope = if t.scope_paths.is_empty() && readonly.is_empty() && forbid.is_empty() {
        None
    } else {
        Some(TaskScope {
            paths: t.scope_paths.clone(),
            readonly,
            forbid,
        })
    };
    let role = t
        .role
        .as_deref()
        .and_then(TaskRole::parse)
        .or_else(|| match t.kind {
            CcoTaskKind::Check => Some(TaskRole::Inspect),
            _ => None,
        });
    let provider = t
        .provider
        .clone()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| default_provider.to_string());

    // W2-3: worker scaffold must not be the first line the executor sees.
    let prompt = {
        let stripped = super::humanize::strip_worker_scaffold(&t.body);
        if stripped.trim().is_empty() {
            t.body.clone()
        } else {
            stripped
        }
    };
    TaskIR {
        id: t.task_id.clone(),
        title: t.title.clone(),
        depends_on: t.depends_on.clone(),
        group,
        provider,
        mode,
        prompt,
        // H2: verify_cmd is the only shell slot; acceptance kept for wire compat
        // (same runnable value) so old tools still see a command when present.
        verify_cmd: t
            .verify_cmd
            .as_ref()
            .filter(|s| crate::domain::plan::is_runnable_verify(s))
            .cloned()
            .or_else(|| {
                t.done_when
                    .as_ref()
                    .filter(|s| crate::domain::plan::is_runnable_verify(s))
                    .cloned()
            }),
        acceptance: t
            .verify_cmd
            .as_ref()
            .filter(|s| crate::domain::plan::is_runnable_verify(s))
            .cloned()
            .or_else(|| {
                t.done_when
                    .as_ref()
                    .filter(|s| crate::domain::plan::is_runnable_verify(s))
                    .cloned()
            }),
        timeout_secs: None,
        worktree,
        provider_opts,
        optional: t.optional,
        include: t.enabled,
        role,
        scope,
        outputs,
        tags,
    }
}
