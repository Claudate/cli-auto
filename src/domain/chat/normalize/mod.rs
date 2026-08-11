//! Plan markdown normalize / acceptance / verification (G0 · P1-4 · P2-1).
//!
//! Split by pure-function boundary (arch hard ≤600):
//! - [`plan_md`] — normalize / structure plan markdown
//! - [`acceptance`] — acceptance quality + checklist
//! - [`verification`] — plan vs inspect side-by-side view
//!
//! [INPUT]: plan markdown strings · verification inputs
//! [OUTPUT]: pure transforms; no path / fs / provider
//! [POS]: domain/chat
//! [PROTOCOL]: 变更时更新 domain/CLAUDE.md

mod acceptance;
mod plan_md;
mod verification;

pub use acceptance::{
    acceptance_hint, acceptance_is_stub, acceptance_quality, collect_task_acceptance_items,
    parse_acceptance_checklist, AcceptanceQuality, PlanChecklistItem, TaskAcceptanceItem,
};
pub use plan_md::{normalize_plan_markdown, structure_plan_markdown};
pub use verification::{
    build_verification, VerificationInputs, VerificationItem, VerificationItemStatus,
    VerificationSource, VerificationView,
};
