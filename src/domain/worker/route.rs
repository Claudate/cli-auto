//! Soft-fill / force route fill (A1-4). Never silent-overwrite explicit engines on Soft.
//!
//! [INPUT]: PlanIR · desired provider · fill mode
//! [OUTPUT]: mutated plan + RouteFillReport (filled_ids / kept_ids for provenance)
//! [POS]: domain/worker — CLI `--provider` / planner confirm / desktop defaults
//! [PROTOCOL]: Force only with explicit force semantics; Soft keeps mixed plans mixed.
//!   Domain **never** writes paths / RunState — app stamps `route_source` from reports.

use crate::domain::plan::PlanIR;

/// How to apply a run-level provider choice onto tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteFillMode {
    /// Rewrite only empty / `"default"` / still-on-prior-default tasks.
    Soft,
    /// Wipe every task.provider + default_provider (legacy `--force-provider`).
    Force,
}

/// Result of soft/force fill. IDs let app/runtime stamp `TaskState.route_source`
/// without re-deriving policy (P1-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteFillReport {
    pub mode: RouteFillMode,
    pub provider: String,
    pub filled: usize,
    pub kept_explicit: usize,
    /// Task ids whose provider was rewritten by this fill.
    pub filled_ids: Vec<String>,
    /// Soft only: task ids that kept a non-default explicit provider.
    pub kept_ids: Vec<String>,
}

impl RouteFillReport {
    pub fn summary_line(&self) -> String {
        match self.mode {
            RouteFillMode::Force => format!(
                "force-provider: all {} task(s) → {}",
                self.filled, self.provider
            ),
            RouteFillMode::Soft => format!(
                "provider: default → {} (filled {} default task(s), kept {} explicit)",
                self.provider, self.filled, self.kept_explicit
            ),
        }
    }
}

/// True when task.provider is empty, placeholder `"default"`, or equals `old_default`.
pub fn is_still_default_route(task_provider: &str, old_default: &str) -> bool {
    let p = task_provider.trim();
    p.is_empty()
        || p.eq_ignore_ascii_case("default")
        || (!old_default.is_empty() && p.eq_ignore_ascii_case(old_default))
}

/// Apply soft or force provider fill. Returns None when `provider` is empty (no-op).
pub fn apply_route_fill(
    plan: &mut PlanIR,
    provider: &str,
    mode: RouteFillMode,
) -> Option<RouteFillReport> {
    let p = provider.trim();
    if p.is_empty() {
        return None;
    }
    match mode {
        RouteFillMode::Force => {
            let filled_ids: Vec<String> = plan.tasks.iter().map(|t| t.id.clone()).collect();
            let n = filled_ids.len();
            for t in &mut plan.tasks {
                t.provider = p.to_string();
            }
            plan.default_provider = p.to_string();
            Some(RouteFillReport {
                mode: RouteFillMode::Force,
                provider: p.to_string(),
                filled: n,
                kept_explicit: 0,
                filled_ids,
                kept_ids: vec![],
            })
        }
        RouteFillMode::Soft => {
            let old = plan.default_provider.clone();
            let mut filled_ids = Vec::new();
            let mut kept_ids = Vec::new();
            for t in &mut plan.tasks {
                if is_still_default_route(&t.provider, &old) {
                    t.provider = p.to_string();
                    filled_ids.push(t.id.clone());
                } else {
                    kept_ids.push(t.id.clone());
                }
            }
            plan.default_provider = p.to_string();
            Some(RouteFillReport {
                mode: RouteFillMode::Soft,
                provider: p.to_string(),
                filled: filled_ids.len(),
                kept_explicit: kept_ids.len(),
                filled_ids,
                kept_ids,
            })
        }
    }
}

/// Soft-fill job defaults (provider + exec mode) onto tasks.
///
/// - Always sets `default_provider` / `default_mode` and each task's `mode`.
/// - Provider is **soft**: only rewrite still-default routes (see [`is_still_default_route`]).
/// - Returns a Soft [`RouteFillReport`] so app can stamp `route_source` (P1-2).
pub fn apply_worker_defaults(plan: &mut PlanIR, provider: &str, exec_mode: &str) -> RouteFillReport {
    let old_default = plan.default_provider.clone();
    plan.default_provider = provider.to_string();
    plan.default_mode = exec_mode.to_string();
    let mut filled_ids = Vec::new();
    let mut kept_ids = Vec::new();
    for t in &mut plan.tasks {
        t.mode = exec_mode.to_string();
        if is_still_default_route(&t.provider, &old_default) {
            t.provider = provider.to_string();
            filled_ids.push(t.id.clone());
        } else {
            kept_ids.push(t.id.clone());
        }
    }
    RouteFillReport {
        mode: RouteFillMode::Soft,
        provider: provider.to_string(),
        filled: filled_ids.len(),
        kept_explicit: kept_ids.len(),
        filled_ids,
        kept_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{OnFailure, TaskIR};
    use std::path::PathBuf;

    fn task(id: &str, provider: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: provider.into(),
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
        }
    }

    fn mixed_plan() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "mixed".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("mixed.cco.yaml"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: true,
            require_inspect: false,
            tasks: vec![
                task("t1", "claude"),
                task("t2", "codex"),
                task("t3", "default"),
                task("t4", ""),
            ],
        }
    }

    #[test]
    fn soft_keeps_explicit_codex() {
        let mut ir = mixed_plan();
        let r = apply_route_fill(&mut ir, "fake", RouteFillMode::Soft).unwrap();
        assert_eq!(r.filled, 3);
        assert_eq!(r.kept_explicit, 1);
        assert_eq!(r.kept_ids, vec!["t2".to_string()]);
        assert!(r.filled_ids.contains(&"t1".into()));
        assert!(r.filled_ids.contains(&"t3".into()));
        assert!(r.filled_ids.contains(&"t4".into()));
        assert_eq!(ir.tasks[0].provider, "fake");
        assert_eq!(ir.tasks[1].provider, "codex");
        assert_eq!(ir.tasks[2].provider, "fake");
        assert_eq!(ir.tasks[3].provider, "fake");
    }

    #[test]
    fn force_wipes_all() {
        let mut ir = mixed_plan();
        let r = apply_route_fill(&mut ir, "fake", RouteFillMode::Force).unwrap();
        assert_eq!(r.mode, RouteFillMode::Force);
        assert_eq!(r.filled_ids.len(), 4);
        assert!(r.kept_ids.is_empty());
        assert!(ir.tasks.iter().all(|t| t.provider == "fake"));
        assert_eq!(ir.default_provider, "fake");
    }

    #[test]
    fn worker_defaults_soft_and_modes() {
        let mut ir = mixed_plan();
        let r = apply_worker_defaults(&mut ir, "fake", "bg");
        assert_eq!(ir.default_mode, "bg");
        assert_eq!(ir.tasks[1].provider, "codex");
        assert!(ir.tasks.iter().all(|t| t.mode == "bg"));
        assert_eq!(r.mode, RouteFillMode::Soft);
        assert_eq!(r.kept_ids, vec!["t2".to_string()]);
        assert_eq!(r.filled, 3);
    }
}
