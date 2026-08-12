//! End-to-end integration test: serve → test with wait_for runtime collaboration
//!
//! Verifies the full collaboration pipeline:
//! 1. Scheduler publishes Output/Step/StatusChange events during execution
//! 2. The waiting task is deferred (non-blocking gate) until the serve task's
//!    output matches, then spawns — without any depends_on edge
//! 3. An unsatisfiable wait (dep terminal, no match) fails the task instead of
//!    hanging the run

use std::sync::Arc;
use std::time::Duration;

fn make_scheduler(
    plan: cco::plan::PlanIR,
    tmp: &std::path::Path,
) -> cco::runtime::Scheduler {
    use cco::runtime::collab::CollabBus;
    use cco::runtime::provider::ProviderRegistry;
    use cco::state::RunState;

    let fake = Arc::new(cco::runtime::provider::fake::FakeProvider::with_name(
        "inline".into(),
        "claude",
    ));
    let registry = ProviderRegistry::from_providers(vec![fake]).expect("registry");
    let state = RunState::new("e2e-run".into(), tmp.to_path_buf(), &plan, tmp.join("run"));

    cco::runtime::Scheduler {
        max_parallel: 2,
        plan,
        state,
        registry,
        poll_interval: Duration::from_millis(10),
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
        retry_max: 0,
        stall_secs: 600,
        failover_enabled: false,
        fallback_extra_attempts: 1,
        failover_order: vec![],
        cost_escalate_enabled: false,
        browser: cco::config::BrowserConfig::default(),
        provider_unhealthy: vec![],
        collab_bus: Some(Arc::new(CollabBus::new())),
        memory: None,
    }
}

#[tokio::test]
async fn e2e_serve_then_test_with_output_match() {
    // Load the serve-then-test plan
    let plan_path = std::path::PathBuf::from("examples/plans/serve-then-test.cco.yaml");
    let project_root = std::env::current_dir().unwrap();
    let config = cco::config::Config::default();
    let plan = cco::plan::load_plan(&project_root, &plan_path, None, &config).expect("load plan");

    let tmp = std::env::temp_dir().join(format!("cco-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let scheduler = make_scheduler(plan, &tmp);

    // Run scheduler and verify it completes
    let run_status = scheduler.run().await.expect("scheduler run");

    // run() returns RunStatus enum from state module
    use cco::state::RunStatus;
    assert!(
        matches!(run_status, RunStatus::Completed),
        "scheduler should complete successfully, got {:?}",
        run_status
    );

    // Verify both tasks completed
    let run_json_path = tmp.join("run").join("run.json");
    let run_json = std::fs::read_to_string(&run_json_path).expect("read run.json");
    let run_data: serde_json::Value = serde_json::from_str(&run_json).expect("parse run.json");

    let serve_status = run_data["tasks"]["serve"]["status"]
        .as_str()
        .expect("serve status");
    let test_status = run_data["tasks"]["test"]["status"]
        .as_str()
        .expect("test status");

    assert_eq!(serve_status, "done", "serve task should complete");
    assert_eq!(test_status, "done", "test task should complete");

    let _ = std::fs::remove_dir_all(tmp);
}

/// Dep finished without ever matching the pattern → waiting task must fail
/// fast (not hang until a wall-clock timeout).
#[tokio::test]
async fn e2e_unsatisfiable_wait_fails_instead_of_hanging() {
    let tmp = std::env::temp_dir().join(format!("cco-e2e-fail-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let plan_yaml = r#"
schema: cco-plan/v1
name: wait-never-matches
max_parallel: 2
tasks:
  - id: serve
    title: short-lived producer
    provider: claude
    prompt: just finish quickly
  - id: test
    title: waits for a line that never appears
    provider: claude
    prompt: should never spawn
    wait_for:
      - task_id: serve
        condition: output_match
        pattern: "NEVER_MATCHES_XYZ"
"#;
    let plan_path = tmp.join("wait-never-matches.cco.yaml");
    std::fs::write(&plan_path, plan_yaml).unwrap();
    let config = cco::config::Config::default();
    let plan = cco::plan::load_plan(&tmp, &plan_path, None, &config).expect("load plan");

    let scheduler = make_scheduler(plan, &tmp);
    let run_status = scheduler.run().await.expect("scheduler run");

    use cco::state::RunStatus;
    assert!(
        !matches!(run_status, RunStatus::Completed),
        "run must not complete when a wait_for can never be satisfied"
    );

    let run_json = std::fs::read_to_string(tmp.join("run").join("run.json")).expect("run.json");
    let run_data: serde_json::Value = serde_json::from_str(&run_json).unwrap();
    assert_eq!(run_data["tasks"]["serve"]["status"].as_str(), Some("done"));
    assert_eq!(run_data["tasks"]["test"]["status"].as_str(), Some("failed"));
    let err = run_data["tasks"]["test"]["error"].as_str().unwrap_or("");
    assert!(
        err.contains("wait_for"),
        "task error should explain the wait failure, got: {err}"
    );

    let _ = std::fs::remove_dir_all(tmp);
}
