use super::*;
use crate::domain::plan::types::{
    OnFailure, PlanIR, TaskIR, TaskRole, TaskScope, IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER,
    INSPECT_SYSTEM_PROMPT_MARKER,
};

    fn task(id: &str, role: Option<TaskRole>, deps: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
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
            role,
            scope: Some(TaskScope {
                paths: vec![format!(".cco-out/{id}/**")],
                readonly: vec![],
                forbid: vec![],
            }),
            outputs: vec![],
            tags: vec![],
            wait_for: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "test".into(),
            source_path: std::path::PathBuf::from("test.md"),
            max_parallel: 4,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn empty_inspect_depends_wires_to_business_leaves() {
        // t1 → t2, t3; leaves = t2,t3; inspect had []
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Scout), &[]),
            task("t2", Some(TaskRole::Implement), &["t1"]),
            task("t3", Some(TaskRole::Implement), &["t1"]),
            task("t7-inspect", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        assert!(
            insp.depends_on.iter().any(|d| d == "t2"),
            "deps={:?}",
            insp.depends_on
        );
        assert!(insp.depends_on.iter().any(|d| d == "t3"));
        assert!(!insp.depends_on.iter().any(|d| d == "t1"), "only leaves");
    }

    #[test]
    fn explicit_inspect_depends_preserved() {
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Implement), &[]),
            task("t2", Some(TaskRole::Implement), &[]),
            task("t7-inspect", Some(TaskRole::Inspect), &["t1"]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        assert_eq!(insp.depends_on, vec!["t1".to_string()]);
    }

    #[test]
    fn docs_cleanup_shape_inspect_not_parallel_to_t1() {
        // Real failure shape: t7-inspect [] raced t1-inventory at run start.
        let mut ir = plan(vec![
            task("t1-inventory", Some(TaskRole::Scout), &[]),
            task(
                "t2-delete-one",
                Some(TaskRole::Implement),
                &["t1-inventory"],
            ),
            task("t3-archive-b", Some(TaskRole::Implement), &["t1-inventory"]),
            task(
                "t4-c1-split-merge",
                Some(TaskRole::Implement),
                &["t3-archive-b"],
            ),
            task(
                "t5-c2c3c4-light",
                Some(TaskRole::Implement),
                &["t3-archive-b"],
            ),
            task(
                "t6-index-refresh",
                Some(TaskRole::Integrate),
                &["t3-archive-b"],
            ),
            task("t7-inspect", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);
        let insp = ir.tasks.iter().find(|t| t.id == "t7-inspect").unwrap();
        for leaf in [
            "t2-delete-one",
            "t4-c1-split-merge",
            "t5-c2c3c4-light",
            "t6-index-refresh",
        ] {
            assert!(
                insp.depends_on.iter().any(|d| d == leaf),
                "missing leaf {leaf}; deps={:?}",
                insp.depends_on
            );
        }
        assert!(!insp.depends_on.iter().any(|d| d == "t1-inventory"));
        assert!(!insp.depends_on.iter().any(|d| d == "t3-archive-b"));
    }

    fn sys_of(task: &TaskIR) -> String {
        task.provider_opts
            .get("append_system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn implement_and_role_unset_get_usability_floor() {
        let mut ir = plan(vec![
            task("t-impl", Some(TaskRole::Implement), &[]),
            task("t-do", None, &[]),
            task("t-int", Some(TaskRole::Integrate), &[]),
            task("t-scout", Some(TaskRole::Scout), &[]),
            task("t-insp", Some(TaskRole::Inspect), &[]),
        ]);
        materialize_role_defaults(&mut ir);

        for id in ["t-impl", "t-do", "t-int"] {
            let t = ir.tasks.iter().find(|x| x.id == id).unwrap();
            let sys = sys_of(t);
            assert!(
                sys.contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
                "{id} missing usability floor: {sys}"
            );
            assert!(
                !sys.contains(INSPECT_SYSTEM_PROMPT_MARKER),
                "{id} must not get inspect prompt"
            );
        }

        let scout = ir.tasks.iter().find(|x| x.id == "t-scout").unwrap();
        assert!(
            !sys_of(scout).contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
            "scout must not get implement usability"
        );

        let insp = ir.tasks.iter().find(|x| x.id == "t-insp").unwrap();
        let insp_sys = sys_of(insp);
        assert!(insp_sys.contains(INSPECT_SYSTEM_PROMPT_MARKER));
        assert!(
            insp_sys.contains("Usability floor"),
            "inspect prompt should carry usability severity floor"
        );
        assert!(
            !insp_sys.contains(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER),
            "inspect must not get implement-usability marker"
        );
    }

    #[test]
    fn implement_usability_inject_is_idempotent() {
        let mut ir = plan(vec![task("t1", Some(TaskRole::Implement), &[])]);
        materialize_role_defaults(&mut ir);
        materialize_role_defaults(&mut ir);
        let sys = sys_of(ir.tasks.iter().find(|t| t.id == "t1").unwrap());
        assert_eq!(
            sys.matches(IMPLEMENT_USABILITY_SYSTEM_PROMPT_MARKER)
                .count(),
            1,
            "usability marker duplicated: {sys}"
        );
    }

    #[test]
    fn browser_tag_injects_browser_prompt_idempotent() {
        use super::super::types::BROWSER_SYSTEM_PROMPT_MARKER;
        let mut t = task("ui", Some(TaskRole::Implement), &[]);
        t.tags = vec!["browser".into(), "ui-verify".into()];
        let mut ir = plan(vec![t]);
        materialize_role_defaults(&mut ir);
        materialize_role_defaults(&mut ir);
        let sys = sys_of(ir.tasks.iter().find(|x| x.id == "ui").unwrap());
        assert!(
            sys.contains(BROWSER_SYSTEM_PROMPT_MARKER),
            "missing browser prompt: {sys}"
        );
        assert_eq!(
            sys.matches(BROWSER_SYSTEM_PROMPT_MARKER).count(),
            1,
            "browser marker duplicated: {sys}"
        );
    }

    #[test]
    fn ui_verify_gets_default_shot_and_report_outputs() {
        let mut t = task("ui-shot", Some(TaskRole::Implement), &[]);
        t.tags = vec!["browser".into(), "ui-verify".into()];
        t.outputs.clear();
        let mut ir = plan(vec![t]);
        materialize_role_defaults(&mut ir);
        let t = ir.tasks.iter().find(|x| x.id == "ui-shot").unwrap();
        assert!(
            t.outputs
                .iter()
                .any(|o| o == ".cco-out/browser/ui-shot/shot.png"),
            "outputs={:?}",
            t.outputs
        );
        assert!(
            t.outputs
                .iter()
                .any(|o| o == ".cco-out/browser/ui-shot/report.md"),
            "outputs={:?}",
            t.outputs
        );
        let scope = t.scope.as_ref().unwrap();
        assert!(
            scope
                .paths
                .iter()
                .any(|p| p.contains(".cco-out/browser/ui-shot")),
            "scope={:?}",
            scope.paths
        );
    }

    #[test]
    fn ui_smoke_and_scrape_default_outputs() {
        let mut smoke = task("sm", Some(TaskRole::Implement), &[]);
        smoke.tags = vec!["browser".into(), "ui-smoke".into()];
        smoke.outputs.clear();
        let mut scrape = task("sc", Some(TaskRole::Implement), &[]);
        scrape.tags = vec!["browser".into(), "scrape".into()];
        scrape.outputs.clear();
        scrape.scope = Some(TaskScope {
            paths: vec!["content/**".into()],
            readonly: vec![],
            forbid: vec![],
        });
        let mut ir = plan(vec![smoke, scrape]);
        materialize_role_defaults(&mut ir);
        let sm = ir.tasks.iter().find(|x| x.id == "sm").unwrap();
        assert!(sm
            .outputs
            .iter()
            .any(|o| o == ".cco-out/browser/sm/smoke.md"));
        let sc = ir.tasks.iter().find(|x| x.id == "sc").unwrap();
        assert!(sc.outputs.iter().any(|o| o == ".cco-out/browser/sc/raw.md"));
        // author content/** preserved + evidence glob
        let paths = &sc.scope.as_ref().unwrap().paths;
        assert!(paths.iter().any(|p| p == "content/**"));
        assert!(paths.iter().any(|p| p.contains(".cco-out/browser/sc")));
    }

    #[test]
    fn scrape_without_scope_fails_validate() {
        let mut t = task("sc1", Some(TaskRole::Implement), &[]);
        t.tags = vec!["browser".into(), "scrape".into()];
        t.scope = Some(TaskScope {
            paths: vec![],
            readonly: vec![],
            forbid: vec![],
        });
        let ir = plan(vec![t]);
        let err = ir.validate().unwrap_err().to_string();
        assert!(
            err.contains("scrape") || err.contains("scope"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn scrape_with_scope_validates() {
        let mut t = task("sc1", Some(TaskRole::Implement), &[]);
        t.tags = vec!["browser".into(), "scrape".into()];
        t.scope = Some(TaskScope {
            paths: vec!["content/**".into()],
            readonly: vec![],
            forbid: vec![],
        });
        let ir = plan(vec![t]);
        ir.validate().expect("scrape with scope ok");
    }
