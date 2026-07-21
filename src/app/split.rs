//! Split use case (A1 · Mode B confirm / plan-job surface · **A5-1** CLI confirm).
//!
//! [INPUT]: Config · job_id · StartPlanJobRequest · proposed edits
//! [OUTPUT]: run_id · PlanJobView · proposed PlanIR · confirm materialize
//! [POS]: Application 层；**唯一业务开跑** = [`confirm`] / [`confirm_materialize`]
//! [PROTOCOL]: 禁止 UI/CLI 旁路本模块直接起调度；搬家时只改本文件委托目标
//!
//! ## Presentation map (A1-7 + A5-1)
//! | CLI / Tauri | app::split |
//! |-------------|------------|
//! | `confirm_start_cmd` / `services::confirm_start` | [`confirm`] |
//! | `cco run` Mode B open-run (foreground) | [`confirm_materialize`] + `app::run::prepare_scheduler` |
//! | `start_plan_job_cmd` / `cco plan` | [`start_job`] |
//! | `get_plan_job_cmd` / poll | [`get_job`] |
//! | `latest_plan_job_cmd` | [`latest_job_for_project`] |
//! | `update_plan_task_cmd` | [`edit_task`] |
//! | `remove_plan_task_cmd` | [`remove_task`] |
//! | `sanitize_plan_deps_cmd` | [`sanitize_deps`] |

use anyhow::Result;

use crate::config::Config;
use crate::plan::planner::{
    get_plan_job, latest_plan_job_for_project, load_proposed, load_proposed_for_exec,
    mark_confirmed, remove_proposed_task, sanitize_proposed_deps, start_plan_job,
    update_proposed_task, PlanJobView, SanitizeDepsResult, StartPlanJobRequest,
};
use crate::plan::PlanIR;
use crate::state::RunState;

/// Freeze proposed plan and start the run (background) — **sole business open-run entry**.
///
/// Contract (A0-R1): plan job must be `planned` or `confirmed`; optional tasks
/// with `include=false` are dropped before spawn (A0-R4); soft-fill does not
/// overwrite explicit provider routes (A0-R3).
///
/// Desktop / async path. CLI foreground uses [`confirm_materialize`] then
/// `app::run::prepare_scheduler` so the same materialize + optional-drop runs.
pub fn confirm(config: Config, job_id: &str) -> Result<String> {
    let (job, ir) = load_proposed_for_exec(&config, job_id)?;
    let run_id = crate::services::start_run_from_plan(config.clone(), job.project.clone(), &ir)?;
    mark_confirmed(&config, job_id, &run_id, &ir)?;
    Ok(run_id)
}

/// Optional CLI patches applied **after** optional-drop / worker defaults.
///
/// Soft/force goes through [`crate::app::run::apply_provider_override`] only
/// (no second soft-fill loop in CLI).
#[derive(Debug, Clone, Default)]
pub struct ConfirmPatches {
    pub provider: Option<String>,
    pub force_provider: Option<String>,
    pub mode: Option<String>,
    pub max_parallel: Option<usize>,
}

/// Mode B open-run for **foreground** CLI: same contract as [`confirm`] but does
/// not spawn a background scheduler.
///
/// Steps: `load_proposed_for_exec` (optional drop + job soft defaults) →
/// apply [`ConfirmPatches`] → [`crate::app::run::materialize_run`] →
/// [`mark_confirmed`]. Caller runs the loop via `app::run::prepare_scheduler`.
///
/// Returns `(run_id, state, ir, soft_fill_log_line)`.
pub fn confirm_materialize(
    config: &Config,
    job_id: &str,
    patches: ConfirmPatches,
) -> Result<(String, RunState, PlanIR, Option<String>)> {
    let (job, mut ir) = load_proposed_for_exec(config, job_id)?;
    let fill_msg = crate::app::run::apply_provider_override(
        &mut ir,
        patches.provider,
        patches.force_provider,
    );
    if let Some(m) = patches.mode {
        for t in &mut ir.tasks {
            t.mode = m.clone();
        }
        ir.default_mode = m;
    }
    if let Some(mp) = patches.max_parallel {
        ir.max_parallel = mp;
    }
    let (run_id, st) =
        crate::app::run::materialize_run(config, job.project.clone(), &ir)?;
    mark_confirmed(config, job_id, &run_id, &ir)?;
    Ok((run_id, st, ir, fill_msg))
}

/// Start a Mode B plan job (parse | fake | ai).
pub fn start_job(config: &Config, req: StartPlanJobRequest) -> Result<PlanJobView> {
    start_plan_job(config, req)
}

/// Poll plan job status / layers for the confirm desk.
pub fn get_job(config: &Config, job_id: &str) -> Result<PlanJobView> {
    get_plan_job(config, job_id)
}

/// Latest job for a project (desktop attach).
pub fn latest_job_for_project(config: &Config, project: &std::path::Path) -> Result<Option<PlanJobView>> {
    latest_plan_job_for_project(config, project)
}

/// Load proposed PlanIR (pre-confirm edit surface).
pub fn load_proposed_plan(config: &Config, job_id: &str) -> Result<PlanIR> {
    load_proposed(config, job_id)
}

/// Patch one proposed task (title/prompt/include/provider/depends_on/role/scope_paths).
/// Soft-fill is **not** applied here — only explicit user fields.
pub fn edit_task(
    config: &Config,
    job_id: &str,
    task_id: &str,
    title: Option<String>,
    prompt: Option<String>,
    include: Option<bool>,
    provider: Option<String>,
    depends_on: Option<Vec<String>>,
    role: Option<String>,
    scope_paths: Option<Vec<String>>,
) -> Result<PlanJobView> {
    update_proposed_task(
        config,
        job_id,
        task_id,
        title,
        prompt,
        include,
        provider,
        depends_on,
        role,
        scope_paths,
    )
}

/// Remove a task from the proposed graph.
pub fn remove_task(config: &Config, job_id: &str, task_id: &str) -> Result<PlanJobView> {
    remove_proposed_task(config, job_id, task_id)
}

/// Sanitize broken depends_on edges on the proposed graph.
pub fn sanitize_deps(config: &Config, job_id: &str) -> Result<SanitizeDepsResult> {
    sanitize_proposed_deps(config, job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn wait_planned(cfg: &Config, job_id: &str) -> PlanJobView {
        let mut view = get_job(cfg, job_id).unwrap();
        for _ in 0..80 {
            if view.status != "planning" {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
            view = get_job(cfg, job_id).unwrap();
        }
        view
    }

    #[test]
    fn split_confirm_creates_run_via_use_case() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("docs/plans")).unwrap();
        let plan_path = project.join("docs/plans/hello.cco.yaml");
        let example =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plans/hello.cco.yaml");
        std::fs::copy(&example, &plan_path).unwrap();

        let mut cfg = Config::default();
        cfg.state_root = tmp.path().join("state");
        cfg.default.default_provider = "fake".into();
        cfg.default.worktree = false;
        cfg.default.post_inspect_enabled = false;
        cfg.default.post_git_push_enabled = false;
        cfg.default.post_open_pr_enabled = false;
        std::fs::create_dir_all(cfg.runs_dir()).unwrap();

        let view = start_job(
            &cfg,
            StartPlanJobRequest {
                project: project.clone(),
                plan: PathBuf::from("docs/plans/hello.cco.yaml"),
                plan_mode: Some("parse".into()),
                provider: Some("fake".into()),
                mode: Some("print".into()),
                max_parallel: Some(1),
                preserve_from_job_id: None,
            },
        )
        .unwrap();
        let view = wait_planned(&cfg, &view.job_id);
        assert_eq!(view.status, "planned", "err={:?}", view.error);

        let run_id = confirm(cfg.clone(), &view.job_id).unwrap();
        assert!(cfg.runs_dir().join(&run_id).join("run.json").exists());
    }
}
