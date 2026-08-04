//! Read / write plan prose under project (save_plan binds session draft; **no** worker spawn).
//! W2: [`chat_save_wave_bundle`] lands wave-index + N plans — still **no** confirm/start_run.

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::domain::chat::{
    extract_all_plan_fences, extract_title_from_md, extract_wave_index_fence,
    normalize_plan_markdown, sanitize_plan_title, DEFAULT_SESSION,
};

use super::paths::resolve_project_plan_file;
use super::session::{chat_session_get, save_session};
use super::types::{ChatDraftPlan, ChatSavePlanResponse, ChatSaveWaveResponse};

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
        let project_canon = project
            .canonicalize()
            .unwrap_or_else(|_| project.to_path_buf());
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

/// W2: from assistant prose, save optional ```wave-index + all ```plan fences.
///
/// Layout: `plans/wave-YYYYMMDD-HHMM/INDEX.md` + `01-….md` …
/// Single plan only → still writes one file under the wave dir (or falls back
/// to [`chat_save_plan`] when there is no index and exactly one plan — caller
/// may prefer that). **Never** spawns workers / confirm_start.
///
/// `markdown` may be the full assistant reply (fences extracted) or raw bodies.
pub fn chat_save_wave_bundle(
    project: &Path,
    session_id: Option<&str>,
    markdown: &str,
    plans_dir: Option<&str>,
) -> Result<ChatSaveWaveResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let raw = markdown.trim();
    if raw.is_empty() {
        bail!("empty wave markdown");
    }

    let index_body = extract_wave_index_fence(raw).map(|s| normalize_plan_markdown(&s));
    let mut plans: Vec<String> = extract_all_plan_fences(raw)
        .into_iter()
        .map(|s| normalize_plan_markdown(&s))
        .filter(|s| !s.trim().is_empty())
        .collect();

    // If no fences, treat whole text as one plan body (compat).
    if plans.is_empty() && index_body.is_none() {
        let one = normalize_plan_markdown(raw);
        if one.trim().is_empty() {
            bail!("no wave-index or plan fence found");
        }
        plans.push(one);
    }

    // Cap single wave (04): ≤7 execution plans.
    const MAX_PLANS: usize = 7;
    if plans.len() > MAX_PLANS {
        plans.truncate(MAX_PLANS);
    }

    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let mut sess = chat_session_get(project, Some(sid))?;
    let stamp = Utc::now().format("%Y%m%d-%H%M").to_string();

    let dir_rel = plans_dir
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("plans");
    if Path::new(dir_rel).is_absolute() || dir_rel.split(['/', '\\']).any(|p| p == "..") {
        bail!("invalid plans_dir: {dir_rel}");
    }
    let dir_rel = dir_rel.trim_matches('/').trim_matches('\\');
    let wave_name = format!("wave-{stamp}");
    let wave_rel = format!("{}/{}", dir_rel.replace('\\', "/"), wave_name);
    let wave_abs = project.join(dir_rel).join(&wave_name);
    let project_canon = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    if let Ok(pc) = wave_abs.canonicalize() {
        if !pc.starts_with(&project_canon) {
            bail!("wave dir escapes project root");
        }
    }
    std::fs::create_dir_all(&wave_abs)
        .with_context(|| format!("create wave dir {}", wave_abs.display()))?;

    let mut index_rel = None;
    if let Some(idx) = index_body.as_ref() {
        let body = ensure_h1(idx, &format!("本波索引 {stamp}"));
        let rel = format!("{wave_rel}/INDEX.md");
        let abs = wave_abs.join("INDEX.md");
        std::fs::write(&abs, &body).with_context(|| format!("write index {}", abs.display()))?;
        index_rel = Some(rel);
    }

    let mut plan_rels = Vec::new();
    for (i, plan_md) in plans.iter().enumerate() {
        let title = extract_title_from_md(plan_md)
            .map(|t| sanitize_plan_title(&t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("计划 {}", i + 1));
        let slug = slugify_plan_stem(&title);
        let file = format!("{:02}-{slug}.md", i + 1);
        let rel = format!("{wave_rel}/{file}");
        let abs = wave_abs.join(&file);
        let body = ensure_h1(plan_md, &title);
        std::fs::write(&abs, &body).with_context(|| format!("write plan {}", abs.display()))?;
        plan_rels.push(rel);
    }

    // Session draft points at first execution plan (or index if plans empty).
    let primary = plan_rels
        .first()
        .cloned()
        .or_else(|| index_rel.clone())
        .unwrap_or_else(|| wave_rel.clone());
    let primary_md = plans.first().cloned().or(index_body).unwrap_or_default();
    let primary_title =
        extract_title_from_md(&primary_md).or_else(|| Some(format!("本波 {stamp}")));
    sess.draft_plan = Some(ChatDraftPlan {
        path: primary,
        title: primary_title,
        markdown: Some(primary_md),
        saved: true,
    });
    save_session(project, &sess)?;

    let summary = match (index_rel.as_ref(), plan_rels.len()) {
        (Some(_), n) if n > 0 => format!("已保存本波索引 + {n} 份执行计划（未开跑）"),
        (Some(_), _) => "已保存本波索引（未开跑）".into(),
        (_, n) if n > 1 => format!("已保存 {n} 份执行计划（未开跑）"),
        (_, 1) => "已保存 1 份执行计划（未开跑）".into(),
        _ => "本波目录已创建（未开跑）".into(),
    };

    Ok(ChatSaveWaveResponse {
        index_rel,
        plan_rels,
        session_id: sess.session_id,
        summary,
    })
}

fn ensure_h1(md: &str, heading: &str) -> String {
    let md = md.trim();
    if md.starts_with('#') {
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
    }
}

/// File-stem slug from plan title (ASCII-ish; CJK kept as short hash fallback).
fn slugify_plan_stem(title: &str) -> String {
    let t = title.trim();
    let mut s = String::new();
    for ch in t.chars().take(40) {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' || ch == ' ' || ch == '·' {
            if !s.ends_with('-') {
                s.push('-');
            }
        } else if ch > '\u{7f}' {
            // keep a few CJK as codepoints for uniqueness without path bombs
            s.push('p');
        }
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "plan".into()
    } else {
        s
    }
}
