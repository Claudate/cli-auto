//! Application use-case layer (A1 · P2-17 · A1-7 presentation entry).
//!
//! [INPUT]: Presentation (CLI / Tauri / TUI) 命令
//! [OUTPUT]: 用例 API；DTO 形状稳定后迁 `app/dto`
//! [POS]: Presentation → **App** → Domain/Ports；CLI 与桌面共用
//! [PROTOCOL]: 业务策略只写在用例内；handler 只解析参数并委托
//!
//! Open-run hard rule (L1 #10): only [`split::confirm`]. Chat never opens a run.
//!
//! ## A1-7 / A5-1 presentation map (handler → app)
//! | Surface | Module |
//! |---------|--------|
//! | Mode B plan job + confirm / confirm_materialize | [`split`] |
//! | Run list/load/stop/resume/materialize/prepare_scheduler + soft-fill | [`run`] |
//! | Chat session/send/save_plan (no open-run) | [`chat`] |
//!
//! DTOs may still live under `services` / `plan::planner` for wire shape; ops go here.
//! `services::*` remains a **deprecated thin facade** for transitional call sites.

/// Mode B split desk: plan job + confirm (sole business open-run).
pub mod split;

/// Run lifecycle surface (A1-3/A1-7); loop in `runtime/scheduler`, rules in `domain/run`.
pub mod run;

/// Chat / plan-authoring surface (A1-6/A1-7); IO via `services/chat`; pure in `domain/chat`.
pub mod chat;

/// Project light memory (P2-2): last_summary + pins ≤3; prompt context only.
pub mod memory;

/// Project UI prefs (SQLite): dismissed_run_id · durable shell state.
pub mod project_ui;

/// A0 baseline marker.
pub const A0_BASELINE: &str = "app-a0";

#[cfg(test)]
mod tests {
    #[test]
    fn a0_app_skeleton_loads() {
        assert_eq!(super::A0_BASELINE, "app-a0");
    }
}
