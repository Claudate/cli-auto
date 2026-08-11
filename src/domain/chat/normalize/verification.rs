//! P2-1: plan checklist vs inspect verification assemble.

use serde::Serialize;

use super::acceptance::{PlanChecklistItem, TaskAcceptanceItem};

// ─── Verification types (P2-1) ─────────────────────────────────────────────

/// Where the result-desk verification block drew its primary story from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSource {
    /// Real inspect product present — inspect is authoritative; plan list is sidebar.
    Inspect,
    /// No usable inspect; plan wrote acceptance items that were not auto-checked.
    PlanOnly,
    /// Neither inspect product nor plan checklist.
    None,
}

/// One inspect-side item (issue preview / residual), not auto-matched to plan lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationItemStatus {
    Pass,
    Fail,
    Unknown,
    Skipped,
}

/// Inspect-side verification row (issue text + status).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationItem {
    pub text: String,
    pub status: VerificationItemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

/// Side-by-side plan checklist vs inspect snapshot (P2-1 live / report DTO).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerificationView {
    pub source: VerificationSource,
    /// Plan-level checklist lines under 验收 / 成功标准 / acceptance.
    #[serde(default)]
    pub plan_items: Vec<PlanChecklistItem>,
    /// Count of plan-level checklist lines (same as `plan_items.len()`).
    pub plan_count: usize,
    /// Task-level acceptance / done_when (optional; may be empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_items: Vec<TaskAcceptanceItem>,
    /// Inspect-side rows when `source == Inspect` (issues / residuals).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<VerificationItem>,
    /// Human note e.g. 「计划写了 N 条验收，本轮未自动对照」.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_count: Option<usize>,
}

/// Inputs for pure verification assembly (no IO / no inspect re-parse).
#[derive(Debug, Clone, Default)]
pub struct VerificationInputs {
    pub plan_items: Vec<PlanChecklistItem>,
    pub task_items: Vec<TaskAcceptanceItem>,
    /// True when a real inspect product is available (PASS/FAIL or blocking with product).
    pub has_real_inspect: bool,
    pub blocking_count: usize,
    pub residual_count: usize,
    /// Issue preview lines from inspect_loop (authoritative when inspect present).
    pub issue_preview: Vec<String>,
    /// True when plan required inspect but no real product (pending / unclear).
    pub inspect_pending: bool,
}

/// Assemble plan checklist vs inspect side-by-side view (pure).
///
/// Rules (P2-1):
/// 1. Real inspect → `source=inspect`; plan list is sidebar; inspect issues in `items`.
/// 2. No inspect but plan wrote N criteria → `source=plan_only` + note
///    「计划写了 N 条验收，本轮未自动对照」.
/// 3. Neither → `source=none`.
pub fn build_verification(input: VerificationInputs) -> VerificationView {
    let plan_count = input.plan_items.len();
    let task_n = input.task_items.len();
    let total_planish = plan_count + task_n;

    if input.has_real_inspect {
        let mut items = Vec::new();
        for line in &input.issue_preview {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            // Heuristic status from issue text (no LLM): blocking/fail markers → fail.
            let lower = t.to_ascii_lowercase();
            let status = if lower.contains("severity")
                || lower.contains("fail")
                || lower.contains("missing")
                || lower.contains("遗漏")
                || lower.contains("阻塞")
            {
                VerificationItemStatus::Fail
            } else {
                VerificationItemStatus::Unknown
            };
            items.push(VerificationItem {
                text: t.chars().take(200).collect(),
                status,
                task_id: None,
            });
        }
        let plan_note = if total_planish > 0 {
            Some(format!(
                "原计划写了 {total_planish} 条验收（巡检为准 · 清单仅作对照）"
            ))
        } else {
            None
        };
        return VerificationView {
            source: VerificationSource::Inspect,
            plan_items: input.plan_items,
            plan_count,
            task_items: input.task_items,
            items,
            plan_note,
            blocking_count: Some(input.blocking_count),
            residual_count: Some(input.residual_count),
        };
    }

    if total_planish > 0 {
        let note = if input.inspect_pending {
            format!("计划写了 {total_planish} 条验收，本轮未自动对照（巡检尚未产出结论）")
        } else {
            format!("计划写了 {total_planish} 条验收，本轮未自动对照")
        };
        return VerificationView {
            source: VerificationSource::PlanOnly,
            plan_items: input.plan_items,
            plan_count,
            task_items: input.task_items,
            items: Vec::new(),
            plan_note: Some(note),
            blocking_count: None,
            residual_count: None,
        };
    }

    VerificationView {
        source: VerificationSource::None,
        plan_items: Vec::new(),
        plan_count: 0,
        task_items: Vec::new(),
        items: Vec::new(),
        plan_note: None,
        blocking_count: None,
        residual_count: None,
    }
}


#[cfg(test)]
mod checklist_verification_tests {
    use super::*;
    use super::super::acceptance::{collect_task_acceptance_items, parse_acceptance_checklist};

    #[test]
    fn parse_checklist_real_items_skips_stubs() {
        let md = r#"# 出海落地页

## 成功标准（怎样算做完）

- [ ] 主标题 + 副标题说清「给谁 · 解决什么」
- [x] 至少 3 个利益点
- [ ] …
- [ ] 请补充
- 主 CTA 清晰

## 建议步骤
1. 定受众
"#;
        let items = parse_acceptance_checklist(md);
        assert_eq!(items.len(), 3, "got: {items:?}");
        assert_eq!(items[0].text, "主标题 + 副标题说清「给谁 · 解决什么」");
        assert!(!items[0].checked);
        assert!(items[0].has_checkbox);
        assert_eq!(items[1].text, "至少 3 个利益点");
        assert!(items[1].checked);
        assert_eq!(items[2].text, "主 CTA 清晰");
        assert!(!items[2].has_checkbox);
    }

    #[test]
    fn parse_checklist_empty_when_missing_section() {
        let md = "# 计划\n\n## 目标\n做点事\n";
        assert!(parse_acceptance_checklist(md).is_empty());
    }

    #[test]
    fn parse_checklist_numbered_and_english() {
        let md = "# Plan\n\n## Acceptance\n1. Login works with SSO\n2. API returns 200\n";
        let items = parse_acceptance_checklist(md);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "Login works with SSO");
        assert_eq!(items[1].text, "API returns 200");
    }

    #[test]
    fn collect_task_acceptance_filters_empty() {
        let rows = collect_task_acceptance_items([
            ("t1", Some("file exists")),
            ("t2", Some("")),
            ("t3", None),
            ("t4", Some("…")),
            ("t5", Some("- [ ] 写完 report.md\n- [ ] 有 VERDICT")),
        ]);
        assert_eq!(rows.len(), 3, "got: {rows:?}");
        assert_eq!(rows[0].task_id, "t1");
        assert_eq!(rows[0].text, "file exists");
        assert_eq!(rows[1].task_id, "t5");
        assert_eq!(rows[1].text, "写完 report.md");
        assert_eq!(rows[2].task_id, "t5");
        assert_eq!(rows[2].text, "有 VERDICT");
    }

    #[test]
    fn build_verification_inspect_authoritative() {
        let plan_items =
            parse_acceptance_checklist("## 验收\n- [ ] 主 CTA 清晰\n- [ ] 至少 3 个利益点\n");
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![],
            has_real_inspect: true,
            blocking_count: 1,
            residual_count: 0,
            issue_preview: vec!["I-1 severity=blocking missing CTA".into()],
            inspect_pending: false,
        });
        assert_eq!(v.source, VerificationSource::Inspect);
        assert_eq!(v.plan_count, 2);
        assert_eq!(v.items.len(), 1);
        assert_eq!(v.items[0].status, VerificationItemStatus::Fail);
        assert!(
            v.plan_note.as_deref().unwrap_or("").contains("巡检为准"),
            "got: {:?}",
            v.plan_note
        );
    }

    #[test]
    fn build_verification_plan_only_note() {
        let plan_items =
            parse_acceptance_checklist("## 成功标准\n- [ ] 登录可过\n- [ ] 结账可过\n");
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![TaskAcceptanceItem {
                task_id: "t1".into(),
                text: "file exists".into(),
            }],
            has_real_inspect: false,
            blocking_count: 0,
            residual_count: 0,
            issue_preview: vec![],
            inspect_pending: false,
        });
        assert_eq!(v.source, VerificationSource::PlanOnly);
        assert_eq!(v.plan_count, 2);
        assert!(v.items.is_empty());
        assert_eq!(
            v.plan_note.as_deref(),
            Some("计划写了 3 条验收，本轮未自动对照")
        );
    }

    #[test]
    fn build_verification_none_when_empty() {
        let v = build_verification(VerificationInputs::default());
        assert_eq!(v.source, VerificationSource::None);
        assert_eq!(v.plan_count, 0);
        assert!(v.plan_note.is_none());
    }

    #[test]
    fn build_verification_pending_inspect_wording() {
        let plan_items = parse_acceptance_checklist("## 验收\n- [ ] 有 VERDICT\n");
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![],
            has_real_inspect: false,
            blocking_count: 0,
            residual_count: 0,
            issue_preview: vec![],
            inspect_pending: true,
        });
        assert_eq!(v.source, VerificationSource::PlanOnly);
        assert!(
            v.plan_note
                .as_deref()
                .unwrap_or("")
                .contains("巡检尚未产出"),
            "got: {:?}",
            v.plan_note
        );
    }
}

