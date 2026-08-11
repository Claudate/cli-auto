//! Clarify-phase contract: required slots, three on-ramps, missing-slot pure rules.
//!
//! Split by pure-function boundary (arch hard ≤600):
//! - [`entry`] — three on-ramps (ClarifyEntry)
//! - [`slot`] — required slots + detection report
//! - [`state`] — ClarifyState + pure mutators
//!
//! [INPUT]: slot fill state only (no path / fs / provider)
//! [OUTPUT]: missing list · assumption-pass flags · updated state
//! [POS]: Domain Chat 澄清相真相源；**禁止** confirm_start / spawn / 第二 Planner
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md 与本头部

mod entry;
mod slot;
mod state;

pub use entry::ClarifyEntry;
pub use slot::{
    detect_missing_slots, ClarifySlotId, MissingSlotsReport, REQUIRED_SLOTS,
};
pub use state::{
    apply_skip_with_assumptions, has_assumed_fills, set_slot_fill, ClarifyAssumption,
    ClarifyOptionalFill, ClarifyPhase, ClarifySlotFill, ClarifyState, SlotFillKind,
    CLARIFY_SCHEMA_VERSION,
};
