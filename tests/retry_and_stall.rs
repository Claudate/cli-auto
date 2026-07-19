//! Auto-retry on fail + stall patrol → pause when exhausted.
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};

fn sched(
    ir: cco::plan::PlanIR,
    run_state: RunState,
    registry: ProviderRegistry,
    retry_max: u32,
    stall_secs: u64,
) -> Scheduler {
    Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(30),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: cco::SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: None,
        provider_max_parallel: Default::default(),
        retry_max,
        stall_secs,
    }
}

#[tokio::test]
async fn fail_once_auto_retries_and_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/fail-once.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: fail-once
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: flaky
    prompt: |
      CCO_FAKE_FAIL_ONCE
      CCO_DONE ok
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    let status = sched(ir, run_state, registry, /*retry_max*/ 2, 600)
        .run()
        .await
        .unwrap();
    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert!(
        st.tasks["a"].attempt >= 2,
        "expected at least 2 attempts, got {}",
        st.tasks["a"].attempt
    );
    assert!(
        run_dir.join("tasks/a/attempt-1.stdout.json").exists()
            || run_dir.join("tasks/a/attempt-1.meta.json").exists(),
        "first attempt logs should be archived"
    );
}

#[tokio::test]
async fn stall_then_retry_exhausted_pauses() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hang.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: hang
defaults:
  provider: fake
  mode: print
tasks:
  - id: hang
    title: stuck
    prompt: |
      CCO_FAKE_HANG
      CCO_DONE ok
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();

    // stall_secs=1 → detect quickly; retry_max=1 → 2 total attempts then pause
    let status = sched(ir, run_state, registry, /*retry_max*/ 1, /*stall*/ 1)
        .run()
        .await
        .unwrap();
    assert!(
        matches!(status, RunStatus::Paused | RunStatus::Failed),
        "expected pause after stall retries, got {status:?}"
    );
    let st = RunState::load(&run_dir).unwrap();
    assert!(
        matches!(
            st.tasks["hang"].status,
            TaskStatus::Timeout | TaskStatus::Failed
        ),
        "hang task should be terminal fail/timeout, got {:?}",
        st.tasks["hang"].status
    );
    assert!(
        st.tasks["hang"].attempt >= 2,
        "stall should have retried, attempt={}",
        st.tasks["hang"].attempt
    );
    assert_eq!(
        st.tasks["hang"].last_retry_reason.as_deref(),
        Some("stall")
    );
}
