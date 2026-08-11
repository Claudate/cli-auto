//! C3 streaming partial: read in-flight `__chat__` stdout (non-blocking).

use std::path::Path;

use anyhow::{bail, Result};

use crate::domain::chat::extract_assistant_text;

use super::paths::chat_work_task_dir;
use super::types::ChatStreamPartial;

/// Wipe leftover stdout / `.done` from a previous chat turn so polls never
/// re-surface the last reply as if it were the new stream.
pub(crate) fn clear_chat_stream_work(project: &Path) {
    let task_dir = chat_work_task_dir(project);
    let _ = std::fs::create_dir_all(&task_dir);
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");
    let _ = std::fs::remove_file(task_dir.join("cancelled.flag"));
}

/// C3 streaming partial: best-effort assistant text while `chat_send` is still running.
/// Reads the same `__chat__` stdout file that `call_claude_chat` writes; never panics on
/// truncated NDJSON / CJK mid-rune (uses char-safe extract). Empty when idle or unavailable.
pub fn chat_stream_partial(project: &Path, session_id: Option<&str>) -> Result<ChatStreamPartial> {
    let _ = session_id; // reserved: multi-session work dirs stay shared under __chat__ for now
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let task_dir = chat_work_task_dir(project);

    // Quick stop check for UI/polling: if cancelled or stopped, treat as done immediately.
    if task_dir.join("cancelled.flag").is_file() || task_dir.join("stopped.flag").is_file() {
        let done = task_dir.join(".done").is_file();
        return Ok(ChatStreamPartial {
            text: String::new(),
            done,
            bytes: 0,
        });
    }

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

/// Stop the in-flight chat Claude CLI (best-effort).
/// Writes `.done=130` first so stream_child / poll treat it as orchestrator stop,
/// then SIGTERM+SIGKILL the pid from `meta.json`.
/// Returns whether a pid was targeted (true) or nothing was running (false).
pub fn chat_cancel(project: &Path) -> Result<bool> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let task_dir = chat_work_task_dir(project);
    let _ = std::fs::create_dir_all(&task_dir);

    // Marker for send path: distinguish user cancel from CLI crash soft-fallback.
    let _ = std::fs::write(task_dir.join("cancelled.flag"), "1");
    // Stop marker for worker: tell worker to stop immediately (before kill).
    let _ = std::fs::write(task_dir.join("stopped.flag"), "1");
    // Prefer stop marker before kill so finalize_stream_exit keeps 130.
    let _ = std::fs::write(task_dir.join(".done"), "130");

    let meta_path = task_dir.join("meta.json");
    let pid = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            v.get("pid").and_then(|p| p.as_u64()).or_else(|| {
                v.get("opaque_id")
                    .and_then(|o| o.as_str())
                    .and_then(|s| s.strip_prefix("pid:"))
                    .and_then(|n| n.parse::<u64>().ok())
            })
        })
        .map(|p| p as u32)
        .filter(|&p| p > 1);

    let Some(pid) = pid else {
        return Ok(false);
    };
    super::super::util::kill_pid(pid);
    // Brief settle so poll loop observes Stopped.
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(true)
}
