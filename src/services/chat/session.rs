//! Chat session load/list/new/delete + TTL cleanup (`.cco/chat/*.json`).

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::domain::chat::{sanitize_session_id, truncate_chars, DEFAULT_SESSION};

use super::paths::{chat_dir, session_path};
use super::types::{ChatSession, ChatSessionSummary};

/// G3: chat session JSON retention (hours). 0 = disable TTL cleanup.
pub(crate) const DEFAULT_CHAT_RETENTION_HOURS: i64 = 48;

pub(crate) fn empty_session(project: &Path, session_id: &str) -> ChatSession {
    ChatSession {
        session_id: session_id.to_string(),
        project: project.display().to_string(),
        messages: vec![],
        draft_plan: None,
        updated_at: None,
        title: None,
    }
}

fn preview_from_session(sess: &ChatSession) -> Option<String> {
    if let Some(t) = sess.title.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return Some(truncate_chars(t, 48));
    }
    if let Some(dt) = sess
        .draft_plan
        .as_ref()
        .and_then(|d| d.title.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Some(truncate_chars(dt, 48));
    }
    for m in &sess.messages {
        if m.role == "user" {
            let line = m
                .content
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("");
            if !line.is_empty() {
                return Some(truncate_chars(line, 48));
            }
        }
    }
    None
}

fn summary_from_session(sess: &ChatSession) -> ChatSessionSummary {
    ChatSessionSummary {
        session_id: sess.session_id.clone(),
        title: sess.title.clone(),
        updated_at: sess.updated_at.clone(),
        message_count: sess.messages.len(),
        preview: preview_from_session(sess),
        draft_plan_path: sess
            .draft_plan
            .as_ref()
            .filter(|d| d.saved && !d.path.is_empty())
            .map(|d| d.path.clone()),
        draft_plan_title: sess.draft_plan.as_ref().and_then(|d| d.title.clone()),
    }
}

/// Normalize plan body for identity compare (trim + collapse trailing spaces).
fn norm_plan_key(md: &str) -> String {
    md.replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Drop stale plan_rel when draft body no longer matches the bound file.
/// Heals sessions polluted by older builds that kept path/saved across new fences.
fn heal_stale_draft_binding(project: &Path, sess: &mut ChatSession) -> bool {
    let Some(draft) = sess.draft_plan.as_mut() else {
        return false;
    };
    if !draft.saved || draft.path.is_empty() {
        return false;
    }
    let Some(md) = draft.markdown.as_ref().map(|s| s.as_str()) else {
        // Saved path but no body in session — keep binding (preview may load from disk).
        return false;
    };
    let abs = project.join(draft.path.trim_start_matches('/'));
    let disk = match std::fs::read_to_string(&abs) {
        Ok(t) => t,
        Err(_) => {
            // Missing file → unbind so next save creates a new chat-*.md.
            draft.path.clear();
            draft.saved = false;
            return true;
        }
    };
    if norm_plan_key(&disk) != norm_plan_key(md) {
        // Body is a different plan than the bound path (classic pilotdeck hang).
        draft.path.clear();
        draft.saved = false;
        return true;
    }
    false
}

/// Load chat session from disk; missing → empty default session.
/// G3: opportunistically purge sessions older than retention hours.
pub fn chat_session_get(project: &Path, session_id: Option<&str>) -> Result<ChatSession> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let _ = cleanup_expired_chat_sessions(project, DEFAULT_CHAT_RETENTION_HOURS);
    let sid = sanitize_session_id(session_id.unwrap_or(DEFAULT_SESSION));
    let path = session_path(project, &sid);
    if !path.is_file() {
        return Ok(empty_session(project, &sid));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read chat session {}", path.display()))?;
    let mut sess: ChatSession = serde_json::from_str(&text)
        .with_context(|| format!("parse chat session {}", path.display()))?;
    sess.session_id = sid;
    sess.project = project.display().to_string();
    if heal_stale_draft_binding(project, &mut sess) {
        // Persist healed identity so UI reload stays clean.
        if let Err(e) = save_session(project, &sess) {
            tracing::warn!(error = %e, "chat: heal stale draft binding save failed");
        }
    }
    Ok(sess)
}

/// C3: list chat sessions under `.cco/chat/*.json` (newest first).
/// Always includes a synthetic `default` row when no files exist.
pub fn chat_list_sessions(project: &Path) -> Result<Vec<ChatSessionSummary>> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let _ = cleanup_expired_chat_sessions(project, DEFAULT_CHAT_RETENTION_HOURS);
    let dir = chat_dir(project);
    let mut out: Vec<ChatSessionSummary> = Vec::new();
    if dir.is_dir() {
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("read chat dir {}", dir.display()))?;
        for ent in entries.flatten() {
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) if !s.is_empty() && s != "_work" => s.to_string(),
                _ => continue,
            };
            match chat_session_get(project, Some(&stem)) {
                Ok(sess) => out.push(summary_from_session(&sess)),
                Err(_) => {
                    // Corrupt file: still list by id so UI can delete.
                    out.push(ChatSessionSummary {
                        session_id: stem,
                        title: None,
                        updated_at: None,
                        message_count: 0,
                        preview: Some("(无法读取)".into()),
                        draft_plan_path: None,
                        draft_plan_title: None,
                    });
                }
            }
        }
    }
    // Ensure default is always present so the switcher has a home base.
    if !out.iter().any(|s| s.session_id == DEFAULT_SESSION) {
        out.push(summary_from_session(&empty_session(project, DEFAULT_SESSION)));
    }
    out.sort_by(|a, b| {
        // default first when both empty of times; else newest updated_at first
        match (&b.updated_at, &a.updated_at) {
            (Some(bu), Some(au)) => bu.cmp(au),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                if a.session_id == DEFAULT_SESSION {
                    std::cmp::Ordering::Less
                } else if b.session_id == DEFAULT_SESSION {
                    std::cmp::Ordering::Greater
                } else {
                    a.session_id.cmp(&b.session_id)
                }
            }
        }
    });
    Ok(out)
}

/// C3: create a new empty session with a unique id (`s-YYYYMMDD-HHMMSS` + suffix if clash).
/// Optional `title` is stored on the session for the switcher label.
pub fn chat_new_session(project: &Path, title: Option<&str>) -> Result<ChatSession> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let mut base = format!("s-{stamp}");
    // Rare same-second clash: append -2, -3, ...
    let mut n = 1u32;
    while session_path(project, &base).is_file() {
        n += 1;
        base = format!("s-{stamp}-{n}");
        if n > 50 {
            bail!("could not allocate unique session id");
        }
    }
    let mut sess = empty_session(project, &base);
    if let Some(t) = title.map(str::trim).filter(|s| !s.is_empty()) {
        sess.title = Some(truncate_chars(t, 80));
    }
    save_session(project, &sess)?;
    // Re-read so updated_at matches disk.
    chat_session_get(project, Some(&base))
}

/// C3: delete a session JSON (+ best-effort attachments dir).
/// Refuses empty / unsafe ids. Deleting `default` is allowed (next get returns empty).
pub fn chat_delete_session(project: &Path, session_id: &str) -> Result<()> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let sid = sanitize_session_id(session_id);
    if sid.is_empty() {
        bail!("session_id is empty");
    }
    let path = session_path(project, &sid);
    if path.is_file() {
        std::fs::remove_file(&path)
            .with_context(|| format!("delete chat session {}", path.display()))?;
    }
    let att = chat_dir(project).join("attachments").join(&sid);
    if att.is_dir() {
        let _ = std::fs::remove_dir_all(att);
    }
    Ok(())
}

/// G3: delete `.cco/chat/*.json` whose `updated_at` (or file mtime) is older than `hours`.
/// Returns number of files removed. `hours <= 0` → no-op.
pub fn cleanup_expired_chat_sessions(project: &Path, hours: i64) -> Result<usize> {
    if hours <= 0 {
        return Ok(0);
    }
    let dir = chat_dir(project);
    if !dir.is_dir() {
        return Ok(0);
    }
    let cutoff = Utc::now() - chrono::Duration::hours(hours);
    let mut removed = 0usize;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        // Never delete work dir artifacts
        if path.file_name().and_then(|s| s.to_str()) == Some("_work") {
            continue;
        }
        let expired = if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(at) = v.get("updated_at").and_then(|x| x.as_str()) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(at) {
                        dt.with_timezone(&Utc) < cutoff
                    } else {
                        false
                    }
                } else {
                    // no updated_at → use mtime
                    ent.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<Utc> = t.into();
                            dt < cutoff
                        })
                        .unwrap_or(false)
                }
            } else {
                false
            }
        } else {
            false
        };
        if expired {
            if std::fs::remove_file(&path).is_ok() {
                removed += 1;
                // Best-effort: drop sibling attachments dir named like session stem
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let att = dir.join("attachments").join(stem);
                    let _ = std::fs::remove_dir_all(att);
                }
            }
        }
    }
    Ok(removed)
}

pub(crate) fn save_session(project: &Path, sess: &ChatSession) -> Result<()> {
    let dir = chat_dir(project);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create chat dir {}", dir.display()))?;
    let path = session_path(project, &sess.session_id);
    let mut out = sess.clone();
    out.updated_at = Some(Utc::now().to_rfc3339());
    out.project = project.display().to_string();
    let json = serde_json::to_string_pretty(&out)?;
    std::fs::write(&path, json).with_context(|| format!("write chat session {}", path.display()))?;
    Ok(())
}
