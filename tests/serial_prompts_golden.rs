use cco::config::Config;
use cco::plan::load_plan;

#[test]
fn serial_prompts_golden_fixture() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("tests/fixtures/serial-prompts-sample.md");
    assert!(fixture.exists(), "missing {}", fixture.display());

    let mut config = Config::default();
    config.default.default_provider = "fake".into();
    config.default.worktree = false;

    let plan = load_plan(
        root.as_path(),
        &fixture,
        Some("serial-prompts/v0"),
        &config,
    )
    .expect("parse serial-prompts");

    assert_eq!(plan.adapter, "serial-prompts/v0");
    let ids: Vec<_> = plan.tasks.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"t1"), "got {ids:?}");
    assert!(ids.contains(&"t2"), "got {ids:?}");
    assert!(ids.contains(&"t3"), "got {ids:?}");

    let t3 = plan.task("t3").expect("t3");
    assert!(
        t3.depends_on.iter().any(|d| d == "t1") || t3.depends_on.iter().any(|d| d == "t2"),
        "t3 deps = {:?}",
        t3.depends_on
    );
    assert!(t3.prompt.contains("INTEGRATION") || t3.prompt.contains("CCO_DONE"));

    plan.validate().unwrap();
}
