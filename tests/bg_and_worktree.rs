use std::process::Command;
use std::time::Duration;

use cco::config::Config;
use cco::plan::load_plan;
use cco::runtime::provider::{ProviderRegistry, TaskStatus};
use cco::runtime::worktree;
use cco::runtime::Scheduler;
use cco::state::{self, RunState, RunStatus};
use cco::SessionKind;

#[tokio::test]
async fn fake_bg_mode_polls_until_done() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(project.join("docs/plans")).unwrap();
    let plan_path = project.join("docs/plans/bg.yaml");
    std::fs::write(
        &plan_path,
        r#"
schema: cco-plan/v1
name: bg
defaults:
  provider: fake
  mode: bg
  worktree: false
tasks:
  - id: a
    title: bg task
    mode: bg
    prompt: "background work\nCCO_DONE ok"
"#,
    )
    .unwrap();

    let mut config = Config::default();
    config.state_root = tmp.path().join("state");
    config.default.default_provider = "fake".into();
    std::fs::create_dir_all(config.runs_dir()).unwrap();
    std::env::set_var("CCO_FAKE_BG_MS", "50");

    let ir = load_plan(&project, &plan_path, Some("cco-plan/v1"), &config).unwrap();
    assert_eq!(ir.tasks[0].mode, "bg");

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
        poll_interval: Duration::from_millis(15),
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

    assert_eq!(status, RunStatus::Completed);
    let st = RunState::load(&run_dir).unwrap();
    assert_eq!(st.tasks["a"].status, TaskStatus::Done);
    assert!(st.tasks["a"]
        .agent_id
        .as_deref()
        .unwrap_or("")
        .contains("fake-bg"));
}

#[test]
fn worktree_creates_branch_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("repo");
    std::fs::create_dir_all(&project).unwrap();
    // init git
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["init"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["config", "user.email", "cco@test.local"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["config", "user.name", "cco"])
        .status()
        .unwrap()
        .success());
    std::fs::write(project.join("README"), "hi").unwrap();
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .args(["-C"])
        .arg(&project)
        .args(["commit", "-m", "init"])
        .status()
        .unwrap()
        .success());

    let info = worktree::ensure_worktree(&project, "run1", "t1").unwrap();
    assert!(info.path.exists());
    assert!(info.branch.contains("cco/"));
    assert!(info.created);

    let (wd, meta) = worktree::resolve_work_dir(&project, "run1", "t1", true).unwrap();
    assert_eq!(wd, info.path);
    assert!(meta.is_some());
}

#[test]
fn parse_agent_id_from_bg_output() {
    use cco::runtime::provider::claude::parse_agent_id;
    let s = "backgrounded · 895cb666 (idle — send a prompt to start)";
    assert_eq!(parse_agent_id(s).as_deref(), Some("895cb666"));
}
