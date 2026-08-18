//! Event emitter port: bridge from runtime events to frontend (B1 wave 0).
//!
//! [INPUT]: run_id · type_name · payload (serde_json::Value)
//! [OUTPUT]: fire-and-forget emit to frontend subscribers
//! [POS]: ports trait; cco crate does NOT depend on tauri; adapter lives in src-tauri
//! [PROTOCOL]: emit only forwards events; no business policy (rule 7); no Manager (rule 8)

use serde_json::Value;

/// Emit run-lifecycle events to the frontend (B1).
///
/// Implementations live in Presentation (src-tauri wraps `tauri::AppHandle`).
/// The runtime Scheduler holds `Option<Arc<dyn EventEmitter>>`; CLI/TUI pass `None`
/// so emit is silently skipped and behavior is unchanged (rule 12).
pub trait EventEmitter: Send + Sync {
    /// Forward one run event. `payload` is the same JSON written to events.jsonl
    /// (minus the `ts`/`type` envelope added by `RunState::event`). Fire-and-forget.
    fn emit_run_event(&self, run_id: &str, type_name: &str, payload: Value);
}
