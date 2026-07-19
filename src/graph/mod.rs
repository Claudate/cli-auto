//! DAG helpers: ready set and stage ordering.
//!
//! [INPUT]: PlanIR · done/started 集合
//! [OUTPUT]: ready_tasks · topo_layers · format_graph
//! [POS]: 调度就绪集与展示波次
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/graph/CLAUDE.md

use std::collections::{HashMap, HashSet};

use crate::plan::PlanIR;

/// Task ids with all dependencies in `done`.
pub fn ready_tasks(plan: &PlanIR, done: &HashSet<String>, started: &HashSet<String>) -> Vec<String> {
    let mut ready = Vec::new();
    for t in &plan.tasks {
        if done.contains(&t.id) || started.contains(&t.id) {
            continue;
        }
        if t.depends_on.iter().all(|d| done.contains(d)) {
            ready.push(t.id.clone());
        }
    }
    ready
}

/// Topological layers (for display). Each layer can run in parallel.
pub fn topo_layers(plan: &PlanIR) -> Vec<Vec<String>> {
    let mut indeg: HashMap<String, usize> = plan
        .tasks
        .iter()
        .map(|t| (t.id.clone(), t.depends_on.len()))
        .collect();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for t in &plan.tasks {
        for d in &t.depends_on {
            adj.entry(d.clone()).or_default().push(t.id.clone());
        }
    }

    let mut layers = Vec::new();
    let mut remaining: HashSet<String> = plan.tasks.iter().map(|t| t.id.clone()).collect();

    while !remaining.is_empty() {
        let layer: Vec<String> = remaining
            .iter()
            .filter(|id| indeg.get(*id).copied().unwrap_or(0) == 0)
            .cloned()
            .collect();
        if layer.is_empty() {
            // cycle already validated; shouldn't happen
            break;
        }
        for id in &layer {
            remaining.remove(id);
            if let Some(nexts) = adj.get(id) {
                for n in nexts {
                    if let Some(e) = indeg.get_mut(n) {
                        *e = e.saturating_sub(1);
                    }
                }
            }
        }
        layers.push(layer);
    }
    layers
}

pub fn format_graph(plan: &PlanIR) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "plan: {}  adapter: {}  max_parallel: {}\n",
        plan.name, plan.adapter, plan.max_parallel
    ));
    out.push_str(&format!(
        "default_provider: {}  default_mode: {}\n\n",
        plan.default_provider, plan.default_mode
    ));
    let layers = topo_layers(plan);
    for (i, layer) in layers.iter().enumerate() {
        out.push_str(&format!("stage {i}:\n"));
        for id in layer {
            if let Some(t) = plan.task(id) {
                let deps = if t.depends_on.is_empty() {
                    "—".into()
                } else {
                    t.depends_on.join(",")
                };
                out.push_str(&format!(
                    "  - {id}  [{}]  provider={} mode={}  depends=[{deps}]  \"{}\"\n",
                    t.group.as_deref().unwrap_or("-"),
                    t.provider,
                    t.mode,
                    t.title
                ));
            }
        }
    }
    out
}
