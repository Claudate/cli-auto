//! chat_send: append user · CLI/fake · draft from ```plan · persist (no confirm/start_run).

use std::path::Path;

use anyhow::{bail, Result};
use chrono::Utc;

use crate::config::Config;
use crate::domain::chat::{
    extract_plan_fence, extract_title_from_md, normalize_plan_markdown, structure_plan_markdown,
    DEFAULT_SESSION,
};

use super::attachment::{
    allowed_image_mime, format_attachments_block, MAX_ATTACHMENTS_PER_MSG,
};
use super::cli_call::{
    call_claude_chat, fake_chat_reply, soft_fallback_assistant_reply, soft_fallback_env_note,
};
use super::normalize::chat_normalize_plan;
use super::session::{chat_session_get, save_session};
use super::types::{ChatAttachment, ChatMessage, ChatSendResponse};

const MAX_HISTORY_MSGS: usize = 24;
const MAX_MSG_CHARS: usize = 12_000;

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
