//! cco-native split document (independent of PlanIR wire).
//!
//! [INPUT]: 无 IO — 纯模型
//! [OUTPUT]: CcoSplitJob/Task · soft_accept · waves · from/to PlanIR
//! [POS]: domain/plan — 拆分 SoT 形状；SQLite 适配器在 state/
//! [PROTOCOL]: 变更时更新此头部与 domain/CLAUDE.md · docs/cco-split-format-sqlite-2026-07-21.md
//!
//! Product: split desk + SQLite own this shape; PlanIR is materialized only at confirm.

mod accept;
mod convert;
mod humanize;
mod types;

pub use accept::{
    recompute_waves, run_gate_ok, sanitize_cco_split_deps, soft_accept_split, split_topo_layers,
};
pub use convert::{from_plan_ir, to_plan_ir};
pub use humanize::{
    dep_cell_is_none, display_title, human_summary, is_worker_noise_line, parse_dep_cell,
    parse_done_when, resolve_deps_from_sections, strip_worker_scaffold, work_id_from_title,
};

// Re-export names used by split_agent parse without deep paths in callers.
pub use types::{
    CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask, CcoTaskKind, CcoTaskStatus,
    CCO_SPLIT_SCHEMA,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::{OnFailure, PlanIR, TaskIR, TaskRole, TaskScope};
    use std::path::PathBuf;

    fn sample_task(id: &str, deps: &[&str]) -> CcoSplitTask {
        CcoSplitTask {
            task_id: id.into(),
            ord: 0,
            title: id.into(),
            summary: String::new(),
            body: format!("do {id}"),
            depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
            wave: 0,
            enabled: true,
            optional: false,
            done_when: None,
            plan_ref: None,
            kind: CcoTaskKind::Do,
            status: CcoTaskStatus::Pending,
            provider: None,
            role: None,
            scope_paths: vec![],
            meta_json: None,
        }
    }

    #[test]
    fn soft_accept_fills_and_waves() {
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "x".into(),
            max_parallel: 2,
            source: CcoSplitSource::Heuristic,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![
                sample_task("t1", &[]),
                sample_task("t2", &["t1"]),
                sample_task("t3", &["t1"]),
            ],
        };
        soft_accept_split(&mut doc);
        assert_eq!(doc.tasks[0].wave, 0);
        assert_eq!(doc.tasks[1].wave, 1);
        assert_eq!(doc.tasks[2].wave, 1);
        let layers = split_topo_layers(&doc);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0], vec!["t1".to_string()]);
    }

    #[test]
    fn soft_accept_breaks_cycle() {
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "x".into(),
            max_parallel: 1,
            source: CcoSplitSource::Import,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![sample_task("a", &["b"]), sample_task("b", &["a"])],
        };
        soft_accept_split(&mut doc);
        let mut indeg = 0;
        for t in &doc.tasks {
            indeg += t.depends_on.len();
        }
        assert!(indeg < 2);
        recompute_waves(&mut doc);
        assert!(run_gate_ok(&doc).is_ok());
    }

    #[test]
    fn roundtrip_plan_ir_preserves_core() {
        let ir = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "demo".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("docs/x.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![TaskIR {
                id: "t1".into(),
                title: "First".into(),
                depends_on: vec![],
                group: Some("A".into()),
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "body one\nmore".into(),
                acceptance: Some("file exists".into()),
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: serde_json::json!({}),
                optional: true,
                include: false,
                role: Some(TaskRole::Implement),
                scope: Some(TaskScope {
                    paths: vec!["src/".into()],
                    readonly: vec![],
                    forbid: vec![],
                }),
                outputs: vec![],
                tags: vec!["frontend".into()],
            }],
        };
        let doc = from_plan_ir(
            "plan-1",
            PathBuf::from("/p"),
            PathBuf::from("docs/x.md"),
            &ir,
            CcoSplitSource::Llm,
            CcoSplitStatus::Ready,
            "t0",
            "t0",
        );
        assert_eq!(doc.tasks.len(), 1);
        assert!(!doc.tasks[0].enabled);
        assert_eq!(doc.tasks[0].done_when.as_deref(), Some("file exists"));
        let back = to_plan_ir(&doc, "claude", "print");
        assert_eq!(back.tasks[0].id, "t1");
        assert_eq!(back.tasks[0].prompt, "body one\nmore");
        assert!(!back.tasks[0].include);
        assert!(back.tasks[0].optional);
        assert_eq!(
            back.tasks[0]
                .scope
                .as_ref()
                .map(|s| s.paths.clone())
                .unwrap_or_default(),
            vec!["src/".to_string()]
        );
    }

    #[test]
    fn run_gate_requires_enabled() {
        let mut doc = CcoSplitJob {
            job_id: "j".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("p.md"),
            status: CcoSplitStatus::Ready,
            title: "t".into(),
            max_parallel: 1,
            source: CcoSplitSource::Manual,
            error: None,
            run_id: None,
            created_at: "t".into(),
            updated_at: "t".into(),
            tasks: vec![{
                let mut t = sample_task("t1", &[]);
                t.optional = true;
                t.enabled = false;
                t
            }],
        };
        soft_accept_split(&mut doc);
        assert!(run_gate_ok(&doc).is_err());
        doc.tasks[0].enabled = true;
        assert!(run_gate_ok(&doc).is_ok());
    }

    #[test]
    fn sanitize_cco_split_drops_bare_edges() {
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "x".into(),
            max_parallel: 2,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![
                sample_task("t1", &[]),
                {
                    let mut t = sample_task("t2", &["t1"]);
                    // Body does not mention t1 → bare edge should drop.
                    t.body = "独立做第二步".into();
                    t
                },
                {
                    let mut t = sample_task("t3", &["t1"]);
                    t.body = "依赖原因：等待产物来自 t1\n做第三步".into();
                    t
                },
            ],
        };
        soft_accept_split(&mut doc);
        let removed = sanitize_cco_split_deps(&mut doc);
        assert_eq!(removed, 1);
        assert!(doc.tasks[1].depends_on.is_empty());
        assert_eq!(doc.tasks[2].depends_on, vec!["t1".to_string()]);
        // After drop, t2 can share wave 0 with t1.
        assert_eq!(doc.tasks[0].wave, 0);
        assert_eq!(doc.tasks[1].wave, 0);
    }

    #[test]
    fn soft_accept_serializes_overlapping_scope_to_different_waves() {
        let mut a = sample_task("x", &[]);
        a.scope_paths = vec!["web/index.html".into()];
        a.body = "【做什么】改 index A".into();
        let mut b = sample_task("y", &[]);
        b.scope_paths = vec!["web/index.html".into()];
        b.body = "【做什么】改 index B".into();
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "ov".into(),
            max_parallel: 2,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![a, b],
        };
        soft_accept_split(&mut doc);
        let x = doc.tasks.iter().find(|t| t.task_id == "x").unwrap();
        let y = doc.tasks.iter().find(|t| t.task_id == "y").unwrap();
        assert!(y.depends_on.iter().any(|d| d == "x") || x.depends_on.iter().any(|d| d == "y"));
        assert_ne!(x.wave, y.wave, "same-file tasks must not share a wave");
    }

    #[test]
    fn soft_accept_empty_scopes_stay_parallel() {
        let mut a = sample_task("p1", &[]);
        a.body = "文案任务甲".into();
        let mut b = sample_task("p2", &[]);
        b.body = "文案任务乙".into();
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "empty-scope".into(),
            max_parallel: 2,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![a, b],
        };
        soft_accept_split(&mut doc);
        assert_eq!(doc.tasks[0].wave, 0);
        assert_eq!(doc.tasks[1].wave, 0);
        assert!(doc.tasks[0].depends_on.is_empty());
        assert!(doc.tasks[1].depends_on.is_empty());
    }

    #[test]
    fn soft_accept_dir_and_file_scope_overlap_serializes() {
        // W2-1: directory vs file under it → same ownership, not parallel.
        let mut a = sample_task("dir", &[]);
        a.scope_paths = vec!["web/js/features/split/".into()];
        a.body = "【做什么】改 split 目录".into();
        let mut b = sample_task("file", &[]);
        b.scope_paths = vec!["web/js/features/split/splitDetail.js".into()];
        b.body = "【做什么】改 splitDetail".into();
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "dir-file".into(),
            max_parallel: 2,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![a, b],
        };
        soft_accept_split(&mut doc);
        let d = doc.tasks.iter().find(|t| t.task_id == "dir").unwrap();
        let f = doc.tasks.iter().find(|t| t.task_id == "file").unwrap();
        assert!(
            f.depends_on.iter().any(|x| x == "dir") || d.depends_on.iter().any(|x| x == "file"),
            "dir∩file must serialize, got dir={:?} file={:?}",
            d.depends_on,
            f.depends_on
        );
        assert_ne!(d.wave, f.wave, "dir and file scopes must not share a wave");
    }

    #[test]
    fn soft_accept_strips_worker_scaffold_from_body() {
        let mut t = sample_task("w1", &[]);
        t.body = "你是执行任务 w1 的 worker\n项目根目录：/p\n\n【做什么】真正要干的事\n【改哪里】src/x.rs".into();
        t.scope_paths = vec!["src/x.rs".into()];
        let mut doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "strip-body".into(),
            max_parallel: 1,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![t],
        };
        soft_accept_split(&mut doc);
        let body = &doc.tasks[0].body;
        assert!(
            !body.lines().next().unwrap_or("").starts_with("你是执行"),
            "soft_accept body first line must not be worker scaffold: {body:?}"
        );
        assert!(body.contains("【做什么】") || body.contains("真正要干"));
        assert!(!doc.tasks[0].summary.contains("你是执行"));
    }

    #[test]
    fn to_plan_ir_strips_worker_scaffold_from_prompt() {
        let mut t = sample_task("w1", &[]);
        t.body = "你是执行任务 w1 的 worker\n项目根目录：/p\n\n【做什么】真正要干的事\n【改哪里】src/x.rs".into();
        t.scope_paths = vec!["src/x.rs".into()];
        // Bypass soft_accept so convert itself must strip (desk may already be clean).
        let doc = CcoSplitJob {
            job_id: "j1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            status: CcoSplitStatus::Ready,
            title: "strip".into(),
            max_parallel: 1,
            source: CcoSplitSource::Llm,
            error: None,
            run_id: None,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            tasks: vec![t],
        };
        let ir = to_plan_ir(&doc, "claude", "print");
        let prompt = &ir.tasks[0].prompt;
        assert!(
            !prompt.lines().next().unwrap_or("").starts_with("你是执行"),
            "first line must not be worker scaffold: {prompt:?}"
        );
        assert!(prompt.contains("【做什么】") || prompt.contains("真正要干"));
        assert_eq!(
            ir.tasks[0]
                .scope
                .as_ref()
                .map(|s| s.paths.clone())
                .unwrap_or_default(),
            vec!["src/x.rs".to_string()]
        );
    }
}
