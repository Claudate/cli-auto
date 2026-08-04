//! Clarify-phase contract: required slots, three on-ramps, missing-slot pure rules.
//!
//! ## Pure contract vs session IO
//! | Pure (this module) | IO (`services/chat`) |
//! |--------------------|----------------------|
//! | ClarifyEntry · ClarifySlotId · ClarifyState | session JSON load/save |
//! | detect_missing_slots · apply_skip_with_assumptions | UI / prompt wiring |
//! | set_slot_fill (no silent overwrite of Explicit) | — |
//!
//! [INPUT]: slot fill state only (no path / fs / provider)
//! [OUTPUT]: missing list · assumption-pass flags · updated state
//! [POS]: Domain Chat 澄清相真相源；**禁止** confirm_start / spawn / 第二 Planner
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md 与本头部

use serde::{Deserialize, Serialize};

/// Wire schema tag for clarify session meta (forward-compat).
pub const CLARIFY_SCHEMA_VERSION: u32 = 1;

// ─── Three on-ramps ───────────────────────────────────────────────────────────

/// Product on-ramps for the clarify phase (vibe-check subset).
///
/// Chinese product labels:
/// - [`ThinkFirst`]: 想清楚再说
/// - [`IdeaToPlan`]: 从想法到计划（默认）
/// - [`PlanOnly`]: 已想清，直接写计划
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyEntry {
    /// 想清楚再说 — Brief only; plan optional.
    ThinkFirst,
    /// 从想法到计划 — default: clarify → Brief → plan fence.
    #[default]
    IdeaToPlan,
    /// 已想清，直接写计划 — skip grilling; still force min plan chapters.
    PlanOnly,
}

impl ClarifyEntry {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThinkFirst => "think_first",
            Self::IdeaToPlan => "idea_to_plan",
            Self::PlanOnly => "plan_only",
        }
    }

    /// Chinese product label (UI / prompt).
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::ThinkFirst => "想清楚再说",
            Self::IdeaToPlan => "从想法到计划",
            Self::PlanOnly => "已想清，直接写计划",
        }
    }

    /// Parse wire key or common Chinese label. Unknown → None.
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() {
            return None;
        }
        match s {
            "think_first" | "think-first" | "想清楚再说" | "想清楚" => {
                Some(Self::ThinkFirst)
            }
            "idea_to_plan" | "idea-to-plan" | "从想法到计划" | "default" => {
                Some(Self::IdeaToPlan)
            }
            "plan_only"
            | "plan-only"
            | "已想清直接写计划"
            | "已想清，直接写计划"
            | "直接写计划" => Some(Self::PlanOnly),
            _ => None,
        }
    }
}

// ─── Required slots ───────────────────────────────────────────────────────────

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
    /// 怎样算做完 — done-when / acceptance.
    DoneWhen,
}

/// Canonical order of required slots (product checklist order).
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
            "target_audience" | "目标对象" => Some(Self::TargetAudience),
            "pain_moment" | "痛苦时刻" => Some(Self::PainMoment),
            "observable_outcome" | "可观察结果" => Some(Self::ObservableOutcome),
            "non_goals" | "明确不做" | "不做" => Some(Self::NonGoals),
            "done_when" | "怎样算做完" | "验收" => Some(Self::DoneWhen),
            _ => None,
        }
    }
}

// ─── Fill kinds & phase ───────────────────────────────────────────────────────

/// How a slot got its value. Assumed ≠ clarified fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SlotFillKind {
    /// User explicitly answered.
    #[default]
    Explicit,
    /// User said「你定 / 直接出计划」— recorded as hypothesis, not fact.
    Assumed,
    /// Soft-filled from context. Must not silently overwrite Explicit.
    SoftFill,
}

/// Coarse phase marker for UI/prompt (not a second planner state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClarifyPhase {
    #[default]
    NotStarted,
    Clarifying,
    BriefReady,
    /// User claimed Brief → plan draft (still **not** confirm_start).
    ClaimedToPlan,
    /// Escape hatch: skip grilling → plan path with assumptions.
    SkippedToPlan,
}

// ─── State shapes ─────────────────────────────────────────────────────────────

/// One filled required slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifySlotFill {
    pub id: ClarifySlotId,
    pub value: String,
    #[serde(default)]
    pub kind: SlotFillKind,
}

/// Optional extensible slots (string key; never blocks main path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifyOptionalFill {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub kind: SlotFillKind,
}

/// Assumption recorded on skip /「你定」— never rewritten as explicit fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClarifyAssumption {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<ClarifySlotId>,
    pub text: String,
}

fn is_false(b: &bool) -> bool {
    !*b
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

fn default_clarify_schema() -> u32 {
    CLARIFY_SCHEMA_VERSION
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

// ─── Detection report ─────────────────────────────────────────────────────────

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

// ─── Pure functions ───────────────────────────────────────────────────────────

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
    fn empty_slots_detect_at_least_one_required_missing() {
        let state = ClarifyState::default();
        let report = detect_missing_slots(&state);
        assert!(
            report.missing_required.len() >= 1,
            "empty → ≥1 missing, got {:?}",
            report.missing_required
        );
        assert_eq!(report.missing_required.len(), REQUIRED_SLOTS.len());
        assert!(report.blocks_plan);
        assert!(!report.may_proceed_with_assumptions);
    }

    #[test]
    fn five_slots_filled_no_required_missing() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        fill_all_explicit(&mut state);
        let report = detect_missing_slots(&state);
        assert!(
            report.missing_required.is_empty(),
            "full → no missing, got {:?}",
            report.missing_required
        );
        assert!(!report.blocks_plan);
        assert!(report.assumed_slot_ids.is_empty());
    }

    #[test]
    fn skip_marks_assumptions_and_does_not_forge_explicit_facts() {
        let mut state = ClarifyState::new(ClarifyEntry::IdeaToPlan);
        // Only one explicit answer
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
        // After skip, assumed placeholders fill the gaps → no missing
        assert!(
            report.missing_required.is_empty(),
            "skip fills assumed placeholders; missing={:?}",
            report.missing_required
        );
        assert!(report.may_proceed_with_assumptions);
        assert!(!report.blocks_plan);
        // Assumed slots present; TargetAudience stays Explicit
        assert!(report.assumed_slot_ids.len() >= 4);
        assert_eq!(
            state.slot(ClarifySlotId::TargetAudience).map(|s| s.kind),
            Some(SlotFillKind::Explicit)
        );
        for id in report.assumed_slot_ids {
            let fill = state.slot(id).expect("assumed fill");
            assert_eq!(fill.kind, SlotFillKind::Assumed);
            assert!(
                fill.value.contains("假设"),
                "must label hypothesis, not forge fact: {}",
                fill.value
            );
        }
        assert!(
            state
                .assumptions
                .iter()
                .any(|a| a.text.contains("直接出计划")),
            "assumptions audit: {:?}",
            state.assumptions
        );
        // has_assumed_fills helper
        assert!(has_assumed_fills(&state));
    }

    #[test]
    fn plan_only_entry_allows_assumption_pass_without_skip_flag() {
        let state = ClarifyState::new(ClarifyEntry::PlanOnly);
        let report = detect_missing_slots(&state);
        assert!(!report.missing_required.is_empty());
        assert!(report.may_proceed_with_assumptions);
        assert!(!report.blocks_plan);
    }

    #[test]
    fn soft_fill_does_not_overwrite_explicit() {
        let mut state = ClarifyState::default();
        assert!(set_slot_fill(
            &mut state,
            ClarifySlotId::DoneWhen,
            "用户写的验收",
            SlotFillKind::Explicit
        ));
        let changed = set_slot_fill(
            &mut state,
            ClarifySlotId::DoneWhen,
            "系统瞎猜的验收",
            SlotFillKind::SoftFill,
        );
        assert!(!changed);
        assert_eq!(
            state
                .slot(ClarifySlotId::DoneWhen)
                .map(|s| s.value.as_str()),
            Some("用户写的验收")
        );
        assert_eq!(
            state.slot(ClarifySlotId::DoneWhen).map(|s| s.kind),
            Some(SlotFillKind::Explicit)
        );
    }

    #[test]
    fn soft_fill_can_seed_empty_slot() {
        let mut state = ClarifyState::default();
        assert!(set_slot_fill(
            &mut state,
            ClarifySlotId::NonGoals,
            "暂不做社区",
            SlotFillKind::SoftFill
        ));
        assert_eq!(
            state.slot(ClarifySlotId::NonGoals).map(|s| s.kind),
            Some(SlotFillKind::SoftFill)
        );
    }

    #[test]
    fn entry_enum_serde_roundtrip() {
        for entry in [
            ClarifyEntry::ThinkFirst,
            ClarifyEntry::IdeaToPlan,
            ClarifyEntry::PlanOnly,
        ] {
            let json = serde_json::to_string(&entry).expect("ser");
            let back: ClarifyEntry = serde_json::from_str(&json).expect("de");
            assert_eq!(back, entry, "json={json}");
            // parse accepts as_str
            assert_eq!(ClarifyEntry::parse(entry.as_str()), Some(entry));
            // Chinese labels parse
            assert_eq!(ClarifyEntry::parse(entry.label_zh()), Some(entry));
        }
        // default is idea_to_plan
        assert_eq!(ClarifyEntry::default(), ClarifyEntry::IdeaToPlan);
        let def_json = serde_json::to_string(&ClarifyEntry::default()).unwrap();
        assert_eq!(def_json, "\"idea_to_plan\"");
    }

    #[test]
    fn clarify_state_serde_roundtrip_with_slots() {
        let mut state = ClarifyState::new(ClarifyEntry::ThinkFirst);
        fill_all_explicit(&mut state);
        state.phase = ClarifyPhase::BriefReady;
        state.optional.push(ClarifyOptionalFill {
            key: "competitor".into(),
            value: "竞品只做提醒".into(),
            kind: SlotFillKind::Explicit,
        });

        let json = serde_json::to_string_pretty(&state).expect("ser");
        let back: ClarifyState = serde_json::from_str(&json).expect("de");
        assert_eq!(back, state);
        assert_eq!(back.schema_version, CLARIFY_SCHEMA_VERSION);
        assert_eq!(back.entry, ClarifyEntry::ThinkFirst);
        assert_eq!(back.slots.len(), 5);
        assert_eq!(back.optional.len(), 1);
    }

    #[test]
    fn blank_value_counts_as_missing() {
        let mut state = ClarifyState::default();
        // Directly push blank — set_slot_fill rejects empty
        state.slots.push(ClarifySlotFill {
            id: ClarifySlotId::TargetAudience,
            value: "   ".into(),
            kind: SlotFillKind::Explicit,
        });
        let report = detect_missing_slots(&state);
        assert!(report
            .missing_required
            .contains(&ClarifySlotId::TargetAudience));
    }

    #[test]
    fn required_slots_are_exactly_five_named() {
        assert_eq!(REQUIRED_SLOTS.len(), 5);
        let labels: Vec<_> = REQUIRED_SLOTS.iter().map(|s| s.label_zh()).collect();
        assert_eq!(
            labels,
            vec![
                "目标对象",
                "痛苦时刻",
                "可观察结果",
                "明确不做",
                "怎样算做完"
            ]
        );
    }
}
