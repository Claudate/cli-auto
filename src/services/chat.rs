//! Chat → plan document builder (desktop Mode B front-door).
//!
//! [INPUT]: project path · user message · optional session_id · Config (provider bin)
//! [OUTPUT]: chat_session_get · chat_list_sessions · chat_new_session · chat_delete_session · chat_send · chat_save_plan · read_plan_md · session JSON under .cco/chat/
//! [POS]: services 子模块；只写散文 .md，不 spawn worker / 不走 confirm_start
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//! note: empty CLI reply → soft human note (no plan fence); CCO_CHAT_FAKE keeps template fence
//! note: non-zero CLI exit still yields text when stream has assistant prose (max_turns etc.)
//! note: ChatSendResponse.env_note for UI system bar (diagnostics never in assistant body)
//! note: extract_plan_fence / history truncate 必须 char-boundary 安全（CJK 禁字节硬切）
//! note: extract_plan_fence 嵌套 fence 按行首 depth 计数（```text 图示不得截断 ```plan）
//! note: chat_save_plan 可选 plan_rel 覆盖已有未执行计划；read_plan_md 供右轨全文 modal
//! note: G0 plan 标题截断（H1 遇 ## / 最长 80 字）+ 写盘换行规范化；G0b 可选 CLI 再整理
//! note: G4 chat_save_attachment · ChatMessage.attachments；chat_save_plan 可选 plans_dir
//! note: C3 多会话：list/new/delete + 可选 title；默认 session_id=default 仍兼容
//! note: C3 流式 partial：chat_stream_partial 读 stdout 增量；失败降级整段 reply（不 panic）

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::plan::TaskIR;
use crate::runtime::provider::{
    claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
};

const DEFAULT_SESSION: &str = "default";
const MAX_HISTORY_MSGS: usize = 24;
const MAX_MSG_CHARS: usize = 12_000;
/// G3: chat session JSON retention (hours). 0 = disable TTL cleanup.
const DEFAULT_CHAT_RETENTION_HOURS: i64 = 48;
/// G4: max attachment bytes (5 MiB).
const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
/// G4: max images per message.
const MAX_ATTACHMENTS_PER_MSG: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachment {
    /// Project-relative path (e.g. .cco/chat/attachments/default/uuid.png)
    pub path: String,
    pub mime: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatDraftPlan {
    /// Relative to project root (e.g. plans/chat-20260718-1530.md)
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Markdown body ready to save (from ```plan fence); not yet on disk unless path set after save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default)]
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub session_id: String,
    pub project: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan: Option<ChatDraftPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// C3: optional human label for multi-session switcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// C3: lightweight row for multi-session list (no full messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionSummary {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub message_count: usize,
    /// Short preview: first user line or draft title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSendResponse {
    pub session_id: String,
    pub reply: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_plan: Option<ChatDraftPlan>,
    /// true when reply came from fake template (no CLI / forced)
    pub fake: bool,
    /// Short human-readable env/CLI fault for UI system bar (not assistant body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSavePlanResponse {
    pub plan_rel: String,
    pub abs_path: String,
    pub session_id: String,
}

fn chat_dir(project: &Path) -> PathBuf {
    project.join(".cco").join("chat")
}

fn session_path(project: &Path, session_id: &str) -> PathBuf {
    let safe = sanitize_session_id(session_id);
    chat_dir(project).join(format!("{safe}.json"))
}

fn empty_session(project: &Path, session_id: &str) -> ChatSession {
    ChatSession {
        session_id: session_id.to_string(),
        project: project.display().to_string(),
        messages: vec![],
        draft_plan: None,
        updated_at: None,
        title: None,
    }
}

/// Sanitize session_id to filesystem-safe token (same rules as session_path).
fn sanitize_session_id(session_id: &str) -> String {
    let safe: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        DEFAULT_SESSION.to_string()
    } else {
        safe
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
        draft_plan_title: sess
            .draft_plan
            .as_ref()
            .and_then(|d| d.title.clone()),
    }
}

fn allowed_image_mime(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif"
    )
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

/// G4: write one image under `.cco/chat/attachments/<session>/`.
pub fn chat_save_attachment(
    project: &Path,
    session_id: Option<&str>,
    file_name: &str,
    mime: &str,
    data: &[u8],
) -> Result<ChatAttachment> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    if !allowed_image_mime(mime) {
        bail!("unsupported image type: {mime} (use png/jpeg/webp/gif)");
    }
    if data.is_empty() {
        bail!("empty attachment");
    }
    if data.len() > MAX_ATTACHMENT_BYTES {
        bail!(
            "attachment too large (max {} MB)",
            MAX_ATTACHMENT_BYTES / (1024 * 1024)
        );
    }
    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let safe_sid: String = sid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let ext = ext_for_mime(mime);
    let stamp = Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let base = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let safe_base: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let safe_base = if safe_base.is_empty() {
        "image".into()
    } else {
        safe_base
    };
    let file = format!("{safe_base}-{stamp}.{ext}");
    let rel = format!(".cco/chat/attachments/{safe_sid}/{file}");
    let abs = project.join(&rel);
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create attachment dir {}", parent.display()))?;
    }
    std::fs::write(&abs, data).with_context(|| format!("write attachment {}", abs.display()))?;
    let display_name = {
        let n = file_name.trim();
        if n.is_empty() {
            file.clone()
        } else {
            Path::new(n)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(file.clone())
        }
    };
    Ok(ChatAttachment {
        path: rel,
        mime: mime.trim().to_ascii_lowercase(),
        name: display_name,
    })
}

fn format_attachments_block(atts: &[ChatAttachment]) -> String {
    if atts.is_empty() {
        return String::new();
    }
    let mut lines = vec!["\n\n--- 附图（项目相对路径，请结合图片理解需求）---".to_string()];
    for (i, a) in atts.iter().enumerate() {
        lines.push(format!("{}. {} ({}) → {}", i + 1, a.name, a.mime, a.path));
    }
    lines.join("\n")
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
        let entries = std::fs::read_dir(&dir)
            .with_context(|| format!("read chat dir {}", dir.display()))?;
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

fn save_session(project: &Path, sess: &ChatSession) -> Result<()> {
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

/// One round-trip: append user message, call Claude print (or fake), append assistant.
/// G4: `attachments` are project-relative paths already saved via `chat_save_attachment`.
pub fn chat_send(
    config: &Config,
    project: &Path,
    message: &str,
    session_id: Option<&str>,
    attachments: Option<Vec<ChatAttachment>>,
) -> Result<ChatSendResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let msg = message.trim();
    let atts: Vec<ChatAttachment> = attachments.unwrap_or_default();
    if atts.len() > MAX_ATTACHMENTS_PER_MSG {
        bail!("too many attachments (max {MAX_ATTACHMENTS_PER_MSG})");
    }
    for a in &atts {
        if !allowed_image_mime(&a.mime) {
            bail!("unsupported attachment mime: {}", a.mime);
        }
        let abs = project.join(&a.path);
        if !abs.is_file() {
            bail!("attachment missing on disk: {}", a.path);
        }
        // Must stay under project
        let canon_proj = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
        if let Ok(canon_f) = abs.canonicalize() {
            if !canon_f.starts_with(&canon_proj) {
                bail!("attachment path escapes project: {}", a.path);
            }
        }
    }
    if msg.is_empty() && atts.is_empty() {
        bail!("empty message");
    }
    if msg.chars().count() > MAX_MSG_CHARS {
        bail!("message too long (max {MAX_MSG_CHARS} chars)");
    }

    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let mut sess = chat_session_get(project, Some(sid))?;
    let now = Utc::now().to_rfc3339();
    let user_content = if atts.is_empty() {
        msg.to_string()
    } else if msg.is_empty() {
        format!("（见附图）{}", format_attachments_block(&atts))
    } else {
        format!("{msg}{}", format_attachments_block(&atts))
    };
    sess.messages.push(ChatMessage {
        role: "user".into(),
        content: user_content,
        at: Some(now.clone()),
        attachments: atts.clone(),
    });
    // Persist the user turn *before* the long CLI call so leaving the chat page
    // and reloading still shows the question (reply may still be pending).
    if let Err(e) = save_session(project, &sess) {
        tracing::warn!(error = %e, "chat: early save of user message failed");
    }

    let force_fake = std::env::var("CCO_CHAT_FAKE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || config.default.default_provider.eq_ignore_ascii_case("fake");

    // force_fake (CCO_CHAT_FAKE / provider=fake): full template with ```plan for UI联调.
    // production soft-fallback: short human reply + env_note; **no** plan fence → 不点亮就绪分配.
    let (reply, used_fake, env_note) = if force_fake {
        (fake_chat_reply(msg, project), true, None)
    } else {
        match call_claude_chat(config, project, &sess) {
            Ok(r) => (r, false, None),
            Err(e) => {
                let diagnostic = e.to_string();
                tracing::warn!(
                    error = %diagnostic,
                    project = %project.display(),
                    "chat: soft-fallback (CLI unavailable or empty reply)"
                );
                let env = soft_fallback_env_note(&diagnostic);
                let human = soft_fallback_assistant_reply();
                (human, true, Some(env))
            }
        }
    };

    // Only extract plan fence for real AI or forced fake-template联调.
    // Production soft-fallback has no fence; do not fabricate draft_plan.
    let draft_from_reply = if used_fake && env_note.is_some() {
        None
    } else {
        extract_plan_fence(&reply)
    };
    if let Some(md) = draft_from_reply {
        // G0/G0b local: break single-line walls + ensure basic section skeleton
        let md = structure_plan_markdown(&normalize_plan_markdown(&md));
        // Optional second CLI pass (expensive): CCO_CHAT_AUTO_NORMALIZE=1
        let md = if !force_fake
            && std::env::var("CCO_CHAT_AUTO_NORMALIZE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
        {
            match chat_normalize_plan(config, project, &md, None) {
                Ok(r) => r.markdown,
                Err(e) => {
                    tracing::warn!(error = %e, "chat: auto normalize failed; keep local structure");
                    md
                }
            }
        } else {
            md
        };
        let title = extract_title_from_md(&md);
        let mut draft = sess.draft_plan.take().unwrap_or_default();
        draft.markdown = Some(md);
        draft.title = title.or(draft.title);
        // Not saved until chat_save_plan
        draft.saved = draft.saved && !draft.path.is_empty();
        if draft.path.is_empty() {
            draft.saved = false;
        }
        sess.draft_plan = Some(draft);
    }

    sess.messages.push(ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
        at: Some(Utc::now().to_rfc3339()),
        attachments: vec![],
    });
    // Cap history
    if sess.messages.len() > MAX_HISTORY_MSGS {
        let drop_n = sess.messages.len() - MAX_HISTORY_MSGS;
        sess.messages.drain(0..drop_n);
    }
    save_session(project, &sess)?;

    Ok(ChatSendResponse {
        session_id: sess.session_id.clone(),
        reply,
        messages: sess.messages.clone(),
        draft_plan: sess.draft_plan.clone(),
        fake: used_fake,
        env_note,
    })
}

/// C3 streaming partial: best-effort assistant text while `chat_send` is still running.
/// Reads the same `__chat__` stdout file that `call_claude_chat` writes; never panics on
/// truncated NDJSON / CJK mid-rune (uses char-safe extract). Empty when idle or unavailable.
pub fn chat_stream_partial(project: &Path, session_id: Option<&str>) -> Result<ChatStreamPartial> {
    let _ = session_id; // reserved: multi-session work dirs stay shared under __chat__ for now
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let task_dir = project
        .join(".cco")
        .join("chat")
        .join("_work")
        .join("tasks")
        .join("__chat__");
    // Prefer live NDJSON; fall back to stdout.json (provider may rename).
    let candidates = [
        task_dir.join("stdout.raw.ndjson"),
        task_dir.join("stdout.json"),
    ];
    let mut raw = String::new();
    for p in &candidates {
        if p.is_file() {
            if let Ok(s) = std::fs::read_to_string(p) {
                if s.len() >= raw.len() {
                    raw = s;
                }
            }
        }
    }
    let done = task_dir.join(".done").is_file();
    // extract_assistant_text is already char-boundary safe and tolerates partial lines.
    let text = if raw.trim().is_empty() {
        String::new()
    } else {
        extract_assistant_text(&raw)
    };
    Ok(ChatStreamPartial {
        text,
        done,
        bytes: raw.len() as u64,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatStreamPartial {
    /// Accumulated assistant prose so far (may be empty while CLI starts).
    pub text: String,
    /// True when worker left a `.done` marker (turn finished or aborted).
    pub done: bool,
    /// Raw stdout bytes observed (for UI "still growing" hint).
    pub bytes: u64,
}

/// Resolve a project-relative plan path; reject `..` / absolute escape.
fn resolve_project_plan_file(project: &Path, plan_rel: &str) -> Result<(String, PathBuf)> {
    let rel = plan_rel.trim().trim_start_matches('/');
    if rel.is_empty() {
        bail!("empty plan path");
    }
    if rel.contains("..") {
        bail!("plan path must not contain '..'");
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        bail!("plan path must be project-relative");
    }
    let project_canon = project
        .canonicalize()
        .with_context(|| format!("canonicalize project {}", project.display()))?;
    let abs = project_canon.join(rel_path);
    // Ensure still under project (even without existing file).
    if let Some(parent) = abs.parent().map(|p| {
        if p.exists() {
            p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
        } else {
            p.to_path_buf()
        }
    }) {
        let parent_s = parent.to_string_lossy();
        let root_s = project_canon.to_string_lossy();
        if parent_s != root_s && !parent_s.starts_with(root_s.as_ref()) {
            // Best-effort; also check prefix with separator
            let prefix = format!(
                "{}{}",
                root_s,
                std::path::MAIN_SEPARATOR
            );
            if !parent_s.starts_with(&prefix) && parent != project_canon {
                bail!("plan path escapes project root");
            }
        }
    }
    let rel_norm = rel.replace('\\', "/");
    Ok((rel_norm, abs))
}

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
        if Path::new(dir_rel).is_absolute()
            || dir_rel.split(['/', '\\']).any(|p| p == "..")
        {
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
        .map(|t| sanitize_plan_title(t))
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

fn system_prompt(project: &Path) -> String {
    format!(
        r#"你是 cco 桌面应用里的「计划写作助手」。用户在项目目录中与你对话，目标是共建一份可执行的**计划文档**（Markdown 散文/大纲），不是直接写代码或执行任务。

项目路径：{}

职责：
1. 用简短中文澄清：目标、范围、约束、验收标准、风险。
2. 当信息足够，或用户要求「生成计划/收口/写计划」时，输出完整 Markdown 计划。
3. 计划正文必须用下面 fence 包起来（便于应用解析预填；用户仍需点「保存」才会落盘）：

```plan
# 计划标题
## 目标
…
## 范围
…
## 任务大纲
1. …
2. …
## 验收
- …
```

硬规则：
- **不要**输出 cco-plan/v1 JSON 或任务图 JSON（那是后续「分配计划」阶段 Planner 的事）。
- **不要**假装已经执行了任务；你只写计划文档。
- 日常澄清轮可先不写 fence；收口轮务必带 ```plan。
- 保持简洁，优先可分配、可拆分的任务大纲。"#,
        project.display()
    )
}

fn build_user_prompt(sess: &ChatSession, project: &Path) -> String {
    let mut parts = vec![system_prompt(project)];
    parts.push("\n\n--- 对话历史 ---\n".into());
    for m in &sess.messages {
        let role = match m.role.as_str() {
            "assistant" => "助手",
            "system" => "系统",
            _ => "用户",
        };
        let content = truncate_chars(&m.content, 4000);
        parts.push(format!("\n[{role}]\n{content}\n"));
    }
    parts.push(
        "\n请根据最新用户消息回复。若应输出计划，请使用 ```plan 代码块。\n".into(),
    );
    parts.join("")
}

/// Char-count truncate (never mid-rune). Appends `…` when shortened.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max_chars).collect::<String>())
}

fn call_claude_chat(config: &Config, project: &Path, sess: &ChatSession) -> Result<String> {
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = project.join(".cco").join("chat").join("_work");
    let task_dir = work.join("tasks").join("__chat__");
    std::fs::create_dir_all(&task_dir)?;
    // Defense-in-depth: provider.start also clears this; chat reuses a fixed dir.
    let _ = std::fs::remove_file(task_dir.join(".done"));
    // Drop prior stream so collect cannot pick up a truncated previous turn.
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");

    let prompt = build_user_prompt(sess, project);
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;

    let chat_task = TaskIR {
        id: "__chat__".into(),
        title: "plan chat".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        acceptance: None,
        // Wall-clock only (process timeout). Chat must NOT pass --max-turns /
        // --max-budget-usd: null omits those flags so Claude is not turn-capped.
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            // null = omit CLI limit flags (see ClaudeProvider::opt_limit_*).
            "max_turns": null,
            "max_budget_usd": null,
            "permission_mode": "dontAsk",
            // No allowed_tools key → CLI default tools (Read/Bash/Edit…), scope-locked
            // via --append-system-prompt. Empty [] used to pass --allowedTools "" which
            // Claude 2.1.x still seeds with defaults and then hits error_max_turns at 2.
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
    };

    let ctx = StartCtx {
        run_id: format!("chat-{}", sess.session_id),
        project_root: project.to_path_buf(),
        work_dir: project.to_path_buf(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for chat")?;

    // Match timeout_secs (~10 min) at 400ms poll interval + small slack.
    const MAX_POLL_TICKS: u32 = 1_600;

    let raw_out = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&chat_task)?;
        let handle = provider.start(&chat_task, &ctx).await?;
        let mut ticks = 0u32;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    ticks += 1;
                    if ticks > MAX_POLL_TICKS {
                        bail!("chat Claude CLI timeout");
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => break,
            }
        }
        let result = provider.collect(&handle).await?;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        // Always keep a copy for "empty reply" post-mortems.
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), &stdout);
        // Chat is text product, not a task graph: non-zero exit (e.g. error_max_turns)
        // is fine when stream-json already has assistant prose. Soft-template only
        // when we truly have nothing usable.
        let text = extract_assistant_text(&stdout);
        if !text.trim().is_empty() {
            return Ok::<String, anyhow::Error>(stdout);
        }
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result.error.unwrap_or_else(|| "chat worker failed".into());
            let detail = stream_result_summary(&stdout);
            let snip: String = stdout.chars().take(240).collect();
            bail!("chat worker not done: {err}{detail} · stdout_snip={snip}");
        }
        Ok::<String, anyhow::Error>(stdout)
    })?;

    let text = extract_assistant_text(&raw_out);
    if text.trim().is_empty() {
        // Persist full raw so the user/doctor can open .cco/chat/_work/…
        let snip: String = raw_out.chars().take(280).collect();
        let detail = stream_result_summary(&raw_out);
        let _ = std::fs::write(
            project
                .join(".cco")
                .join("chat")
                .join("_work")
                .join("last_empty_reply.txt"),
            &raw_out,
        );
        bail!(
            "empty assistant reply from Claude CLI ({} bytes stdout{detail}; snip: {snip})",
            raw_out.len()
        );
    }
    Ok(text)
}

/// Summarize the terminal stream-json `result` line for diagnostics.
fn stream_result_summary(raw: &str) -> String {
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("result") {
            continue;
        }
        let subtype = v
            .get("subtype")
            .and_then(|s| s.as_str())
            .unwrap_or("result");
        let mut parts = vec![format!("subtype={subtype}")];
        if let Some(errs) = v.get("errors").and_then(|e| e.as_array()) {
            let joined: Vec<&str> = errs.iter().filter_map(|x| x.as_str()).collect();
            if !joined.is_empty() {
                parts.push(format!("errors={}", joined.join("; ")));
            }
        }
        if let Some(n) = v.get("num_turns").and_then(|x| x.as_u64()) {
            parts.push(format!("turns={n}"));
        }
        if let Some(sr) = v.get("stop_reason").and_then(|x| x.as_str()) {
            parts.push(format!("stop={sr}"));
        }
        return format!(" · {}", parts.join(", "));
    }
    String::new()
}

/// Extract human-readable assistant text from stream-json / plain stdout.
fn extract_assistant_text(raw: &str) -> String {
    // 1) Prefer last successful stream-json `result.result` (string or nested text).
    //    Error envelopes (error_max_turns / is_error) fall through to assistant prose.
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "result" {
            continue;
        }
        let is_err = v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false)
            || v.get("subtype")
                .and_then(|s| s.as_str())
                .is_some_and(|s| {
                    s.eq_ignore_ascii_case("error")
                        || s.starts_with("error_")
                        || s.eq_ignore_ascii_case("error_max_turns")
                        || s.eq_ignore_ascii_case("error_max_budget_usd")
                });
        if let Some(s) = v.get("result").and_then(|r| r.as_str()) {
            if !s.trim().is_empty() {
                return s.to_string();
            }
        }
        // Some builds nest the final text under content[].text on the result line.
        if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
            let mut parts = Vec::new();
            for part in content {
                if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                        if !t.trim().is_empty() {
                            parts.push(t.to_string());
                        }
                    }
                }
            }
            if !parts.is_empty() {
                return parts.join("\n");
            }
        }
        if is_err {
            // Fall through to assistant deltas / plain text below.
            break;
        }
    }

    // 2) Collect assistant message text (full blocks + streaming deltas).
    let mut block_texts: Vec<String> = Vec::new();
    let mut delta_buf = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "assistant" {
            // Flush any in-flight deltas before a full assistant block.
            if !delta_buf.trim().is_empty() {
                block_texts.push(std::mem::take(&mut delta_buf));
            }
            let mut parts = Vec::new();
            if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                for part in content {
                    if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                parts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
                for part in content {
                    if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                parts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            if !parts.is_empty() {
                block_texts.push(parts.join("\n"));
            }
        } else if ty == "content_block_delta" {
            if let Some(t) = v.pointer("/delta/text").and_then(|x| x.as_str()) {
                if !t.is_empty() {
                    delta_buf.push_str(t);
                }
            }
        } else if ty == "content_block_stop" || ty == "message_stop" {
            if !delta_buf.trim().is_empty() {
                block_texts.push(std::mem::take(&mut delta_buf));
            }
        }
    }
    if !delta_buf.trim().is_empty() {
        block_texts.push(delta_buf);
    }
    if !block_texts.is_empty() {
        // Prefer the longest complete prose block (final answer over short tool preambles).
        // Fall back to joining all blocks when none is clearly dominant.
        let best = block_texts
            .iter()
            .max_by_key(|s| s.chars().count())
            .cloned()
            .unwrap_or_default();
        if best.chars().count() >= 40 {
            return best;
        }
        // Short-only stream (e.g. max_turns cut mid-tool): return whatever we have.
        let joined = block_texts.join("\n\n");
        if !joined.trim().is_empty() {
            return joined;
        }
        return best;
    }

    // 3) Plain text fallback: strip pure-JSON NDJSON lines, keep non-JSON tails.
    let mut plain_parts: Vec<&str> = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('{') {
            // already handled as JSON above
            continue;
        }
        plain_parts.push(t);
    }
    if !plain_parts.is_empty() {
        return plain_parts.join("\n");
    }
    String::new()
}

/// Byte length of a markdown fence language tag at the start of `after`.
/// Tag is ASCII `[A-Za-z0-9_+-]*` only, so the returned index is always a char boundary.
fn fence_lang_tag_len(after: &str) -> usize {
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '+' || *c == '-')
        .map(|c| c.len_utf8())
        .sum()
}

/// True when `idx` is at the start of `s` or immediately after `\n` / `\r`.
/// Markdown fences are line-oriented; mid-line `` ` `` sequences are ignored.
fn is_line_start_fence(s: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    matches!(s.as_bytes().get(idx.saturating_sub(1)), Some(b'\n' | b'\r'))
}

/// Find the next line-start ``` fence at or after `from` (byte index into `s`).
/// Returns `None` if none remain. ``` is ASCII → returned index is a char boundary.
fn find_line_fence(s: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if from >= bytes.len() {
        return None;
    }
    let mut i = from;
    // If `from` is mid-line, jump to the next line first.
    if i > 0 && !matches!(bytes.get(i.saturating_sub(1)), Some(b'\n' | b'\r')) {
        if let Some(rel) = s[i..].find(|c| c == '\n' || c == '\r') {
            i += rel + 1;
            // handle \r\n
            if i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n' {
                i += 1;
            }
        } else {
            return None;
        }
    }
    while i < bytes.len() {
        if s[i..].starts_with("```") && is_line_start_fence(s, i) {
            return Some(i);
        }
        // next line
        if let Some(rel) = s[i..].find(|c| c == '\n' || c == '\r') {
            i += rel + 1;
            if i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n' {
                i += 1;
            }
        } else {
            break;
        }
    }
    None
}

/// Close a fence body starting at `body` (content after opener tag + newline trim).
/// Supports **nested** fenced blocks (```text / ``` / ```rust inside ```plan):
/// first-naive `body.find("```")` used to cut at the first nested opener and save a
/// truncated plan (desktop 2026-07-20: plans/chat-20260719-0902.md only ~120 chars).
///
/// Returns `(body_end_byte_in_body, absolute_scan_continue_from_body)` when closed.
fn close_fence_body(body: &str) -> Option<(usize, usize)> {
    let mut depth: i32 = 1;
    let mut pos = 0usize;
    while let Some(j) = find_line_fence(body, pos) {
        let after = &body[j + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        // Opening if tag non-empty (```text / ```rust / ```plan …);
        // closing if bare ``` (optional trailing spaces/newline only after tag).
        // Markdown: "```" alone closes; "```lang" opens nested.
        if !tag.is_empty() {
            depth += 1;
            // skip past this opener tag (and the rest of its line is body of nested)
            pos = j + 3 + tag_len;
        } else {
            depth -= 1;
            if depth == 0 {
                // end points at the closing ```; continue after it
                return Some((j, j + 3));
            }
            pos = j + 3;
        }
    }
    None
}

/// Pull ```plan … ``` body (last fence wins).
///
/// CJK-safe: never advances with a fixed byte offset into multi-byte runes.
/// Desktop crash (2026-07-19): `after[3..]` landed inside `若`/`和` after a plain
/// ``` fence → `chat_send` join panic.
///
/// Nested fences (2026-07-20): plan bodies often embed ```text diagrams; extraction
/// must nest-count line-start fences so the outer ```plan is not closed early.
pub fn extract_plan_fence(text: &str) -> Option<String> {
    let mut search = text;
    let mut best: Option<String> = None;
    while let Some(idx) = search.find("```") {
        // Only treat line-start ``` as a fence opener (skip mid-line triple-backticks).
        if !is_line_start_fence(search, idx) {
            search = &search[idx + 3..];
            continue;
        }
        // ``` is ASCII; idx and idx+3 are always char boundaries.
        let after = &search[idx + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        if tag.eq_ignore_ascii_case("plan") {
            let body = after[tag_len..]
                .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if let Some((end, cont)) = close_fence_body(body) {
                let block = body[..end].trim();
                if !block.is_empty() {
                    best = Some(block.to_string());
                }
                search = &body[cont..];
            } else {
                // Unclosed ```plan — stop; keep last complete if any.
                break;
            }
        } else {
            // Not a plan fence (plain / markdown / rust / …). Skip this opener;
            // jump past its closer (nesting-aware so ``` inside markdown fences is fine).
            let body = after[tag_len..]
                .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if let Some((_end, cont)) = close_fence_body(body) {
                search = &body[cont..];
            } else if let Some(end) = after.find("```") {
                // fallback: naive skip so we do not stall on odd shapes
                search = &after[end + 3..];
            } else {
                search = after;
            }
        }
    }
    best
}

/// Max chars for list / rail titles (G0). Longer H1s get ellipsis.
const PLAN_TITLE_MAX_CHARS: usize = 80;

/// Sanitize a raw H1 body into a short list title.
/// Cuts at embedded `##` (single-line "wall" plans) and clamps length.
pub fn sanitize_plan_title(raw: &str) -> String {
    let mut s = raw.trim();
    // Single-line dumps often jam "# Title## 目标…" — stop before next heading.
    if let Some(idx) = s.find("##") {
        s = s[..idx].trim_end();
    }
    // Also stop at an accidental second "# " mid-string (rare).
    if let Some(idx) = s.find("\n# ") {
        s = s[..idx].trim_end();
    }
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let count = s.chars().count();
    if count <= PLAN_TITLE_MAX_CHARS {
        return s.to_string();
    }
    format!("{}…", s.chars().take(PLAN_TITLE_MAX_CHARS).collect::<String>())
}

/// Extract short plan title from markdown (H1). Safe for no-newline walls (G0).
pub fn extract_title_from_md(md: &str) -> Option<String> {
    for line in md.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = sanitize_plan_title(rest);
            if !title.is_empty() {
                return Some(title);
            }
        } else if let Some(rest) = t.strip_prefix("#") {
            // "#Title" without space
            let rest = rest.trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                let title = sanitize_plan_title(rest);
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }
    // Whole file may be one line with no \n — lines() still yields it once.
    None
}

/// Normalize plan markdown before disk write (G0).
/// - Unify newlines
/// - If essentially one line, insert breaks before `##` / `###` headings
pub fn normalize_plan_markdown(md: &str) -> String {
    let mut s = md.replace("\r\n", "\n").replace('\r', "\n");
    let nl = s.matches('\n').count();
    if nl <= 1 && s.chars().count() > 60 {
        // Recover jammed single-line structure for Mode B + human read.
        s = s.replace("### ", "\n\n### ");
        s = s.replace("## ", "\n\n## ");
        // "# title" already at start; if mid-string "# " appears after content, break.
        // Avoid touching the leading H1: only replace " ##" patterns already handled.
        s = s.trim().to_string();
        // Ensure H1 is followed by blank line when next is ##
        if let Some(rest) = s.strip_prefix("# ") {
            if let Some(pos) = rest.find("\n\n##") {
                let title = &rest[..pos];
                let body = &rest[pos..];
                s = format!("# {}\n{}", title.trim_end(), body);
            } else if !rest.contains('\n') {
                // still one line after ## inject failed (no ##) — keep as is
            }
        }
    }
    // Guarantee trailing newline
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// G0b local: ensure draft has short H1 + core sections (no CLI).
/// Idempotent when already structured; fills missing headings only.
pub fn structure_plan_markdown(md: &str) -> String {
    let mut s = normalize_plan_markdown(md);
    let lower = s.to_lowercase();
    let has_h1 = s.lines().any(|l| {
        let t = l.trim();
        t.starts_with("# ") || (t.starts_with('#') && !t.starts_with("##"))
    });
    if !has_h1 {
        let title = extract_title_from_md(&s).unwrap_or_else(|| "聊天生成计划".into());
        s = format!("# {title}\n\n{s}");
    }
    // Re-extract short title and rewrite first H1 if wall-like
    if let Some(title) = extract_title_from_md(&s) {
        if let Some(rest_start) = s.find('\n') {
            let rest = &s[rest_start..];
            s = format!("# {title}{rest}");
        } else {
            s = format!("# {title}\n");
        }
    }
    let mut missing = Vec::new();
    if !lower.contains("## 目标") && !lower.contains("## goal") {
        missing.push("## 目标\n（请补充 1～3 句目标）\n");
    }
    if !lower.contains("## 范围") && !lower.contains("## scope") {
        missing.push("## 范围\n- 做：…\n- 不做：…\n");
    }
    if !lower.contains("## 任务") && !lower.contains("## tasks") {
        missing.push("## 任务大纲\n### T1 · （可执行标题）\n- 说明：…\n- 验收：…\n");
    }
    if !lower.contains("## 验收") && !lower.contains("## acceptance") {
        missing.push("## 验收（整计划）\n- [ ] …\n");
    }
    if !missing.is_empty() {
        s = s.trim_end().to_string();
        s.push_str("\n\n---\n\n");
        s.push_str(&missing.join("\n"));
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Response of G0b normalize (CLI or local).
#[derive(Debug, Clone, Serialize)]
pub struct ChatNormalizePlanResponse {
    pub markdown: String,
    pub title: Option<String>,
    /// true when CLI was used; false = local structure only
    pub used_cli: bool,
}

/// G0b: reshape free-form plan markdown into cco template.
/// Tries Claude CLI with a short independent prompt; on failure / fake → local `structure_plan_markdown`.
pub fn chat_normalize_plan(
    config: &Config,
    project: &Path,
    markdown: &str,
    hint: Option<&str>,
) -> Result<ChatNormalizePlanResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let md = markdown.trim();
    if md.is_empty() {
        bail!("empty plan markdown");
    }
    let local = structure_plan_markdown(md);
    let force_fake = std::env::var("CCO_CHAT_FAKE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || config.default.default_provider.eq_ignore_ascii_case("fake");
    if force_fake {
        return Ok(ChatNormalizePlanResponse {
            title: extract_title_from_md(&local),
            markdown: local,
            used_cli: false,
        });
    }

    let hint_line = hint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n用户补充约束：{s}\n"))
        .unwrap_or_default();
    let prompt = format!(
        r#"你是计划文档整理器。把下面「草稿」改写成结构清晰的 Markdown 计划（不要 JSON 任务图）。

硬规则：
1. 必须多行；禁止整篇挤成一行
2. 首行必须是单一「# 短标题」（≤40 字，标题内禁止 ##）
3. 必须包含：## 目标 · ## 范围 · ## 任务大纲（### T1…）· ## 验收
4. 任务标题要可执行，每任务带验收；不写「已完成」
5. 若输入已合格，只做轻量补全
6. 只输出 Markdown 正文，不要用 ``` 包裹，不要解释
{hint_line}
--- 草稿 ---
{md}
"#
    );

    match call_claude_normalize(config, project, &prompt) {
        Ok(raw) => {
            let body = extract_plan_fence(&raw)
                .or_else(|| {
                    let t = raw.trim();
                    if t.starts_with('#') {
                        Some(t.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or(raw);
            let out = structure_plan_markdown(&normalize_plan_markdown(&body));
            Ok(ChatNormalizePlanResponse {
                title: extract_title_from_md(&out),
                markdown: out,
                used_cli: true,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "chat_normalize_plan: CLI failed, local structure");
            Ok(ChatNormalizePlanResponse {
                title: extract_title_from_md(&local),
                markdown: local,
                used_cli: false,
            })
        }
    }
}

fn call_claude_normalize(config: &Config, project: &Path, prompt: &str) -> Result<String> {
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = project.join(".cco").join("chat").join("_work");
    let task_dir = work.join("tasks").join("__normalize__");
    std::fs::create_dir_all(&task_dir)?;
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");
    std::fs::write(task_dir.join("prompt.md"), prompt)?;

    let chat_task = TaskIR {
        id: "__normalize__".into(),
        title: "plan normalize".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt: prompt.to_string(),
        acceptance: None,
        timeout_secs: Some(120),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            "max_turns": null,
            "max_budget_usd": null,
            "permission_mode": "dontAsk",
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
    tags: vec![],
    };

    let ctx = StartCtx {
        run_id: "chat-normalize".into(),
        project_root: project.to_path_buf(),
        work_dir: project.to_path_buf(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    // ~120s @ 400ms + slack
    const MAX_POLL_TICKS: u32 = 400;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio runtime for chat normalize")?;
    let raw_out = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&chat_task)?;
        let handle = provider.start(&chat_task, &ctx).await?;
        let mut ticks = 0u32;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    ticks += 1;
                    if ticks > MAX_POLL_TICKS {
                        bail!("normalize Claude CLI timeout");
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => break,
            }
        }
        let result = provider.collect(&handle).await?;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), &stdout);
        let text = extract_assistant_text(&stdout);
        if !text.trim().is_empty() {
            return Ok::<String, anyhow::Error>(text);
        }
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result.error.unwrap_or_else(|| "normalize worker failed".into());
            bail!("normalize worker not done: {err}");
        }
        Ok::<String, anyhow::Error>(text)
    })?;
    if raw_out.trim().is_empty() {
        bail!("empty normalize reply");
    }
    Ok(raw_out)
}

/// Production soft-fallback assistant body: short human note, **no** ```plan fence.
fn soft_fallback_assistant_reply() -> String {
    "暂时无法联系本机 Claude CLI。请到「环境检查」确认 CLI 与密钥后重试，或设置 CCO_CHAT_FAKE=1 仅作 UI 联调。"
        .to_string()
}

/// Short env note for UI system bar; full diagnostic stays in logs only.
fn soft_fallback_env_note(diagnostic: &str) -> String {
    let short = diagnostic.chars().take(160).collect::<String>();
    let short = if diagnostic.chars().count() > 160 {
        format!("{short}…")
    } else {
        short
    };
    format!("本机 Claude CLI 暂不可用：{short}")
}

/// Forced mock (CCO_CHAT_FAKE / provider=fake) — keeps ```plan for UI 联调就绪条.
fn fake_chat_reply(user_msg: &str, project: &Path) -> String {
    let name = project
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "项目".into());
    let short = if user_msg.chars().count() > 80 {
        format!(
            "{}…",
            user_msg.chars().take(80).collect::<String>()
        )
    } else {
        user_msg.to_string()
    };
    format!(
        r#"好的，我根据你的描述整理了一份计划草稿（模拟回复，便于无 CLI 时联调 UI）。

你提到：{short}

```plan
# {name}：协作计划草稿

## 目标
根据用户描述完成可验证的交付。

## 范围
- 纳入：与「{short}」直接相关的实现与验收
- 不纳入：无关重构、范围外功能

## 任务大纲
1. 澄清需求与验收标准，对齐目录与约束
2. 实现核心改动并保证可编译/可运行
3. 补最小验证（单测或手工检查清单）
4. 整理变更说明与回滚点

## 验收
- [ ] 主路径可走通
- [ ] 无新增编译错误
- [ ] 文档/注释与行为一致

## 约束
- 仅改项目内必要文件
- 不引入第二套执行入口
```

若需调整范围或拆分粒度，直接说；满意后点「保存为计划」，再「分配计划」。"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Force-fake via Config only — avoids process-wide `CCO_CHAT_FAKE` races under
    /// `cargo test` parallel threads (set_var/remove_var interleaved → flaky `r.fake`).
    fn fake_cfg() -> Config {
        let mut cfg = Config::default();
        cfg.default.default_provider = "fake".into();
        cfg
    }

    #[test]
    fn extract_plan_fence_last_wins() {
        let t = "intro\n```plan\n# A\n```\nmore\n```plan\n# B\nbody\n```\n";
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# B\nbody"));
    }

    /// Desktop 2026-07-19: plain ``` then CJK used to panic with
    /// `start byte index 3 is not a char boundary; it is inside '若'/'和'`.
    #[test]
    fn extract_plan_fence_cjk_after_plain_fence_no_panic() {
        let cases = [
            "```\n若无异议\n```\n",
            "```\n和xx\n```\n",
            "好的\n```\n若需调整\n```\n后面",
            "text ``` 若xxx",
            "x```若y",
            "x```和y",
            "```ab若",
            "```\u{00e9}若", // 2-byte latin + CJK after non-plan tag path
            "聊天界面设计进度不合理",
            "a```b若c",
        ];
        for c in cases {
            let r = std::panic::catch_unwind(|| extract_plan_fence(c));
            assert!(r.is_ok(), "panicked on {c:?}: {r:?}");
            // plain fences must not be treated as plan
            assert!(r.unwrap().is_none(), "unexpected plan from {c:?}");
        }
    }

    #[test]
    fn extract_plan_fence_plan_after_cjk_plain_fence() {
        // Real chat shape: assistant writes Chinese + a non-plan code block, then ```plan.
        let t = "先说明一下\n```\n若无异议，可直接点保存\n```\n\n```plan\n# 真实计划\n## 目标\n做完\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# 真实计划\n## 目标\n做完")
        );
    }

    #[test]
    fn extract_plan_fence_plan_with_cjk_body() {
        let t = "```plan\n# 若和计划\n若无异议\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# 若和计划\n若无异议")
        );
    }

    #[test]
    fn extract_plan_fence_skips_markdown_and_keeps_later_plan() {
        let t = "```markdown\n# not plan\n```\n```plan\n# yes\n```\n";
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# yes"));
    }

    /// Real chat shape (2026-07-20): assistant put ```text flow diagrams *inside*
    /// ```plan. Naive first-``` closed the plan at the diagram opener → ~120 char
    /// stub written to plans/chat-20260719-0902.md.
    #[test]
    fn extract_plan_fence_nested_text_block_keeps_full_body() {
        let t = r#"先说明

```plan
# cco 全量落地总计划

## 目标
把多轮诉求收成一条产品：

```text
选项目 → 聊天共建 → 落盘 .md
     → 计划管理 → 分配 → 跑
```

## 任务大纲
### T1 · 修 fence
- 说明：嵌套 ```text 不得截断
- 验收：全文落盘

## 验收
- [ ] 计划管理预览可见全文
```
"#;
        let got = extract_plan_fence(t).expect("plan fence");
        assert!(
            got.contains("## 任务大纲"),
            "must not stop at nested ```text; got:\n{got}"
        );
        assert!(
            got.contains("选项目 → 聊天共建"),
            "nested diagram body must be kept; got:\n{got}"
        );
        assert!(
            got.contains("- [ ] 计划管理预览可见全文"),
            "tail after nested fence must remain; got:\n{got}"
        );
        // outer close not included
        assert!(!got.trim_end().ends_with("```"));
    }

    #[test]
    fn extract_plan_fence_nested_multiple_diagrams() {
        let t = "```plan\n# A\n\n```text\none\n```\n\nmid\n\n```text\ntwo\n```\n\ntail\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# A\n\n```text\none\n```\n\nmid\n\n```text\ntwo\n```\n\ntail")
        );
    }

    #[test]
    fn extract_plan_fence_ignores_midline_triple_backticks() {
        // Inline ``` in a sentence must not open/close fences.
        let t = "```plan\n# T\nuse ```inline``` sparingly\n## 尾\n```\n";
        let got = extract_plan_fence(t).expect("plan");
        assert!(got.contains("## 尾"), "got:\n{got}");
        assert!(got.contains("```inline```"), "got:\n{got}");
    }

    #[test]
    fn truncate_chars_cjk_safe() {
        let s = "若和计划".repeat(10); // each char 3 bytes; 40 chars
        let out = truncate_chars(&s, 5);
        assert_eq!(out.chars().count(), 6); // 5 + …
        assert!(out.ends_with('…'));
        assert!(!out.contains('\u{FFFD}'));
        assert_eq!(truncate_chars("短", 4000), "短");
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("abcd", 3), "abc…");
    }

    #[test]
    fn extract_assistant_text_from_result_line() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}
{"type":"result","result":"final answer"}
"#;
        assert_eq!(extract_assistant_text(raw), "final answer");
    }

    #[test]
    fn extract_assistant_text_falls_back_to_longest_assistant() {
        // Build lines without raw-string traps around `"##` in JSON text fields.
        let long = "## 较长的计划草稿\n\n- a\n- b";
        let long_line = serde_json::json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": long }] }
        })
        .to_string();
        let raw = format!(
            "{}\n{}\n{}\n{}\n",
            r#"{"type":"system","subtype":"init"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"short"}]}}"#,
            long_line,
            r#"{"type":"result","subtype":"success","result":""}"#,
        );
        let t = extract_assistant_text(&raw);
        assert!(t.contains("较长的计划草稿"), "got: {t}");
    }

    #[test]
    fn extract_assistant_text_from_error_max_turns_keeps_prose() {
        // Real failure mode: CLI exits 1 with subtype=error_max_turns and null result,
        // but earlier assistant text is still useful (must not soft-fallback to template).
        let prose = "先摸清项目结构和现有计划/进度文档，再据此梳理框架与进度。";
        let raw = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"system","subtype":"init"}"#,
            serde_json::json!({
                "type": "assistant",
                "message": { "content": [{ "type": "text", "text": prose }] }
            }),
            serde_json::json!({
                "type": "result",
                "subtype": "error_max_turns",
                "is_error": true,
                "result": null,
                "errors": ["Reached maximum number of turns (2)"],
                "num_turns": 3,
                "stop_reason": "tool_use",
            }),
        );
        let t = extract_assistant_text(&raw);
        assert!(t.contains("先摸清项目结构"), "got: {t}");
        let summary = stream_result_summary(&raw);
        assert!(summary.contains("error_max_turns"), "got: {summary}");
        assert!(summary.contains("turns=3"), "got: {summary}");
    }

    #[test]
    fn extract_assistant_text_plain_non_json() {
        assert_eq!(
            extract_assistant_text("hello plan\nsecond line\n"),
            "hello plan\nsecond line"
        );
    }

    #[test]
    fn extract_title_cuts_jammed_single_line_wall() {
        let wall = "# cco 全局体验优化：简单主路径 · 计划管理## 目标把桌面端收成一条## 任务大纲1. 做";
        let t = extract_title_from_md(wall).expect("title");
        assert!(t.contains("全局体验优化") || t.contains("简单主路径"), "got: {t}");
        assert!(!t.contains("##"), "title must not include ##: {t}");
        assert!(!t.contains("目标把"), "title must stop before body: {t}");
        assert!(t.chars().count() <= PLAN_TITLE_MAX_CHARS + 1, "got len {}", t.chars().count());
    }

    #[test]
    fn extract_title_clamps_long_h1() {
        let long = format!("# {}\n\nbody\n", "字".repeat(120));
        let t = extract_title_from_md(&long).expect("title");
        assert!(t.ends_with('…'), "got: {t}");
        assert!(t.chars().count() <= PLAN_TITLE_MAX_CHARS + 1);
    }

    #[test]
    fn normalize_plan_markdown_breaks_single_line_headings() {
        let wall = "# 标题短## 目标说明一下### T1 · 任务";
        let out = normalize_plan_markdown(wall);
        assert!(out.contains('\n'), "expected newlines: {out}");
        assert!(out.contains("## 目标"), "got: {out}");
        assert!(out.contains("### T1"), "got: {out}");
        let title = extract_title_from_md(&out).unwrap();
        assert_eq!(title, "标题短");
    }

    #[test]
    fn structure_plan_markdown_fills_missing_sections() {
        let thin = "# 登录优化\n\n做快点\n";
        let out = structure_plan_markdown(thin);
        assert!(out.contains("## 目标"), "got: {out}");
        assert!(out.contains("## 范围"), "got: {out}");
        assert!(out.contains("## 任务"), "got: {out}");
        assert!(out.contains("## 验收"), "got: {out}");
        assert_eq!(extract_title_from_md(&out).as_deref(), Some("登录优化"));
    }

    #[test]
    fn cleanup_expired_chat_sessions_removes_old() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        let chat = project.join(".cco").join("chat");
        std::fs::create_dir_all(&chat).unwrap();
        let old_path = chat.join("old.json");
        let new_path = chat.join("new.json");
        let old_at = (Utc::now() - chrono::Duration::hours(72)).to_rfc3339();
        let new_at = Utc::now().to_rfc3339();
        std::fs::write(
            &old_path,
            format!(r#"{{"session_id":"old","project":"p","messages":[],"updated_at":"{old_at}"}}"#),
        )
        .unwrap();
        std::fs::write(
            &new_path,
            format!(r#"{{"session_id":"new","project":"p","messages":[],"updated_at":"{new_at}"}}"#),
        )
        .unwrap();
        let n = cleanup_expired_chat_sessions(&project, 48).unwrap();
        assert_eq!(n, 1);
        assert!(!old_path.is_file());
        assert!(new_path.is_file());
    }

    #[test]
    fn chat_list_new_delete_sessions_roundtrip() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();

        // Empty project → synthetic default only
        let list0 = chat_list_sessions(&project).unwrap();
        assert_eq!(list0.len(), 1);
        assert_eq!(list0[0].session_id, "default");
        assert_eq!(list0[0].message_count, 0);

        let created = chat_new_session(&project, Some("登录优化")).unwrap();
        assert!(created.session_id.starts_with("s-"), "got {}", created.session_id);
        assert_eq!(created.title.as_deref(), Some("登录优化"));
        assert!(created.messages.is_empty());
        assert!(session_path(&project, &created.session_id).is_file());

        // Seed default with a user msg so list preview works
        let cfg = fake_cfg();
        let _ = chat_send(&cfg, &project, "默认会话第一条", Some("default"), None).unwrap();

        let list = chat_list_sessions(&project).unwrap();
        assert!(list.len() >= 2, "got {list:?}");
        let ids: Vec<_> = list.iter().map(|s| s.session_id.as_str()).collect();
        assert!(ids.contains(&"default"));
        assert!(ids.contains(&created.session_id.as_str()));
        let row = list.iter().find(|s| s.session_id == created.session_id).unwrap();
        assert_eq!(row.title.as_deref(), Some("登录优化"));
        assert!(row.preview.as_deref().unwrap_or("").contains("登录") || row.title.is_some());

        chat_delete_session(&project, &created.session_id).unwrap();
        assert!(!session_path(&project, &created.session_id).is_file());
        let list2 = chat_list_sessions(&project).unwrap();
        assert!(!list2.iter().any(|s| s.session_id == created.session_id));
        // default still listed
        assert!(list2.iter().any(|s| s.session_id == "default"));
    }

    #[test]
    fn sanitize_session_id_strips_unsafe() {
        assert_eq!(sanitize_session_id("default"), "default");
        assert_eq!(sanitize_session_id("s-20260720-120000"), "s-20260720-120000");
        assert_eq!(sanitize_session_id("../evil"), "___evil");
        assert_eq!(sanitize_session_id(""), "default");
    }

    #[test]
    fn session_roundtrip_and_save_plan() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let sess = chat_session_get(&project, None).unwrap();
        assert!(sess.messages.is_empty());
        assert_eq!(sess.session_id, "default");

        // manual save without send
        let resp = chat_save_plan(
            &project,
            Some("default"),
            Some("测试计划"),
            "## 目标\n做一件事\n",
            None,
            None,
        )
        .unwrap();
        assert!(resp.plan_rel.starts_with("plans/chat-"));
        assert!(PathBuf::from(&resp.abs_path).is_file());

        let sess2 = chat_session_get(&project, Some("default")).unwrap();
        assert!(sess2.draft_plan.as_ref().unwrap().saved);
        assert_eq!(
            sess2.draft_plan.as_ref().unwrap().path,
            resp.plan_rel
        );

        // H1: overwrite existing plan_rel
        let body2 = "# 覆盖\n\n## 目标\n改一版\n";
        let resp2 = chat_save_plan(
            &project,
            Some("default"),
            None,
            body2,
            Some(&resp.plan_rel),
            None,
        )
        .unwrap();
        assert_eq!(resp2.plan_rel, resp.plan_rel);
        let disk = std::fs::read_to_string(&resp2.abs_path).unwrap();
        assert!(disk.contains("改一版"));
        let read_back = read_plan_md(&project, &resp.plan_rel).unwrap();
        assert_eq!(read_back, disk);

        // G1: custom plans_dir
        let resp3 = chat_save_plan(
            &project,
            Some("default"),
            Some("自定义夹"),
            "# 自定义夹\n\n## 目标\nx\n",
            None,
            Some("docs/plans"),
        )
        .unwrap();
        assert!(
            resp3.plan_rel.starts_with("docs/plans/chat-"),
            "got {}",
            resp3.plan_rel
        );
        assert!(PathBuf::from(&resp3.abs_path).is_file());
    }

    #[test]
    fn chat_save_attachment_png_roundtrip() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        // minimal 1x1 PNG
        let png: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xfe,
            0xd4, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let att = chat_save_attachment(
            &project,
            Some("default"),
            "shot.png",
            "image/png",
            png,
        )
        .unwrap();
        assert!(att.path.contains(".cco/chat/attachments/default/"));
        assert!(att.path.ends_with(".png"));
        assert_eq!(att.mime, "image/png");
        assert!(project.join(&att.path).is_file());

        let cfg = fake_cfg();
        let r = chat_send(
            &cfg,
            &project,
            "看这张图优化登录",
            None,
            Some(vec![att.clone()]),
        )
        .unwrap();
        assert!(r.fake);
        let user = r.messages.iter().find(|m| m.role == "user").unwrap();
        assert!(!user.attachments.is_empty());
        assert!(user.content.contains(&att.path));
    }

    #[test]
    fn chat_save_attachment_rejects_bad_mime() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let err = chat_save_attachment(&project, None, "x.exe", "application/octet-stream", b"hi")
            .unwrap_err();
        assert!(err.to_string().contains("unsupported"), "{err}");
    }

    #[test]
    fn fake_send_persists_messages() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();
        let cfg = fake_cfg();
        let r = chat_send(&cfg, &project, "帮我写个登录页计划", None, None).unwrap();
        assert!(r.fake);
        assert!(!r.reply.is_empty());
        assert!(r.messages.len() >= 2);
        assert!(r.draft_plan.as_ref().and_then(|d| d.markdown.as_ref()).is_some());
        assert!(r.env_note.is_none(), "forced fake has no env_note");
        // 联调路径仍产出 fence
        assert!(r.reply.contains("```plan"), "got: {}", r.reply);
    }

    #[test]
    fn soft_fallback_reply_has_no_plan_fence() {
        let reply = soft_fallback_assistant_reply();
        assert!(extract_plan_fence(&reply).is_none());
        assert!(!reply.contains("```plan"));
        assert!(reply.contains("Claude CLI") || reply.contains("环境检查"));
    }

    #[test]
    fn soft_fallback_env_note_truncates() {
        let long = "x".repeat(300);
        let n = soft_fallback_env_note(&long);
        assert!(n.chars().count() < 220, "got len {}", n.chars().count());
        assert!(n.contains("暂不可用"));
    }

    #[test]
    fn call_prep_clears_stale_chat_work_dir() {
        // Mirrors the desktop bug: second chat_send reused __chat__ with leftover .done
        // and empty stdout → extract_assistant_text("") → soft local template.
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        let task_dir = project
            .join(".cco")
            .join("chat")
            .join("_work")
            .join("tasks")
            .join("__chat__");
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(task_dir.join(".done"), "0").unwrap();
        std::fs::write(task_dir.join("stdout.json"), "stale").unwrap();
        std::fs::write(task_dir.join("stdout.raw.ndjson"), "stale").unwrap();

        // Same cleanup call_claude_chat performs before provider.start.
        let _ = std::fs::remove_file(task_dir.join(".done"));
        let _ = std::fs::write(task_dir.join("stdout.json"), "");
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");

        assert!(!task_dir.join(".done").exists());
        assert_eq!(
            std::fs::read_to_string(task_dir.join("stdout.json")).unwrap(),
            ""
        );
        assert_eq!(
            std::fs::read_to_string(task_dir.join("stdout.raw.ndjson")).unwrap(),
            ""
        );
    }

    #[test]
    fn chat_stream_partial_reads_growing_stdout() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        let task_dir = project
            .join(".cco")
            .join("chat")
            .join("_work")
            .join("tasks")
            .join("__chat__");
        std::fs::create_dir_all(&task_dir).unwrap();
        // Idle: empty files → empty partial
        std::fs::write(task_dir.join("stdout.raw.ndjson"), "").unwrap();
        let empty = chat_stream_partial(&project, Some("default")).unwrap();
        assert!(empty.text.is_empty());
        assert!(!empty.done);

        // Partial assistant block (no result line yet)
        let partial = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"你好，这是计划草稿"}]}}
{"type":"content_block_delta","delta":{"text":"·续写"}}
"#;
        std::fs::write(task_dir.join("stdout.raw.ndjson"), partial).unwrap();
        let mid = chat_stream_partial(&project, None).unwrap();
        assert!(
            mid.text.contains("计划草稿") || mid.text.contains("续写"),
            "got: {}",
            mid.text
        );
        assert!(!mid.done);
        assert!(mid.bytes > 0);

        // CJK-safe: incomplete trailing bytes must not panic
        let cjk = format!(
            "{}\n{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"中文\"}}]}}}}\n",
            partial
        );
        std::fs::write(task_dir.join("stdout.raw.ndjson"), cjk).unwrap();
        let cjk_out = std::panic::catch_unwind(|| chat_stream_partial(&project, None));
        assert!(cjk_out.is_ok());
        let cjk_out = cjk_out.unwrap().unwrap();
        assert!(cjk_out.text.contains("中文") || cjk_out.text.contains("计划"));

        std::fs::write(task_dir.join(".done"), "0").unwrap();
        let done = chat_stream_partial(&project, None).unwrap();
        assert!(done.done);
    }
}
