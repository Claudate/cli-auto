//! Tauri desktop shell for cco (A1-7 · thin presentation).
//!
//! [INPUT]: webview invoke · AppState(Config mutex)
//! [OUTPUT]: tauri commands → **cco::app** (split/run/chat) + thin services adapters (live/projects/settings)
//! [POS]: 桌面薄壳；禁止堆业务逻辑；handler = 解析 IPC → app → DTO/错误字符串
//! note: chat_send_cmd 必须 async + spawn_blocking，禁止同步堵 UI
//! note: C3 多会话 chat_list/new/rename/delete_session_cmd
//! note: P2-4 open_monitor_window_cmd — 系统级第二窗（可拖到另一显示器）
//! note: A1-7 — chat/split/run 走 app::*；IPC 命令名与 JSON 字段保持兼容
//! [PROTOCOL]: 变更时更新此头部，然后检查 src-tauri/CLAUDE.md
//!
//! ## Command → app map (A1-7)
//! | Tauri command | Application |
//! |---------------|-------------|
//! | confirm_start_cmd | app::split::confirm |
//! | start_plan_job_cmd / get_plan_job_cmd / latest_plan_job_cmd | app::split::* |
//! | update/remove/sanitize plan task | app::split::* |
//! | stop_run_cmd / pause_run_cmd / resume_run_cmd / retry_task_cmd / rework / residual | app::run::* |
//! | get_runs / get_run / plan meta / preview | app::run::* |
//! | start_run (legacy ParseOnly) | app::run::start_from_request |
//! | chat_* / read_plan_md / chat_read_image_data_url / preview_* | app::chat::* |
//! | live / projects / settings / doctor | services thin adapters (not yet app modules) |

use std::path::PathBuf;
use std::sync::Mutex;

use cco::app::{
    chat as chat_uc, guide as guide_uc, memory as memory_uc, project_ui as project_ui_uc,
    run as run_uc, split as split_uc,
};
use cco::config::normalize_region as cfg_normalize_region;
use cco::config::Config;
use cco::services::{
    add_project, add_remote as svc_git_add_remote, apply_remotes as svc_git_apply_remotes,
    get_settings, git_commit as svc_git_commit, git_doctor as svc_git_doctor,
    git_push as svc_git_push, git_set_identity as svc_git_set_identity,
    git_status as svc_git_status, list_projects, open_task_terminal, project_live_view,
    remove_project, remove_remote as svc_git_remove_remote, run_doctor, set_settings, task_logs,
    BranchInfo as GitBranchInfo, ChatAttachment, ChatNormalizePlanResponse, ChatSavePlanResponse,
    ChatSaveWaveResponse, ChatSendResponse, ChatSession, ChatSessionSummary, ChatStreamPartial,
    CommitResult as GitCommitResult, GitDoctorLine, GitStatusView, LogEntry,
    PlanJobView, PlanMeta, PlanPreview, PreviewStatus, ProjectLiveView, ProjectSummary,
    PullResult as GitPullResult, PushResult as GitPushResult, ReworkStartResponse, RunSummary,
    SanitizeDepsResult, SettingsUpdate, SettingsView, SlashCommandInfo, StashEntry as GitStashEntry,
    StartPlanJobRequest, StartRunRequest, TagInfo as GitTagInfo,
};
use cco::domain::guide::GuideSession;
use cco::services::ChatCliInfo;
use cco::state::{ProjectLastSummary, ProjectMemoryView, ProjectPin};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

struct AppState {
    config: Mutex<Config>,
}

/// System-level second window for live CLI board (P2-4).
const MONITOR_WINDOW_LABEL: &str = "cco-monitor";

fn map_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Debug, Serialize)]
struct DoctorLineDto {
    name: String,
    ok: bool,
    detail: String,
    /// Official docs / download page when CLI missing (desktop 「官网下载」).
    #[serde(skip_serializing_if = "Option::is_none")]
    help_url: Option<String>,
}

#[tauri::command]
fn meta(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(json!({
        "name": "cco",
        "version": env!("CARGO_PKG_VERSION"),
        "shell": "tauri",
        "state_root": config.state_root,
        "runs_dir": config.runs_dir(),
        "default_provider": config.default.default_provider,
        "project_count": config.projects.len(),
    }))
}

#[tauri::command]
fn get_runs(state: tauri::State<'_, AppState>) -> Result<Vec<RunSummary>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::list(&config).map_err(map_err)
}

#[tauri::command]
fn get_run(state: tauri::State<'_, AppState>, run_id: String) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let rs = run_uc::load(&config, &run_id).map_err(map_err)?;
    let report_md = std::fs::read_to_string(rs.run_dir.join("report.md")).ok();
    Ok(json!({
        "run_id": rs.run_id,
        "status": rs.status,
        "project_root": rs.project_root,
        "plan_path": rs.plan_path,
        "adapter": rs.adapter,
        "started_at": rs.started_at,
        "finished_at": rs.finished_at,
        "tasks": rs.tasks,
        "run_dir": rs.run_dir,
        "report_md": report_md,
    }))
}

#[tauri::command]
fn get_run_status_cmd(run_id: String) -> Result<String, String> {
    let config = Config::load().unwrap_or_default();
    let dir = cco::state::resolve_run_dir(&config.runs_dir(), Some(&run_id))
        .map_err(|e| e.to_string())?;
    let rs = cco::state::RunState::load(&dir).map_err(|e| e.to_string())?;
    Ok(format!("{:?}", rs.status).to_ascii_lowercase())
}

#[tauri::command]
fn get_plans(project: String) -> Result<Vec<String>, String> {
    run_uc::plans(PathBuf::from(project).as_path()).map_err(map_err)
}

/// H2: plans + ever_completed / last_run_* (chooser & plan-rail).
#[tauri::command]
fn get_plan_meta(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Vec<PlanMeta>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::plan_meta(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn preview_plan_cmd(project: String, plan: String) -> Result<PlanPreview, String> {
    let config = Config::load().unwrap_or_default();
    run_uc::plan_preview(
        PathBuf::from(project).as_path(),
        PathBuf::from(plan).as_path(),
        &config,
    )
    .map_err(map_err)
}

#[tauri::command]
fn get_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectSummary>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    list_projects(&config).map_err(map_err)
}

#[tauri::command]
fn add_project_cmd(
    state: tauri::State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let entry = add_project(&mut config, PathBuf::from(path), name).map_err(map_err)?;
    Ok(json!({
        "path": entry.path,
        "name": entry.display_name(),
        "added_at": entry.added_at,
    }))
}

#[tauri::command]
fn remove_project_cmd(state: tauri::State<'_, AppState>, path: String) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let removed = remove_project(&mut config, PathBuf::from(path).as_path()).map_err(map_err)?;
    Ok(json!({ "ok": removed }))
}

#[tauri::command]
fn get_project_live(
    state: tauri::State<'_, AppState>,
    project: String,
    log_max_bytes: Option<usize>,
) -> Result<ProjectLiveView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    project_live_view(
        &config,
        PathBuf::from(project).as_path(),
        log_max_bytes.unwrap_or(24_000),
    )
    .map_err(map_err)
}

#[tauri::command]
fn get_task_logs(
    state: tauri::State<'_, AppState>,
    run_id: String,
    task_id: String,
    max_bytes: Option<usize>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let view =
        task_logs(&config, &run_id, &task_id, max_bytes.unwrap_or(48_000)).map_err(map_err)?;
    serde_json::to_value(view).map_err(|e| e.to_string())
}

/// Open external terminal following task logs (P1-2).
#[tauri::command]
fn open_task_terminal_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
    #[allow(non_snake_case)] taskId: String,
    kind: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let session = open_task_terminal(&config, &runId, &taskId, kind.as_deref()).map_err(map_err)?;
    serde_json::to_value(session).map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_task_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
    #[allow(non_snake_case)] taskId: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::stop_task(&config, &runId, taskId.as_deref()).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "task_id": taskId }))
}

#[tauri::command]
async fn doctor_cmd(
    state: tauri::State<'_, AppState>,
    project: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    let proj = project.filter(|s| !s.trim().is_empty()).map(PathBuf::from);
    let report = run_doctor(&config, proj.as_deref())
        .await
        .map_err(map_err)?;
    let lines: Vec<DoctorLineDto> = report
        .lines
        .into_iter()
        .map(|l| DoctorLineDto {
            name: l.name,
            ok: l.ok,
            detail: l.detail,
            help_url: l.help_url,
        })
        .collect();
    Ok(json!({ "ok": report.ok, "lines": lines }))
}

/// Legacy ParseOnly / direct disk plan start (IPC name kept for web compatibility).
/// Mode B open-run is **only** [`confirm_start_cmd`] → app::split::confirm.
#[tauri::command]
fn start_run(state: tauri::State<'_, AppState>, req: StartRunRequest) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let _ = add_project(&mut config, req.project.clone(), None);
    if let Some(proj) = config.projects.iter_mut().find(|p| p.path == req.project) {
        proj.last_plan = Some(req.plan.clone());
        let _ = config.save();
    }
    let cfg = config.clone();
    drop(config);
    let run_id = run_uc::start_from_request(cfg, req).map_err(map_err)?;
    Ok(json!({
        "run_id": run_id,
        "status": "started",
        "message": "scheduler started in background",
    }))
}

#[tauri::command]
fn start_plan_job_cmd(
    state: tauri::State<'_, AppState>,
    req: StartPlanJobRequest,
) -> Result<PlanJobView, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let _ = add_project(&mut config, req.project.clone(), None);
    let cfg = config.clone();
    drop(config);
    split_uc::start_job(&cfg, req).map_err(map_err)
}

#[tauri::command]
fn get_plan_job_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] jobId: String,
) -> Result<PlanJobView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::get_job(&config, &jobId).map_err(map_err)
}

#[tauri::command]
fn latest_plan_job_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Option<PlanJobView>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::latest_job_for_project(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

/// Plan-list reopen: latest restorable split for a plan **path** (SQLite index + disk).
#[tauri::command]
fn latest_plan_job_for_plan_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    #[allow(non_snake_case)] planPath: String,
) -> Result<Option<PlanJobView>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::latest_job_for_plan_path(&config, PathBuf::from(project).as_path(), planPath.trim())
        .map_err(map_err)
}

/// Plan list badge: which plan_paths already have a restorable split.
#[tauri::command]
fn list_plan_split_index_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Vec<cco::state::sqlite::PlanSplitIndexRow>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::list_plan_split_index(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn update_plan_task_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] jobId: String,
    #[allow(non_snake_case)] taskId: String,
    title: Option<String>,
    prompt: Option<String>,
    include: Option<bool>,
    provider: Option<String>,
    #[allow(non_snake_case)] dependsOn: Option<Vec<String>>,
    // S-role: scout|implement|integrate|inspect; empty clears.
    role: Option<String>,
    // S-role: writable scope path globs (empty clears paths).
    #[allow(non_snake_case)] scopePaths: Option<Vec<String>>,
) -> Result<PlanJobView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::edit_task(
        &config, &jobId, &taskId, title, prompt, include, provider, dependsOn, role, scopePaths,
    )
    .map_err(map_err)
}

#[tauri::command]
fn remove_plan_task_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] jobId: String,
    #[allow(non_snake_case)] taskId: String,
) -> Result<PlanJobView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::remove_task(&config, &jobId, &taskId).map_err(map_err)
}

#[tauri::command]
fn sanitize_plan_deps_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] jobId: String,
) -> Result<SanitizeDepsResult, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    split_uc::sanitize_deps(&config, &jobId).map_err(map_err)
}

/// **Sole Mode B business open-run** — app::split::confirm (A0-R1).
/// `effort`: optional execute-time pick from split desk (low…max|ultracode).
#[tauri::command]
fn confirm_start_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] jobId: String,
    effort: Option<String>,
) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let cfg_for_job = config.clone();
    if let Ok(view) = split_uc::get_job(&cfg_for_job, &jobId) {
        let project = PathBuf::from(&view.project);
        let plan = PathBuf::from(&view.plan_path);
        if let Some(proj) = config.projects.iter_mut().find(|p| p.path == project) {
            proj.last_plan = Some(plan);
            let _ = config.save();
        }
    }
    let cfg = config.clone();
    drop(config);
    let run_id = split_uc::confirm(cfg, &jobId, effort.as_deref()).map_err(map_err)?;
    Ok(json!({
        "run_id": run_id,
        "status": "started",
        "job_id": jobId,
        "message": "confirmed plan; scheduler started",
    }))
}

#[tauri::command]
fn stop_run_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::stop(&config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "status": "aborted" }))
}

#[tauri::command]
fn pause_run_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::pause(&config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "status": "paused" }))
}

#[tauri::command]
fn resume_run_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    run_uc::resume(config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "status": "resuming" }))
}

/// Manual re-run of one failed/stopped/timeout task (same run; not re-split).
#[tauri::command]
fn retry_task_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
    #[allow(non_snake_case)] taskId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    run_uc::retry_task(config, &runId, &taskId).map_err(map_err)?;
    Ok(json!({
        "ok": true,
        "run_id": runId,
        "task_id": taskId,
        "status": "retrying"
    }))
}

#[tauri::command]
fn start_rework_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<ReworkStartResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    run_uc::start_rework(config, &runId).map_err(map_err)
}

#[tauri::command]
fn accept_residual_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
    note: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    run_uc::accept_residual(&config, &runId, note.as_deref()).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "accepted_residual": true }))
}

/// P2-2: write project last_summary from a finished run (rule template).
#[tauri::command]
fn writeback_memory_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let row = run_uc::writeback_memory(&config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "last_summary": row }))
}

/// P2-2: project memory view (last_summary + pins).
#[tauri::command]
fn project_memory_get_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<ProjectMemoryView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    memory_uc::get(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn project_memory_last_summary_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Option<ProjectLastSummary>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    memory_uc::last_summary(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn project_pins_list_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Vec<ProjectPin>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    memory_uc::list_pins(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn project_pin_upsert_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    key: String,
    value: String,
) -> Result<ProjectPin, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    memory_uc::upsert_pin(&config, PathBuf::from(project).as_path(), &key, &value).map_err(map_err)
}

#[tauri::command]
fn project_pin_delete_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    key: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let deleted =
        memory_uc::delete_pin(&config, PathBuf::from(project).as_path(), &key).map_err(map_err)?;
    Ok(json!({ "ok": true, "deleted": deleted, "key": key }))
}

/// G0-3: list guided sessions for a project.
#[tauri::command]
fn guide_sessions_list_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Vec<GuideSession>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    guide_uc::list(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

/// G0-3: start a guided session (mode/entry strings; parse fails fast).
#[tauri::command]
fn guide_session_start_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    mode: String,
    entry: String,
    #[allow(non_snake_case)] rolePack: String,
) -> Result<GuideSession, String> {
    use cco::domain::guide::{SessionEntry, SessionMode};
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let mode = SessionMode::parse(&mode).ok_or_else(|| format!("unknown guide mode: {mode}"))?;
    let entry =
        SessionEntry::parse(&entry).ok_or_else(|| format!("unknown guide entry: {entry}"))?;
    guide_uc::start(&config, PathBuf::from(project).as_path(), mode, entry, &rolePack)
        .map_err(map_err)
}

/// G0-3: get one guided session by id.
#[tauri::command]
fn guide_session_get_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] sessionId: String,
) -> Result<Option<GuideSession>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    guide_uc::get(&config, &sessionId).map_err(map_err)
}

#[tauri::command]
fn get_project_persona_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Option<serde_json::Value>, String> {
    use cco::state::persona_store::try_get_project_persona;
    let config = state.config.lock().map_err(|e| e.to_string())?;
    match try_get_project_persona(&config, &project) {
        Some(p) => Ok(Some(json!({
            "persona_id": p.persona_id,
            "clarify_depth": p.clarify_depth,
            "split_grain": p.split_grain,
        }))),
        None => Ok(None),
    }
}

#[tauri::command]
fn set_project_persona_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    persona_id: Option<String>,
    clarify_depth: Option<String>,
    split_grain: Option<String>,
) -> Result<bool, String> {
    use cco::state::persona_store::{set_project_persona, ProjectPersona};
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let persona = ProjectPersona {
        persona_id,
        clarify_depth,
        split_grain,
    };
    // best-effort: return success even if storage fails (caller should handle errors)
    let _ = set_project_persona(&config, &project, &persona);
    Ok(true)
}

/// SQLite SoT: user finished this run in UI — project_live must not re-bind it.
#[tauri::command]
fn project_dismiss_run_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    #[allow(non_snake_case)] runId: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    project_ui_uc::dismiss_run(&config, PathBuf::from(project).as_path(), &runId)
        .map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "dismissed": true }))
}

/// Clear dismissed run (e.g. new confirm_start).
#[tauri::command]
fn project_clear_dismissed_run_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    project_ui_uc::clear_dismissed_run(&config, PathBuf::from(project).as_path())
        .map_err(map_err)?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn project_get_dismissed_run_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let rid = project_ui_uc::get_dismissed_run(&config, PathBuf::from(project).as_path())
        .map_err(map_err)?;
    Ok(json!({ "run_id": rid }))
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let raw = path.trim();
    // Chat / markdown external links (http://localhost:4322/) — never mkdir as a path.
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("mailto:") {
        let status = std::process::Command::new("open")
            .arg(raw)
            .status()
            .map_err(map_err)?;
        if !status.success() {
            return Err(format!("open url failed: {raw} (exit {status})"));
        }
        return Ok(());
    }

    // Normalize trailing slashes for existence checks (macOS open tolerates them).
    let trimmed = path.trim_end_matches(['/', '\\']);
    let p = std::path::PathBuf::from(if trimmed.is_empty() { &path } else { trimmed });
    let want_dir =
        path.ends_with('/') || path.ends_with('\\') || (!p.exists() && p.extension().is_none());
    if want_dir {
        if !p.exists() {
            std::fs::create_dir_all(&p).map_err(map_err)?;
        }
    } else if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(map_err)?;
        }
    }
    // Prefer path without trailing slash for `open` when it's a directory on disk.
    let open_arg = if p.is_dir() {
        p.as_os_str()
    } else {
        std::ffi::OsStr::new(&path)
    };
    let status = std::process::Command::new("open")
        .arg(open_arg)
        .status()
        .map_err(map_err)?;
    if !status.success() {
        return Err(format!("open failed: {path} (exit {status})"));
    }
    Ok(())
}

/// P2-4: open (or focus) a real OS window for the live CLI board.
#[tauri::command]
async fn open_monitor_window_cmd(app: AppHandle, project: Option<String>) -> Result<Value, String> {
    let label = MONITOR_WINDOW_LABEL;
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.set_focus();
        let _ = existing.unminimize();
        return Ok(json!({
            "ok": true,
            "created": false,
            "label": label,
            "focused": true,
        }));
    }

    let mut url_path = "index.html?cco_window=monitor".to_string();
    if let Some(p) = project.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let enc = p
            .replace('%', "%25")
            .replace('&', "%26")
            .replace('#', "%23")
            .replace('+', "%2B")
            .replace(' ', "%20");
        url_path.push_str("&project=");
        url_path.push_str(&enc);
    }

    let mut builder = WebviewWindowBuilder::new(&app, label, WebviewUrl::App(url_path.into()))
        .title("cco · 监视")
        .inner_size(980.0, 720.0)
        .min_inner_size(640.0, 420.0)
        .resizable(true)
        .focused(true);

    if let Ok(monitors) = app.available_monitors() {
        let primary = app.primary_monitor().ok().flatten();
        let primary_pos = primary.as_ref().map(|m| *m.position());
        let target = monitors.iter().find(|m| {
            primary_pos
                .map(|pp| m.position().x != pp.x || m.position().y != pp.y)
                .unwrap_or(true)
        });
        if let Some(m) = target {
            let scale = m.scale_factor();
            let pos = m.position();
            let x = (pos.x as f64 / scale) + 40.0;
            let y = (pos.y as f64 / scale) + 40.0;
            builder = builder.position(x, y);
        }
    }

    builder
        .build()
        .map_err(|e| format!("open monitor window: {e}"))?;

    Ok(json!({
        "ok": true,
        "created": true,
        "label": label,
        "focused": true,
    }))
}

#[tauri::command]
fn set_project_default_plan(
    state: tauri::State<'_, AppState>,
    project: String,
    plan: String,
) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let project_path = PathBuf::from(project);
    let plan_path = PathBuf::from(plan);
    if let Some(proj) = config.projects.iter_mut().find(|p| p.path == project_path) {
        proj.default_plan = Some(plan_path);
        config.save().map_err(map_err)?;
    }
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn get_settings_cmd(state: tauri::State<'_, AppState>) -> Result<SettingsView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    Ok(get_settings(&config))
}

#[tauri::command]
fn set_settings_cmd(
    state: tauri::State<'_, AppState>,
    update: SettingsUpdate,
) -> Result<SettingsView, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    set_settings(&mut config, update).map_err(map_err)?;
    Ok(get_settings(&config))
}

// ── Git (host-level: status / remote / identity / commit / push / doctor) ──

#[tauri::command]
fn git_status_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<GitStatusView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    svc_git_status(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn git_remote_add_cmd(
    state: tauri::State<'_, AppState>,
    name: String,
    url: String,
    region: String,
    note: Option<String>,
) -> Result<Value, String> {
    let reg = cfg_normalize_region(&region)
        .ok_or_else(|| format!("invalid region: {region} (use domestic|overseas)"))?;
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    svc_git_add_remote(&mut config, &name, &url, reg, note).map_err(map_err)?;
    Ok(json!({ "ok": true, "name": name, "url": url, "region": region }))
}

#[tauri::command]
fn git_remote_remove_cmd(state: tauri::State<'_, AppState>, name: String) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    let removed = svc_git_remove_remote(&mut config, &name).map_err(map_err)?;
    Ok(json!({ "ok": removed, "name": name }))
}

#[tauri::command]
fn git_remote_apply_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let actions =
        svc_git_apply_remotes(&config, PathBuf::from(project).as_path()).map_err(map_err)?;
    Ok(json!({ "ok": true, "actions": actions }))
}

#[tauri::command]
fn git_set_identity_cmd(
    project: String,
    name: Option<String>,
    email: Option<String>,
) -> Result<Value, String> {
    svc_git_set_identity(
        PathBuf::from(project).as_path(),
        name.as_deref(),
        email.as_deref(),
    )
    .map_err(map_err)?;
    Ok(json!({ "ok": true, "name": name, "email": email }))
}

#[tauri::command]
fn git_commit_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    message: String,
    dry_run: Option<bool>,
    push: Option<bool>,
    all: Option<bool>,
    paths: Option<Vec<String>>,
    force: Option<bool>,
) -> Result<GitCommitResult, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    svc_git_commit(
        &config,
        PathBuf::from(project).as_path(),
        &message,
        dry_run.unwrap_or(false),
        push.unwrap_or(false),
        all.unwrap_or(true),
        &paths.unwrap_or_default(),
        force.unwrap_or(false),
    )
    .map_err(map_err)
}

#[tauri::command]
fn git_push_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    remote: Option<String>,
    branch: Option<String>,
    force: Option<bool>,
) -> Result<GitPushResult, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    svc_git_push(
        &config,
        PathBuf::from(project).as_path(),
        remote.as_deref(),
        branch.as_deref(),
        force.unwrap_or(false),
    )
    .map_err(map_err)
}

#[tauri::command]
fn git_doctor_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Vec<GitDoctorLine>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    svc_git_doctor(&config, PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn git_pull_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    remote: Option<String>,
    branch: Option<String>,
    strategy: Option<String>,
) -> Result<GitPullResult, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let strat = match strategy.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some("merge") => cco::services::git::PullStrategy::Merge,
        Some("fail") => cco::services::git::PullStrategy::Fail,
        _ => cco::services::git::PullStrategy::Rebase,
    };
    cco::services::git::pull(
        &config,
        PathBuf::from(project).as_path(),
        remote.as_deref(),
        branch.as_deref(),
        strat,
    )
    .map_err(map_err)
}

#[tauri::command]
fn git_fetch_cmd(
    project: String,
    remote: Option<String>,
    prune: Option<bool>,
) -> Result<GitPullResult, String> {
    cco::services::git::fetch(
        PathBuf::from(project).as_path(),
        remote.as_deref(),
        prune.unwrap_or(false),
    )
    .map_err(map_err)
}

#[tauri::command]
fn git_branch_list_cmd(project: String) -> Result<Vec<GitBranchInfo>, String> {
    cco::services::git::list_branches(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn git_branch_create_cmd(
    project: String,
    name: String,
    base: Option<String>,
) -> Result<Value, String> {
    let msg = cco::services::git::create_branch(
        PathBuf::from(project).as_path(),
        &name,
        base.as_deref(),
    )
    .map_err(map_err)?;
    Ok(json!({ "ok": true, "message": msg }))
}

#[tauri::command]
fn git_branch_switch_cmd(project: String, name: String) -> Result<Value, String> {
    let msg =
        cco::services::git::switch_branch(PathBuf::from(project).as_path(), &name).map_err(map_err)?;
    Ok(json!({ "ok": true, "message": msg }))
}

#[tauri::command]
fn git_branch_delete_cmd(project: String, name: String, force: Option<bool>) -> Result<Value, String> {
    let msg = cco::services::git::delete_branch(
        PathBuf::from(project).as_path(),
        &name,
        force.unwrap_or(false),
    )
    .map_err(map_err)?;
    Ok(json!({ "ok": true, "message": msg }))
}

#[tauri::command]
fn git_log_cmd(project: String, n: Option<usize>) -> Result<Vec<LogEntry>, String> {
    cco::services::git::log(PathBuf::from(project).as_path(), n).map_err(map_err)
}

#[tauri::command]
fn git_diff_cmd(
    project: String,
    staged: Option<bool>,
    stat: Option<bool>,
    name_only: Option<bool>,
) -> Result<Value, String> {
    let path = PathBuf::from(project);
    if name_only.unwrap_or(false) {
        let files = cco::services::git::diff_name_only(&path).map_err(map_err)?;
        return Ok(json!({ "files": files }));
    }
    if stat.unwrap_or(false) {
        let out = cco::services::git::diff_stat(&path).map_err(map_err)?;
        return Ok(json!({ "diff": out }));
    }
    if staged.unwrap_or(false) {
        let out = cco::services::git::diff_staged(&path).map_err(map_err)?;
        return Ok(json!({ "diff": out }));
    }
    let out = cco::services::git::diff(&path).map_err(map_err)?;
    Ok(json!({ "diff": out }))
}
#[tauri::command]
fn git_stash_list_cmd(project: String) -> Result<Vec<GitStashEntry>, String> {
    cco::services::git::stash_list(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn git_stash_push_cmd(project: String, message: Option<String>) -> Result<Value, String> {
    let out = cco::services::git::stash_push(
        PathBuf::from(project).as_path(),
        message.as_deref(),
    )
    .map_err(map_err)?;
    Ok(json!({ "ok": true, "message": out }))
}

#[tauri::command]
fn git_stash_pop_cmd(project: String, index: Option<usize>) -> Result<Value, String> {
    let out =
        cco::services::git::stash_pop(PathBuf::from(project).as_path(), index).map_err(map_err)?;
    Ok(json!({ "ok": true, "message": out }))
}

#[tauri::command]
fn git_stash_apply_cmd(project: String, index: Option<usize>) -> Result<Value, String> {
    let out = cco::services::git::stash_apply(PathBuf::from(project).as_path(), index)
        .map_err(map_err)?;
    Ok(json!({ "ok": true, "message": out }))
}

#[tauri::command]
fn git_stash_drop_cmd(project: String, index: Option<usize>) -> Result<Value, String> {
    let out = cco::services::git::stash_drop(PathBuf::from(project).as_path(), index)
        .map_err(map_err)?;
    Ok(json!({ "ok": true, "message": out }))
}

#[tauri::command]
fn git_stash_show_cmd(project: String, index: Option<usize>) -> Result<Value, String> {
    let out = cco::services::git::stash_show(PathBuf::from(project).as_path(), index)
        .map_err(map_err)?;
    Ok(json!({ "diff": out }))
}

#[tauri::command]
fn git_tag_list_cmd(project: String) -> Result<Vec<GitTagInfo>, String> {
    cco::services::git::list_tags(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn git_tag_create_cmd(
    project: String,
    name: String,
    commit: Option<String>,
    message: Option<String>,
) -> Result<Value, String> {
    let path = PathBuf::from(project);
    let msg = if let Some(m) = message {
        cco::services::git::create_annotated_tag(&path, &name, &m, commit.as_deref())
            .map_err(map_err)?
    } else {
        cco::services::git::create_tag(&path, &name, commit.as_deref()).map_err(map_err)?
    };
    Ok(json!({ "ok": true, "message": msg }))
}

#[tauri::command]
fn git_tag_delete_cmd(project: String, name: String) -> Result<Value, String> {
    let msg =
        cco::services::git::delete_tag(PathBuf::from(project).as_path(), &name).map_err(map_err)?;
    Ok(json!({ "ok": true, "message": msg }))
}

#[tauri::command]
fn git_tag_show_cmd(project: String, name: String) -> Result<Value, String> {
    let out =
        cco::services::git::show_tag(PathBuf::from(project).as_path(), &name).map_err(map_err)?;
    Ok(json!({ "output": out }))
}



// ── Chat (app::chat only · no open-run) ──────────────────────────────

#[tauri::command]
fn chat_session_get_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
) -> Result<ChatSession, String> {
    chat_uc::get_session(PathBuf::from(project).as_path(), sessionId.as_deref()).map_err(map_err)
}

#[tauri::command]
fn chat_list_sessions_cmd(project: String) -> Result<Vec<ChatSessionSummary>, String> {
    chat_uc::list_sessions(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn chat_new_session_cmd(project: String, title: Option<String>) -> Result<ChatSession, String> {
    chat_uc::new_session(PathBuf::from(project).as_path(), title.as_deref()).map_err(map_err)
}

#[tauri::command]
fn chat_rename_session_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: String,
    title: Option<String>,
) -> Result<ChatSession, String> {
    chat_uc::rename_session(
        PathBuf::from(project).as_path(),
        &sessionId,
        title.as_deref(),
    )
    .map_err(map_err)
}

#[tauri::command]
fn chat_delete_session_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: String,
) -> Result<(), String> {
    chat_uc::delete_session(PathBuf::from(project).as_path(), &sessionId).map_err(map_err)
}

#[tauri::command]
async fn chat_send_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    message: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
    attachments: Option<Vec<ChatAttachment>>,
    // Optional per-send: low|medium|high|xhigh|max|ultracode
    effort: Option<String>,
    // Optional chat CLI provider id (None → claude default; fake → template reply)
    cli: Option<String>,
) -> Result<ChatSendResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tokio::task::spawn_blocking(move || {
        chat_uc::send(
            &config,
            PathBuf::from(project).as_path(),
            &message,
            sessionId.as_deref(),
            attachments,
            effort.as_deref(),
            cli.as_deref(),
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_send join error: {e}"))?
}

/// L1: chat-capable CLI list for the composer dropdown.
#[tauri::command]
fn chat_clis_list_cmd(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChatCliInfo>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    chat_uc::available_clis(&config).map_err(map_err)
}

#[tauri::command]
fn chat_slash_catalog_cmd(cli: Option<String>) -> Result<Vec<SlashCommandInfo>, String> {
    Ok(chat_uc::slash_catalog(cli.as_deref()))
}

#[tauri::command]
fn chat_stream_partial_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
) -> Result<ChatStreamPartial, String> {
    chat_uc::stream_partial(PathBuf::from(project).as_path(), sessionId.as_deref()).map_err(map_err)
}

#[tauri::command]
fn chat_cancel_cmd(project: String) -> Result<bool, String> {
    chat_uc::cancel(PathBuf::from(project).as_path()).map_err(map_err)
}

/// Detached local dev/preview (not Mode B worker; survives chat Claude exit).
#[tauri::command]
async fn preview_start_cmd(project: String) -> Result<PreviewStatus, String> {
    tokio::task::spawn_blocking(move || {
        chat_uc::preview_start(PathBuf::from(project).as_path()).map_err(map_err)
    })
    .await
    .map_err(|e| format!("preview_start join error: {e}"))?
}

#[tauri::command]
fn preview_stop_cmd(project: String) -> Result<PreviewStatus, String> {
    chat_uc::preview_stop(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn preview_status_cmd(project: String) -> Result<PreviewStatus, String> {
    chat_uc::preview_status(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn read_plan_md_cmd(project: String, plan: String) -> Result<String, String> {
    chat_uc::read_plan_md(PathBuf::from(project).as_path(), &plan).map_err(map_err)
}

#[tauri::command]
async fn chat_save_attachment_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
    #[allow(non_snake_case)] fileName: String,
    mime: String,
    #[allow(non_snake_case)] dataBase64: String,
) -> Result<ChatAttachment, String> {
    tokio::task::spawn_blocking(move || {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(dataBase64.trim())
            .map_err(|e| format!("invalid base64: {e}"))?;
        chat_uc::save_attachment(
            PathBuf::from(project).as_path(),
            sessionId.as_deref(),
            &fileName,
            &mime,
            &bytes,
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_save_attachment join error: {e}"))?
}

/// Project-relative image → data URL for chat markdown / attachment thumbs.
#[tauri::command]
async fn chat_read_image_data_url_cmd(project: String, path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        chat_uc::read_image_data_url(PathBuf::from(project).as_path(), &path).map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_read_image_data_url join error: {e}"))?
}

#[tauri::command]
async fn chat_save_plan_cmd(
    project: String,
    markdown: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
    title: Option<String>,
    #[allow(non_snake_case)] planRel: Option<String>,
    #[allow(non_snake_case)] plansDir: Option<String>,
) -> Result<ChatSavePlanResponse, String> {
    tokio::task::spawn_blocking(move || {
        chat_uc::save_plan(
            PathBuf::from(project).as_path(),
            sessionId.as_deref(),
            title.as_deref(),
            &markdown,
            planRel.as_deref(),
            plansDir.as_deref(),
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_save_plan join error: {e}"))?
}

/// W2: claim wave-index + N plans under plans/wave-…/ — **never** confirm/start_run.
#[tauri::command]
async fn chat_save_wave_bundle_cmd(
    project: String,
    markdown: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
    #[allow(non_snake_case)] plansDir: Option<String>,
) -> Result<ChatSaveWaveResponse, String> {
    tokio::task::spawn_blocking(move || {
        chat_uc::save_wave_bundle(
            PathBuf::from(project).as_path(),
            sessionId.as_deref(),
            &markdown,
            plansDir.as_deref(),
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_save_wave_bundle join error: {e}"))?
}

#[tauri::command]
async fn chat_normalize_plan_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    markdown: String,
    hint: Option<String>,
) -> Result<ChatNormalizePlanResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tokio::task::spawn_blocking(move || {
        chat_uc::normalize_plan(
            &config,
            PathBuf::from(project).as_path(),
            &markdown,
            hint.as_deref(),
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_normalize_plan join error: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("cco=info,warn")),
        )
        .with_target(false)
        .try_init();

    let config = Config::load().unwrap_or_default();
    let _ = std::fs::create_dir_all(config.runs_dir());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Mutex::new(config),
        })
        .invoke_handler(tauri::generate_handler![
            meta,
            get_runs,
            get_run,
            get_run_status_cmd,
            get_plans,
            get_plan_meta,
            preview_plan_cmd,
            get_projects,
            add_project_cmd,
            remove_project_cmd,
            get_project_live,
            get_task_logs,
            open_task_terminal_cmd,
            stop_task_cmd,
            doctor_cmd,
            start_run,
            start_plan_job_cmd,
            get_plan_job_cmd,
            latest_plan_job_cmd,
            latest_plan_job_for_plan_cmd,
            list_plan_split_index_cmd,
            confirm_start_cmd,
            update_plan_task_cmd,
            remove_plan_task_cmd,
            sanitize_plan_deps_cmd,
            stop_run_cmd,
            pause_run_cmd,
            resume_run_cmd,
            retry_task_cmd,
            start_rework_cmd,
            accept_residual_cmd,
            writeback_memory_cmd,
            project_memory_get_cmd,
            project_memory_last_summary_cmd,
            project_pins_list_cmd,
            project_pin_upsert_cmd,
            project_pin_delete_cmd,
            guide_sessions_list_cmd,
            guide_session_start_cmd,
            guide_session_get_cmd,
            get_project_persona_cmd,
            set_project_persona_cmd,
            project_dismiss_run_cmd,
            project_clear_dismissed_run_cmd,
            project_get_dismissed_run_cmd,
            open_path,
            open_monitor_window_cmd,
            get_settings_cmd,
            set_settings_cmd,
            set_project_default_plan,
            chat_session_get_cmd,
            chat_list_sessions_cmd,
            chat_new_session_cmd,
            chat_rename_session_cmd,
            chat_delete_session_cmd,
            chat_send_cmd,
            chat_clis_list_cmd,
            chat_slash_catalog_cmd,
            chat_stream_partial_cmd,
            chat_cancel_cmd,
            preview_start_cmd,
            preview_stop_cmd,
            preview_status_cmd,
            chat_save_plan_cmd,
            chat_save_wave_bundle_cmd,
            chat_normalize_plan_cmd,
            chat_save_attachment_cmd,
            chat_read_image_data_url_cmd,
            read_plan_md_cmd,
            // git host-level commands
            git_status_cmd,
            git_remote_add_cmd,
            git_remote_remove_cmd,
            git_remote_apply_cmd,
            git_set_identity_cmd,
            git_commit_cmd,
            git_push_cmd,
            git_doctor_cmd,
            git_pull_cmd,
            git_fetch_cmd,
            git_branch_list_cmd,
            git_branch_create_cmd,
            git_branch_switch_cmd,
            git_branch_delete_cmd,
            git_log_cmd,
            git_diff_cmd,
            git_stash_list_cmd,
            git_stash_push_cmd,
            git_stash_pop_cmd,
            git_stash_apply_cmd,
            git_stash_drop_cmd,
            git_stash_show_cmd,
            git_tag_list_cmd,
            git_tag_create_cmd,
            git_tag_delete_cmd,
            git_tag_show_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
