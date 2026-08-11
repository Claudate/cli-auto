//! Required clarify slots + missing-slot pure detection.

use serde::{Deserialize, Serialize};

use super::entry::ClarifyEntry;
use super::state::{ClarifyState, SlotFillKind};

/// Minimal required clarify slots (stable wire keys).
///
/// Chinese:
/// - TargetAudience · 目标对象
/// - PainMoment · 痛苦时刻
/// - ObservableOutcome · 可观察结果
/// - NonGoals · 明确不做
/// - DoneWhen · 怎样算做完
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClarifySlotId {
    /// 目标对象 — who is this for.
    TargetAudience,
    /// 痛苦时刻 — when it hurts / trigger context.
    PainMoment,
    /// 可观察结果 — what success looks like from outside.
    ObservableOutcome,
    /// 明确不做 — non-goals / out of scope.
    NonGoals,
    /// 怎样算做完 — acceptance / done-when.
    DoneWhen,
}

/// Required slots in stable product order (detection + UI progress).
pub const REQUIRED_SLOTS: &[ClarifySlotId] = &[
    ClarifySlotId::TargetAudience,
    ClarifySlotId::PainMoment,
    ClarifySlotId::ObservableOutcome,
    ClarifySlotId::NonGoals,
    ClarifySlotId::DoneWhen,
];

impl ClarifySlotId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetAudience => "target_audience",
            Self::PainMoment => "pain_moment",
            Self::ObservableOutcome => "observable_outcome",
            Self::NonGoals => "non_goals",
            Self::DoneWhen => "done_when",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::TargetAudience => "目标对象",
            Self::PainMoment => "痛苦时刻",
            Self::ObservableOutcome => "可观察结果",
            Self::NonGoals => "明确不做",
            Self::DoneWhen => "怎样算做完",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        match s {
            "target_audience" | "目标对象" | "给谁" => Some(Self::TargetAudience),
            "pain_moment" | "痛苦时刻" | "痛点" => Some(Self::PainMoment),
            "observable_outcome" | "可观察结果" | "做成什么样" => Some(Self::ObservableOutcome),
            "non_goals" | "明确不做" | "不做" | "非目标" => Some(Self::NonGoals),
            "done_when" | "怎样算做完" | "验收" | "成功标准" => Some(Self::DoneWhen),
            _ => None,
        }
    }
}

/// Pure result of missing-slot detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingSlotsReport {
    /// Required slots with no non-empty fill.
    pub missing_required: Vec<ClarifySlotId>,
    /// Slot ids currently filled as Assumed (not clarified facts).
    pub assumed_slot_ids: Vec<ClarifySlotId>,
    /// True when skip / PlanOnly allows proceeding despite gaps (with assumptions).
    pub may_proceed_with_assumptions: bool,
    /// True when required gaps remain **and** assumption-pass is not granted.
    pub blocks_plan: bool,
}

/// Detect missing required slots from a fill state.
///
/// - Empty / blank values count as missing.
/// - Assumed fills count as "present" for gap list, but stay in `assumed_slot_ids`
///   so callers never treat them as clarified facts.
/// - `may_proceed_with_assumptions` when `skip_requested` or entry is [`ClarifyEntry::PlanOnly`].
/// - `blocks_plan` when there are missing required slots and assumption-pass is false.
///
/// Does **not** spawn, confirm, or write plan fences.
pub fn detect_missing_slots(state: &ClarifyState) -> MissingSlotsReport {
    let mut missing_required = Vec::new();
    let mut assumed_slot_ids = Vec::new();

    for &id in REQUIRED_SLOTS {
        match state.slot(id) {
            Some(fill) if !fill.value.trim().is_empty() => {
                if fill.kind == SlotFillKind::Assumed {
                    assumed_slot_ids.push(id);
                }
            }
            _ => missing_required.push(id),
        }
    }

    let may_proceed_with_assumptions =
        state.skip_requested || state.entry == ClarifyEntry::PlanOnly;
    let blocks_plan = !missing_required.is_empty() && !may_proceed_with_assumptions;

    MissingSlotsReport {
        missing_required,
        assumed_slot_ids,
        may_proceed_with_assumptions,
        blocks_plan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::{set_slot_fill, ClarifyState, SlotFillKind};
    use super::super::entry::ClarifyEntry;

    #[test]
    fn empty_slots_detect_all_required_missing() {
        let state = ClarifyState::default();
        let report = detect_missing_slots(&state);
        assert_eq!(report.missing_required.len(), REQUIRED_SLOTS.len());
        assert!(report.blocks_plan);
        assert!(!report.may_proceed_with_assumptions);
    }

    #[test]
    fn five_slots_filled_no_required_missing() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        for (id, v) in [
            (ClarifySlotId::TargetAudience, "出海运营同学"),
            (ClarifySlotId::PainMoment, "模糊一句就空心出稿"),
            (ClarifySlotId::ObservableOutcome, "Brief 可认领并落 plan"),
            (ClarifySlotId::NonGoals, "不做 Crazy 8 / ODI 全量"),
            (ClarifySlotId::DoneWhen, "五槽齐全且可分配计划"),
        ] {
            assert!(set_slot_fill(&mut state, id, v, SlotFillKind::Explicit));
        }
        let report = detect_missing_slots(&state);
        assert!(report.missing_required.is_empty());
        assert!(!report.blocks_plan);
        assert!(report.assumed_slot_ids.is_empty());
    }

    #[test]
    fn plan_only_may_proceed_with_gaps() {
        let state = ClarifyState::new(ClarifyEntry::PlanOnly);
        let report = detect_missing_slots(&state);
        assert!(!report.missing_required.is_empty());
        assert!(report.may_proceed_with_assumptions);
        assert!(!report.blocks_plan);
    }
}
