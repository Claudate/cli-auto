//! cco — CLI orchestrator host library.
//!
//! Architecture: plan adapters → PlanIR → scheduler → WorkerProvider → TaskResult.
//! Desktop shell is Tauri (`src-tauri`); CLI is `cco` binary.

pub mod cli;
pub mod config;
pub mod doctor;
pub mod graph;
pub mod plan;
pub mod report;
pub mod runtime;
pub mod services;
pub mod state;
pub mod terminal;
pub mod tui;

pub use config::Config;
pub use plan::{PlanIR, TaskIR};
pub use runtime::provider::{
    Capabilities, ProviderRegistry, StartCtx, TaskResult, TaskStatus, WorkerHandle,
    WorkerProvider, WorkerStatus,
};
pub use services::{
    add_project, confirm_start, get_plan_job, get_settings, list_plans, list_projects, list_runs,
    load_run, preview_plan, project_live_view, remove_project, resume_run_async, run_doctor,
    set_settings, start_plan_job, start_run_async, start_run_from_plan, stop_run, stop_task,
    task_logs, TaskLogsView, PlanJobView, PlanPreview, ProjectLiveView, ProjectSummary, RunSummary,
    SettingsUpdate, SettingsView, StartPlanJobRequest, StartRunRequest, TaskLiveView,
};
pub use state::{RunState, RunStatus, TaskState};
pub use terminal::{SessionKind, TerminalManager, TerminalSession};
pub use tui::{options_from_config, run_tui, TuiOptions};
