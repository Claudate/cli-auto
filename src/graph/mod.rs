//! DAG helpers: ready set and stage ordering.
//!
//! [INPUT]: PlanIR · done/started 集合
//! [OUTPUT]: ready_tasks · topo_layers · format_graph · format_mermaid（P2-7 薄切片）
//! [POS]: 调度就绪集与展示波次
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/graph/CLAUDE.md

use std::collections::{HashMap, HashSet};

use crate::plan::PlanIR;

/// Task ids with all dependencies in `done`.
pub fn ready_tasks(
    plan: &PlanIR,
    done: &HashSet<String>,
    started: &HashSet<String>,
) -> Vec<String> {
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

/// Mermaid `flowchart TD` for docs / preview (P2-7 thin slice).
/// Node id = task id (sanitized); label = id + short title + provider.
pub fn format_mermaid(plan: &PlanIR) -> String {
    let mut out = String::new();
    out.push_str("```mermaid\n");
    out.push_str("flowchart TD\n");
    let safe_name = mermaid_escape(&plan.name);
    out.push_str(&format!(
        "  %% plan: {safe_name} · adapter: {}\n",
        plan.adapter
    ));
    for t in &plan.tasks {
        let nid = mermaid_node_id(&t.id);
        let title = short_label(&t.title, 40);
        let label = mermaid_escape(&format!("{} · {} ({})", t.id, title, t.provider));
        out.push_str(&format!("  {nid}[\"{label}\"]\n"));
    }
    for t in &plan.tasks {
        let nid = mermaid_node_id(&t.id);
        for d in &t.depends_on {
            let did = mermaid_node_id(d);
            out.push_str(&format!("  {did} --> {nid}\n"));
        }
    }
    if plan.tasks.is_empty() {
        out.push_str("  empty[[no tasks]]\n");
    }
    out.push_str("```\n");
    out
}

fn mermaid_node_id(id: &str) -> String {
    let mut s: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        s.push_str("t");
    }
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        s.insert(0, 't');
    }
    s
}

fn mermaid_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "#quot;")
        .replace('\n', " ")
        .replace('\r', "")
}

fn short_label(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let clipped: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{clipped}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::plan::{OnFailure, PlanIR, TaskIR};

    fn task(id: &str, title: &str, provider: &str, depends_on: Vec<String>) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: title.into(),
            depends_on,
            group: None,
            provider: provider.into(),
            mode: "print".into(),
            prompt: "x".into(),
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
        }
    }

    fn sample_plan() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "demo".into(),
            adapter: "cco-plan/v1".into(),
            source_path: PathBuf::from("plans/demo.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![
                task("t1", "First", "fake", vec![]),
                task("t2", "Second with \"quotes\"", "claude", vec!["t1".into()]),
            ],
        }
    }

    #[test]
    fn mermaid_contains_nodes_and_edge() {
        let m = format_mermaid(&sample_plan());
        assert!(m.contains("flowchart TD"), "{m}");
        assert!(m.contains("t1["), "{m}");
        assert!(m.contains("t2["), "{m}");
        assert!(m.contains("t1 --> t2"), "{m}");
        assert!(m.contains("```mermaid"), "{m}");
        assert!(
            !m.contains("Second with \"quotes\""),
            "raw quotes escaped: {m}"
        );
    }

    #[test]
    fn mermaid_node_id_sanitizes() {
        assert_eq!(mermaid_node_id("t1"), "t1");
        assert_eq!(mermaid_node_id("a/b"), "a_b");
        assert_eq!(mermaid_node_id("1x"), "t1x");
    }
}
