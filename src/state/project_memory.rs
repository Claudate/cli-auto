//! Project light memory: last_summary + pins (P2-2 · thin).
//!
//! [INPUT]: Config · project_id (path string) · summary text · pin key/value
//! [OUTPUT]: project_last_summary / project_pins CRUD · format_memory_context
//! [POS]: state adapter — SQLite SoT for per-project memory
//! [PROTOCOL]: 变更时更新此头部与 src/state/CLAUDE.md · docs/pilotdeck-borrow-landing
//!
//! Hard caps: pin count ≤ 3 per project; summary text truncated.
//! Best-effort: callers may use try_* helpers so memory never blocks the main path.
//! **Does not** change route, auto-confirm, or open runs.

use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;

use crate::config::Config;

use super::sqlite::with_conn;

/// Max pins per project (hard).
pub const MAX_PINS_PER_PROJECT: usize = 3;

/// Soft cap for stored summary text (chars).
pub const MAX_SUMMARY_CHARS: usize = 480;

/// Soft cap for pin key / value.
pub const MAX_PIN_KEY_CHARS: usize = 64;
pub const MAX_PIN_VALUE_CHARS: usize = 240;

/// Last summary row for a project.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectLastSummary {
    pub project_id: String,
    pub text: String,
    pub updated_at: String,
}

/// One pin row.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectPin {
    pub project_id: String,
    pub key: String,
    pub value: String,
    pub pinned_at: String,
}

/// Bundle returned to UI / prompt inject.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct ProjectMemoryView {
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_summary: Option<ProjectLastSummary>,
    pub pins: Vec<ProjectPin>,
}

fn normalize_project_id(project_id: &str) -> String {
    project_id.trim().trim_end_matches('/').to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Rule-template summary (no LLM): plan stem · status · done/total · optional residual.
pub fn compose_last_summary(
    plan_stem: &str,
    status_label: &str,
    done: usize,
    total: usize,
    residual_note: Option<&str>,
) -> String {
    let stem = if plan_stem.trim().is_empty() {
        "未命名计划"
    } else {
        plan_stem.trim()
    };
    let mut s = format!("《{stem}》· {status_label} · 完成 {done}/{total} 步");
    if let Some(note) = residual_note.map(str::trim).filter(|n| !n.is_empty()) {
        let short = truncate_chars(note, 120);
        s.push_str(" · 残留：");
        s.push_str(&short);
    }
    truncate_chars(&s, MAX_SUMMARY_CHARS)
}

/// Upsert last summary for a project.
pub fn set_last_summary(config: &Config, project_id: &str, text: &str) -> Result<ProjectLastSummary> {
    let pid = normalize_project_id(project_id);
    if pid.is_empty() {
        bail!("project_id empty");
    }
    let text = truncate_chars(text.trim(), MAX_SUMMARY_CHARS);
    if text.is_empty() {
        bail!("summary text empty");
    }
    let updated_at = Utc::now().to_rfc3339();
    with_conn(config, |conn| {
        conn.execute(
            r#"INSERT INTO project_last_summary (project_id, text, updated_at)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(project_id) DO UPDATE SET
                 text=excluded.text,
                 updated_at=excluded.updated_at"#,
            params![pid, text, updated_at],
        )?;
        Ok(())
    })?;
    Ok(ProjectLastSummary {
        project_id: pid,
        text,
        updated_at,
    })
}

/// Best-effort set_last_summary (log + never fail callers).
pub fn try_set_last_summary(config: &Config, project_id: &str, text: &str) {
    if let Err(e) = set_last_summary(config, project_id, text) {
        tracing::warn!(error = %e, project_id = %project_id, "set_last_summary failed");
    }
}

/// Get last summary if any.
pub fn get_last_summary(config: &Config, project_id: &str) -> Result<Option<ProjectLastSummary>> {
    let pid = normalize_project_id(project_id);
    if pid.is_empty() {
        return Ok(None);
    }
    with_conn(config, |conn| {
        let row = conn
            .query_row(
                "SELECT project_id, text, updated_at FROM project_last_summary WHERE project_id = ?1",
                params![pid],
                |r| {
                    Ok(ProjectLastSummary {
                        project_id: r.get(0)?,
                        text: r.get(1)?,
                        updated_at: r.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    })
}

/// List pins for a project (oldest pin first; UI can reverse).
pub fn list_pins(config: &Config, project_id: &str) -> Result<Vec<ProjectPin>> {
    let pid = normalize_project_id(project_id);
    if pid.is_empty() {
        return Ok(vec![]);
    }
    with_conn(config, |conn| {
        let mut stmt = conn.prepare(
            "SELECT project_id, key, value, pinned_at FROM project_pins
             WHERE project_id = ?1
             ORDER BY pinned_at ASC, key ASC",
        )?;
        let rows = stmt
            .query_map(params![pid], |r| {
                Ok(ProjectPin {
                    project_id: r.get(0)?,
                    key: r.get(1)?,
                    value: r.get(2)?,
                    pinned_at: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Upsert a pin. New keys are rejected when the project already has MAX_PINS.
pub fn upsert_pin(config: &Config, project_id: &str, key: &str, value: &str) -> Result<ProjectPin> {
    let pid = normalize_project_id(project_id);
    if pid.is_empty() {
        bail!("project_id empty");
    }
    let key = key.trim();
    if key.is_empty() {
        bail!("pin key empty");
    }
    let key = truncate_chars(key, MAX_PIN_KEY_CHARS);
    let value = truncate_chars(value.trim(), MAX_PIN_VALUE_CHARS);
    if value.is_empty() {
        bail!("pin value empty");
    }
    let pinned_at = Utc::now().to_rfc3339();
    with_conn(config, |conn| {
        let existing: Option<String> = conn
            .query_row(
                "SELECT key FROM project_pins WHERE project_id = ?1 AND key = ?2",
                params![pid, key],
                |r| r.get(0),
            )
            .optional()?;
        if existing.is_none() {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM project_pins WHERE project_id = ?1",
                params![pid],
                |r| r.get(0),
            )?;
            if n as usize >= MAX_PINS_PER_PROJECT {
                bail!("每项目最多 {MAX_PINS_PER_PROJECT} 条 pin，请先删除一条");
            }
        }
        conn.execute(
            r#"INSERT INTO project_pins (project_id, key, value, pinned_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(project_id, key) DO UPDATE SET
                 value=excluded.value,
                 pinned_at=excluded.pinned_at"#,
            params![pid, key, value, pinned_at],
        )?;
        Ok(())
    })?;
    Ok(ProjectPin {
        project_id: pid,
        key,
        value,
        pinned_at,
    })
}

/// Delete one pin by key.
pub fn delete_pin(config: &Config, project_id: &str, key: &str) -> Result<bool> {
    let pid = normalize_project_id(project_id);
    let key = key.trim();
    if pid.is_empty() || key.is_empty() {
        return Ok(false);
    }
    with_conn(config, |conn| {
        let n = conn.execute(
            "DELETE FROM project_pins WHERE project_id = ?1 AND key = ?2",
            params![pid, key],
        )?;
        Ok(n > 0)
    })
}

/// Full memory view for a project.
pub fn get_memory(config: &Config, project_id: &str) -> Result<ProjectMemoryView> {
    let pid = normalize_project_id(project_id);
    let last_summary = get_last_summary(config, &pid)?;
    let pins = list_pins(config, &pid)?;
    Ok(ProjectMemoryView {
        project_id: pid,
        last_summary,
        pins,
    })
}

/// Format memory as prompt context only (no route / no auto-confirm).
/// Returns empty string when nothing stored.
pub fn format_memory_context(view: &ProjectMemoryView) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(s) = &view.last_summary {
        let t = s.text.trim();
        if !t.is_empty() {
            lines.push(format!("上次摘要：{t}"));
        }
    }
    
    // P0-B: Inject persona preferences from pins into prompt context
    for pin in &view.pins {
        if pin.key == "persona" && !pin.value.is_empty() {
            lines.push(format!("上次选择的角色：{}", pin.value));
        }
    }
    
    if !view.pins.is_empty() {
        lines.push("项目 pin（仅上下文，勿改路由/勿自动确认）：".into());
        for p in &view.pins {
            lines.push(format!("- {}：{}", p.key, p.value));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    let body = lines.join("\n");
    format!("## 项目记忆（仅上下文）\n{body}\n")
}

/// Best-effort load + format for chat/planner prompt inject.
pub fn try_format_memory_context(config: &Config, project_id: &str) -> String {
    match get_memory(config, project_id) {
        Ok(v) => format_memory_context(&v),
        Err(e) => {
            tracing::warn!(error = %e, project_id = %project_id, "get_memory for prompt failed");
            String::new()
        }
    }
}

/// Generic pin operations for arbitrary key/value storage (P0-B extensibility).

/// Set a generic pin by key (same constraints as upsert_pin but exposes public API).
pub fn set_project_pin(config: &Config, project_id: &str, key: &str, value: &str) -> Result<()> {
    let _ = upsert_pin(config, project_id, key, value)?;
    Ok(())
}

/// Get all pins for a project filtered by provided keys (or all if keys is empty).
pub fn get_project_pins(config: &Config, project_id: &str, keys: &[&str]) -> Result<HashMap<String, String>> {
    let pins = list_pins(config, project_id)?;
    let mut result = HashMap::new();

    if keys.is_empty() {
        // Return all pins
        for pin in pins {
            result.insert(pin.key, pin.value);
        }
    } else {
        // Return only specified keys
        for pin in pins {
            if keys.contains(&pin.key.as_str()) {
                result.insert(pin.key, pin.value);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::sqlite::reset_for_test;
    use tempfile::tempdir;

    fn test_cfg() -> (tempfile::TempDir, Config) {
        reset_for_test();
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().to_path_buf();
        (dir, cfg)
    }

    #[test]
    fn compose_summary_rule_template() {
        let s = compose_last_summary("demo-plan", "已完成", 2, 3, None);
        assert!(s.contains("《demo-plan》"));
        assert!(s.contains("已完成"));
        assert!(s.contains("2/3"));
        let with_note = compose_last_summary("x", "失败", 0, 1, Some("  open risk  "));
        assert!(with_note.contains("残留：open risk"));
    }

    #[test]
    fn last_summary_round_trip() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-a";
        assert!(get_last_summary(&cfg, pid).unwrap().is_none());
        let saved = set_last_summary(&cfg, pid, "hello summary").unwrap();
        assert_eq!(saved.text, "hello summary");
        let got = get_last_summary(&cfg, pid).unwrap().unwrap();
        assert_eq!(got.text, "hello summary");
        assert_eq!(got.project_id, pid);
        // overwrite
        set_last_summary(&cfg, pid, "second").unwrap();
        assert_eq!(
            get_last_summary(&cfg, pid).unwrap().unwrap().text,
            "second"
        );
    }

    #[test]
    fn pins_hard_cap_three() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-pins";
        upsert_pin(&cfg, pid, "a", "1").unwrap();
        upsert_pin(&cfg, pid, "b", "2").unwrap();
        upsert_pin(&cfg, pid, "c", "3").unwrap();
        let err = upsert_pin(&cfg, pid, "d", "4").unwrap_err();
        assert!(err.to_string().contains("最多"));
        // update existing ok
        let u = upsert_pin(&cfg, pid, "a", "1-updated").unwrap();
        assert_eq!(u.value, "1-updated");
        let pins = list_pins(&cfg, pid).unwrap();
        assert_eq!(pins.len(), 3);
        assert!(delete_pin(&cfg, pid, "b").unwrap());
        assert_eq!(list_pins(&cfg, pid).unwrap().len(), 2);
        upsert_pin(&cfg, pid, "d", "4").unwrap();
        assert_eq!(list_pins(&cfg, pid).unwrap().len(), 3);
    }

    #[test]
    fn format_memory_context_only_when_present() {
        let empty = ProjectMemoryView {
            project_id: "/p".into(),
            last_summary: None,
            pins: vec![],
        };
        assert!(format_memory_context(&empty).is_empty());
        let v = ProjectMemoryView {
            project_id: "/p".into(),
            last_summary: Some(ProjectLastSummary {
                project_id: "/p".into(),
                text: "上次完成登录".into(),
                updated_at: "t".into(),
            }),
            pins: vec![ProjectPin {
                project_id: "/p".into(),
                key: "stack".into(),
                value: "rust".into(),
                pinned_at: "t".into(),
            }],
        };
        let ctx = format_memory_context(&v);
        assert!(ctx.contains("项目记忆"));
        assert!(ctx.contains("上次完成登录"));
        assert!(ctx.contains("stack：rust"));
        assert!(ctx.contains("勿改路由"));
    }

    #[test]
    fn get_memory_bundle() {
        let (_dir, cfg) = test_cfg();
        let pid = "/tmp/proj-bundle";
        set_last_summary(&cfg, pid, "sum").unwrap();
        upsert_pin(&cfg, pid, "k", "v").unwrap();
        let m = get_memory(&cfg, pid).unwrap();
        assert_eq!(m.last_summary.as_ref().unwrap().text, "sum");
        assert_eq!(m.pins.len(), 1);
        assert!(!try_format_memory_context(&cfg, pid).is_empty());
    }
}
