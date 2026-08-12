use std::path::PathBuf;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::ProviderRegistry;
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};
use cco::terminal::{SessionKind, TerminalManager};
use cco::TaskStatus;

#[tokio::test]
async fn acceptance_failure_marks_task_failed() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/acc.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: acc
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
    acceptance: "exit 1"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.worktree = false;
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
    let status = Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(20),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
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
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
    .run()
    .await
    .unwrap();

    // on_failure default pause
    assert!(matches!(status, RunStatus::Paused | RunStatus::Failed));
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Failed);
    assert!(run_dir.join("tasks/a/acceptance.json").exists() || st.tasks["a"].error.is_some());
}

/// H0-7: human Chinese criteria must not mark the task Failed via `sh -c`.
#[tokio::test]
async fn human_acceptance_does_not_fail_task() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/human-acc.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: human-acc
defaults:
  provider: fake
  mode: print
  worktree: false
on_failure: continue
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
    acceptance: "存在 VERDICT 与 ISSUES；阻塞项必须 FAIL"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    config.default.worktree = false;
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert!(
        !cco::plan::is_runnable_verify(ir.tasks[0].acceptance.as_deref().unwrap_or("")),
        "fixture must be non-shell"
    );
    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );
    let registry = ProviderRegistry::from_config(&config).unwrap();
    let status = Scheduler {
        max_parallel: 1,
        plan: ir,
        state: run_state,
        registry,
        poll_interval: Duration::from_millis(20),
        yes: true,
        only: None,
        from_task: None,
        dry_run: false,
        mirror_state: None,
        auto_open_terminal: false,
        terminal_kind: SessionKind::Embedded,
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
        provider_unhealthy: Vec::new(),
        collab_bus: None,
        memory: None,
    }
    .run()
    .await
    .unwrap();

    assert!(
        matches!(status, RunStatus::Completed),
        "human acceptance must not fail the run, got {status:?}"
    );
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    let acc_path = run_dir.join("tasks/a/acceptance.json");
    if acc_path.exists() {
        let body = std::fs::read_to_string(&acc_path).unwrap();
        assert!(
            body.contains("skipped_not_shell") || body.contains("\"skipped\": true"),
            "acceptance.json should note skip, got {body}"
        );
        assert!(
            !body.contains("\"ok\": true") || body.contains("skipped"),
            "skip must not be reported as shell pass"
        );
    }
}

#[tokio::test]
async fn terminal_embedded_session_persists() {
    let tmp = tempfile::tempdir().unwrap();
    let run_dir = tmp.path().join("run");
    std::fs::create_dir_all(&run_dir).unwrap();
    let log = run_dir.join("out.log");
    std::fs::write(&log, "hello\n").unwrap();

    let tm = TerminalManager::for_run(&run_dir, "auto", None).with_limits(4, 4);
    let s = tm
        .open_embedded("t1", tmp.path(), &log)
        .expect("open embedded");
    assert_eq!(s.task_id, "t1");
    assert!(!s.closed);
    assert_eq!(s.kind, SessionKind::Embedded);

    let list = tm.list().unwrap();
    assert_eq!(list.len(), 1);
    let closed = tm.close(&s.id).unwrap();
    assert!(closed.closed);
}

#[test]
fn detect_launcher_auto() {
    let l = cco::terminal::detect_launcher("auto");
    // just ensure it returns something stable
    assert!(!l.as_str().is_empty());
    let _ = PathBuf::from(".");
}

#[test]
fn detect_launcher_windows_names() {
    // Prefer aliases must resolve even on macOS/Linux host (no spawn).
    assert_eq!(cco::terminal::detect_launcher("wt").as_str(), "wt");
    assert_eq!(
        cco::terminal::detect_launcher("powershell").as_str(),
        "powershell"
    );
    assert_eq!(cco::terminal::detect_launcher("cmd").as_str(), "cmd");
    assert_eq!(
        cco::terminal::detect_launcher("windows_terminal").as_str(),
        "wt"
    );
}
