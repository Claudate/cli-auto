//! PlanIR validate + collab rules (pure).
//!
//! [INPUT]: PlanIR
//! [OUTPUT]: PlanIR::validate / task()
//! [POS]: domain/plan
//! [PROTOCOL]: 变更时更新此头部

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};

use super::system_ids::is_system_post_task;
use super::types::{
    PlanIR, TaskIR, TaskRole, MAX_PROMPT_CHARS, MAX_TASKS, MAX_TIMEOUT_SECS,
};

impl PlanIR {
    pub fn task(&self, id: &str) -> Option<&TaskIR> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() {
            bail!("plan has no tasks");
        }
        if self.tasks.len() > MAX_TASKS {
            bail!(
                "plan has {} tasks (max {MAX_TASKS}); split the plan or reduce scope",
                self.tasks.len()
            );
        }
        let ids: HashSet<_> = self.tasks.iter().map(|t| t.id.as_str()).collect();
        if ids.len() != self.tasks.len() {
            bail!("duplicate task ids");
        }
        for t in &self.tasks {
            if t.id.trim().is_empty() {
                bail!("empty task id");
            }
            if t.prompt.trim().is_empty() {
                bail!("task {} has empty prompt", t.id);
            }
            let prompt_len = t.prompt.chars().count();
            if prompt_len > MAX_PROMPT_CHARS {
                bail!(
                    "task {} prompt is too long ({} chars, max {MAX_PROMPT_CHARS})",
                    t.id,
                    prompt_len
                );
            }
            if let Some(to) = t.timeout_secs {
                if to > MAX_TIMEOUT_SECS {
                    bail!(
                        "task {} timeout_secs={to} exceeds max {MAX_TIMEOUT_SECS} (24h)",
                        t.id
                    );
                }
            }
            for dep in &t.depends_on {
                if !ids.contains(dep.as_str()) {
                    bail!("task {} depends on unknown task {}", t.id, dep);
                }
                if dep == &t.id {
                    bail!("task {} depends on itself", t.id);
                }
            }
        }
        // Cycle detection via Kahn
        let mut indeg: HashMap<&str, usize> = self.tasks.iter().map(|t| (t.id.as_str(), 0)).collect();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for t in &self.tasks {
            for dep in &t.depends_on {
                adj.entry(dep.as_str()).or_default().push(t.id.as_str());
                *indeg.get_mut(t.id.as_str()).unwrap() += 1;
            }
        }
        let mut queue: Vec<&str> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut seen = 0;
        while let Some(n) = queue.pop() {
            seen += 1;
            if let Some(nexts) = adj.get(n) {
                for m in nexts {
                    let e = indeg.get_mut(m).unwrap();
                    *e -= 1;
                    if *e == 0 {
                        queue.push(m);
                    }
                }
            }
        }
        if seen != self.tasks.len() {
            bail!("plan DAG contains a cycle");
        }
        // P1-2: multi-CLI collaboration hard rules (no CLI spawn here).
        self.validate_collab_rules()?;
        Ok(())
    }

    /// P1-2 illegal mix-run / role-scope / inspect-gate / codex-bg checks.
    ///
    /// Old single-provider plans without `role` keep passing (back-compat).
    fn validate_collab_rules(&self) -> Result<()> {
        // ── 4. non-claude shell providers + mode=bg ─────────────────────
        for t in &self.tasks {
            if t.mode.eq_ignore_ascii_case("bg")
                && !t.provider.eq_ignore_ascii_case("claude")
                && !t.provider.eq_ignore_ascii_case("fake")
                && !t.provider.eq_ignore_ascii_case("mock")
            {
                bail!(
                    "task {}: provider `{}` does not support mode=bg (use print/exec)",
                    t.id,
                    t.provider
                );
            }
        }

        // ── 1. multi-provider + parallel wave → worktree fully on ────────
        let provider_set: HashSet<&str> = self.tasks.iter().map(|t| t.provider.as_str()).collect();
        if provider_set.len() > 1 && dag_has_parallel_wave(&self.tasks) {
            for t in &self.tasks {
                let effective = t.worktree.unwrap_or(self.worktree);
                if !effective {
                    bail!(
                        "multi-provider plan with a parallel wave requires worktree \
                         (task {} has worktree off; set plan worktree: true or task.worktree: true)",
                        t.id
                    );
                }
            }
        }

        // ── 2. implement scope.paths required + same-wave no overlap ─────
        let implements: Vec<&TaskIR> = self
            .tasks
            .iter()
            .filter(|t| t.role == Some(TaskRole::Implement))
            .collect();
        for t in &implements {
            let paths = t
                .scope
                .as_ref()
                .map(|s| s.paths.as_slice())
                .unwrap_or(&[]);
            if paths.is_empty() {
                // Explicit role=implement without writable scope is a hard error (P1).
                // role missing → not forced (legacy plans).
                bail!(
                    "task {}: role=implement requires non-empty scope.paths (writable whitelist)",
                    t.id
                );
            }
        }

        // ── 2b. scrape (+ browser) must declare writable scope for fill-back ──
        // Soft-ish hard error only when browser+scrape (outbound write-back).
        use super::risk::{task_has_browser_tag, task_has_scrape_tag};
        for t in &self.tasks {
            if !task_has_scrape_tag(&t.tags) || !task_has_browser_tag(&t.tags) {
                continue;
            }
            let paths = t
                .scope
                .as_ref()
                .map(|s| s.paths.as_slice())
                .unwrap_or(&[]);
            if paths.is_empty() {
                bail!(
                    "task {}: browser+scrape 须填写 scope.paths（抓取结果写入的白名单路径）；见 docs/browser-automation-cco.md",
                    t.id
                );
            }
        }
        // Same-wave = neither task is a transitive ancestor of the other.
        let ancestors = transitive_ancestors(&self.tasks);
        for i in 0..implements.len() {
            for j in (i + 1)..implements.len() {
                let a = implements[i];
                let b = implements[j];
                let a_anc = ancestors.get(a.id.as_str());
                let b_anc = ancestors.get(b.id.as_str());
                let a_before_b = b_anc.map(|s| s.contains(a.id.as_str())).unwrap_or(false);
                let b_before_a = a_anc.map(|s| s.contains(b.id.as_str())).unwrap_or(false);
                if a_before_b || b_before_a {
                    continue; // serial chain — overlap allowed (sequential writers)
                }
                let pa = a.scope.as_ref().map(|s| s.paths.as_slice()).unwrap_or(&[]);
                let pb = b.scope.as_ref().map(|s| s.paths.as_slice()).unwrap_or(&[]);
                // Both have non-empty paths (enforced above); check intersection.
                if let Some((x, y)) = first_overlapping_paths(pa, pb) {
                    bail!(
                        "parallel implement tasks {} and {} have overlapping scope.paths \
                         ('{x}' ∩ '{y}'); isolate writable ranges or serialize with depends_on",
                        a.id,
                        b.id
                    );
                }
            }
        }

        // ── 3. inspect terminal gate ─────────────────────────────────────
        // Build reverse edges: id → tasks that depend on it.
        let mut successors: HashMap<&str, Vec<&TaskIR>> = HashMap::new();
        for t in &self.tasks {
            for dep in &t.depends_on {
                successors.entry(dep.as_str()).or_default().push(t);
            }
        }
        let inspect_ids: Vec<&str> = self
            .tasks
            .iter()
            .filter(|t| t.role == Some(TaskRole::Inspect))
            .map(|t| t.id.as_str())
            .collect();

        for insp_id in &inspect_ids {
            if let Some(succs) = successors.get(insp_id) {
                for s in succs {
                    // inspect may be followed by: another inspect, or host system
                    // post-tasks (git push 等 · 非业务回补). Business roles = fail.
                    if s.role == Some(TaskRole::Inspect) || is_system_post_task(&s.id) {
                        continue;
                    }
                    match s.role {
                        Some(other) => {
                            bail!(
                                "inspect task {insp_id} is not a terminal gate: \
                                 task {} (role={other:?}) depends on it",
                                s.id
                            );
                        }
                        None => {
                            bail!(
                                "inspect task {insp_id} is not a terminal gate: \
                                 task {} depends on it (inspect must have no business downstream)",
                                s.id
                            );
                        }
                    }
                }
            }
        }

        if self.require_inspect {
            if inspect_ids.is_empty() {
                bail!("require_inspect=true but plan has no role=inspect task");
            }
            // At least one inspect must be a sink for *business* work.
            // System post successors (sys-post-*) do not count as business.
            let has_sink = inspect_ids.iter().any(|id| {
                successors
                    .get(id)
                    .map(|s| {
                        s.iter()
                            .all(|t| is_system_post_task(&t.id) || t.role == Some(TaskRole::Inspect))
                    })
                    .unwrap_or(true)
            });
            // Prefer a true sink (no successors) OR only system-post after inspect.
            let has_true_or_sys_sink = has_sink;
            if !has_true_or_sys_sink {
                bail!(
                    "require_inspect=true but no terminal inspect sink \
                     (every inspect has business successors)"
                );
            }
        }

        Ok(())
    }
}

/// True when the DAG has at least two tasks that can be ready in the same wave
/// (neither is a transitive ancestor of the other).
pub(crate) fn dag_has_parallel_wave(tasks: &[TaskIR]) -> bool {
    if tasks.len() < 2 {
        return false;
    }
    let ancestors = transitive_ancestors(tasks);
    for i in 0..tasks.len() {
        for j in (i + 1)..tasks.len() {
            let a = tasks[i].id.as_str();
            let b = tasks[j].id.as_str();
            let a_before_b = ancestors
                .get(b)
                .map(|s| s.contains(a))
                .unwrap_or(false);
            let b_before_a = ancestors
                .get(a)
                .map(|s| s.contains(b))
                .unwrap_or(false);
            if !a_before_b && !b_before_a {
                return true;
            }
        }
    }
    false
}

/// Map task id → set of transitive ancestor ids (depends_on closure).
pub(crate) fn transitive_ancestors(tasks: &[TaskIR]) -> HashMap<String, HashSet<String>> {
    let direct: HashMap<&str, &Vec<String>> = tasks
        .iter()
        .map(|t| (t.id.as_str(), &t.depends_on))
        .collect();
    let mut out: HashMap<String, HashSet<String>> = HashMap::new();
    for t in tasks {
        let mut seen = HashSet::new();
        let mut stack: Vec<&str> = t.depends_on.iter().map(|s| s.as_str()).collect();
        while let Some(n) = stack.pop() {
            if !seen.insert(n.to_string()) {
                continue;
            }
            if let Some(deps) = direct.get(n) {
                for d in *deps {
                    stack.push(d.as_str());
                }
            }
        }
        out.insert(t.id.clone(), seen);
    }
    out
}

/// Normalize a scope glob to a comparable directory/file prefix.
///
/// Returns `None` when the glob matches the whole tree (`**`, `*`, empty).
pub(crate) fn scope_glob_prefix(glob: &str) -> Option<String> {
    let mut p = glob.trim().replace('\\', "/");
    while p.starts_with("./") {
        p = p[2..].to_string();
    }
    p = p.trim_matches('/').to_string();
    if p.is_empty() || p == "**" || p == "*" || p == "**/*" {
        return None;
    }
    // Strip common trailing wildcards: /**  /*  /**
    if let Some(s) = p.strip_suffix("/**/*") {
        p = s.trim_end_matches('/').to_string();
    } else if let Some(s) = p.strip_suffix("/**") {
        p = s.trim_end_matches('/').to_string();
    } else if let Some(s) = p.strip_suffix("/*") {
        p = s.trim_end_matches('/').to_string();
    } else if p.ends_with('*') {
        p = p.trim_end_matches('*').trim_end_matches('/').to_string();
    }
    // Mid-path wildcards: use longest literal prefix before first * or ?
    if let Some(idx) = p.find(['*', '?']) {
        p = p[..idx].trim_end_matches('/').to_string();
    }
    if p.is_empty() {
        return None;
    }
    Some(p)
}

/// True when two scope globs may write the same path tree.
pub(crate) fn scope_paths_overlap(a: &str, b: &str) -> bool {
    match (scope_glob_prefix(a), scope_glob_prefix(b)) {
        (None, _) | (_, None) => true, // universal ∩ anything
        (Some(pa), Some(pb)) => {
            pa == pb || pa.starts_with(&format!("{pb}/")) || pb.starts_with(&format!("{pa}/"))
        }
    }
}

/// First overlapping path pair across two path lists, if any.
pub(crate) fn first_overlapping_paths<'a>(
    a: &'a [String],
    b: &'a [String],
) -> Option<(&'a str, &'a str)> {
    for x in a {
        for y in b {
            if scope_paths_overlap(x, y) {
                return Some((x.as_str(), y.as_str()));
            }
        }
    }
    None
}

