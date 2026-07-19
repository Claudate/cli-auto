//! Shared backend calls used by the native GUI (same logic as CLI).
//!
//! [INPUT]: config::Config · plan::PlanIR/planner · runtime::{Scheduler,log_events,provider} · state · terminal
//! [OUTPUT]: list/start/stop/resume runs · plan job · project_live_view · task_logs · open_task_terminal · settings · chat
//! [POS]: CLI 与 Tauri 共用服务层；禁止 UI 细节；D4 已目录化
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

mod chat;
mod live;
mod projects;
mod runs;
mod settings;
mod util;

pub use chat::{
    chat_save_plan, chat_send, chat_session_get, ChatDraftPlan, ChatMessage, ChatSavePlanResponse,
    ChatSendResponse, ChatSession,
};
pub use live::{
    open_task_terminal, project_live_view, stop_task, task_logs, ProjectLiveView, TaskLiveView,
    TaskLogsView,
};
pub use projects::{add_project, list_projects, remove_project, ProjectSummary};
pub use runs::{
    accept_run_residual, confirm_start, get_plan_job, latest_plan_job_for_project, list_plans,
    list_runs, load_run, preview_plan, resume_run_async, run_doctor, start_plan_job,
    start_rework_from_run, start_run_async, start_run_from_plan, stop_run, update_proposed_task,
    PlanJobView, PlanPreview, ReworkStartResponse, RunSummary, StartPlanJobRequest, StartRunRequest,
};
pub use settings::{get_settings, set_settings, SettingsUpdate, SettingsView};

#[cfg(test)]
mod tests {
    use super::util::compact_log_tail_for_live;

    #[test]
    fn compact_log_tail_does_not_panic_on_mid_char_cut() {
        // Multi-byte CJK + emoji so soft_cap lands inside a rune for many offsets.
        let body = "TOOL Edit /tmp/中文路径/计划.md\n已更新成功 ✅ 继续\n".repeat(400);
        assert!(body.len() > 6_000);
        // Soft caps that historically panic'd when start was mid-char.
        for cap in [1, 2, 3, 7, 13, 100, 599, 6000, 6001, body.len() - 1] {
            let out = compact_log_tail_for_live(&body, true, cap);
            assert!(out.contains("live compact") || out == body);
            // Must be valid UTF-8 (already a String) and not empty for non-zero body.
            assert!(!out.is_empty());
        }
        // No events / short body → passthrough.
        assert_eq!(compact_log_tail_for_live("短", true, 6000), "短");
        assert_eq!(compact_log_tail_for_live(&body, false, 10), body);
    }

    #[test]
    fn compact_log_tail_prefers_line_boundary() {
        let full = format!("{}{}", "x".repeat(100), "\nline-two\nline-three");
        let out = compact_log_tail_for_live(&full, true, 20);
        assert!(out.starts_with("… (live compact)\n"));
        assert!(out.contains("line-three") || out.contains("line-two"));
        assert!(!out.contains('\u{FFFD}')); // no replacement char from bad slice
    }
}
