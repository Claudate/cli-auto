//! Mode B golden matrix (P1-6 / B3):
//! 1. 散文 md → plan job (fake) → confirm → exec
//! 2. 半结构化 serial-prompts → parse plan job → confirm → exec
//! 3. 已是 cco-plan/v1 → parse plan job → confirm → exec
//!
//! Uses fake provider so CI needs no Claude CLI.

use std::path::PathBuf;
use std::time::Duration;

use cco::config::Config;
use cco::plan::planner::{
    get_plan_job, load_proposed, start_plan_job, StartPlanJobRequest,
};
use cco::plan::{is_structured_adapter, load_plan, peek_adapter, MAX_TASKS};
use cco::report;
use cco::services::{confirm_start, project_live_view};
use cco::state::{self, RunStatus};

fn wait_planned(cfg: &Config, job_id: &str) -> cco::plan::planner::PlanJobView {
    let mut view = get_plan_job(cfg, job_id).unwrap();
    for _ in 0..80 {
        if view.status != "planning" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        view = get_plan_job(cfg, job_id).unwrap();
    }
    view
}

fn wait_run_terminal(cfg: &Config, run_id: &str) {
    for _ in 0..200 {
        let dir = cfg.runs_dir().join(run_id);
        if let Ok(st) = state::RunState::load(&dir) {
            if matches!(
                st.status,
                RunStatus::Completed | RunStatus::Failed | RunStatus::Aborted | RunStatus::Paused
            ) {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn test_config(tmp: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.state_root = tmp.join("state");
    config.default.default_provider = "fake".into();
    config.default.worktree = false;
    config.default.poll_interval_secs = 1;
    std::fs::create_dir_all(config.runs_dir()).unwrap();
    config
}

/// 1) Prose markdown → Mode B plan job (fake) → confirm → workers done.
#[tokio::test]
async fn golden_prose_md_plan_confirm_exec() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/idea.md");
    std::fs::write(
        &plan_path,
        "# Feature idea\n\nBuild a tiny hello helper and a README note.\nEnd workers with CCO_DONE ok.\n",
    )
    .unwrap();

    let cfg = test_config(tmp.path());
    // Routing: prose is raw-single, not structured.
    let adapter = peek_adapter(&project, &plan_path).unwrap();
    assert_eq!(adapter, "raw-single");
    assert!(!is_structured_adapter(&adapter));

    let view = start_plan_job(
        &cfg,
        StartPlanJobRequest {
            project: project.clone(),
            plan: PathBuf::from("docs/plans/idea.md"),
            plan_mode: Some("fake".into()),
            provider: Some("fake".into()),
            mode: Some("print".into()),
            max_parallel: Some(2),
            preserve_from_job_id: None,
        },
    )
    .unwrap();
    let view = wait_planned(&cfg, &view.job_id);
    assert_eq!(view.status, "planned", "err={:?}", view.error);
    assert!(view.task_count.unwrap_or(0) >= 2);
    assert!(view.task_count.unwrap_or(0) <= MAX_TASKS);
    assert!(!view.layers.is_empty());

    let ir = load_proposed(&cfg, &view.job_id).unwrap();
    ir.validate().unwrap();

    let run_id = confirm_start(cfg.clone(), &view.job_id).unwrap();
    wait_run_terminal(&cfg, &run_id);

    let st = state::RunState::load(&cfg.runs_dir().join(&run_id)).unwrap();
    assert!(
        matches!(st.status, RunStatus::Completed | RunStatus::Paused),
        "status={:?}",
        st.status
    );
    report::write_reports(&st).unwrap();
    let md = std::fs::read_to_string(st.run_dir.join("report.md")).unwrap();
    // Budget section present (may be empty costs with fake).
    assert!(md.contains("## Tasks") || md.contains("Budget") || md.contains("status"));

    let live = project_live_view(&cfg, &project, 4_000).unwrap();
    assert_eq!(live.run_id.as_deref(), Some(run_id.as_str()));
    assert!(!live.tasks.is_empty());
}

/// 2) Semi-structured serial-prompts → parse plan job → confirm → exec.
#[tokio::test]
async fn golden_serial_prompts_plan_confirm_exec() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/serial-prompts-sample.md");
    let plan_path = project.join("docs/plans/serial.md");
    std::fs::copy(&fixture, &plan_path).unwrap();

    let cfg = test_config(tmp.path());
    let adapter = peek_adapter(&project, &plan_path).unwrap();
    assert_eq!(adapter, "serial-prompts/v0");
    assert!(is_structured_adapter(&adapter));

    // Direct parse path (skip AI) still goes through plan job for Mode B UX.
    let view = start_plan_job(
        &cfg,
        StartPlanJobRequest {
            project: project.clone(),
            plan: PathBuf::from("docs/plans/serial.md"),
            plan_mode: Some("parse".into()),
            provider: Some("fake".into()),
            mode: Some("print".into()),
            max_parallel: Some(2),
            preserve_from_job_id: None,
        },
    )
    .unwrap();
    let view = wait_planned(&cfg, &view.job_id);
    assert_eq!(view.status, "planned", "err={:?}", view.error);
    assert_eq!(view.task_count, Some(3));
    assert!(view.layers.len() >= 2, "t3 depends on t1,t2 → ≥2 waves");

    let ir = load_proposed(&cfg, &view.job_id).unwrap();
    assert!(ir.task("t3").unwrap().depends_on.len() >= 1);
    ir.validate().unwrap();

    let run_id = confirm_start(cfg.clone(), &view.job_id).unwrap();
    wait_run_terminal(&cfg, &run_id);
    let st = state::RunState::load(&cfg.runs_dir().join(&run_id)).unwrap();
    assert!(
        matches!(st.status, RunStatus::Completed | RunStatus::Paused),
        "status={:?}",
        st.status
    );
    for id in ["t1", "t2", "t3"] {
        assert!(st.tasks.contains_key(id), "missing {id}");
    }
}

/// 3) Already cco-plan/v1 → parse plan job → confirm → exec.
#[tokio::test]
async fn golden_cco_v1_plan_confirm_exec() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hello.cco.yaml");
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/plans/hello.cco.yaml");
    std::fs::copy(&example, &plan_path).unwrap();

    let cfg = test_config(tmp.path());
    let adapter = peek_adapter(&project, &plan_path).unwrap();
    assert_eq!(adapter, "cco-plan/v1");
    assert!(is_structured_adapter(&adapter));

    // load_plan sanity
    let ir_direct = load_plan(&project, &plan_path, None, &cfg).unwrap();
    assert_eq!(ir_direct.adapter, "cco-plan/v1");
    ir_direct.validate().unwrap();

    let view = start_plan_job(
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
    assert_eq!(view.task_count, Some(2));
    assert_eq!(view.adapter.as_deref(), Some("cco-plan/v1"));

    // Inject planner cost file to exercise budget columns (P1-5).
    let job_dir = cfg.state_root.join("plan_jobs").join(&view.job_id);
    std::fs::write(
        job_dir.join("planner_cost.json"),
        r#"{"cost_usd": 0.12}"#,
    )
    .unwrap();
    // Patch job.json so mark_confirmed copies cost.
    let mut job: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(job_dir.join("job.json")).unwrap()).unwrap();
    job["planner_cost_usd"] = serde_json::json!(0.12);
    std::fs::write(job_dir.join("job.json"), serde_json::to_string_pretty(&job).unwrap()).unwrap();

    let run_id = confirm_start(cfg.clone(), &view.job_id).unwrap();
    wait_run_terminal(&cfg, &run_id);

    let st = state::RunState::load(&cfg.runs_dir().join(&run_id)).unwrap();
    assert!(
        matches!(st.status, RunStatus::Completed | RunStatus::Paused),
        "status={:?}",
        st.status
    );
    // Planner cost attached to run dir
    let cost_path = st.run_dir.join("planner_cost.json");
    assert!(cost_path.exists(), "planner_cost.json should be copied on confirm");
    report::write_reports(&st).unwrap();
    let md = std::fs::read_to_string(st.run_dir.join("report.md")).unwrap();
    assert!(md.contains("## Budget") || md.contains("规划"), "report should split budget:\n{md}");
    assert!(md.contains("0.12") || md.contains("规划"), "{md}");

    let live = project_live_view(&cfg, &project, 4_000).unwrap();
    assert_eq!(live.planner_cost_usd, Some(0.12));
}

/// Limits: validate rejects > MAX_TASKS (unit covered in plan mod; smoke here via load).
/// MAX_TASKS = hard cap including system-post tails; PLANNER_MAX_TASKS is the softer split cap.
#[test]
fn golden_limits_constants_exported() {
    assert_eq!(MAX_TASKS, 23);
    assert_eq!(cco::plan::PLANNER_MAX_TASKS, 20);
    assert!(cco::plan::MAX_PROMPT_CHARS >= 8_000);
    assert_eq!(cco::plan::MAX_TIMEOUT_SECS, 86_400);
}
