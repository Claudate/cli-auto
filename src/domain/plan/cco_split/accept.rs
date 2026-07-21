//! Soft accept + wave recompute + run gate for CcoSplit.
//!
//! [INPUT]: CcoSplitJob mut
//! [OUTPUT]: soft_accept notes · waves · run_gate_ok
//! [POS]: domain/plan/cco_split
//! [PROTOCOL]: 变更时更新此头部

use std::collections::{BTreeMap, HashMap, HashSet};

use super::types::CcoSplitJob;
use crate::domain::plan::types::MAX_TASKS;

/// Soft accept: keep displayable graphs; never discard whole doc for collab strictness.
///
/// Fixes: empty ids → synthetic; drop self/unknown deps; break simple cycles;
/// fill missing title/body; recompute waves; cap task count.
pub fn soft_accept_split(doc: &mut CcoSplitJob) -> Vec<String> {
    let mut notes = Vec::new();

    if doc.tasks.len() > MAX_TASKS {
        let n = doc.tasks.len() - MAX_TASKS;
        doc.tasks.truncate(MAX_TASKS);
        notes.push(format!("truncated to {MAX_TASKS} tasks (dropped {n})"));
    }

    let mut used: HashSet<String> = HashSet::new();
    for (i, t) in doc.tasks.iter_mut().enumerate() {
        t.ord = i as i32;
        let mut id = t.task_id.trim().to_string();
        if id.is_empty() {
            id = format!("t{}", i + 1);
            notes.push(format!("task #{i}: empty id → {id}"));
        }
        let base = id.clone();
        let mut n = 2;
        while used.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        if id != t.task_id {
            if !t.task_id.trim().is_empty() {
                notes.push(format!("task id {} → {id} (unique)", t.task_id));
            }
            t.task_id = id.clone();
        }
        used.insert(id);

        if t.title.trim().is_empty() {
            t.title = format!("步骤 {}", i + 1);
            notes.push(format!("task {}: empty title → default", t.task_id));
        }
        if t.body.trim().is_empty() {
            t.body = if t.summary.trim().is_empty() {
                t.title.clone()
            } else {
                t.summary.clone()
            };
            notes.push(format!("task {}: empty body → filled", t.task_id));
        }
        if t.summary.trim().is_empty() {
            t.summary = first_line_summary(&t.body);
        }
        if !t.optional {
            t.enabled = true;
        }
    }

    let ids: HashSet<String> = doc.tasks.iter().map(|t| t.task_id.clone()).collect();
    for t in doc.tasks.iter_mut() {
        let before = t.depends_on.len();
        t.depends_on
            .retain(|d| d != &t.task_id && ids.contains(d));
        if t.depends_on.len() != before {
            notes.push(format!(
                "task {}: pruned {} bad depends_on",
                t.task_id,
                before - t.depends_on.len()
            ));
        }
    }

    break_cycles(doc, &mut notes);
    recompute_waves(doc);

    if doc.max_parallel == 0 {
        doc.max_parallel = 1;
        notes.push("max_parallel 0 → 1".into());
    }
    if doc.title.trim().is_empty() {
        doc.title = doc
            .plan_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plan")
            .to_string();
    }

    notes
}

pub(crate) fn first_line_summary(body: &str) -> String {
    let line = body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or(body)
        .trim();
    let s: String = line.chars().take(80).collect();
    if line.chars().count() > 80 {
        format!("{s}…")
    } else {
        s
    }
}

fn break_cycles(doc: &mut CcoSplitJob, notes: &mut Vec<String>) {
    loop {
        let mut indeg: HashMap<String, usize> = HashMap::new();
        for t in &doc.tasks {
            indeg.insert(t.task_id.clone(), t.depends_on.len());
        }
        let mut q: Vec<String> = indeg
            .iter()
            .filter(|(_, &n)| n == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut seen = 0usize;
        let mut remaining = indeg.clone();
        while let Some(u) = q.pop() {
            seen += 1;
            for t in &doc.tasks {
                if t.depends_on.iter().any(|d| d == &u) {
                    if let Some(n) = remaining.get_mut(&t.task_id) {
                        *n = n.saturating_sub(1);
                        if *n == 0 {
                            q.push(t.task_id.clone());
                        }
                    }
                }
            }
        }
        if seen >= doc.tasks.len() {
            break;
        }
        let mut candidates: Vec<(i32, String, String)> = Vec::new();
        for t in &doc.tasks {
            if remaining.get(&t.task_id).copied().unwrap_or(0) > 0 {
                if let Some(dep) = t.depends_on.first() {
                    candidates.push((t.ord, t.task_id.clone(), dep.clone()));
                }
            }
        }
        candidates.sort_by_key(|(ord, _, _)| *ord);
        let Some((_, tid, dep)) = candidates.into_iter().next() else {
            break;
        };
        if let Some(t) = doc.tasks.iter_mut().find(|t| t.task_id == tid) {
            t.depends_on.retain(|d| d != &dep);
            notes.push(format!("cycle: dropped {tid} depends_on {dep}"));
        } else {
            break;
        }
    }
}

/// Assign wave index (0-based) via topological layers.
pub fn recompute_waves(doc: &mut CcoSplitJob) {
    let mut remaining: HashSet<String> = doc.tasks.iter().map(|t| t.task_id.clone()).collect();
    let mut done: HashSet<String> = HashSet::new();
    let mut wave = 0i32;
    let mut wave_of: HashMap<String, i32> = HashMap::new();
    while !remaining.is_empty() {
        let mut layer: Vec<String> = Vec::new();
        for t in &doc.tasks {
            if !remaining.contains(&t.task_id) {
                continue;
            }
            if t.depends_on.iter().all(|d| done.contains(d)) {
                layer.push(t.task_id.clone());
            }
        }
        if layer.is_empty() {
            for id in remaining.iter() {
                wave_of.insert(id.clone(), wave);
            }
            break;
        }
        for id in &layer {
            wave_of.insert(id.clone(), wave);
            done.insert(id.clone());
            remaining.remove(id);
        }
        wave += 1;
    }
    for t in doc.tasks.iter_mut() {
        t.wave = wave_of.get(&t.task_id).copied().unwrap_or(0);
    }
}

/// Soft run gate: at least one enabled task; deps point to existing ids.
pub fn run_gate_ok(doc: &CcoSplitJob) -> Result<(), String> {
    if !doc.tasks.iter().any(|t| t.enabled) {
        return Err("没有选中任何任务：请至少勾选一项后再开始".into());
    }
    let ids: HashSet<&str> = doc.tasks.iter().map(|t| t.task_id.as_str()).collect();
    for t in &doc.tasks {
        if !t.enabled {
            continue;
        }
        for d in &t.depends_on {
            if !ids.contains(d.as_str()) {
                return Err(format!("任务 {} 依赖不存在的步骤 {d}", t.task_id));
            }
        }
    }
    Ok(())
}

/// Topo layers of task ids (for desk DTO layers field).
pub fn split_topo_layers(doc: &CcoSplitJob) -> Vec<Vec<String>> {
    let mut by_wave: BTreeMap<i32, Vec<String>> = BTreeMap::new();
    for t in &doc.tasks {
        by_wave.entry(t.wave).or_default().push(t.task_id.clone());
    }
    by_wave.into_values().collect()
}
