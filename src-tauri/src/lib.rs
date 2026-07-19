//! Tauri desktop shell for cco.
//!
//! [INPUT]: webview invoke · AppState(Config mutex)
//! [OUTPUT]: tauri commands → cco::services（meta/runs/plans/plan_job/live/settings/chat…）
//! [POS]: 桌面薄壳；禁止堆业务逻辑；P1-2 open_task_terminal 已接；chat 建计划已接
//! note: chat_send_cmd 必须 async + spawn_blocking，禁止同步堵 UI
//! [PROTOCOL]: 变更时更新此头部，然后检查 src-tauri/CLAUDE.md

use std::path::PathBuf;
use std::sync::Mutex;

use cco::config::Config;
use cco::services::{
    accept_run_residual, add_project, chat_save_plan, chat_send, chat_session_get, confirm_start,
    get_plan_job, get_settings, latest_plan_job_for_project, list_plans, list_projects, list_runs,
    load_run, open_task_terminal, preview_plan, project_live_view, remove_project, resume_run_async,
    run_doctor, set_settings, start_plan_job, start_rework_from_run, start_run_async, stop_run,
    stop_task, task_logs, update_proposed_task, ChatSavePlanResponse, ChatSendResponse, ChatSession,
    PlanJobView, PlanPreview, ProjectLiveView, ProjectSummary, ReworkStartResponse, RunSummary,
    SettingsUpdate, SettingsView, StartPlanJobRequest, StartRunRequest,
};
use serde::Serialize;
use serde_json::{json, Value};

struct AppState {
    config: Mutex<Config>,
}

fn map_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Debug, Serialize)]
struct DoctorLineDto {
    name: String,
    ok: bool,
    detail: String,
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
    list_runs(&config).map_err(map_err)
}

#[tauri::command]
fn get_run(state: tauri::State<'_, AppState>, run_id: String) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let rs = load_run(&config, &run_id).map_err(map_err)?;
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
fn get_plans(project: String) -> Result<Vec<String>, String> {
    list_plans(PathBuf::from(project).as_path()).map_err(map_err)
}

#[tauri::command]
fn preview_plan_cmd(project: String, plan: String) -> Result<PlanPreview, String> {
    let config = Config::load().unwrap_or_default();
    preview_plan(
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
    let view = task_logs(&config, &run_id, &task_id, max_bytes.unwrap_or(48_000)).map_err(map_err)?;
    serde_json::to_value(view).map_err(|e| e.to_string())
}

/// Open external terminal following task logs (P1-2).
#[tauri::command]
fn open_task_terminal_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)]
    runId: String,
    #[allow(non_snake_case)]
    taskId: String,
    kind: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let session = open_task_terminal(&config, &runId, &taskId, kind.as_deref()).map_err(map_err)?;
    serde_json::to_value(session).map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_task_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)]
    runId: String,
    #[allow(non_snake_case)]
    taskId: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    stop_task(&config, &runId, taskId.as_deref()).map_err(map_err)?;
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
        })
        .collect();
    Ok(json!({ "ok": report.ok, "lines": lines }))
}

#[tauri::command]
fn start_run(state: tauri::State<'_, AppState>, req: StartRunRequest) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    // Auto-pin project to allowed list when starting a run.
    let _ = add_project(&mut config, req.project.clone(), None);
    // Record last_plan on the allowed project entry.
    if let Some(proj) = config.projects.iter_mut().find(|p| p.path == req.project) {
        proj.last_plan = Some(req.plan.clone());
        let _ = config.save();
    }
    let cfg = config.clone();
    drop(config);
    let run_id = start_run_async(cfg, req).map_err(map_err)?;
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
    start_plan_job(&cfg, req).map_err(map_err)
}

#[tauri::command]
fn get_plan_job_cmd(
    state: tauri::State<'_, AppState>,
    // Tauri 2 IPC expects camelCase keys from the webview.
    #[allow(non_snake_case)]
    jobId: String,
) -> Result<PlanJobView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    get_plan_job(&config, &jobId).map_err(map_err)
}

#[tauri::command]
fn latest_plan_job_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Option<PlanJobView>, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    latest_plan_job_for_project(&config, PathBuf::from(project).as_path()).map_err(map_err)
}


#[tauri::command]
fn update_plan_task_cmd(
    state: tauri::State<'_, AppState>,
    // Tauri 2 IPC expects camelCase keys from the webview.
    #[allow(non_snake_case)]
    jobId: String,
    #[allow(non_snake_case)]
    taskId: String,
    title: Option<String>,
    prompt: Option<String>,
    // Optional-task checkbox; ignored / rejected for required tasks.
    include: Option<bool>,
) -> Result<PlanJobView, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    update_proposed_task(&config, &jobId, &taskId, title, prompt, include).map_err(map_err)
}

#[tauri::command]
fn confirm_start_cmd(
    state: tauri::State<'_, AppState>,
    // Tauri 2 IPC expects camelCase keys from the webview.
    #[allow(non_snake_case)]
    jobId: String,
) -> Result<Value, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    // Refresh last_plan from job if possible
    let cfg_for_job = config.clone();
    if let Ok(view) = get_plan_job(&cfg_for_job, &jobId) {
        let project = PathBuf::from(&view.project);
        let plan = PathBuf::from(&view.plan_path);
        if let Some(proj) = config.projects.iter_mut().find(|p| p.path == project) {
            proj.last_plan = Some(plan);
            let _ = config.save();
        }
    }
    let cfg = config.clone();
    drop(config);
    let run_id = confirm_start(cfg, &jobId).map_err(map_err)?;
    Ok(json!({
        "run_id": run_id,
        "status": "started",
        "job_id": jobId,
        "message": "confirmed plan; scheduler started",
    }))
}

#[tauri::command]
fn stop_run_cmd(state: tauri::State<'_, AppState>, #[allow(non_snake_case)] runId: String) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    stop_run(&config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "status": "aborted" }))
}

#[tauri::command]
fn resume_run_cmd(state: tauri::State<'_, AppState>, #[allow(non_snake_case)] runId: String) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    resume_run_async(config, &runId).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "status": "resuming" }))
}

/// P-loop L2: generate rework wave from inspect ISSUES and start a new run.
#[tauri::command]
fn start_rework_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
) -> Result<ReworkStartResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    start_rework_from_run(config, &runId).map_err(map_err)
}

/// P-loop L2: user explicitly accepts residual open risks.
#[tauri::command]
fn accept_residual_cmd(
    state: tauri::State<'_, AppState>,
    #[allow(non_snake_case)] runId: String,
    note: Option<String>,
) -> Result<Value, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?;
    accept_run_residual(&config, &runId, note.as_deref()).map_err(map_err)?;
    Ok(json!({ "ok": true, "run_id": runId, "accepted_residual": true }))
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(&path)
        .status()
        .map_err(map_err)?;
    Ok(())
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

#[tauri::command]
fn chat_session_get_cmd(
    project: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
) -> Result<ChatSession, String> {
    let sid = sessionId.as_deref();
    chat_session_get(PathBuf::from(project).as_path(), sid).map_err(map_err)
}

/// Chat → Claude CLI can run for minutes. Must not block the Tauri async runtime
/// or the webview freezes (send button stuck, whole app "dead").
#[tauri::command]
async fn chat_send_cmd(
    state: tauri::State<'_, AppState>,
    project: String,
    message: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
) -> Result<ChatSendResponse, String> {
    let config = state.config.lock().map_err(|e| e.to_string())?.clone();
    tokio::task::spawn_blocking(move || {
        chat_send(
            &config,
            PathBuf::from(project).as_path(),
            &message,
            sessionId.as_deref(),
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_send join error: {e}"))?
}

#[tauri::command]
async fn chat_save_plan_cmd(
    project: String,
    markdown: String,
    #[allow(non_snake_case)] sessionId: Option<String>,
    title: Option<String>,
) -> Result<ChatSavePlanResponse, String> {
    tokio::task::spawn_blocking(move || {
        chat_save_plan(
            PathBuf::from(project).as_path(),
            sessionId.as_deref(),
            title.as_deref(),
            &markdown,
        )
        .map_err(map_err)
    })
    .await
    .map_err(|e| format!("chat_save_plan join error: {e}"))?
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
            get_plans,
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
            confirm_start_cmd,
            update_plan_task_cmd,
            stop_run_cmd,
            resume_run_cmd,
            start_rework_cmd,
            accept_residual_cmd,
            open_path,
            get_settings_cmd,
            set_settings_cmd,
            set_project_default_plan,
            chat_session_get_cmd,
            chat_send_cmd,
            chat_save_plan_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
