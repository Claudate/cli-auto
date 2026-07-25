//! Host-owned run handoff ledger (handoff.md + handoff.json) — A1-5 multi-file adapter.
//!
//! ## Pure parse/gate vs disk IO
//! | Pure (`domain::inspect`) | IO (this module) |
//! |--------------------------|------------------|
//! | parse_verdict_text · parse_issues_text | read VERDICT/ISSUES files |
//! | task_has_verdict_gate · inspect_gate_fail_reason | missing_outputs · resolve paths |
//! | push_inspect_gate_decision · can_start_rework | board load/save · git CHANGED |
//! | REWORK_MAX · MAP whitelist constants | build_rework_plan · lifecycle |
//!
//! [INPUT]: run_dir · PlanIR · RunState · task terminal results
//! [OUTPUT]: handoff.md / handoff.json；outputs 缺失检查；inspect VERDICT/ISSUES；
//!           REWORK_HOOK · build_rework_plan · accept_residual · inspect_loop_view；[CCO_HANDOFF] 前缀；
//!           system_push_inspect_gate；write_task_diff
//! [POS]: 事中账本适配器；实现 `ports::HandoffStore`；scheduler 只经本 facade，**禁止**正文解析
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md；磁盘 schema 勿静默改

mod inspect_io;
mod lifecycle;
mod model;
mod paths;
mod prefix;
mod rework;
mod store;

#[cfg(test)]
mod tests;

pub use inspect_io::{
    collect_inspect_issues, inspect_pass_blocked_by_issues, load_inspect_gate_doc,
    load_parsed_inspect_issues, read_inspect_issues_text, read_inspect_verdict,
    system_push_inspect_gate, task_has_verdict_gate,
};
pub use lifecycle::{on_run_end, on_task_end, on_task_start, write_shell};
pub use model::{
    BoardRow, Fragment, Handoff, HANDOFF_PROMPT_CLOSE, HANDOFF_PROMPT_OPEN, HANDOFF_SCHEMA,
    TASK_CHANGED_REL,
};
pub use paths::{missing_outputs, resolve_output_path, write_task_diff};
pub use prefix::{build_prompt_prefix, with_handoff_prefix};
pub use rework::{
    accept_residual_on_handoff, build_rework_plan, count_rework_rounds, inspect_loop_view,
    InspectLoopView,
};
pub use store::FsHandoffStore;

// Domain pure surface re-exported for stable `crate::runtime::handoff::*` call sites.
pub use crate::domain::inspect::{
    count_blocking_issues, inspect_gate_fail_reason, issues_candidate_paths, parse_gate_json,
    parse_issues_text, parse_verdict_text, rework_placeholder_note, verdict_candidate_paths,
    InspectGateDoc, InspectVerdict, IssueSeverity, ParsedIssue, INSPECT_GATE_REL,
    INSPECT_ISSUES_REL, INSPECT_VERDICT_REL, MAP_REWORK_PATH_WHITELIST, REWORK_MAX_ROUNDS,
};
