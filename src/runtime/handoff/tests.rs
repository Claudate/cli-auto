//! Unit tests for handoff adapter (migrated from monolith · A1-5).

use std::path::PathBuf;

use crate::plan::{OnFailure, PlanIR, TaskIR, TaskRole};
use crate::runtime::provider::{TaskResult, TaskStatus};
use crate::state::RunState;
use tempfile::tempdir;

use super::*;

fn sample_plan(outputs_a: Vec<String>) -> PlanIR {
    PlanIR {
        schema: "cco-plan/v1".into(),
        name: "t".into(),
        adapter: "cco-plan/v1".into(),
        source_path: PathBuf::from("plan.yaml"),
        max_parallel: 2,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: "fake".into(),
        default_mode: "print".into(),
        worktree: false,
        require_inspect: false,
        tasks: vec![
            TaskIR {
                id: "a".into(),
                title: "a".into(),
                depends_on: vec![],
                group: None,
                provider: "fake".into(),
                mode: "print".into(),
                prompt: "do a".into(),
                verify_cmd: None,
                acceptance: None,
                timeout_secs: None,
                worktree: None,
                provider_opts: serde_json::json!({}),
                optional: false,
                include: true,
                role: None,
                scope: None,
                outputs: outputs_a,
                tags: vec![],
            },
            TaskIR {
                id: "b".into(),
                title: "b".into(),
                depends_on: vec!["a".into()],
                group: None,
                provider: "fake".into(),
                mode: "print".into(),
                prompt: "do b".into(),
                verify_cmd: None,
                acceptance: None,
                timeout_secs: None,
                worktree: None,
                provider_opts: serde_json::json!({}),
                optional: false,
                include: true,
                role: None,
                scope: None,
                outputs: vec![],
                tags: vec![],
            },
        ],
    }
}

#[test]
fn shell_and_task_lifecycle() {
    let tmp = tempdir().unwrap();
    let run_dir = tmp.path().join("run1");
    std::fs::create_dir_all(&run_dir).unwrap();
    let plan = sample_plan(vec![".cco-out/a/SUMMARY.md".into()]);
    let state = RunState::new(
        "run1".into(),
        tmp.path().to_path_buf(),
        &plan,
        run_dir.clone(),
    );

    write_shell(&plan, &state).unwrap();
    assert!(Handoff::path_md(&run_dir).exists());
    assert!(Handoff::path_json(&run_dir).exists());
    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.board.len(), 2);
    assert!(h.board.iter().all(|r| r.status == "pending"));

    on_task_start(&plan, &state, "a").unwrap();
    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(
        h.board.iter().find(|r| r.id == "a").unwrap().status,
        "running"
    );

    let out = tmp.path().join(".cco-out/a");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("SUMMARY.md"), "did a\n").unwrap();

    let result = TaskResult {
        status: TaskStatus::Done,
        exit_code: Some(0),
        stdout_path: None,
        session_id: None,
        agent_id: None,
        cost_usd: Some(0.01),
        raw: serde_json::json!({"result": "fake ok"}),
        error: None,
        done_marker: true,
        execution_evidence: true,
    };
    on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();
    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(h.board.iter().find(|r| r.id == "a").unwrap().status, "done");
    assert!(h.fragments.contains_key("a"));
    assert!(h.fragments["a"].summary.contains("did a") || !h.fragments["a"].summary.is_empty());
    let md = std::fs::read_to_string(Handoff::path_md(&run_dir)).unwrap();
    assert!(md.contains("## Board"));
    assert!(md.contains("## Fragments"));
    assert!(md.contains("## Open risks"));
    assert!(md.contains("## Instructions for next worker"));
}

#[test]
fn missing_outputs_detected() {
    let tmp = tempdir().unwrap();
    let plan = sample_plan(vec![".cco-out/missing.md".into()]);
    let missing = missing_outputs(&plan.tasks[0], tmp.path(), tmp.path());
    assert_eq!(missing, vec![".cco-out/missing.md".to_string()]);
}

/// P1-5: missing handoff file → identity shell, no panic.
#[test]
fn prompt_prefix_without_handoff_file() {
    let tmp = tempdir().unwrap();
    let plan = sample_plan(vec![]);
    let task = &plan.tasks[0];
    let prefix = build_prompt_prefix(task, tmp.path());
    assert!(prefix.contains(HANDOFF_PROMPT_OPEN));
    assert!(prefix.contains(HANDOFF_PROMPT_CLOSE));
    assert!(prefix.contains("task=a"));
    assert!(prefix.contains("provider=fake"));
    assert!(prefix.contains("CCO_DONE ok"));
    assert!(prefix.contains("(no handoff yet)") || prefix.contains("## Board"));
    let wrapped = with_handoff_prefix("do a\nCCO_DONE ok", task, tmp.path());
    assert!(wrapped.starts_with(HANDOFF_PROMPT_OPEN));
    assert!(wrapped.contains("do a"));
    // idempotent
    let twice = with_handoff_prefix(&wrapped, task, tmp.path());
    assert_eq!(twice.matches(HANDOFF_PROMPT_OPEN).count(), 1);
}

/// P1-5: after task a ends, task b prefix includes Board + fragment a.
#[test]
fn prompt_prefix_includes_depends_on_fragment() {
    let tmp = tempdir().unwrap();
    let run_dir = tmp.path().join("run1");
    std::fs::create_dir_all(&run_dir).unwrap();
    let plan = sample_plan(vec![".cco-out/a/SUMMARY.md".into()]);
    let state = RunState::new(
        "run1".into(),
        tmp.path().to_path_buf(),
        &plan,
        run_dir.clone(),
    );
    write_shell(&plan, &state).unwrap();
    on_task_start(&plan, &state, "a").unwrap();
    let out = tmp.path().join(".cco-out/a");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("SUMMARY.md"), "summary from a\n").unwrap();
    let result = TaskResult {
        status: TaskStatus::Done,
        exit_code: Some(0),
        stdout_path: None,
        session_id: None,
        agent_id: None,
        cost_usd: Some(0.01),
        raw: serde_json::json!({"result": "fake ok"}),
        error: None,
        done_marker: true,
        execution_evidence: true,
    };
    on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();

    let prefix = build_prompt_prefix(&plan.tasks[1], &run_dir);
    assert!(prefix.contains(HANDOFF_PROMPT_OPEN));
    assert!(prefix.contains("task=b"));
    assert!(prefix.contains("depends_on: a"));
    assert!(prefix.contains("## Board"));
    assert!(prefix.contains("| a |"));
    assert!(prefix.contains("### a"));
    assert!(
        prefix.contains("summary from a") || prefix.contains("fake ok"),
        "prefix should include dep fragment summary: {prefix}"
    );
    assert!(prefix.contains(HANDOFF_PROMPT_CLOSE));
    let full = with_handoff_prefix("do b\nCCO_DONE ok", &plan.tasks[1], &run_dir);
    assert!(full.contains("do b"));
}

// ── P2-3 unit tests ──────────────────────────────────────────────────

#[test]
fn parse_verdict_fail_and_pass() {
    assert_eq!(parse_verdict_text("FAIL\nreason"), InspectVerdict::Fail);
    assert_eq!(parse_verdict_text("PASS\nok"), InspectVerdict::Pass);
    assert_eq!(
        parse_verdict_text("VERDICT: FAIL — scope leak"),
        InspectVerdict::Fail
    );
    assert_eq!(parse_verdict_text("VERDICT=PASS"), InspectVerdict::Pass);
    assert_eq!(parse_verdict_text("maybe later"), InspectVerdict::Unknown);
    // FAIL wins when both present in body
    assert_eq!(
        parse_verdict_text("notes\nPASS was hoped\nbut VERDICT=FAIL overall"),
        InspectVerdict::Fail
    );
}

#[test]
fn on_task_end_folds_issues_on_verdict_fail() {
    let tmp = tempdir().unwrap();
    let run_dir = tmp.path().join("run-inspect");
    std::fs::create_dir_all(&run_dir).unwrap();
    let inspect_dir = tmp.path().join(".cco-out/inspect");
    std::fs::create_dir_all(&inspect_dir).unwrap();
    std::fs::write(
        inspect_dir.join("VERDICT.md"),
        "FAIL\nscope leak in feat-a\n",
    )
    .unwrap();
    std::fs::write(
        inspect_dir.join("ISSUES.md"),
        "- file: examples/demo_a/x.rs\n- symptom: wrote outside scope\n- suggestion: revert + narrow edit\n",
    )
    .unwrap();

    let mut plan = sample_plan(vec![
        ".cco-out/inspect/VERDICT.md".into(),
        ".cco-out/inspect/ISSUES.md".into(),
    ]);
    plan.tasks[0].id = "inspect".into();
    plan.tasks[0].role = Some(TaskRole::Inspect);
    plan.tasks[0].outputs = vec![
        ".cco-out/inspect/VERDICT.md".into(),
        ".cco-out/inspect/ISSUES.md".into(),
    ];

    let state = RunState::new(
        "run-inspect".into(),
        tmp.path().to_path_buf(),
        &plan,
        run_dir.clone(),
    );
    write_shell(&plan, &state).unwrap();

    let result = TaskResult {
        status: TaskStatus::Failed,
        exit_code: Some(0),
        stdout_path: None,
        session_id: None,
        agent_id: None,
        cost_usd: Some(0.02),
        raw: serde_json::json!({}),
        error: Some("inspect VERDICT=FAIL (2 ISSUES line(s) for rework)".into()),
        done_marker: false,
        execution_evidence: false,
    };
    on_task_end(&plan, &state, &plan.tasks[0], &result, Some(tmp.path())).unwrap();

    let h = Handoff::load(&run_dir).unwrap();
    assert_eq!(
        h.board.iter().find(|r| r.id == "inspect").unwrap().status,
        "failed"
    );
    assert!(
        h.open_risks.iter().any(|r| r.contains("ISSUES[inspect]")),
        "open_risks={:?}",
        h.open_risks
    );
    assert!(
        h.open_risks.iter().any(|r| r.contains("REWORK_HOOK")),
        "expected REWORK_HOOK in open_risks={:?}",
        h.open_risks
    );
    assert!(
        h.instructions_for_next.contains("REWORK_HOOK")
            || h.instructions_for_next.contains("ISSUES"),
        "instructions={}",
        h.instructions_for_next
    );
    let md = std::fs::read_to_string(Handoff::path_md(&run_dir)).unwrap();
    assert!(md.contains("ISSUES[inspect]") || md.contains("REWORK_HOOK"));
}

// ── P-loop unit tests ───────────────────────────────────────────────

#[test]
fn parse_issues_grades_severity() {
    let text = r#"
- id: I-1
  severity=map
  plan_ref: §8 GEB
  path: CLAUDE.md
  symptom: L1 still says 待验
  fix_wp: Update CLAUDE.md config row to F0+F1 closed

- id: I-2 severity=blocking plan_ref=S5 path=web/
  symptom: desktop Chinese path not verified
  fix_wp: Re-run GUI or mark DEGRADED only if plan allows

- id: I-3
  severity: residual
  plan_ref: F2
  symptom: optional polish
"#;
    let parsed = parse_issues_text(text);
    assert!(parsed.len() >= 3, "parsed={parsed:?}");
    let i1 = parsed.iter().find(|i| i.id.contains("I-1")).unwrap();
    assert_eq!(i1.severity, IssueSeverity::Map);
    assert!(i1.severity.is_blocking_for_gate());
    let i2 = parsed.iter().find(|i| i.id.contains("I-2")).unwrap();
    assert_eq!(i2.severity, IssueSeverity::Blocking);
    let i3 = parsed.iter().find(|i| i.id.contains("I-3")).unwrap();
    assert_eq!(i3.severity, IssueSeverity::Residual);
    assert!(!i3.severity.is_blocking_for_gate());
    assert_eq!(count_blocking_issues(&parsed), 2);
}

#[test]
fn parse_issues_fail_closed_without_severity() {
    let parsed = parse_issues_text("- missing plan pointer in CLAUDE.md\n");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].severity, IssueSeverity::Blocking);
}

#[test]
fn parse_verdict_result_prefix() {
    assert_eq!(
        parse_verdict_text("**Result: FAIL**\n\n| plan_ref |"),
        InspectVerdict::Fail
    );
    assert_eq!(parse_verdict_text("Result: PASS\nok"), InspectVerdict::Pass);
}

#[test]
fn build_rework_plan_has_inspect_sink_and_plan_refs() {
    let base = sample_plan(vec![]);
    let issues = vec![
        ParsedIssue {
            id: "I-1".into(),
            severity: IssueSeverity::Map,
            plan_ref: "§8".into(),
            path: "CLAUDE.md".into(),
            symptom: "stale".into(),
            fix_wp: "fix pointer".into(),
            raw: "severity=map plan_ref=§8 path=CLAUDE.md".into(),
        },
        ParsedIssue {
            id: "I-2".into(),
            severity: IssueSeverity::Blocking,
            plan_ref: "S5".into(),
            path: "src/lib.rs".into(),
            symptom: "missing".into(),
            fix_wp: "implement".into(),
            raw: "severity=blocking plan_ref=S5".into(),
        },
    ];
    let ir = build_rework_plan(&base, &issues, 1, "run-src").unwrap();
    assert!(ir.require_inspect);
    assert_eq!(ir.tasks.len(), 2);
    assert_eq!(ir.tasks[0].role, Some(TaskRole::Implement));
    assert_eq!(ir.tasks[1].role, Some(TaskRole::Inspect));
    assert!(ir.tasks[1].depends_on.contains(&ir.tasks[0].id));
    assert!(ir.tasks[0].prompt.contains("I-1") || ir.tasks[0].prompt.contains("severity"));
    assert!(ir.tasks[0].prompt.contains("plan_ref") || ir.tasks[0].prompt.contains("S5"));
    assert!(ir.tasks[1].prompt.contains("禁止") || ir.tasks[1].prompt.contains("blocking"));
    ir.validate().unwrap();
}

#[test]
fn map_only_rework_uses_whitelist_scope() {
    let base = sample_plan(vec![]);
    let issues = vec![ParsedIssue {
        id: "I-map".into(),
        severity: IssueSeverity::Map,
        plan_ref: "GEB".into(),
        path: "CLAUDE.md".into(),
        symptom: "stale".into(),
        fix_wp: "update L1".into(),
        raw: "severity=map".into(),
    }];
    let ir = build_rework_plan(&base, &issues, 1, "r1").unwrap();
    let scope = ir.tasks[0].scope.as_ref().unwrap();
    assert!(
        scope
            .paths
            .iter()
            .any(|p| p.contains("CLAUDE") || p.contains("docs")),
        "paths={:?}",
        scope.paths
    );
    assert!(ir.tasks[0].title.contains("地图") || ir.tasks[0].prompt.contains("GEB"));
}

#[test]
fn system_push_gate_blocks_without_verdict() {
    use crate::plan::{SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID};
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let root = dir.path();
    let mut plan = sample_plan(vec![]);
    // inject-like inspect + push
    plan.tasks.push(TaskIR {
        id: SYS_POST_INSPECT_ID.into(),
        title: "巡检".into(),
        depends_on: vec![plan.tasks[0].id.clone()],
        group: Some("系统收尾".into()),
        provider: "claude".into(),
        mode: "print".into(),
        prompt: "inspect".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: serde_json::json!({}),
        optional: true,
        include: true,
        role: Some(TaskRole::Inspect),
        scope: None,
        outputs: vec![INSPECT_VERDICT_REL.into()],
        tags: vec![],
    });
    plan.tasks.push(TaskIR {
        id: SYS_POST_GIT_PUSH_ID.into(),
        title: "push".into(),
        depends_on: vec![SYS_POST_INSPECT_ID.into()],
        group: Some("系统收尾".into()),
        provider: "claude".into(),
        mode: "print".into(),
        prompt: "push".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: serde_json::json!({}),
        optional: true,
        include: true,
        role: Some(TaskRole::Integrate),
        scope: None,
        outputs: vec![],
        tags: vec![],
    });
    let push = plan.task(SYS_POST_GIT_PUSH_ID).unwrap();
    let err = system_push_inspect_gate(&plan, push, root).unwrap_err();
    assert!(err.contains("CCO_PUSH_SKIPPED"), "{err}");
    assert!(
        err.contains("inspect_unknown") || err.contains("inspect_not_pass"),
        "{err}"
    );

    // PASS file → Ok
    let vdir = root.join(".cco-out/inspect");
    std::fs::create_dir_all(&vdir).unwrap();
    std::fs::write(vdir.join("VERDICT.md"), "Result: PASS\n").unwrap();
    std::fs::write(vdir.join("ISSUES.md"), "无\n").unwrap();
    let push = plan.task(SYS_POST_GIT_PUSH_ID).unwrap();
    system_push_inspect_gate(&plan, push, root).expect("PASS should allow push");

    // FAIL → skip
    std::fs::write(vdir.join("VERDICT.md"), "Result: FAIL\n").unwrap();
    let err = system_push_inspect_gate(&plan, push, root).unwrap_err();
    assert!(err.contains("inspect_not_pass"), "{err}");
}

/// P2-2: host writes CHANGED.md from declared outputs when git is unavailable.
#[test]
fn write_task_diff_lists_outputs_without_git() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let wd = root.join("wt");
    std::fs::create_dir_all(&wd).unwrap();
    let out = wd.join(".cco-out/t1/SUMMARY.md");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, "summary\n").unwrap();

    let task = TaskIR {
        id: "t1".into(),
        title: "impl".into(),
        depends_on: vec![],
        group: None,
        provider: "fake".into(),
        mode: "print".into(),
        prompt: "do\nCCO_DONE ok".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: serde_json::json!({}),
        optional: false,
        include: true,
        role: Some(TaskRole::Implement),
        scope: None,
        outputs: vec![".cco-out/t1/SUMMARY.md".into()],
        tags: vec![],
    };
    let rel = write_task_diff(&task, &wd, root)
        .unwrap()
        .expect("should write CHANGED.md");
    assert_eq!(rel, ".cco-out/t1/CHANGED.md");
    let text = std::fs::read_to_string(wd.join(&rel)).unwrap();
    assert!(text.contains("CHANGED"), "{text}");
    assert!(
        text.contains("SUMMARY.md") || text.contains("OUT "),
        "expected outputs listed: {text}"
    );
}

/// Host SoT: GATE.json wins over VERDICT.md prose that mentions FAIL.
#[test]
fn gate_json_wins_over_verdict_prose_fail() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let inspect_dir = root.join(".cco-out/inspect");
    std::fs::create_dir_all(&inspect_dir).unwrap();
    std::fs::write(
        inspect_dir.join("VERDICT.md"),
        "# VERDICT\n\nResult: **PASS**\n\nP1b 可选 FAIL 不阻塞\n",
    )
    .unwrap();
    std::fs::write(
        inspect_dir.join("ISSUES.md"),
        "### issue_id=R1\n- severity: residual\n- symptom: uncommitted\n",
    )
    .unwrap();
    std::fs::write(
        inspect_dir.join("GATE.json"),
        r#"{"schema":"cco-inspect-gate/v1","result":"pass","blocking":0,"map":0,"residual":1}"#,
    )
    .unwrap();

    let task = TaskIR {
        id: "inspect".into(),
        title: "inspect".into(),
        depends_on: vec![],
        group: None,
        provider: "fake".into(),
        mode: "print".into(),
        prompt: "p".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: None,
        provider_opts: serde_json::json!({}),
        optional: false,
        include: true,
        role: Some(TaskRole::Inspect),
        scope: None,
        outputs: vec![
            ".cco-out/inspect/VERDICT.md".into(),
            ".cco-out/inspect/ISSUES.md".into(),
        ],
        tags: vec![],
    };

    assert_eq!(
        read_inspect_verdict(&task, root, root),
        InspectVerdict::Pass
    );
    let (blocked, n) = inspect_pass_blocked_by_issues(&task, root, root);
    assert!(!blocked && n == 0, "GATE residual must not block");
    let reason = inspect_gate_fail_reason(
        read_inspect_verdict(&task, root, root),
        n,
        1,
        true,
        "inspect",
    );
    assert!(reason.is_none(), "gate must pass, got {reason:?}");
}

/// Agent wrote GATE fail + blocking=1 for handwalk residual — host must not pause.
#[test]
fn handwalk_gate_fail_demoted_pass() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let inspect_dir = root.join(".cco-out/inspect");
    std::fs::create_dir_all(&inspect_dir).unwrap();
    std::fs::write(
        inspect_dir.join("VERDICT.md"),
        "# VERDICT · check-handwalk-logs\n\nResult: **FAIL**\n",
    )
    .unwrap();
    std::fs::write(
        inspect_dir.join("ISSUES.md"),
        r#"### issue_id=B1
- severity: blocking
- plan_ref: UI-4 / 成功标准 #8
- path: docs/one/logs/**
- symptom: 真书 30 秒主路径手点观察未写入可验收 logs；无录像
- fix_wp: optional-gui-handwalk-record
"#,
    )
    .unwrap();
    std::fs::write(
        inspect_dir.join("GATE.json"),
        r#"{"schema":"cco-inspect-gate/v1","result":"fail","blocking":1,"map":0,"residual":3}"#,
    )
    .unwrap();

    let task = TaskIR {
        id: "check-handwalk-logs".into(),
        title: "handwalk".into(),
        depends_on: vec![],
        group: None,
        provider: "fake".into(),
        mode: "print".into(),
        prompt: "p".into(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: None,
        provider_opts: serde_json::json!({}),
        optional: false,
        include: true,
        role: Some(TaskRole::Inspect),
        scope: None,
        outputs: vec![
            ".cco-out/inspect/VERDICT.md".into(),
            ".cco-out/inspect/ISSUES.md".into(),
            ".cco-out/inspect/GATE.json".into(),
        ],
        tags: vec![],
    };

    assert_eq!(
        read_inspect_verdict(&task, root, root),
        InspectVerdict::Pass,
        "residual-only handwalk must host-Pass"
    );
    let (blocked, n) = inspect_pass_blocked_by_issues(&task, root, root);
    assert!(
        !blocked && n == 0,
        "must not block, got blocked={blocked} n={n}"
    );
    let issues = load_parsed_inspect_issues(&task, root, root);
    let reason = inspect_gate_fail_reason(
        read_inspect_verdict(&task, root, root),
        n,
        issues.len(),
        true,
        "check-handwalk-logs",
    );
    assert!(reason.is_none(), "must keep Done, got {reason:?}");
}
