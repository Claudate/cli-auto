//! cco — CLI orchestrator host library.
//!
//! [INPUT]: 无；库根
//! [OUTPUT]: re-export plan(PlanIR/TaskIR/TaskRole/TaskScope)/runtime/services/state/terminal/tui 公共类型
//! [POS]: src 根；二进制 main 与 src-tauri 均依赖本 crate
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
//!
//! Architecture (target P2-17): Presentation → App → Domain ← Ports ← Adapters.
//! A1 ✅: `domain/{plan,run,worker,inspect,chat}` · `app/{split,run,chat}` ·
//! `ports::{WorkerPort,HandoffStore}` · `runtime/{scheduler,handoff}/*` · `services/*` deprecated facade.
//! A1-7: Tauri/CLI handlers → `app::*` (no presentation business policy). A2: frontend MVVM 待.
//! Adapters: `runtime/provider/*` implement WorkerPort. Desktop: Tauri; CLI: `cco` binary.

/// Application use cases (A0 skeleton → A1 real use cases).
pub mod app;
pub mod cli;
pub mod config;
/// Domain models (A0 skeleton → A1 pure models).
pub mod domain;
pub mod doctor;
pub mod graph;
pub mod plan;
/// Port traits only (A0 skeleton → A1 WorkerPort/Store/…).
pub mod ports;
pub mod report;
pub mod runtime;
pub mod services;
pub mod state;
pub mod terminal;
pub mod tui;

pub use config::Config;
pub use plan::{PlanIR, TaskIR, TaskRole, TaskScope};
pub use runtime::provider::{
    Capabilities, ProviderRegistry, StartCtx, TaskResult, TaskStatus, WorkerHandle,
    WorkerPort, WorkerProvider, WorkerStatus,
};
pub use services::{
    add_project, chat_save_plan, chat_send, chat_session_get, confirm_start, get_plan_job,
    get_settings, list_plan_meta, list_plans, list_projects, list_runs, load_run,
    open_task_terminal, preview_plan, project_live_view, read_plan_md, remove_project,
    resume_run_async, run_doctor, set_settings, start_plan_job, start_run_async,
    start_run_from_plan, stop_run, stop_task, task_logs, ChatDraftPlan, ChatMessage,
    ChatSavePlanResponse, ChatSendResponse, ChatSession, PlanJobView, PlanMeta, PlanPreview,
    ProjectLiveView, ProjectSummary, RunSummary, SettingsUpdate, SettingsView, StartPlanJobRequest,
    StartRunRequest, TaskLiveView, TaskLogsView,
};
pub use state::{RunState, RunStatus, TaskState};
pub use terminal::{SessionKind, TerminalManager, TerminalSession};
pub use tui::{options_from_config, run_tui, TuiOptions};
