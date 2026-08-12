//! Host plan checklist for Ensure E1/E3 (plan_ref ↔ evidence).
//!
//! [INPUT]: PlanIR + optional plan markdown body
//! [OUTPUT]: HostChecklistItem list · prompt paste · JSON-ready rows
//! [POS]: domain/plan — pure; disk write is app/runtime
//! [PROTOCOL]: schema_version 变更须同步落盘读者

use serde::{Deserialize, Serialize};

use super::system_ids::is_system_post_task;
use super::types::{PlanIR, TaskRole};
use crate::domain::chat::parse_acceptance_checklist;

/// Disk / prompt schema version for `plan.checklist.json`.
pub const CHECKLIST_SCHEMA_VERSION: u32 = 1;

/// Kind of checklist row (host routing for closeout ownership).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecklistKind {
    Feature,
    Ledger,
    Map,
    Other,
}

impl ChecklistKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Ledger => "ledger",
            Self::Map => "map",
            Self::Other => "other",
        }
    }
}

/// One host-owned success criterion (stable plan_ref).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostChecklistItem {
    pub plan_ref: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_hint: Option<String>,
    pub kind: ChecklistKind,
}

/// Full checklist document written to `run_dir/plan.checklist.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostChecklist {
    pub schema_version: u32,
    pub items: Vec<HostChecklistItem>,
}

impl HostChecklist {
    pub fn new(items: Vec<HostChecklistItem>) -> Self {
        Self {
            schema_version: CHECKLIST_SCHEMA_VERSION,
            items,
        }
    }
}

/// Build host checklist from plan md acceptance section + task acceptance/verify.
///
/// Ledger/map rows without an owner are left `owner_task_id=None` so materialize
/// can assign them to `sys-closeout`.
pub fn build_host_checklist(plan: &PlanIR, plan_md: Option<&str>) -> HostChecklist {
    let mut items: Vec<HostChecklistItem> = Vec::new();
    let mut seen_text: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(md) = plan_md {
        for (i, row) in parse_acceptance_checklist(md).into_iter().enumerate() {
            let text = row.text.trim().to_string();
            if text.is_empty() || !seen_text.insert(text.to_ascii_lowercase()) {
                continue;
            }
            let kind = classify_checklist_text(&text);
            let plan_ref = stable_plan_ref(&text, i);
            items.push(HostChecklistItem {
                plan_ref,
                text,
                owner_task_id: None,
                evidence_hint: None,
                kind,
            });
        }
    }

    for t in &plan.tasks {
        if t.role == Some(TaskRole::Inspect)
            || t.role == Some(TaskRole::Closeout)
            || is_system_post_task(&t.id)
        {
            continue;
        }
        if let Some(acc) = t
            .acceptance
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let key = acc.to_ascii_lowercase();
            if seen_text.insert(key) {
                let kind = classify_checklist_text(acc);
                items.push(HostChecklistItem {
                    plan_ref: t.id.clone(),
                    text: acc.to_string(),
                    owner_task_id: Some(t.id.clone()),
                    evidence_hint: t.verify_cmd.clone().or_else(|| t.outputs.first().cloned()),
                    kind,
                });
            }
        } else if items
            .iter()
            .all(|it| it.owner_task_id.as_deref() != Some(&t.id))
        {
            // Title as weak feature row so every business task has a plan_ref owner.
            let text = t.title.trim();
            if !text.is_empty() {
                let key = format!("title:{}", text.to_ascii_lowercase());
                if seen_text.insert(key) {
                    items.push(HostChecklistItem {
                        plan_ref: t.id.clone(),
                        text: text.to_string(),
                        owner_task_id: Some(t.id.clone()),
                        evidence_hint: t.verify_cmd.clone().or_else(|| t.outputs.first().cloned()),
                        kind: ChecklistKind::Feature,
                    });
                }
            }
        }
    }

    // Assign unowned ledger/map → leave None (caller binds to closeout).
    // Assign unowned feature lines to best-effort first business task.
    let first_business = plan
        .tasks
        .iter()
        .find(|t| {
            t.role != Some(TaskRole::Inspect)
                && t.role != Some(TaskRole::Closeout)
                && !is_system_post_task(&t.id)
        })
        .map(|t| t.id.clone());
    for it in &mut items {
        if it.owner_task_id.is_none() {
            match it.kind {
                ChecklistKind::Ledger | ChecklistKind::Map => {
                    // host injects sys-closeout as owner later
                }
                ChecklistKind::Feature | ChecklistKind::Other => {
                    it.owner_task_id = first_business.clone();
                }
            }
        }
    }

    HostChecklist::new(items)
}

/// Bind unowned ledger/map rows to closeout task id.
pub fn assign_closeout_owners(checklist: &mut HostChecklist, closeout_id: &str) {
    for it in &mut checklist.items {
        if it.owner_task_id.is_none()
            && matches!(it.kind, ChecklistKind::Ledger | ChecklistKind::Map)
        {
            it.owner_task_id = Some(closeout_id.to_string());
        }
    }
}

/// Compact table for closeout / inspect prompts (R-rework-2 grade).
pub fn format_checklist_for_prompt(items: &[HostChecklistItem]) -> String {
    if items.is_empty() {
        return "（主机未抽出勾选行 — 请对照计划原文验收/成功标准）\n".into();
    }
    let mut out = String::from("| plan_ref | kind | owner | 成功标准 |\n|---|---|---|---|\n");
    for it in items {
        let owner = it.owner_task_id.as_deref().unwrap_or("—");
        let text = it.text.replace('|', "\\|");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            it.plan_ref,
            it.kind.as_str(),
            owner,
            text.chars().take(120).collect::<String>()
        ));
    }
    out
}

fn classify_checklist_text(text: &str) -> ChecklistKind {
    let lower = text.to_ascii_lowercase();
    let ledger_tokens = [
        "台账",
        "勾选",
        "回写",
        "进度",
        "readme",
        "索引",
        "index",
        "ledger",
        "gap-audit",
        "§",
        "文档",
        "commit",
    ];
    let map_tokens = ["断链", "指针", "geb", "map", "l1", "l2", "claude.md"];
    if map_tokens
        .iter()
        .any(|t| lower.contains(t) || text.contains(t))
    {
        return ChecklistKind::Map;
    }
    if ledger_tokens
        .iter()
        .any(|t| lower.contains(t) || text.contains(t))
    {
        return ChecklistKind::Ledger;
    }
    ChecklistKind::Feature
}

fn stable_plan_ref(text: &str, index: usize) -> String {
    // Prefer leading P0-1 / §9-W2 style tokens.
    let first = text.split_whitespace().next().unwrap_or("");
    if first.starts_with('P') || first.starts_with('§') || first.starts_with('S') {
        let token: String = first
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '§' || *c == '.')
            .collect();
        if token.len() >= 2 {
            return token;
        }
    }
    // Short hash fallback from text.
    let mut h: u32 = 2166136261;
    for b in text.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    format!("C{:02}-{:04x}", index + 1, h & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::{OnFailure, TaskIR, TaskScope};
    use std::path::PathBuf;

    fn task(id: &str, title: &str, role: Option<TaskRole>, acceptance: Option<&str>) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: title.into(),
            depends_on: vec![],
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: "p".into(),
            verify_cmd: None,
            acceptance: acceptance.map(|s| s.into()),
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: Some(TaskScope {
                paths: vec!["docs/**".into()],
                readonly: vec![],
                forbid: vec![],
            }),
            outputs: vec![],
            tags: vec![],
            wait_for: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("p.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn ledger_rows_unowned_until_assign() {
        let ir = plan(vec![
            task(
                "t1",
                "实现 API",
                Some(TaskRole::Implement),
                Some("API 可调用"),
            ),
            task("t7", "巡检", Some(TaskRole::Inspect), None),
        ]);
        let md = "## 成功标准\n- [ ] P0-1 功能 smoke 绿\n- [ ] 台账 §9 回写完成\n";
        let mut cl = build_host_checklist(&ir, Some(md));
        let ledger = cl
            .items
            .iter()
            .find(|i| i.text.contains("台账"))
            .expect("ledger row");
        assert_eq!(ledger.kind, ChecklistKind::Ledger);
        assert!(ledger.owner_task_id.is_none());
        assign_closeout_owners(&mut cl, "sys-closeout");
        let ledger = cl.items.iter().find(|i| i.text.contains("台账")).unwrap();
        assert_eq!(ledger.owner_task_id.as_deref(), Some("sys-closeout"));
    }

    #[test]
    fn prompt_table_non_empty() {
        let items = vec![HostChecklistItem {
            plan_ref: "P0-1".into(),
            text: "smoke 绿".into(),
            owner_task_id: Some("t1".into()),
            evidence_hint: None,
            kind: ChecklistKind::Feature,
        }];
        let s = format_checklist_for_prompt(&items);
        assert!(s.contains("P0-1"));
        assert!(s.contains("smoke"));
    }
}
