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
