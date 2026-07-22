
    // A1: pure helpers now live in domain::plan (was private in plan/mod).
    use crate::domain::plan::{
        looks_like_work_task_id, materialize_inspect_task, scope_glob_prefix, scope_paths_overlap,
    };

    use super::*;
    use crate::config::Config;

    #[test]
    fn rejects_cycle() {
        let plan = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "c".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 1,
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
                    depends_on: vec!["b".into()],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "p".into(),
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
                TaskIR {
                    id: "b".into(),
                    title: "b".into(),
                    depends_on: vec!["a".into()],
                    group: None,
                    provider: "fake".into(),
                    mode: "print".into(),
                    prompt: "p".into(),
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
        };
        assert!(plan.validate().is_err());
    }

    #[test]
    fn raw_single_ok() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let plan = dir.path().join("p.md");
        std::fs::write(&plan, "hello worker\nCCO_DONE ok\n").unwrap();
        let ir = load_plan(dir.path(), &plan, Some("raw-single"), &cfg).unwrap();
        assert_eq!(ir.tasks.len(), 1);
        assert_eq!(ir.tasks[0].id, "t1");
    }

    #[test]
    fn md_doc_with_schema_string_in_body_is_not_cco_v1() {
        // Design docs may mention "schema: cco-plan/v1" as an example; must not force YAML parse.
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let plan = dir.path().join("design-plan.md");
        std::fs::write(
            &plan,
            "# Plan for AI\n\nDo the work.\n\n```yaml\nschema: cco-plan/v1\nname: example\n```\n",
        )
        .unwrap();
        let ir = load_plan(dir.path(), &plan, None, &cfg).unwrap();
        assert_eq!(ir.adapter, "raw-single");
        assert_eq!(ir.tasks.len(), 1);
    }

    #[test]
    fn md_with_task_sections_is_serial_prompts() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let plan = dir.path().join("wave.md");
        std::fs::write(
            &plan,
            "## Graph\n\n| id | title |\n|----|-------|\n| t1 | a |\n\n## Tasks\n\n### t1 · a\n\n```\ndo a\n```\n",
        )
        .unwrap();
        let ir = load_plan(dir.path(), &plan, None, &cfg).unwrap();
        assert_eq!(ir.adapter, "serial-prompts/v0");
        assert_eq!(ir.tasks[0].id, "t1");
    }

    #[test]
    fn structured_adapter_routing() {
        assert!(is_structured_adapter("cco-plan/v1"));
        assert!(is_structured_adapter("serial-prompts/v0"));
        assert!(!is_structured_adapter("raw-single"));
        assert!(!is_structured_adapter("unknown"));
    }

    fn sample_task(id: &str, prompt: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: prompt.into(),
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
        }
    }

    #[test]
    fn materialize_drops_unselected_optional() {
        let a = sample_task("a", "p");
        let mut b = sample_task("b", "p");
        b.optional = true;
        b.include = false;
        b.title = normalize_optional_title("润色", true);
        b.depends_on = vec!["a".into()];
        let mut c = sample_task("c", "p");
        c.depends_on = vec!["b".into(), "a".into()];
        let plan = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "opt".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![a, b, c],
        };
        let ir = materialize_selected_tasks(plan).unwrap();
        assert_eq!(ir.tasks.len(), 2);
        assert!(ir.tasks.iter().all(|t| t.id != "b"));
        let c = ir.tasks.iter().find(|t| t.id == "c").unwrap();
        assert_eq!(c.depends_on, vec!["a".to_string()]);
    }

    #[test]
    fn normalize_optional_title_adds_marker() {
        assert_eq!(normalize_optional_title("文档", true), "文档（可选）");
        assert_eq!(normalize_optional_title("文档（可选）", true), "文档（可选）");
        assert_eq!(normalize_optional_title("文档", false), "文档");
        assert!(title_looks_optional("缓存层（可选）"));
        assert!(title_looks_optional("optional polish"));
        assert!(!title_looks_optional("实现核心"));
    }

    #[test]
    fn rejects_too_many_tasks() {
        let tasks: Vec<_> = (0..MAX_TASKS + 1)
            .map(|i| sample_task(&format!("t{i}"), "p"))
            .collect();
        let plan = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "big".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 1,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        };
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("max"), "{err}");
    }

    #[test]
    fn rejects_prompt_too_long() {
        let long: String = "x".repeat(MAX_PROMPT_CHARS + 1);
        let plan = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "long".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 1,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![sample_task("t1", &long)],
        };
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("too long"), "{err}");
    }

    #[test]
    fn rejects_timeout_too_large() {
        let mut t = sample_task("t1", "p");
        t.timeout_secs = Some(MAX_TIMEOUT_SECS + 1);
        let plan = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "to".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 1,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![t],
        };
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("timeout"), "{err}");
    }

    #[test]
    fn title_is_meta_heading_catches_board_and_phases() {
        assert!(title_is_meta_heading(
            "id | provider | role | status | scope | outputs | cost | notes |"
        ));
        assert!(title_is_meta_heading("Board"));
        assert!(title_is_meta_heading("Fragments"));
        assert!(title_is_meta_heading("Timeline"));
        assert!(title_is_meta_heading("12. 修订历史"));
        assert!(title_is_meta_heading("P0 — 协议与示例（文档 / 示例为主）"));
        assert!(title_is_meta_heading("协议与示例（文档 / 示例为主）"));
        assert!(title_is_meta_heading("host 硬保障（代码）"));
        assert!(title_is_meta_heading("8. 非目标"));
        assert!(!title_is_meta_heading("准备"));
        assert!(!title_is_meta_heading("实现 handoff 归并"));
        assert!(!title_is_meta_heading("P0 实现示例计划落地"));
        // Landing task ids must NOT be meta (was broken by needle "p0-")
        assert!(!title_is_meta_heading("P0-1 · 结果台消费 live 费用与用时 ☐"));
        assert!(!title_is_meta_heading("P1-2 · confirm / tag / failover 写入 provenance ☐"));
        assert!(!title_is_meta_heading("A1 · 待确认强制进拆分台"));
        assert!(looks_like_work_task_id("P0-1 · 结果台消费 live 费用与用时 ☐"));
        assert!(looks_like_work_task_id("U1-1 · 测"));
    }

    #[test]
    fn peek_adapter_matches_load() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let prose = dir.path().join("prose.md");
        std::fs::write(&prose, "# Need help\n\nWrite a hello world.\n").unwrap();
        assert_eq!(peek_adapter(dir.path(), &prose).unwrap(), "raw-single");
        assert!(!is_structured_adapter(&peek_adapter(dir.path(), &prose).unwrap()));

        let yaml = dir.path().join("hello.cco.yaml");
        std::fs::write(
            &yaml,
            "schema: cco-plan/v1\nname: t\nmax_parallel: 1\ntasks:\n  - id: t1\n    title: a\n    prompt: p\n",
        )
        .unwrap();
        assert_eq!(peek_adapter(dir.path(), &yaml).unwrap(), "cco-plan/v1");
        assert!(is_structured_adapter(&peek_adapter(dir.path(), &yaml).unwrap()));
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        assert_eq!(ir.adapter, "cco-plan/v1");
    }

    /// P1-1: old cco-plan/v1 without role/scope/outputs/require_inspect still loads.
    #[test]
    fn cco_v1_legacy_plan_defaults_collab_fields() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("legacy.cco.yaml");
        std::fs::write(
            &yaml,
            r#"
schema: cco-plan/v1
name: legacy
defaults:
  provider: fake
  mode: print
tasks:
  - id: t1
    title: old style
    prompt: |
      do work
      CCO_DONE ok
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        assert!(!ir.require_inspect);
        assert_eq!(ir.tasks.len(), 1);
        let t = &ir.tasks[0];
        assert!(t.role.is_none());
        assert!(t.scope.is_none());
        assert!(t.outputs.is_empty());
        assert_eq!(t.provider, "fake");
    }

    /// P1-1: full collaboration contract fields parse into TaskIR/PlanIR.
    #[test]
    fn cco_v1_parses_role_scope_outputs_require_inspect() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("collab.cco.yaml");
        std::fs::write(
            &yaml,
            r#"
schema: cco-plan/v1
name: collab
require_inspect: true
defaults:
  provider: claude
  mode: print
  worktree: true
max_parallel: 2
tasks:
  - id: feat-a
    title: implement A
    provider: claude
    role: implement
    scope:
      paths:
        - src/module_a/**
        - .cco-out/feat-a/**
      readonly:
        - docs/**
      forbid:
        - src/module_b/**
    outputs:
      - .cco-out/feat-a/SUMMARY.md
      - .cco-out/feat-a/CHANGED.md
    prompt: |
      implement A
      CCO_DONE ok
  - id: inspect
    title: code inspect
    provider: claude
    role: inspect
    depends_on: [feat-a]
    scope:
      paths:
        - .cco-out/inspect/**
      readonly:
        - src/**
        - .cco-out/**
    outputs:
      - .cco-out/inspect/VERDICT.md
    prompt: |
      inspect only
      CCO_DONE ok
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        assert!(ir.require_inspect);
        assert_eq!(ir.tasks.len(), 2);

        let a = ir.task("feat-a").unwrap();
        assert_eq!(a.role, Some(TaskRole::Implement));
        let scope = a.scope.as_ref().expect("scope");
        assert_eq!(
            scope.paths,
            vec![
                "src/module_a/**".to_string(),
                ".cco-out/feat-a/**".to_string()
            ]
        );
        assert_eq!(scope.readonly, vec!["docs/**".to_string()]);
        assert_eq!(scope.forbid, vec!["src/module_b/**".to_string()]);
        assert_eq!(
            a.outputs,
            vec![
                ".cco-out/feat-a/SUMMARY.md".to_string(),
                ".cco-out/feat-a/CHANGED.md".to_string()
            ]
        );

        let insp = ir.task("inspect").unwrap();
        assert_eq!(insp.role, Some(TaskRole::Inspect));
        assert_eq!(
            insp.outputs,
            vec![".cco-out/inspect/VERDICT.md".to_string()]
        );
        assert_eq!(insp.depends_on, vec!["feat-a".to_string()]);
    }

    /// P1-1: all four TaskRole variants deserialize from YAML.
    #[test]
    fn cco_v1_parses_all_task_roles() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("roles.cco.yaml");
        std::fs::write(
            &yaml,
            r#"
schema: cco-plan/v1
name: roles
tasks:
  - id: s
    role: scout
    prompt: p
  - id: i
    role: implement
    depends_on: [s]
    scope:
      paths: [src/**]
    prompt: p
  - id: g
    role: integrate
    depends_on: [i]
    prompt: p
  - id: x
    role: inspect
    depends_on: [g]
    prompt: p
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        assert_eq!(ir.tasks[0].role, Some(TaskRole::Scout));
        assert_eq!(ir.tasks[1].role, Some(TaskRole::Implement));
        assert_eq!(ir.tasks[2].role, Some(TaskRole::Integrate));
        assert_eq!(ir.tasks[3].role, Some(TaskRole::Inspect));
    }

    /// P1-1: PlanIR/TaskIR serde round-trip keeps defaults for missing collab fields.
    #[test]
    fn collab_fields_serde_default_on_missing() {
        let json = r#"{
            "schema":"cco-plan/v1",
            "name":"j",
            "adapter":"cco-plan/v1",
            "source_path":"x",
            "max_parallel":1,
            "on_failure":"pause",
            "retry_max":0,
            "default_provider":"fake",
            "default_mode":"print",
            "worktree":false,
            "tasks":[{
                "id":"t1",
                "title":"t",
                "depends_on":[],
                "group":null,
                "provider":"fake",
                "mode":"print",
                "prompt":"p",
                "acceptance":null,
                "timeout_secs":null,
                "worktree":null,
                "provider_opts":{},
                "optional":false,
                "include":true
            }]
        }"#;
        let ir: PlanIR = serde_json::from_str(json).expect("legacy json PlanIR");
        assert!(!ir.require_inspect);
        assert!(ir.tasks[0].role.is_none());
        assert!(ir.tasks[0].scope.is_none());
        assert!(ir.tasks[0].outputs.is_empty());
    }

    // ── P1-2 collab validate helpers ─────────────────────────────────────

    fn base_plan(tasks: Vec<TaskIR>, worktree: bool, require_inspect: bool) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "p1-2".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x"),
            max_parallel: 4,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree,
            require_inspect,
            tasks,
        }
    }

    fn task(
        id: &str,
        provider: &str,
        mode: &str,
        role: Option<TaskRole>,
        deps: &[&str],
        paths: Option<&[&str]>,
    ) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
            group: None,
            provider: provider.into(),
            mode: mode.into(),
            prompt: "p".into(),
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: paths.map(|ps| TaskScope {
                paths: ps.iter().map(|s| (*s).to_string()).collect(),
                readonly: vec![],
                forbid: vec![],
            }),
            outputs: vec![],
        tags: vec![],
        }
    }

    /// P1-2 positive: single-provider legacy plan (no role) still validates.
    #[test]
    fn p1_2_legacy_single_provider_ok() {
        let plan = base_plan(
            vec![
                task("a", "claude", "print", None, &[], None),
                task("b", "claude", "print", None, &["a"], None),
            ],
            false,
            false,
        );
        plan.validate().expect("legacy single-provider");
    }

    /// P1-2 positive: multi-provider parallel + worktree + disjoint scopes + terminal inspect.
    #[test]
    fn p1_2_legal_mixed_plan_ok() {
        let plan = base_plan(
            vec![
                task(
                    "a",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/a/**", ".cco-out/a/**"]),
                ),
                task(
                    "b",
                    "codex",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/b/**", ".cco-out/b/**"]),
                ),
                task(
                    "g",
                    "claude",
                    "print",
                    Some(TaskRole::Integrate),
                    &["a", "b"],
                    Some(&["src/a/**", "src/b/**", ".cco-out/g/**"]),
                ),
                task(
                    "x",
                    "claude",
                    "print",
                    Some(TaskRole::Inspect),
                    &["g"],
                    Some(&[".cco-out/inspect/**"]),
                ),
            ],
            true,
            true,
        );
        plan.validate().expect("legal mixed plan");
    }

    /// P1-2 negative: multi-provider + parallel wave without worktree.
    #[test]
    fn p1_2_rejects_multi_provider_parallel_without_worktree() {
        let plan = base_plan(
            vec![
                task("a", "claude", "print", None, &[], None),
                task("b", "codex", "print", None, &[], None),
            ],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("worktree"), "{err}");
        assert!(err.contains("multi-provider") || err.contains("parallel"), "{err}");
    }

    /// P1-2 positive: multi-provider but fully serial → worktree not forced.
    #[test]
    fn p1_2_multi_provider_serial_without_worktree_ok() {
        let plan = base_plan(
            vec![
                task("a", "claude", "print", None, &[], None),
                task("b", "codex", "print", None, &["a"], None),
            ],
            false,
            false,
        );
        plan.validate()
            .expect("serial multi-provider may omit worktree");
    }

    /// P1-2: task.worktree:false on one task fails even if plan.worktree=true?
    /// effective = task.worktree.unwrap_or(plan.worktree) — plan true covers all.
    /// Negative: one task explicitly turns worktree off.
    #[test]
    fn p1_2_rejects_task_worktree_off_in_multi_provider_parallel() {
        let mut a = task("a", "claude", "print", None, &[], None);
        a.worktree = Some(false);
        let b = task("b", "codex", "print", None, &[], None);
        let plan = base_plan(vec![a, b], true, false);
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("worktree"), "{err}");
        assert!(err.contains("a"), "{err}");
    }

    /// P1-2 negative: parallel implement with overlapping scope.paths.
    #[test]
    fn p1_2_rejects_parallel_implement_scope_overlap() {
        let plan = base_plan(
            vec![
                task(
                    "a",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/shared/**"]),
                ),
                task(
                    "b",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/shared/foo.rs"]),
                ),
            ],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("overlapping") || err.contains("scope"), "{err}");
    }

    /// P1-2 positive: serial implement chain may share scope paths.
    #[test]
    fn p1_2_serial_implement_shared_scope_ok() {
        let plan = base_plan(
            vec![
                task(
                    "a",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/**"]),
                ),
                task(
                    "b",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &["a"],
                    Some(&["src/**"]),
                ),
            ],
            false,
            false,
        );
        plan.validate().expect("serial implement may share paths");
    }

    /// P1-2 negative: role=implement without scope.paths.
    #[test]
    fn p1_2_rejects_implement_missing_scope_paths() {
        let plan = base_plan(
            vec![task(
                "a",
                "claude",
                "print",
                Some(TaskRole::Implement),
                &[],
                None,
            )],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("scope.paths"), "{err}");
        assert!(err.contains("implement"), "{err}");
    }

    /// P1-2 negative: empty scope.paths on implement.
    #[test]
    fn p1_2_rejects_implement_empty_scope_paths() {
        let plan = base_plan(
            vec![task(
                "a",
                "claude",
                "print",
                Some(TaskRole::Implement),
                &[],
                Some(&[]),
            )],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("scope.paths"), "{err}");
    }

    /// P1-2 negative: business task depends on inspect (non-terminal).
    #[test]
    fn p1_2_rejects_inspect_with_business_downstream() {
        let plan = base_plan(
            vec![
                task(
                    "x",
                    "claude",
                    "print",
                    Some(TaskRole::Inspect),
                    &[],
                    Some(&[".cco-out/inspect/**"]),
                ),
                task(
                    "a",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &["x"],
                    Some(&["src/**"]),
                ),
            ],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("inspect"), "{err}");
        assert!(err.contains("terminal") || err.contains("downstream") || err.contains("depends"), "{err}");
    }

    /// P1-2 negative: unscoped task after inspect.
    #[test]
    fn p1_2_rejects_inspect_with_unscoped_downstream() {
        let plan = base_plan(
            vec![
                task(
                    "x",
                    "claude",
                    "print",
                    Some(TaskRole::Inspect),
                    &[],
                    None,
                ),
                task("after", "claude", "print", None, &["x"], None),
            ],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("inspect"), "{err}");
    }

    /// P1-2 positive: inspect → inspect chain is allowed (final sink still terminal).
    #[test]
    fn p1_2_inspect_chain_ok() {
        let plan = base_plan(
            vec![
                task(
                    "x1",
                    "claude",
                    "print",
                    Some(TaskRole::Inspect),
                    &[],
                    None,
                ),
                task(
                    "x2",
                    "claude",
                    "print",
                    Some(TaskRole::Inspect),
                    &["x1"],
                    None,
                ),
            ],
            false,
            true,
        );
        plan.validate().expect("inspect chain ok");
    }

    /// P1-2 negative: require_inspect without any inspect task.
    #[test]
    fn p1_2_rejects_require_inspect_without_inspect_task() {
        let plan = base_plan(
            vec![task("a", "claude", "print", None, &[], None)],
            false,
            true,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("require_inspect"), "{err}");
        assert!(err.contains("inspect"), "{err}");
    }

    /// P1-2 negative: codex + mode=bg.
    #[test]
    fn p1_2_rejects_codex_bg() {
        let plan = base_plan(
            vec![task("c", "codex", "bg", None, &[], None)],
            false,
            false,
        );
        let err = plan.validate().unwrap_err().to_string();
        assert!(err.contains("codex"), "{err}");
        assert!(err.contains("bg"), "{err}");
    }

    /// P1-2 positive: parallel implement with disjoint paths + single provider.
    #[test]
    fn p1_2_parallel_implement_disjoint_ok() {
        let plan = base_plan(
            vec![
                task(
                    "a",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["examples/a/**"]),
                ),
                task(
                    "b",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["examples/b/**"]),
                ),
            ],
            false,
            false,
        );
        plan.validate().expect("disjoint parallel implement");
    }

    #[test]
    fn scope_glob_overlap_helpers() {
        assert!(scope_paths_overlap("src/**", "src/foo.rs"));
        assert!(scope_paths_overlap("src/a/**", "src/a/b/**"));
        assert!(!scope_paths_overlap("src/a/**", "src/b/**"));
        assert!(scope_paths_overlap("**", "src/x"));
        assert_eq!(scope_glob_prefix("src/module/**"), Some("src/module".into()));
        assert_eq!(scope_glob_prefix("**"), None);
    }

    /// P2-1: load_plan materializes inspect defaults (tools strip Edit, scope write path, system prompt).
    #[test]
    fn p2_1_inspect_defaults_on_load() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("insp.cco.yaml");
        std::fs::write(
            &yaml,
            r#"
schema: cco-plan/v1
name: insp
require_inspect: true
defaults:
  provider: claude
  mode: print
  allowed_tools: [Read, Edit, Bash, Glob, Grep, Write]
tasks:
  - id: feat
    role: implement
    scope:
      paths: [src/**]
    prompt: implement
  - id: inspect
    role: inspect
    depends_on: [feat]
    prompt: inspect only
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        let insp = ir.task("inspect").unwrap();

        let tools = insp.provider_opts["allowed_tools"]
            .as_array()
            .expect("allowed_tools array");
        let tool_names: Vec<&str> = tools.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            !tool_names.iter().any(|t| t.eq_ignore_ascii_case("Edit")),
            "Edit must be stripped for inspect: {tool_names:?}"
        );
        assert!(
            !tool_names
                .iter()
                .any(|t| t.eq_ignore_ascii_case("MultiEdit")),
            "MultiEdit must be stripped: {tool_names:?}"
        );
        assert!(
            tool_names.iter().any(|t| t.eq_ignore_ascii_case("Write")),
            "Write required for VERDICT: {tool_names:?}"
        );
        assert!(
            tool_names.iter().any(|t| t.eq_ignore_ascii_case("Read")),
            "Read required: {tool_names:?}"
        );

        let scope = insp.scope.as_ref().expect("inspect scope materialized");
        assert_eq!(
            scope.paths,
            vec![INSPECT_DEFAULT_WRITE_SCOPE.to_string()],
            "default write scope"
        );

        let sys = insp.provider_opts["append_system_prompt"]
            .as_str()
            .unwrap_or("");
        assert!(
            sys.contains(INSPECT_SYSTEM_PROMPT_MARKER),
            "inspect system prompt missing: {sys}"
        );
        assert!(
            sys.contains("READ-ONLY"),
            "inspect prompt must stress business read-only: {sys}"
        );

        // implement task must keep full tools (not inspect defaults)
        let feat = ir.task("feat").unwrap();
        let feat_tools = feat.provider_opts["allowed_tools"]
            .as_array()
            .expect("feat tools");
        assert!(
            feat_tools
                .iter()
                .any(|v| v.as_str() == Some("Edit")),
            "implement keeps Edit"
        );
        assert!(
            feat
                .provider_opts
                .get("append_system_prompt")
                .and_then(|v| v.as_str())
                .map(|s| !s.contains(INSPECT_SYSTEM_PROMPT_MARKER))
                .unwrap_or(true),
            "implement must not get inspect system prompt"
        );
    }

    /// P2-1: explicit inspect allowed_tools without Edit are preserved; Write ensured.
    #[test]
    fn p2_1_inspect_preserves_explicit_readonly_tools() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("insp2.cco.yaml");
        std::fs::write(
            &yaml,
            r#"
schema: cco-plan/v1
name: insp2
tasks:
  - id: inspect
    role: inspect
    provider_opts:
      allowed_tools: [Read, Glob, Grep, Bash]
    prompt: inspect
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &yaml, None, &cfg).unwrap();
        let insp = ir.task("inspect").unwrap();
        let tools: Vec<String> = insp.provider_opts["allowed_tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(tools.iter().any(|t| t == "Glob"));
        assert!(tools.iter().any(|t| t == "Grep"));
        assert!(tools.iter().any(|t| t == "Bash"));
        assert!(tools.iter().any(|t| t == "Write"), "Write auto-added: {tools:?}");
        assert!(!tools.iter().any(|t| t == "Edit"));
    }

    /// P2-1: empty allowed_tools after strip → full INSPECT_DEFAULT_ALLOWED_TOOLS.
    #[test]
    fn p2_1_inspect_empty_after_strip_uses_defaults() {
        let mut t = task(
            "x",
            "claude",
            "print",
            Some(TaskRole::Inspect),
            &[],
            None,
        );
        t.provider_opts = serde_json::json!({
            "allowed_tools": ["Edit", "MultiEdit"]
        });
        materialize_inspect_task(&mut t);
        let tools: Vec<String> = t.provider_opts["allowed_tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert_eq!(
            tools,
            INSPECT_DEFAULT_ALLOWED_TOOLS
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>()
        );
    }

    /// P2-1: allow_business_write=true skips tool strip (escape hatch; still injects prompt).
    #[test]
    fn p2_1_allow_business_write_keeps_edit() {
        let mut t = task(
            "x",
            "claude",
            "print",
            Some(TaskRole::Inspect),
            &[],
            None,
        );
        t.provider_opts = serde_json::json!({
            "allowed_tools": ["Read", "Edit", "Write"],
            "allow_business_write": true
        });
        materialize_inspect_task(&mut t);
        let tools: Vec<&str> = t.provider_opts["allowed_tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(tools.contains(&"Edit"), "{tools:?}");
        let sys = t.provider_opts["append_system_prompt"].as_str().unwrap();
        assert!(sys.contains(INSPECT_SYSTEM_PROMPT_MARKER));
        assert!(sys.contains("allow_business_write"));
    }

    /// P2-1: materialize is idempotent; explicit scope.paths preserved.
    #[test]
    fn p2_1_inspect_idempotent_and_keeps_explicit_scope() {
        let mut plan = base_plan(
            vec![task(
                "inspect",
                "claude",
                "print",
                Some(TaskRole::Inspect),
                &[],
                Some(&[".cco-out/custom-inspect/**"]),
            )],
            false,
            false,
        );
        plan.tasks[0].provider_opts = serde_json::json!({
            "allowed_tools": ["Read", "Edit", "Write", "Bash"]
        });
        materialize_role_defaults(&mut plan);
        materialize_role_defaults(&mut plan);
        let t = &plan.tasks[0];
        assert_eq!(
            t.scope.as_ref().unwrap().paths,
            vec![".cco-out/custom-inspect/**".to_string()]
        );
        let tools: Vec<&str> = t.provider_opts["allowed_tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(!tools.contains(&"Edit"));
        assert!(tools.contains(&"Write") && tools.contains(&"Bash"));
        let sys = t.provider_opts["append_system_prompt"].as_str().unwrap();
        assert_eq!(
            sys.matches(INSPECT_SYSTEM_PROMPT_MARKER).count(),
            1,
            "system prompt not duplicated"
        );
    }

    /// P2-1: missing role / non-inspect roles are not rewritten.
    #[test]
    fn p2_1_non_inspect_untouched() {
        let mut plan = base_plan(
            vec![
                task("a", "claude", "print", None, &[], None),
                task(
                    "b",
                    "claude",
                    "print",
                    Some(TaskRole::Implement),
                    &[],
                    Some(&["src/**"]),
                ),
            ],
            false,
            false,
        );
        plan.tasks[0].provider_opts =
            serde_json::json!({"allowed_tools": ["Read", "Edit", "Write"]});
        plan.tasks[1].provider_opts =
            serde_json::json!({"allowed_tools": ["Read", "Edit", "Write"]});
        materialize_role_defaults(&mut plan);
        for t in &plan.tasks {
            let tools = t.provider_opts["allowed_tools"].as_array().unwrap();
            assert!(tools.iter().any(|v| v.as_str() == Some("Edit")));
            assert!(t.provider_opts.get("append_system_prompt").is_none());
        }
    }

    #[test]
    fn apply_tag_routing_codex_on_default_only() {
        let mut plan = base_plan(
            vec![
                {
                    let mut t = task("a", "claude", "print", None, &[], None);
                    t.tags = vec!["codex".into()];
                    t
                },
                {
                    // Already explicit codex — keep
                    let mut t = task("b", "codex", "print", None, &[], None);
                    t.tags = vec!["claude".into()];
                    t
                },
            ],
            false,
            false,
        );
        plan.default_provider = "claude".into();
        apply_tag_routing(&mut plan);
        assert_eq!(plan.tasks[0].provider, "codex");
        assert_eq!(plan.tasks[1].provider, "codex"); // not rewritten by claude tag
    }

    #[test]
    fn cco_v1_parses_tags_field() {
        let cfg = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let plan = dir.path().join("t.cco.yaml");
        std::fs::write(
            &plan,
            r#"
schema: cco-plan/v1
name: tagged
tasks:
  - id: t1
    title: 用 Codex 做后端
    provider: claude
    tags: [codex, backend]
    prompt: |
      do it
      CCO_DONE ok
"#,
        )
        .unwrap();
        let ir = load_plan(dir.path(), &plan, Some("cco-plan/v1"), &cfg).unwrap();
        assert_eq!(ir.tasks[0].tags, vec!["codex".to_string(), "backend".to_string()]);
        // default_provider is claude; task provider was claude (= default) → tag routes to codex
        assert_eq!(ir.tasks[0].provider, "codex");
    }
