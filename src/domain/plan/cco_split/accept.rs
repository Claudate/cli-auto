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
        // W2-3: desk + SQLite body match exec (convert also strips at to_plan_ir).
        let stripped = super::humanize::strip_worker_scaffold(&t.body);
        if !stripped.trim().is_empty() && stripped != t.body {
            t.body = stripped;
            notes.push(format!("task {}: stripped worker scaffold from body", t.task_id));
        }
        // Always prefer human one-liner (worker scaffold must not win).
        let hum = super::humanize::human_summary(
            &t.title,
            &t.body,
            t.done_when.as_deref(),
        );
        if t.summary.trim().is_empty()
            || super::humanize::is_worker_noise_line(t.summary.trim())
            || t.summary.contains("的 worker")
            || t.summary.contains("你是执行")
        {
            t.summary = if hum.is_empty() {
                first_line_summary(&t.body)
            } else {
                hum
            };
        }
        if t.done_when.is_none() {
            t.done_when = super::humanize::parse_done_when(&t.body);
        }
        t.title = super::humanize::display_title(&t.title);
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
    // Q3: same-wave / parallel-ready tasks with overlapping scope_paths → serialize.
    serialize_scope_overlaps(doc, &mut notes);
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

/// If two tasks can run in parallel (neither is ancestor of the other) but their
/// scope_paths overlap, make the later ord wait on the earlier (safe for multi-window).
fn serialize_scope_overlaps(doc: &mut CcoSplitJob, notes: &mut Vec<String>) {
    use super::super::validate::first_overlapping_paths;

    let n = doc.tasks.len();
    if n < 2 {
        return;
    }
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > n * 3 {
            notes.push("scope serialize: stop after many fixes".into());
            break;
        }
        let id_to_deps: HashMap<String, Vec<String>> = doc
            .tasks
            .iter()
            .map(|t| (t.task_id.clone(), t.depends_on.clone()))
            .collect();
        let mut ancestors: HashMap<String, HashSet<String>> = HashMap::new();
        for t in &doc.tasks {
            let mut seen = HashSet::new();
            let mut stack = t.depends_on.clone();
            while let Some(d) = stack.pop() {
                if !seen.insert(d.clone()) {
                    continue;
                }
                if let Some(more) = id_to_deps.get(&d) {
                    stack.extend(more.iter().cloned());
                }
            }
            ancestors.insert(t.task_id.clone(), seen);
        }

        let mut fix: Option<(String, String)> = None; // (later_id, earlier_id)
        'pairs: for i in 0..n {
            for j in (i + 1)..n {
                let a_id = doc.tasks[i].task_id.clone();
                let b_id = doc.tasks[j].task_id.clone();
                let a_anc = ancestors.get(&a_id).cloned().unwrap_or_default();
                let b_anc = ancestors.get(&b_id).cloned().unwrap_or_default();
                if a_anc.contains(&b_id) || b_anc.contains(&a_id) {
                    continue;
                }
                let pa = &doc.tasks[i].scope_paths;
                let pb = &doc.tasks[j].scope_paths;
                if pa.is_empty() || pb.is_empty() {
                    continue;
                }
                if first_overlapping_paths(pa, pb).is_none() {
                    continue;
                }
                let (earlier, later) = if doc.tasks[i].ord <= doc.tasks[j].ord {
                    (a_id, b_id)
                } else {
                    (b_id, a_id)
                };
                fix = Some((later, earlier));
                break 'pairs;
            }
        }
        let Some((later_id, earlier_id)) = fix else {
            break;
        };
        if let Some(t) = doc.tasks.iter_mut().find(|t| t.task_id == later_id) {
            if !t.depends_on.iter().any(|d| d == &earlier_id) {
                t.depends_on.push(earlier_id.clone());
                notes.push(format!(
                    "serialize {later_id} after {earlier_id} (scope_paths overlap)"
                ));
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

pub(crate) fn first_line_summary(body: &str) -> String {
    // Prefer human summary (never worker scaffold first line).
    let s = super::humanize::human_summary("", body, None);
    if !s.is_empty() && s != "查看步骤说明" {
        return s;
    }
    let plain = super::humanize::strip_worker_scaffold(body);
    let line = plain
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#') && !super::humanize::is_worker_noise_line(l))
        .unwrap_or("")
        .trim();
    if line.is_empty() {
        return "查看步骤说明".into();
    }
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

/// Drop unmotivated depends_on edges on a CcoSplit graph (confirm-screen 「让可并行」).
///
/// Keep edge only if dependent **body** mentions dep id/title or an explicit depend-reason line.
/// Returns number of edges removed; recomputes waves when any drop.
pub fn sanitize_cco_split_deps(doc: &mut CcoSplitJob) -> usize {
    let ids: Vec<String> = doc.tasks.iter().map(|t| t.task_id.clone()).collect();
    let titles: Vec<(String, String)> = doc
        .tasks
        .iter()
        .map(|t| (t.task_id.clone(), t.title.clone()))
        .collect();

    let before: usize = doc.tasks.iter().map(|t| t.depends_on.len()).sum();
    for t in doc.tasks.iter_mut() {
        let body_l = t.body.to_ascii_lowercase();
        let body_raw = t.body.clone();
        t.depends_on.retain(|dep| {
            if !ids.iter().any(|id| id == dep) {
                return false;
            }
            if body_raw.contains(dep) {
                return true;
            }
            if let Some((_, title)) = titles.iter().find(|(id, _)| id == dep) {
                if title.chars().count() >= 4 && body_raw.contains(title) {
                    return true;
                }
            }
            if body_raw.contains("依赖原因")
                || body_raw.contains("等待产物")
                || body_raw.contains("depends on")
                || body_l.contains("blocked by")
            {
                return body_raw.lines().any(|line| {
                    let l = line.trim();
                    (l.contains("依赖") || l.contains("depend") || l.contains("等待"))
                        && l.contains(dep.as_str())
                });
            }
            false
        });
    }
    let after: usize = doc.tasks.iter().map(|t| t.depends_on.len()).sum();
    let removed = before.saturating_sub(after);
    if removed > 0 {
        recompute_waves(doc);
    }
    removed
}
