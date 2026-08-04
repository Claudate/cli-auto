//! Active task set filters (`--only` / `--from-task`).
//!
//! [INPUT]: all task ids · depends_on edges · only/from filters
//! [OUTPUT]: active id set or error string
//! [POS]: domain/run — pure DAG filter (ready_tasks stays in graph/)
//! [PROTOCOL]: 变更时更新 domain/run/mod.rs

use std::collections::HashSet;

/// How the orchestrator restricts which plan tasks may run.
#[derive(Debug, Clone)]
pub enum ActiveFilter {
    All,
    Only(HashSet<String>),
    FromTask(String),
}

/// Resolve the active task id set. `edges` = (id, depends_on) in plan order.
pub fn resolve_active_ids(
    all: &HashSet<String>,
    edges: &[(String, Vec<String>)],
    filter: &ActiveFilter,
) -> Result<HashSet<String>, String> {
    match filter {
        ActiveFilter::All => Ok(all.clone()),
        ActiveFilter::Only(only) => {
            for id in only {
                if !all.contains(id) {
                    return Err(format!("--only unknown task: {id}"));
                }
            }
            Ok(only.clone())
        }
        ActiveFilter::FromTask(from) => expand_from_task(all, edges, from),
    }
}

/// Include `from` and every transitive dependent (downstream), not upstream.
pub fn expand_from_task(
    all: &HashSet<String>,
    edges: &[(String, Vec<String>)],
    from: &str,
) -> Result<HashSet<String>, String> {
    if !all.contains(from) {
        return Err(format!("--from-task unknown: {from}"));
    }
    let mut include = HashSet::new();
    include.insert(from.to_string());
    let mut changed = true;
    while changed {
        changed = false;
        for (id, deps) in edges {
            if deps.iter().any(|d| include.contains(d)) && !include.contains(id) {
                include.insert(id.clone());
                changed = true;
            }
        }
    }
    Ok(include)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (HashSet<String>, Vec<(String, Vec<String>)>) {
        let edges = vec![
            ("a".into(), vec![]),
            ("b".into(), vec!["a".into()]),
            ("c".into(), vec!["b".into()]),
            ("d".into(), vec![]),
        ];
        let all: HashSet<String> = edges
            .iter()
            .map(|(id, _): &(String, Vec<String>)| id.clone())
            .collect();
        (all, edges)
    }

    #[test]
    fn only_filter() {
        let (all, edges) = sample();
        let mut only = HashSet::new();
        only.insert("a".into());
        only.insert("b".into());
        let got = resolve_active_ids(&all, &edges, &ActiveFilter::Only(only.clone())).unwrap();
        assert_eq!(got, only);
        let bad = resolve_active_ids(
            &all,
            &edges,
            &ActiveFilter::Only(HashSet::from(["x".into()])),
        );
        assert!(bad.is_err());
    }

    #[test]
    fn from_task_expands_downstream() {
        let (all, edges) = sample();
        let got = resolve_active_ids(&all, &edges, &ActiveFilter::FromTask("b".into())).unwrap();
        assert!(got.contains("b"));
        assert!(got.contains("c"));
        assert!(!got.contains("a"));
        assert!(!got.contains("d"));
    }
}
