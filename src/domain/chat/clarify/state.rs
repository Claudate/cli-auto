//! ClarifyState + pure mutators (set_slot_fill / skip with assumptions).

use serde::{Deserialize, Serialize};

use super::entry::ClarifyEntry;
use super::slot::{detect_missing_slots, ClarifySlotId};

/// Wire schema tag for clarify session meta (forward-compat).
pub const CLARIFY_SCHEMA_VERSION: u32 = 1;

/// How a slot value was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SlotFillKind {
    /// User typed / picked explicitly.
    #[default]
    Explicit,
    /// Soft-fill from prior context / draft — must not overwrite Explicit.
    SoftFill,
    /// Hypothesis from skip /「你定」— never present as user-confirmed fact.
    Assumed,
}

/// One required-slot fill row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifySlotFill {
    pub id: ClarifySlotId,
    pub value: String,
    #[serde(default)]
    pub kind: SlotFillKind,
}

/// Free-form optional slot (competitor one-liner, first users, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifyOptionalFill {
    pub key: String,
    pub value: String,
}

/// Clarify-phase progress marker (session meta only; not a second planner).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyPhase {
    #[default]
    NotStarted,
    Clarifying,
    BriefReady,
    Claimed,
    SkippedToPlan,
}

/// Hypothesis recorded on skip /「你定」.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifyAssumption {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<ClarifySlotId>,
    pub text: String,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_clarify_schema() -> u32 {
    CLARIFY_SCHEMA_VERSION
}

/// Clarify-phase state carried on chat session meta.
///
/// Coexists with messages / draft_plan; does **not** introduce a second Planner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifyState {
    /// Schema tag for forward-compat readers.
    #[serde(default = "default_clarify_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub entry: ClarifyEntry,
    #[serde(default)]
    pub phase: ClarifyPhase,
    /// Required-slot fills (and only those; optional live in `optional`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<ClarifySlotFill>,
    /// Free-form optional slots (e.g. competitor one-liner, first users).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<ClarifyOptionalFill>,
    /// Hypotheses from skip /「你定」. Must not be presented as user-confirmed fact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<ClarifyAssumption>,
    /// User requested「直接出计划 / 你定」escape.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_requested: bool,
}

impl Default for ClarifyState {
    fn default() -> Self {
        Self {
            schema_version: CLARIFY_SCHEMA_VERSION,
            entry: ClarifyEntry::default(),
            phase: ClarifyPhase::default(),
            slots: Vec::new(),
            optional: Vec::new(),
            assumptions: Vec::new(),
            skip_requested: false,
        }
    }
}

impl ClarifyState {
    pub fn new(entry: ClarifyEntry) -> Self {
        Self {
            entry,
            phase: ClarifyPhase::NotStarted,
            ..Self::default()
        }
    }

    /// Lookup a required-slot fill by id.
    pub fn slot(&self, id: ClarifySlotId) -> Option<&ClarifySlotFill> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Non-empty value present (any fill kind).
    pub fn is_slot_filled(&self, id: ClarifySlotId) -> bool {
        self.slot(id)
            .map(|s| !s.value.trim().is_empty())
            .unwrap_or(false)
    }
}

/// Set or replace a required-slot fill.
///
/// Soft-fill **must not** silently overwrite an Explicit value (returns false, no change).
/// Explicit / Assumed may replace any prior fill.
///
/// Returns true when state changed.
pub fn set_slot_fill(
    state: &mut ClarifyState,
    id: ClarifySlotId,
    value: impl Into<String>,
    kind: SlotFillKind,
) -> bool {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let value = trimmed.to_string();

    if let Some(existing) = state.slots.iter_mut().find(|s| s.id == id) {
        if kind == SlotFillKind::SoftFill && existing.kind == SlotFillKind::Explicit {
            // soft-fill 不得静默覆盖已显式指定
            return false;
        }
        if existing.value == value && existing.kind == kind {
            return false;
        }
        existing.value = value;
        existing.kind = kind;
    } else {
        state.slots.push(ClarifySlotFill { id, value, kind });
    }

    if state.phase == ClarifyPhase::NotStarted {
        state.phase = ClarifyPhase::Clarifying;
    }
    true
}

/// Mark user skip（直接出计划 / 你定）and record assumptions for still-missing required slots.
///
/// - Sets `skip_requested = true`
/// - Phase → [`ClarifyPhase::SkippedToPlan`]
/// - For each missing required slot: inserts an Assumed placeholder (does **not** invent
///   product facts — placeholder text is explicitly labeled as hypothesis)
/// - Appends rows to `assumptions` for audit
///
/// `user_note` is optional free text from the user (e.g. 「你定」).
pub fn apply_skip_with_assumptions(state: &mut ClarifyState, user_note: Option<&str>) {
    state.skip_requested = true;
    state.phase = ClarifyPhase::SkippedToPlan;

    let note = user_note.map(str::trim).filter(|s| !s.is_empty());
    let report = detect_missing_slots(state);

    for id in report.missing_required {
        let text = match note {
            Some(n) => format!(
                "假设（用户跳过·{}）：待写计划时补全「{}」",
                n,
                id.label_zh()
            ),
            None => format!("假设（用户跳过）：待写计划时补全「{}」", id.label_zh()),
        };
        // Assumed fill: present for gap list, never Explicit fact.
        let _ = set_slot_fill(state, id, text.clone(), SlotFillKind::Assumed);
        state.assumptions.push(ClarifyAssumption {
            slot: Some(id),
            text,
        });
    }

    if let Some(n) = note {
        // Global skip note (not tied to a single slot) if not already recorded.
        let already = state
            .assumptions
            .iter()
            .any(|a| a.slot.is_none() && a.text.contains(n));
        if !already {
            state.assumptions.push(ClarifyAssumption {
                slot: None,
                text: format!("用户跳过澄清：{n}"),
            });
        }
    }
}

/// Whether any required slot is filled as Assumed (for UI "hypothesis" badges).
pub fn has_assumed_fills(state: &ClarifyState) -> bool {
    state
        .slots
        .iter()
        .any(|s| s.kind == SlotFillKind::Assumed && !s.value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::slot::{detect_missing_slots, ClarifySlotId, REQUIRED_SLOTS};

    fn fill_all_explicit(state: &mut ClarifyState) {
        let pairs = [
            (ClarifySlotId::TargetAudience, "出海运营同学"),
            (ClarifySlotId::PainMoment, "模糊一句就空心出稿"),
            (ClarifySlotId::ObservableOutcome, "Brief 可认领并落 plan"),
            (ClarifySlotId::NonGoals, "不做 Crazy 8 / ODI 全量"),
            (ClarifySlotId::DoneWhen, "五槽齐全且可分配计划"),
        ];
        for (id, v) in pairs {
            assert!(set_slot_fill(state, id, v, SlotFillKind::Explicit));
        }
    }

    #[test]
    fn soft_fill_does_not_overwrite_explicit() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        assert!(set_slot_fill(
            &mut state,
            ClarifySlotId::TargetAudience,
            "PM",
            SlotFillKind::Explicit
        ));
        assert!(!set_slot_fill(
            &mut state,
            ClarifySlotId::TargetAudience,
            "运营",
            SlotFillKind::SoftFill
        ));
        assert_eq!(
            state.slot(ClarifySlotId::TargetAudience).map(|s| s.value.as_str()),
            Some("PM")
        );
    }

    #[test]
    fn skip_marks_assumptions_and_does_not_forge_explicit_facts() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        assert!(set_slot_fill(
            &mut state,
            ClarifySlotId::TargetAudience,
            "PM",
            SlotFillKind::Explicit
        ));

        apply_skip_with_assumptions(&mut state, Some("直接出计划"));

        assert!(state.skip_requested);
        assert_eq!(state.phase, ClarifyPhase::SkippedToPlan);

        let report = detect_missing_slots(&state);
        assert!(
            report.missing_required.is_empty(),
            "skip fills assumed placeholders; missing={:?}",
            report.missing_required
        );
        assert!(report.may_proceed_with_assumptions);
        assert!(!report.blocks_plan);
        assert!(report.assumed_slot_ids.len() >= 4);
        assert_eq!(
            state.slot(ClarifySlotId::TargetAudience).map(|s| s.kind),
            Some(SlotFillKind::Explicit)
        );
        assert!(has_assumed_fills(&state));
        assert_eq!(REQUIRED_SLOTS.len(), 5);
    }

    #[test]
    fn fill_all_then_no_assumed() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        fill_all_explicit(&mut state);
        assert!(!has_assumed_fills(&state));
        let report = detect_missing_slots(&state);
        assert!(report.missing_required.is_empty());
    }
}
