use std::path::PathBuf;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::report;
use cco::runtime::provider::ProviderRegistry;
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};

#[tokio::test]
async fn fake_provider_runs_two_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/hello.cco.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: t
defaults:
  provider: fake
  mode: print
tasks:
  - id: a
    title: a
    prompt: "do a\nCCO_DONE ok"
  - id: b
    title: b
    depends_on: [a]
    prompt: "do b\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks.len(), 2);

    let run_id = state::new_run_id();
    let run_dir = state::prepare_run_dir(&config.runs_dir(), &run_id).unwrap();
    let run_state = RunState::new(
        run_id,
        project.canonicalize().unwrap(),
        &ir,
        run_dir.clone(),
    );

    let registry = ProviderRegistry::from_config(&config).unwrap();
    let sched = Scheduler {
        max_parallel: 2,
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
        terminal_kind: cco::SessionKind::Embedded,
        terminal_manager: None,
        run_max_budget_usd: None,
        provider_max_parallel: Default::default(),
    };

    let status = sched.run().await.unwrap();
    assert_eq!(status, RunStatus::Completed);

    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, cco::runtime::provider::TaskStatus::Done);
    assert_eq!(st.tasks["b"].status, cco::runtime::provider::TaskStatus::Done);
    report::write_reports(&st).unwrap();
    assert!(run_dir.join("report.md").exists());
    assert!(run_dir.join("events.jsonl").exists());
    let _ = PathBuf::from(".");
}
