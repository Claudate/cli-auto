//! Soften planner/LLM graphs for **display + run**, not strict collab policing.
//!
//! [INPUT]: PlanIR (mut)
//! [OUTPUT]: notes of auto-fixes; plan should pass `validate()` after
//! [POS]: domain/plan — used by planner accept path
//! [PROTOCOL]: 变更时更新此头部与 domain/CLAUDE.md
//!
//! Product: split is for UI (order · waves · optional/include) and later AI run.
//! Do not discard a whole LLM graph for scope-overlap / missing scope — auto-fix.

use super::types::{PlanIR, TaskRole, TaskScope};
use super::validate::{first_overlapping_paths, scope_paths_overlap};

/// Auto-fix common collab strictness so planner can accept LLM output.
/// Returns human-readable fix notes (for planner.log).
pub fn soften_plan_for_accept(ir: &mut PlanIR) -> Vec<String> {
    let mut notes = Vec::new();

    // 1) non-claude shell + bg → print (claude/fake may keep bg)
    for t in ir.tasks.iter_mut() {
        if t.mode.eq_ignore_ascii_case("bg")
            && !t.provider.eq_ignore_ascii_case("claude")
            && !t.provider.eq_ignore_ascii_case("fake")
            && !t.provider.eq_ignore_ascii_case("mock")
        {
            let p = t.provider.clone();
            t.mode = "print".into();
            notes.push(format!("task {}: {}+bg → mode=print", t.id, p));
        }
    }

    // 2) implement without scope.paths → private synthetic path (display + validate)
    for t in ir.tasks.iter_mut() {
        if t.role != Some(TaskRole::Implement) {
            continue;
        }
        let empty = t.scope.as_ref().map(|s| s.paths.is_empty()).unwrap_or(true);
        if empty {
            let path = format!(".cco-out/wp/{}/", t.id);
            let mut scope = t.scope.clone().unwrap_or(TaskScope {
                paths: vec![],
                readonly: vec![],
                forbid: vec![],
            });
            scope.paths = vec![path.clone()];
            t.scope = Some(scope);
            notes.push(format!("task {}: empty implement scope → {}", t.id, path));
        }
    }

    // 3) multi-provider + parallel → force worktree on
    let providers: std::collections::HashSet<_> = ir
        .tasks
        .iter()
        .map(|t| t.provider.to_ascii_lowercase())
        .collect();
    if providers.len() > 1 {
        for t in ir.tasks.iter_mut() {
            if t.worktree != Some(true) {
                t.worktree = Some(true);
            }
        }
        if !ir.worktree {
            ir.worktree = true;
            notes.push("multi-provider: plan.worktree=true".into());
        }
    }

    // 4) parallel implement with overlapping scope → serialize with depends_on
    //    (order = appearance order in tasks — enough for UI waves + safe run)
    loop {
        let pair = find_parallel_implement_overlap(ir);
        let Some((a_id, b_id)) = pair else {
            break;
        };
        // Make b wait for a (b is later in list when possible)
        let (earlier, later) = order_pair_by_task_index(ir, &a_id, &b_id);
        if let Some(t) = ir.tasks.iter_mut().find(|t| t.id == later) {
            if !t.depends_on.iter().any(|d| d == &earlier) {
                t.depends_on.push(earlier.clone());
                notes.push(format!(
                    "serialize {later} after {earlier} (scope overlap auto-fix)"
                ));
            } else {
                // already depends — widen later scope out of overlap by private path
                if let Some(scope) = t.scope.as_mut() {
                    let private = format!(".cco-out/wp/{}/", later);
                    if !scope.paths.iter().any(|p| p == &private) {
                        scope.paths.push(private);
                        notes.push(format!(
                            "task {later}: append private scope path to break residual overlap"
                        ));
                    } else {
                        // last resort: clear implement role so collab overlap rule skips
                        t.role = None;
                        notes.push(format!(
                            "task {later}: drop role=implement (could not break scope overlap)"
                        ));
                        break;
                    }
                } else {
                    break;
                }
            }
        } else {
            break;
        }
        // safety: avoid infinite loop
        if notes.len() > ir.tasks.len() * 4 {
            notes.push("soften: stop after many overlap fixes".into());
            break;
        }
    }

    // 5) drop unknown deps / self deps (display must not crash)
    let ids: std::collections::HashSet<_> = ir.tasks.iter().map(|t| t.id.clone()).collect();
    for t in ir.tasks.iter_mut() {
        let before = t.depends_on.len();
        t.depends_on.retain(|d| ids.contains(d) && d != &t.id);
        if t.depends_on.len() != before {
            notes.push(format!("task {}: pruned invalid depends_on", t.id));
        }
    }

    // 6) empty inspect depends_on → business leaves (same as materialize_role_defaults)
    let before_wire: Vec<(String, usize)> = ir
        .tasks
        .iter()
        .filter(|t| t.role == Some(TaskRole::Inspect))
        .map(|t| (t.id.clone(), t.depends_on.len()))
        .collect();
    super::materialize::wire_empty_inspect_depends_on(ir);
    for (id, n0) in before_wire {
        if n0 == 0 {
            if let Some(t) = ir.tasks.iter().find(|t| t.id == id) {
                if !t.depends_on.is_empty() {
                    notes.push(format!(
                        "task {id}: empty inspect depends_on → wait on business leaves {:?}",
                        t.depends_on
                    ));
                }
            }
        }
    }

    notes
}

fn order_pair_by_task_index(ir: &PlanIR, a: &str, b: &str) -> (String, String) {
    let ia = ir.tasks.iter().position(|t| t.id == a).unwrap_or(0);
    let ib = ir.tasks.iter().position(|t| t.id == b).unwrap_or(0);
    if ia <= ib {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn find_parallel_implement_overlap(ir: &PlanIR) -> Option<(String, String)> {
    let implements: Vec<&super::types::TaskIR> = ir
        .tasks
        .iter()
        .filter(|t| t.role == Some(TaskRole::Implement))
        .collect();
    // Build ancestor sets lightly
    let ancestors = super::validate::transitive_ancestors(&ir.tasks);
    for i in 0..implements.len() {
        for j in (i + 1)..implements.len() {
            let a = implements[i];
            let b = implements[j];
            let a_before_b = ancestors
                .get(b.id.as_str())
                .map(|s| s.contains(a.id.as_str()))
                .unwrap_or(false);
            let b_before_a = ancestors
                .get(a.id.as_str())
                .map(|s| s.contains(b.id.as_str()))
                .unwrap_or(false);
            if a_before_b || b_before_a {
                continue;
            }
            let pa = a.scope.as_ref().map(|s| s.paths.as_slice()).unwrap_or(&[]);
            let pb = b.scope.as_ref().map(|s| s.paths.as_slice()).unwrap_or(&[]);
            if pa.is_empty() || pb.is_empty() {
                continue;
            }
            if first_overlapping_paths(pa, pb).is_some()
                || pa
                    .iter()
                    .any(|x| pb.iter().any(|y| scope_paths_overlap(x, y)))
            {
                return Some((a.id.clone(), b.id.clone()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::{OnFailure, TaskIR};
    use std::path::PathBuf;

    fn task(id: &str, role: Option<TaskRole>, deps: &[&str], paths: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: format!("do {id}"),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: if paths.is_empty() {
                None
            } else {
                Some(TaskScope {
                    paths: paths.iter().map(|s| (*s).into()).collect(),
                    readonly: vec![],
                    forbid: vec![],
                })
            },
            outputs: vec![],
            tags: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("x.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn soften_serializes_parallel_scope_overlap() {
        let mut ir = plan(vec![
            task("t2", Some(TaskRole::Implement), &[], &["web/index.html"]),
            task("t8", Some(TaskRole::Implement), &[], &["web/index.html"]),
        ]);
        assert!(ir.validate().is_err(), "strict should reject");
        let notes = soften_plan_for_accept(&mut ir);
        assert!(!notes.is_empty());
        ir.validate().expect("softened plan validates");
        let t8 = ir.tasks.iter().find(|t| t.id == "t8").unwrap();
        assert!(
            t8.depends_on.iter().any(|d| d == "t2"),
            "t8 should wait t2: {:?}",
            t8.depends_on
        );
    }

    #[test]
    fn soften_fills_empty_implement_scope() {
        let mut ir = plan(vec![task("t1", Some(TaskRole::Implement), &[], &[])]);
        assert!(ir.validate().is_err());
        soften_plan_for_accept(&mut ir);
        ir.validate().expect("filled scope");
        let paths = &ir.tasks[0].scope.as_ref().unwrap().paths;
        assert!(paths.iter().any(|p| p.contains("t1")));
    }
}
