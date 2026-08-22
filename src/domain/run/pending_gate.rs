//! Pending user gate — human-readable "waiting on you" object (LX2).
//!
//! LoopX makes "needs a human to decide" a first-class structured object so the
//! loop can name *which* question it is stuck on, not a vague "waiting for owner".
//! Leaf already **stops** for optionals (rule 14); LX2 only upgrades that boolean
//! into a PM-facing sentence: 「当前等你回答：X」.
//!
//! [INPUT]: optional-task snapshot (title · is_system_post · included)
//! [OUTPUT]: Option<PendingUserGate> (pure human copy; no IO, no policy beyond
//!           the existing optional-confirm rule mirrored from the confirm desk)
//! [POS]: domain/run — `app`/`plan` fills the DTO; web only renders (rule 22)
//! [PROTOCOL]: 复用 status_line 人话风格；不新增开跑策略；不违反规则 14；
//!   变更时更新 domain/run/mod.rs

use serde::Serialize;

/// What kind of human decision the run is parked on.
///
/// `OptionalConfirm` is the only kind produced today (mirrors the confirm-desk
/// optional gate). `InspectReview` is reserved for a future巡检-review gate so
/// the wire shape is stable; it is **not** constructed yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    /// A plan has optional steps that will not run unless the user opts in.
    OptionalConfirm,
    /// Reserved: inspect VERDICT needs a human call (future LX).
    InspectReview,
}

/// A single "waiting on you" item, projected to human copy for the desk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingUserGate {
    pub kind: GateKind,
    /// Front-and-center question, e.g.「是否执行可选任务：部署到预览环境？」
    pub question: String,
    /// Why the run is parked here, e.g.「这些步骤标了「可选」，默认不执行」。
    pub why: String,
}

/// Minimal pure snapshot of one optional task (no view/DTO dependency).
#[derive(Debug, Clone)]
pub struct OptionalTaskSnap {
    pub title: String,
    pub is_system_post: bool,
    /// Whether the task is currently checked to run (optional defaults false).
    pub include: bool,
}

/// Strip a leading/trailing 「（可选）」/「(optional)」 marker for cleaner copy.
fn clean_title(title: &str) -> String {
    let t = title.trim();
    let t = t
        .trim_start_matches("（可选）")
        .trim_start_matches("(可选)")
        .trim();
    t.trim_end_matches("（可选）")
        .trim_end_matches("(可选)")
        .trim()
        .to_string()
}

/// Derive the pending optional-confirm gate from the plan's optional tasks.
///
/// Mirrors the confirm desk's existing stop rule (no new policy):
/// - any **business** optional (non system-post) → always parked (default off,
///   needs explicit opt-in);
/// - else **system-post** optionals with any unchecked → parked so the user sees
///   what will be skipped;
/// - otherwise no gate.
pub fn pending_optional_gate(snaps: &[OptionalTaskSnap]) -> Option<PendingUserGate> {
    let business: Vec<&OptionalTaskSnap> = snaps.iter().filter(|s| !s.is_system_post).collect();
    if !business.is_empty() {
        let first = clean_title(&business[0].title);
        let question = if business.len() == 1 {
            format!("是否执行可选任务：{first}？")
        } else {
            format!("是否执行这 {} 个可选任务（如「{first}」）？", business.len())
        };
        return Some(PendingUserGate {
            kind: GateKind::OptionalConfirm,
            question,
            why: "这些步骤标了「可选」，默认不执行，需你确认是否加入。".to_string(),
        });
    }
    let sys_off = snaps
        .iter()
        .filter(|s| s.is_system_post && !s.include)
        .count();
    if sys_off > 0 {
        return Some(PendingUserGate {
            kind: GateKind::OptionalConfirm,
            question: format!("系统收尾有 {sys_off} 项未勾选，确认后再开始？"),
            why: "系统收尾（如推送 / 开 PR）默认可跑，有几项未勾选。".to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(title: &str, sys: bool, inc: bool) -> OptionalTaskSnap {
        OptionalTaskSnap {
            title: title.to_string(),
            is_system_post: sys,
            include: inc,
        }
    }

    #[test]
    fn none_when_no_optionals() {
        assert!(pending_optional_gate(&[]).is_none());
    }

    #[test]
    fn business_optional_always_parks() {
        // Even when checked to run, a business optional needs explicit confirm.
        let g = pending_optional_gate(&[snap("部署到预览环境", false, true)]).unwrap();
        assert_eq!(g.kind, GateKind::OptionalConfirm);
        assert_eq!(g.question, "是否执行可选任务：部署到预览环境？");
    }

    #[test]
    fn business_optional_strips_marker() {
        let g = pending_optional_gate(&[snap("（可选）部署到预览环境", false, false)]).unwrap();
        assert_eq!(g.question, "是否执行可选任务：部署到预览环境？");
    }

    #[test]
    fn multiple_business_optionals_count_and_first() {
        let g = pending_optional_gate(&[
            snap("发布公告", false, false),
            snap("部署预览", false, false),
        ])
        .unwrap();
        assert_eq!(g.question, "是否执行这 2 个可选任务（如「发布公告」）？");
    }

    #[test]
    fn system_post_all_checked_no_gate() {
        // Only system-post optionals, all included → no park (can auto-start).
        assert!(pending_optional_gate(&[snap("sys push", true, true)]).is_none());
    }

    #[test]
    fn system_post_some_unchecked_parks() {
        let g = pending_optional_gate(&[
            snap("推送", true, true),
            snap("开 PR", true, false),
        ])
        .unwrap();
        assert_eq!(g.kind, GateKind::OptionalConfirm);
        assert_eq!(g.question, "系统收尾有 1 项未勾选，确认后再开始？");
    }

    #[test]
    fn business_wins_over_system_post() {
        let g = pending_optional_gate(&[
            snap("业务可选", false, false),
            snap("开 PR", true, false),
        ])
        .unwrap();
        assert_eq!(g.question, "是否执行可选任务：业务可选？");
    }
}
