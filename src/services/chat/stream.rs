//! C3 streaming partial: read in-flight `__chat__` stdout (non-blocking).

use std::path::Path;

use anyhow::{bail, Result};

use crate::domain::chat::extract_assistant_text;

use super::paths::chat_work_task_dir;
use super::types::ChatStreamPartial;

/// C3 streaming partial: best-effort assistant text while `chat_send` is still running.
/// Reads the same `__chat__` stdout file that `call_claude_chat` writes; never panics on
/// truncated NDJSON / CJK mid-rune (uses char-safe extract). Empty when idle or unavailable.
pub fn chat_stream_partial(project: &Path, session_id: Option<&str>) -> Result<ChatStreamPartial> {
    let _ = session_id; // reserved: multi-session work dirs stay shared under __chat__ for now
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let task_dir = chat_work_task_dir(project);
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
