//! Read / write plan prose under project (save_plan binds session draft; **no** worker spawn).

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::domain::chat::{
    extract_title_from_md, normalize_plan_markdown, sanitize_plan_title, DEFAULT_SESSION,
};

use super::paths::resolve_project_plan_file;
use super::session::{chat_session_get, save_session};
use super::types::{ChatDraftPlan, ChatSavePlanResponse};

/// Read plan document text (markdown / yaml prose) for App 内全文预览。
/// Does **not** parse into PlanIR — just file bytes as UTF-8 text.
pub fn read_plan_md(project: &Path, plan_rel: &str) -> Result<String> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let (_rel, abs) = resolve_project_plan_file(project, plan_rel)?;
    if !abs.is_file() {
        bail!("plan file not found: {}", abs.display());
    }
    std::fs::read_to_string(&abs).with_context(|| format!("read plan {}", abs.display()))
}

/// Write markdown plan under project plans/ (or root), bind to session draft_plan.
///
/// - `plan_rel = None` → create a new `{plans_dir}/chat-YYYYMMDD-HHMM.md` (default `plans/`).
/// - `plan_rel = Some(path)` → **overwrite** that project-relative file (H1 未执行可改).
/// - `plans_dir` (G1): project-relative dir for new files only; must stay under project.
pub fn chat_save_plan(
    project: &Path,
    session_id: Option<&str>,
    title: Option<&str>,
    markdown: &str,
    plan_rel: Option<&str>,
    plans_dir: Option<&str>,
) -> Result<ChatSavePlanResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let md_raw = markdown.trim();
    if md_raw.is_empty() {
        bail!("empty plan markdown");
    }
    // G0: recover single-line walls + unify newlines before title extract / write.
    let md = normalize_plan_markdown(md_raw);
    let md = md.trim();
    if md.is_empty() {
        bail!("empty plan markdown");
    }
    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let mut sess = chat_session_get(project, Some(sid))?;

    let stamp = Utc::now().format("%Y%m%d-%H%M").to_string();
    let (rel, abs) = if let Some(existing) = plan_rel.map(str::trim).filter(|s| !s.is_empty()) {
        let (rel, abs) = resolve_project_plan_file(project, existing)?;
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create plan parent {}", parent.display()))?;
        }
        (rel, abs)
    } else {
        let dir_rel = plans_dir
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("plans");
        // Sanitize: no abs path, no `..`
        if Path::new(dir_rel).is_absolute() || dir_rel.split(['/', '\\']).any(|p| p == "..") {
            bail!("invalid plans_dir: {dir_rel}");
        }
        let dir_rel = dir_rel.trim_matches('/').trim_matches('\\');
        let plans_path = project.join(dir_rel);
        // Ensure under project
        let project_canon = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
        if let Ok(pc) = plans_path.canonicalize() {
            if !pc.starts_with(&project_canon) {
                bail!("plans_dir escapes project root");
            }
        }
        if plans_path.is_dir() || std::fs::create_dir_all(&plans_path).is_ok() {
            let name = format!("chat-{stamp}.md");
            let abs = plans_path.join(&name);
            let rel = format!("{}/{}", dir_rel.replace('\\', "/"), name);
            (rel, abs)
        } else {
            let name = format!("cco-plan-{stamp}.md");
            let abs = project.join(&name);
            (name, abs)
        }
    };

    let heading = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(sanitize_plan_title)
        .filter(|t| !t.is_empty())
        .or_else(|| extract_title_from_md(md))
        .unwrap_or_else(|| format!("聊天生成计划 {stamp}"));

    let body = if md.starts_with('#') {
        // Ensure H1 uses short heading when the extracted title is cleaner
        let mut out = md.to_string();
        if let Some(first_line_end) = out.find('\n') {
            let first = out[..first_line_end].trim();
            if first.starts_with("# ") {
                let rest = &out[first_line_end..];
                out = format!("# {heading}{rest}");
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        } else if out.starts_with("# ") {
            out = format!("# {heading}\n");
        }
        out
    } else {
        format!("# {heading}\n\n{md}\n")
    };

    std::fs::write(&abs, &body).with_context(|| format!("write plan {}", abs.display()))?;

    sess.draft_plan = Some(ChatDraftPlan {
        path: rel.clone(),
        title: Some(heading),
        markdown: Some(body),
        saved: true,
    });
    save_session(project, &sess)?;

    Ok(ChatSavePlanResponse {
        plan_rel: rel,
        abs_path: abs.display().to_string(),
        session_id: sess.session_id,
    })
}
