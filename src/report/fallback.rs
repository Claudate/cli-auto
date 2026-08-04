//! Plan-compare section + report skeleton helpers (P0-3 · P0-4 copy lock).
//!
//! Same *shape* as PilotDeck `buildFallbackReport`: always emit a full readable
//! skeleton; never invent PASS when inspect is missing.
//!
//! Headline phrases are the narrative lock for UI
//! (`web/js/features/result/inspectCopy.js` PLAN_COMPARE_COPY). Keep both sides
//! on the same words so result desk and report.md never contradict one round.
//!
//! [INPUT]: RunState · optional plan.resolved.json · project inspect products
//! [OUTPUT]: PlanCompareSection (人话对照计划 + fallback Notes)
//! [POS]: report adapter — uses handoff inspect_loop_view DTO, no re-parse of VERDICT body
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/report/CLAUDE.md

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::domain::chat::{
    build_verification, collect_task_acceptance_items, parse_acceptance_checklist,
    VerificationInputs, VerificationSource, VerificationView,
};
use crate::plan::PlanIR;
use crate::runtime::handoff::{self, InspectLoopView};
use crate::state::RunState;

/// How the plan-compare section was filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanCompareKind {
    /// Real inspect product: PASS with no blocking issues.
    Pass,
    /// Real inspect product: FAIL and/or blocking issues.
    Fail,
    /// require_inspect but no usable verdict yet.
    Pending,
    /// Inspect not enabled on this plan.
    Disabled,
    /// Verdict file present but unclear, or partial data.
    Unclear,
}

/// One plan-compare section ready for report.md / report.json.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanCompareSection {
    pub kind: PlanCompareKind,
    /// First human sentence (no bare run_id / engine id as lead).
    pub headline: String,
    /// Extra bullets under the section.
    pub body_lines: Vec<String>,
    /// When true, content is a placeholder — never claim PASS.
    pub is_fallback: bool,
    /// Machine/human reason for Notes (None when real inspect filled the section).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    /// Echo of require_inspect from plan.resolved when known.
    pub require_inspect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    pub blocking_count: usize,
    pub residual_count: usize,
    /// P2-1: plan checklist vs inspect (sidebar / note under 对照计划).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<VerificationView>,
}

/// Load run-local resolved plan if present (best-effort; missing is fine for report).
pub fn load_plan_resolved(run_dir: &Path) -> Option<PlanIR> {
    let text = std::fs::read_to_string(run_dir.join("plan.resolved.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Build the `## 对照计划` section from handoff inspect view + plan flags.
///
/// Rules (P0-3):
/// - Always returns a section (caller always writes the heading).
/// - No handoff / no VERDICT / require_inspect not met → human placeholder; **never PASS**.
/// - Real FAIL → include omission/issue summary.
///
/// P2-1: also attaches `verification` (plan checklist vs inspect) when plan md readable.
pub fn build_plan_compare(state: &RunState) -> PlanCompareSection {
    let plan = load_plan_resolved(&state.run_dir);
    let require_inspect = plan.as_ref().map(|p| p.require_inspect).unwrap_or(false);
    let handoff_md = handoff::Handoff::path_md(&state.run_dir);
    let handoff_json = handoff::Handoff::path_json(&state.run_dir);
    let handoff_present = handoff_md.exists() || handoff_json.exists();

    let view = handoff::inspect_loop_view(plan.as_ref(), state, &state.project_root);
    let mut section = fill_plan_compare(require_inspect, handoff_present, &view);
    section.verification = Some(build_report_verification(
        state,
        plan.as_ref(),
        &view,
        &section,
    ));
    section
}

/// Pure-ish fill from an already-built InspectLoopView (tests inject mock views).
pub fn fill_plan_compare(
    require_inspect: bool,
    handoff_present: bool,
    view: &InspectLoopView,
) -> PlanCompareSection {
    let verdict_raw = view
        .verdict
        .as_deref()
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty());
    let blocking = view.blocking_count;
    let residual = view.residual_count;
    let has_preview = !view.issue_preview.is_empty();

    // Real PASS only when verdict says so and no blocking leftovers.
    if matches!(verdict_raw.as_deref(), Some("PASS")) && blocking == 0 {
        let mut body = Vec::new();
        if residual > 0 {
            body.push(format!(
                "仍有 {residual} 条非阻塞残留（可接受遗漏或回补）。"
            ));
        }
        if view.accepted_residual {
            body.push("已显式接受残留遗漏。".into());
        }
        append_issue_preview(&mut body, &view.issue_preview);
        return PlanCompareSection {
            kind: PlanCompareKind::Pass,
            headline: "巡检对照计划：通过".into(),
            body_lines: body,
            is_fallback: false,
            fallback_reason: None,
            require_inspect: view.require_inspect || require_inspect,
            verdict: Some("PASS".into()),
            blocking_count: 0,
            residual_count: residual,
            verification: None,
        };
    }

    // Real FAIL / blocking path — surface omissions; not a PASS placeholder.
    if matches!(verdict_raw.as_deref(), Some("FAIL")) || blocking > 0 {
        let mut body = Vec::new();
        if blocking > 0 {
            body.push(format!("需优先处理 {blocking} 项阻塞/地图遗漏。"));
        }
        if residual > 0 {
            body.push(format!("另有 {residual} 条非阻塞残留。"));
        }
        if view.can_rework {
            body.push("可用「回补并再巡检」继续闭环。".into());
        }
        append_issue_preview(&mut body, &view.issue_preview);
        if body.is_empty() {
            body.push("巡检判定未通过，请查看 handoff / `.cco-out/inspect/`。".into());
        }
        return PlanCompareSection {
            kind: PlanCompareKind::Fail,
            headline: "巡检对照计划：有遗漏需处理".into(),
            body_lines: body,
            is_fallback: false,
            fallback_reason: None,
            require_inspect: view.require_inspect || require_inspect,
            verdict: verdict_raw.or(Some("FAIL".into())),
            blocking_count: blocking,
            residual_count: residual,
            verification: None,
        };
    }

    // --- Fallback / placeholder paths (never claim PASS) ---

    let mut reasons: Vec<String> = Vec::new();
    if !handoff_present {
        reasons.push("无 handoff 账本".into());
    }

    if require_inspect || view.require_inspect {
        // Required but no clear product yet.
        if verdict_raw.is_none() {
            reasons.push("已要求巡检但未产出 VERDICT".into());
        } else {
            reasons.push(format!(
                "巡检结论不明确（verdict={}）",
                verdict_raw.as_deref().unwrap_or("?")
            ));
        }
        let mut body = vec![
            "已要求对照计划巡检，但本轮尚无可用的通过/未通过结论。".into(),
            "步骤跑完 ≠ 已按计划验收。".into(),
        ];
        if has_preview {
            append_issue_preview(&mut body, &view.issue_preview);
        }
        return PlanCompareSection {
            kind: if verdict_raw.is_some() {
                PlanCompareKind::Unclear
            } else {
                PlanCompareKind::Pending
            },
            headline: "本轮未产出巡检结论".into(),
            body_lines: body,
            is_fallback: true,
            fallback_reason: Some(reasons.join("；")),
            require_inspect: true,
            verdict: verdict_raw,
            blocking_count: blocking,
            residual_count: residual,
            verification: None,
        };
    }

    // Inspect not required / not enabled.
    if verdict_raw.is_none() && blocking == 0 && !has_preview {
        reasons.push("未开启对照计划巡检（require_inspect=false 或无 inspect 任务）".into());
        return PlanCompareSection {
            kind: PlanCompareKind::Disabled,
            headline: "未开启对照计划巡检".into(),
            body_lines: vec![
                "本轮未开启对照计划巡检：步骤跑完 ≠ 已按计划验收。".into(),
                "可在设置里打开「拆分后附加：任务巡检」，或回聊天补充后再拆。".into(),
            ],
            is_fallback: true,
            fallback_reason: Some(reasons.join("；")),
            require_inspect: false,
            verdict: None,
            blocking_count: 0,
            residual_count: 0,
            verification: None,
        };
    }

    // Partial noise without require_inspect.
    reasons.push("有零散巡检痕迹但无明确 PASS/FAIL".into());
    let mut body = vec!["巡检结果不完整，不能当作已对照计划验收。".into()];
    append_issue_preview(&mut body, &view.issue_preview);
    PlanCompareSection {
        kind: PlanCompareKind::Unclear,
        headline: "本轮未产出巡检结论".into(),
        body_lines: body,
        is_fallback: true,
        fallback_reason: Some(reasons.join("；")),
        require_inspect: false,
        verdict: verdict_raw,
        blocking_count: blocking,
        residual_count: residual,
        verification: None,
    }
}

/// P2-1: attach plan checklist vs inspect (best-effort; no fail).
fn build_report_verification(
    state: &RunState,
    plan: Option<&PlanIR>,
    view: &InspectLoopView,
    section: &PlanCompareSection,
) -> VerificationView {
    let plan_items = std::fs::read_to_string(&state.plan_path)
        .ok()
        .map(|md| parse_acceptance_checklist(&md))
        .unwrap_or_default();
    let task_items = plan
        .map(|p| {
            collect_task_acceptance_items(p.tasks.iter().map(|t| {
                let human = t
                    .acceptance
                    .as_deref()
                    .filter(|s| !crate::domain::plan::is_runnable_verify(s));
                (t.id.as_str(), human)
            }))
        })
        .unwrap_or_default();

    let has_real_inspect = matches!(section.kind, PlanCompareKind::Pass | PlanCompareKind::Fail);
    let inspect_pending = matches!(
        section.kind,
        PlanCompareKind::Pending | PlanCompareKind::Unclear
    );

    build_verification(VerificationInputs {
        plan_items,
        task_items,
        has_real_inspect,
        blocking_count: view.blocking_count,
        residual_count: view.residual_count,
        issue_preview: view.issue_preview.clone(),
        inspect_pending,
    })
}

fn append_issue_preview(body: &mut Vec<String>, preview: &[String]) {
    for (i, line) in preview.iter().take(8).enumerate() {
        let snip: String = line.chars().take(160).collect();
        body.push(format!("{}. {}", i + 1, snip));
    }
}

/// Render `## 对照计划` markdown body (heading written by caller).
///
/// P2-1: when verification present, append「原计划要验收」sidebar (or plan-only note).
pub fn render_plan_compare_md(section: &PlanCompareSection) -> String {
    let mut md = String::new();
    md.push_str(&section.headline);
    md.push('\n');
    if !section.body_lines.is_empty() {
        md.push('\n');
        for line in &section.body_lines {
            md.push_str(&format!("- {line}\n"));
        }
    }
    if let Some(v) = &section.verification {
        md.push_str(&render_verification_md(v));
    }
    md
}

/// Render plan-checklist sidebar under 对照计划 (P2-1).
fn render_verification_md(v: &VerificationView) -> String {
    match v.source {
        VerificationSource::None => String::new(),
        VerificationSource::PlanOnly => {
            let mut md = String::new();
            if let Some(note) = &v.plan_note {
                md.push('\n');
                md.push_str(&format!("> {note}\n"));
            }
            if !v.plan_items.is_empty() {
                md.push_str("\n### 原计划要验收\n\n");
                for item in &v.plan_items {
                    let mark = if item.checked { "x" } else { " " };
                    md.push_str(&format!("- [{mark}] {}\n", item.text));
                }
            }
            if !v.task_items.is_empty() {
                md.push_str("\n### 任务级验收\n\n");
                for t in &v.task_items {
                    md.push_str(&format!("- `{}` · {}\n", t.task_id, t.text));
                }
            }
            md
        }
        VerificationSource::Inspect => {
            let mut md = String::new();
            if let Some(note) = &v.plan_note {
                md.push('\n');
                md.push_str(&format!("> {note}\n"));
            }
            if !v.plan_items.is_empty() {
                md.push_str("\n### 原计划要验收\n\n");
                for item in &v.plan_items {
                    let mark = if item.checked { "x" } else { " " };
                    md.push_str(&format!("- [{mark}] {}\n", item.text));
                }
            }
            if !v.task_items.is_empty() {
                md.push_str("\n### 任务级验收\n\n");
                for t in &v.task_items {
                    md.push_str(&format!("- `{}` · {}\n", t.task_id, t.text));
                }
            }
            md
        }
    }
}

/// Human elapsed between start and finish (or "未计时").
pub fn format_elapsed_human(started: DateTime<Utc>, finished: Option<DateTime<Utc>>) -> String {
    let Some(fin) = finished else {
        return "进行中 / 未计时".into();
    };
    let secs = (fin - started).num_seconds().max(0) as u64;
    if secs < 60 {
        return format!("{secs} 秒");
    }
    let mins = secs / 60;
    let rem = secs % 60;
    if mins < 60 {
        if rem == 0 {
            format!("{mins} 分")
        } else {
            format!("{mins} 分 {rem} 秒")
        }
    } else {
        let hours = mins / 60;
        let m = mins % 60;
        if m == 0 {
            format!("{hours} 小时")
        } else {
            format!("{hours} 小时 {m} 分")
        }
    }
}

/// Follow-up bullets from plan-compare kind (no invented PASS).
pub fn follow_up_lines(section: &PlanCompareSection) -> Vec<String> {
    match section.kind {
        PlanCompareKind::Pass => {
            if section.residual_count > 0 {
                vec![
                    "可接受残留遗漏，或开一轮轻量回补。".into(),
                    "完整证据见 handoff 与 `.cco-out/inspect/`。".into(),
                ]
            } else {
                vec!["本轮对照计划已通过；可归档或继续下一项。".into()]
            }
        }
        PlanCompareKind::Fail => vec![
            "优先处理阻塞遗漏；可用「回补并再巡检」。".into(),
            "对照细节见 `.cco-out/inspect/ISSUES.md` 与 handoff。".into(),
        ],
        PlanCompareKind::Pending | PlanCompareKind::Unclear => vec![
            "等待或补跑巡检任务，产出 VERDICT / ISSUES 后再验收。".into(),
            "勿将步骤全部 Done 直接当作对照计划通过。".into(),
        ],
        PlanCompareKind::Disabled => vec![
            "若需要「对照计划」验收：设置里打开任务巡检，或计划加 role=inspect。".into(),
            "当前结果仅反映步骤执行状态。".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::handoff::InspectLoopView;

    #[test]
    fn fill_disabled_never_pass() {
        let view = InspectLoopView::default();
        let s = fill_plan_compare(false, false, &view);
        assert!(s.is_fallback);
        assert_eq!(s.kind, PlanCompareKind::Disabled);
        assert!(s.headline.contains("未开启"));
        assert!(!s.headline.contains("通过"));
        assert!(s.fallback_reason.is_some());
        let md = render_plan_compare_md(&s);
        assert!(!md.contains("PASS"));
        assert!(!md.contains("通过"));
    }

    #[test]
    fn fill_pending_require_inspect() {
        let view = InspectLoopView {
            require_inspect: true,
            ..Default::default()
        };
        let s = fill_plan_compare(true, true, &view);
        assert!(s.is_fallback);
        assert_eq!(s.kind, PlanCompareKind::Pending);
        assert!(s.headline.contains("未产出巡检结论"));
        assert!(s
            .fallback_reason
            .as_deref()
            .unwrap_or("")
            .contains("未产出 VERDICT"));
    }

    #[test]
    fn fill_real_fail_with_issues() {
        let view = InspectLoopView {
            verdict: Some("FAIL".into()),
            blocking_count: 2,
            residual_count: 1,
            issue_preview: vec![
                "I-1 severity=blocking missing report section".into(),
                "I-2 severity=map GEB pointer".into(),
            ],
            can_rework: true,
            require_inspect: true,
            ..Default::default()
        };
        let s = fill_plan_compare(true, true, &view);
        assert!(!s.is_fallback);
        assert_eq!(s.kind, PlanCompareKind::Fail);
        assert!(s.headline.contains("有遗漏"));
        assert!(s.body_lines.iter().any(|l| l.contains("I-1")));
        assert!(s.fallback_reason.is_none());
    }

    #[test]
    fn fill_real_pass() {
        let view = InspectLoopView {
            verdict: Some("PASS".into()),
            require_inspect: true,
            ..Default::default()
        };
        let s = fill_plan_compare(true, true, &view);
        assert!(!s.is_fallback);
        assert_eq!(s.kind, PlanCompareKind::Pass);
        assert!(s.headline.contains("通过"));
    }

    #[test]
    fn elapsed_human() {
        let start = Utc::now();
        let fin = start + chrono::Duration::seconds(125);
        assert_eq!(format_elapsed_human(start, Some(fin)), "2 分 5 秒");
        assert_eq!(format_elapsed_human(start, None), "进行中 / 未计时");
    }

    #[test]
    fn render_md_with_plan_only_verification() {
        use crate::domain::chat::{PlanChecklistItem, VerificationSource, VerificationView};
        let mut s = fill_plan_compare(false, false, &InspectLoopView::default());
        s.verification = Some(VerificationView {
            source: VerificationSource::PlanOnly,
            plan_items: vec![PlanChecklistItem {
                text: "主 CTA 清晰".into(),
                checked: false,
                has_checkbox: true,
            }],
            plan_count: 1,
            task_items: vec![],
            items: vec![],
            plan_note: Some("计划写了 1 条验收，本轮未自动对照".into()),
            blocking_count: None,
            residual_count: None,
        });
        let md = render_plan_compare_md(&s);
        assert!(md.contains("原计划要验收"), "got:\n{md}");
        assert!(md.contains("主 CTA 清晰"), "got:\n{md}");
        assert!(md.contains("计划写了 1 条验收"), "got:\n{md}");
        // Still never invent PASS.
        assert!(!md.contains("PASS"));
    }
}
