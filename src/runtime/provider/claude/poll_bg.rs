//! Claude background poll: agents --json, log refresh, process helpers.
//!
//! [INPUT]: WorkerHandle · agent_id · bin
//! [OUTPUT]: agent state · refreshed logs · process_alive
//! [POS]: claude provider bg 轮询；D4 自 claude.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md

use std::process::Stdio;

use anyhow::{bail, Context, Result};
use tokio::process::Command;

use super::super::WorkerHandle;
use super::ClaudeProvider;

impl ClaudeProvider {
    pub(super) async fn agents_json(&self) -> Result<serde_json::Value> {
        let out = Command::new(&self.bin)
            .args(["agents", "--json", "--all"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("claude agents --json --all")?;
        let text = String::from_utf8_lossy(&out.stdout);
        if text.trim().is_empty() {
            // try without --all
            let out2 = Command::new(&self.bin)
                .args(["agents", "--json"])
                .output()
                .await
                .context("claude agents --json")?;
            let text2 = String::from_utf8_lossy(&out2.stdout);
            return parse_json_lenient(&text2);
        }
        parse_json_lenient(&text)
    }

    pub(super) fn agent_id_from_handle(handle: &WorkerHandle) -> Option<String> {
        handle
            .opaque_id
            .strip_prefix("agent:")
            .map(|s| s.to_string())
            .or_else(|| {
                let meta = std::fs::read_to_string(&handle.meta_path).ok()?;
                let v: serde_json::Value = serde_json::from_str(&meta).ok()?;
                v.get("agent_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
    }

    pub(super) async fn refresh_bg_logs(&self, handle: &WorkerHandle, agent_id: &str) {
        let out = Command::new(&self.bin)
            .args(["logs", agent_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        if let Ok(o) = out {
            let text = String::from_utf8_lossy(&o.stdout);
            if !text.is_empty() {
                let _ = std::fs::write(&handle.stdout_path, text.as_ref());
            }
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.is_empty() {
                if let Some(parent) = handle.stdout_path.parent() {
                    let _ = std::fs::write(parent.join("stderr.log"), err.as_ref());
                }
            }
        }
    }

    pub(super) fn bg_deadline_passed(meta: &serde_json::Value) -> bool {
        meta.get("deadline")
            .and_then(|d| d.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| chrono::Utc::now() > d.with_timezone(&chrono::Utc))
            .unwrap_or(false)
    }
}

pub(crate) fn parse_json_lenient(text: &str) -> Result<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::json!([]));
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }
    // find array
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]) {
                    return Ok(v);
                }
            }
        }
    }
    bail!("invalid agents json")
}


pub(crate) fn find_agent_state(v: &serde_json::Value, agent_id: &str) -> Option<String> {
    let arr = if let Some(a) = v.as_array() {
        a.clone()
    } else if let Some(a) = v.get("agents").and_then(|x| x.as_array()) {
        a.clone()
    } else if let Some(a) = v.get("sessions").and_then(|x| x.as_array()) {
        a.clone()
    } else {
        return None;
    };
    for item in arr {
        let id = item
            .get("id")
            .or_else(|| item.get("agent_id"))
            .or_else(|| item.get("session_id"))
            .or_else(|| item.get("short_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        // also match sessionId uuid prefix
        let session_id = item
            .get("sessionId")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let matched = id == agent_id
            || (!id.is_empty() && (id.starts_with(agent_id) || agent_id.starts_with(id)))
            || session_id.starts_with(agent_id)
            || (!session_id.is_empty() && agent_id.len() >= 8 && session_id.contains(agent_id));
        if matched {
            let st = item
                .get("state")
                .or_else(|| item.get("status"))
                .or_else(|| item.get("phase"))
                .and_then(|x| x.as_str())
                .unwrap_or("running");
            return Some(st.to_string());
        }
    }
    None
}

/// Pump process stdout/stderr into log files while the child runs.
/// Returns the process exit code (124 on timeout).

pub(crate) fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc_kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

#[cfg(unix)]
pub(crate) unsafe fn libc_kill(pid: i32, sig: i32) -> i32 {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, sig)
}


