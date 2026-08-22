//! Pure per-tick run decision (borrowed from LoopX `should-run`; LX1).
//!
//! Collapses the scheduler's scattered "should this tick advance?" `if`s into one
//! pure decision over already-existing predicates. **No new policy** — only a
//! rename + relocation of `budget_exceeded` / `provider_slot_open` / ready-set
//! logic so the orchestrator loop can consume a single enum (hard rule 8).
//!
//! [INPUT]: spend/cap · ready ids · running count · slot cap · stall flag
//! [OUTPUT]: Spawn | Wait | Halt (no IO, no spawn, no cost mutation)
//! [POS]: domain/run — scheduler only *consumes* the enum
//! [PROTOCOL]: 组合既有谓词；不新增策略；不 IO；变更时更新 domain/run/mod.rs

use super::status::{budget_exceeded, provider_slot_open};

/// Snapshot the orchestrator hands to [`decide_tick`] each loop turn.
///
/// `any_stalled` is carried for future heartbeat/self-repair gating (LoopX
/// `self-repair`); the current three-branch decision does not read it yet.
#[derive(Debug, Clone)]
pub struct RunTickSnapshot {
    /// Total USD spent this run so far.
    pub spent: f64,
    /// Optional run-level USD budget cap (None ⇒ unbounded).
    pub cap: Option<f64>,
    /// Task ids ready to spawn this tick (deps satisfied, active, non-terminal).
    pub ready_ids: Vec<String>,
    /// Workers currently running.
    pub running: usize,
    /// Global parallel cap (None ⇒ unlimited).
    pub slot_cap: Option<usize>,
    /// Any running task tripped the stall patrol (reserved for heartbeat).
    pub any_stalled: bool,
}

/// The single decision for one scheduler tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickDecision {
    /// Ready tasks exist and budget/slots allow → spawn these (≈ LoopX `deliver`).
    Spawn(Vec<String>),
    /// Slots full, or nothing ready while workers still run → look again next
    /// tick and **do not spend** (≈ LoopX `wait` / `quiet`).
    Wait { reason: &'static str },
    /// Budget cap breached → wind down (≈ LoopX quota stop; maps to `__budget__`).
    Halt { reason: &'static str },
}

/// Pure tick decision: budget cap wins, then slot pressure, then ready-set.
///
/// Behaviour is equivalent to the scheduler's previous inline `if`s — no new
/// thresholds. Halt drives the existing budget pause; Wait means "spawn nothing
/// this tick"; Spawn hands the ready ids to the spawn phase.
pub fn decide_tick(s: &RunTickSnapshot) -> TickDecision {
    if let Some(cap) = s.cap {
        if budget_exceeded(s.spent, cap) {
            return TickDecision::Halt {
                reason: "budget_exceeded",
            };
        }
    }
    if !provider_slot_open(s.running, s.slot_cap) {
        return TickDecision::Wait {
            reason: "slots_full",
        };
    }
    if s.ready_ids.is_empty() {
        // quiet skip: distinguish "still awaiting runners" from "fully idle" so a
        // future heartbeat can tell a live-but-quiet run from a drained one.
        return TickDecision::Wait {
            reason: if s.running > 0 {
                "awaiting_running"
            } else {
                "idle"
            },
        };
    }
    TickDecision::Spawn(s.ready_ids.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> RunTickSnapshot {
        RunTickSnapshot {
            spent: 0.0,
            cap: None,
            ready_ids: vec![],
            running: 0,
            slot_cap: None,
            any_stalled: false,
        }
    }

    #[test]
    fn halt_when_budget_breached() {
        let s = RunTickSnapshot {
            spent: 0.02,
            cap: Some(0.01),
            ready_ids: vec!["a".into()],
            ..snap()
        };
        assert_eq!(
            decide_tick(&s),
            TickDecision::Halt {
                reason: "budget_exceeded"
            }
        );
    }

    #[test]
    fn budget_wins_over_ready_and_slots() {
        // Even with ready tasks and open slots, an over-budget run halts.
        let s = RunTickSnapshot {
            spent: 10.0,
            cap: Some(1.0),
            ready_ids: vec!["a".into(), "b".into()],
            running: 0,
            slot_cap: Some(4),
            any_stalled: false,
        };
        assert!(matches!(decide_tick(&s), TickDecision::Halt { .. }));
    }

    #[test]
    fn wait_when_slots_full() {
        let s = RunTickSnapshot {
            ready_ids: vec!["a".into()],
            running: 2,
            slot_cap: Some(2),
            ..snap()
        };
        assert_eq!(
            decide_tick(&s),
            TickDecision::Wait {
                reason: "slots_full"
            }
        );
    }

    #[test]
    fn wait_awaiting_running_when_nothing_ready() {
        let s = RunTickSnapshot {
            ready_ids: vec![],
            running: 1,
            slot_cap: Some(4),
            ..snap()
        };
        assert_eq!(
            decide_tick(&s),
            TickDecision::Wait {
                reason: "awaiting_running"
            }
        );
    }

    #[test]
    fn wait_idle_when_drained() {
        let s = RunTickSnapshot {
            ready_ids: vec![],
            running: 0,
            slot_cap: Some(4),
            ..snap()
        };
        assert_eq!(decide_tick(&s), TickDecision::Wait { reason: "idle" });
    }

    #[test]
    fn spawn_when_ready_and_room() {
        let s = RunTickSnapshot {
            spent: 0.5,
            cap: Some(1.0),
            ready_ids: vec!["a".into(), "b".into()],
            running: 1,
            slot_cap: Some(4),
            any_stalled: false,
        };
        assert_eq!(
            decide_tick(&s),
            TickDecision::Spawn(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn unlimited_slots_never_wait_on_capacity() {
        let s = RunTickSnapshot {
            ready_ids: vec!["a".into()],
            running: 999,
            slot_cap: None,
            ..snap()
        };
        assert_eq!(decide_tick(&s), TickDecision::Spawn(vec!["a".into()]));
    }
}
