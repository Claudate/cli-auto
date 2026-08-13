//! Shared backend calls (migration-era facade · A1-7).
//!
//! **Deprecated for Presentation entry points.** CLI / Tauri should call
//! [`crate::app`] (`split` / `run` / `chat`). This module remains for:
//! - DTO types still imported by the desktop shell
//! - live / projects / settings adapters not yet cut into app
//! - internal IO used by app thin facades
//!
//! [INPUT]: config::Config · plan::PlanIR/planner · runtime::{Scheduler,log_events,provider} · state · terminal
//! [OUTPUT]: list/start/stop/resume runs · list_plan_meta · plan job · project_live_view · task_logs · open_task_terminal · settings · chat
//! [POS]: 迁移期 facade；新逻辑不得进 services，放 app 用例
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md

mod chat;
pub mod git;
mod live;
mod live_status;
mod preview;
mod projects;
mod runs;
mod settings;
mod util;

pub use chat::cli_select::{available_chat_clis, ChatCliInfo};
pub use chat::{
    chat_cancel, chat_delete_session, chat_list_sessions, chat_new_session, chat_normalize_plan,
    chat_read_image_data_url, chat_rename_session, chat_save_attachment, chat_save_plan,
    chat_save_wave_bundle, chat_send, chat_session_get, chat_stream_partial,
    cleanup_expired_chat_sessions, extract_title_from_md, normalize_plan_markdown, read_plan_md,
    sanitize_plan_title, slash_catalog, structure_plan_markdown, ChatAttachment, ChatDraftPlan,
    ChatMessage, ChatNormalizePlanResponse, ChatSavePlanResponse, ChatSaveWaveResponse,
    ChatSendResponse, ChatSession, ChatSessionSummary, ChatStreamPartial, SlashCommandInfo,
};
pub use git::{
    add_remote, apply_remotes, commit as git_commit, doctor as git_doctor,
    is_git_repo as git_is_repo, list_actual_remotes as git_list_actual_remotes,
    list_commit_candidates as git_list_commit_candidates, push as git_push, remove_remote,
    set_identity as git_set_identity, status as git_status, BranchInfo, CommitResult,
    GitActualRemote, GitDoctorLine, GitRemoteView, GitStatusView, LogEntry, PullResult,
    PushResult, StashEntry, TagInfo,
};
pub use live::{
    open_task_terminal, project_live_view, stop_task, task_logs, ProjectLiveView, TaskLiveView,
    TaskLogsView,
};
pub use preview::{
    annotate_false_preview_claims, preview_start, preview_status, preview_stop, PreviewStatus,
};
pub use projects::{add_project, list_projects, remove_project, ProjectSummary};
pub use runs::{
    accept_run_residual, confirm_start, get_plan_job, latest_plan_job_for_project, list_plan_meta,
    list_plans, list_runs, load_run, pause_run, preview_plan, remove_proposed_task,
    resume_run_async, retry_task_async, run_doctor, sanitize_proposed_deps, start_plan_job,
    start_rework_from_run, start_run_async, start_run_from_plan, start_run_from_plan_with_route,
    start_run_from_plan_with_route_opts,
    stop_run, update_proposed_task, PlanJobView, PlanMeta, PlanPreview, ReworkStartResponse,
    RunSummary, SanitizeDepsResult, StartPlanJobRequest, StartRunRequest,
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
