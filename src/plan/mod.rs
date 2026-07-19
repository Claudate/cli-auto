//! Plan loading, adapters, and PlanIR.
//!
//! [INPUT]: 计划文件路径 · config 默认 provider/mode
//! [OUTPUT]: PlanIR/TaskIR(role/scope/outputs/require_inspect) · load_plan · list_plans · materialize_role_defaults(P2-1 inspect) · validate(含 P1-2 混部硬规则) · is_structured_adapter · MAX_TASKS/MAX_PROMPT_CHARS/MAX_TIMEOUT_SECS
//! [POS]: plan 模块入口；adapters 解析，planner 为 Mode B 侧路
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

pub mod adapters;
pub mod planner;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;

// ── Product hard limits (P1-4 / B3) ───────────────────────────────────
/// Max tasks in one plan (planner + validate).
pub const MAX_TASKS: usize = 20;
/// Max characters per task prompt.
pub const MAX_PROMPT_CHARS: usize = 32_000;
/// Max per-task timeout (24h).
pub const MAX_TIMEOUT_SECS: u64 = 86_400;
/// Default planner LLM budget USD (opts; not a validate hard limit).
pub const PLANNER_MAX_BUDGET_USD: f64 = 2.0;

// ── P2-1 role=inspect defaults ───────────────────────────────────────
/// Default Claude tools for `role: inspect` (read + shell + write reports only).
/// No Edit/MultiEdit — inspect is a quality gate, not an implementer (N6).
pub const INSPECT_DEFAULT_ALLOWED_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Bash", "Write"];
/// Default writable whitelist when inspect omits `scope.paths`.
pub const INSPECT_DEFAULT_WRITE_SCOPE: &str = ".cco-out/inspect/**";
/// Marker injected into `provider_opts.append_system_prompt` (idempotent).
pub const INSPECT_SYSTEM_PROMPT_MARKER: &str = "CCO role=inspect:";
/// System-prompt segment for inspect workers (Claude append-system-prompt / host).
pub const INSPECT_SYSTEM_PROMPT: &str = "CCO role=inspect: terminal quality gate, not an implementer. Business tree is READ-ONLY. You may WRITE only under `.cco-out/inspect/**` (VERDICT.md, ISSUES.md, etc.). Do not edit application source to force a pass. On FAIL, document issues for a future rework wave; do not silently rework.";
/// Tools that mutate business source — stripped for inspect unless `allow_business_write`.
const INSPECT_STRIP_TOOLS: &[&str] = &["Edit", "MultiEdit", "NotebookEdit"];

/// Collaboration role for multi-CLI plans (P1-1).
///
/// Serialized as snake_case: `scout` | `implement` | `integrate` | `inspect`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    Scout,
    Implement,
    Integrate,
    Inspect,
}

/// Per-task path contract (P1-1). All globs relative to project/worktree root.
///
/// - `paths`: writable whitelist (implement/integrate should set)
/// - `readonly`: extra readable ranges (scout may leave empty = full project)
/// - `forbid`: hard deny list
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskScope {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub readonly: Vec<String>,
    #[serde(default)]
    pub forbid: Vec<String>,
}

/// Resolved plan host understands (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanIR {
    pub schema: String,
    pub name: String,
    pub adapter: String,
    pub source_path: PathBuf,
    pub max_parallel: usize,
    pub on_failure: OnFailure,
    pub retry_max: u32,
    pub default_provider: String,
    pub default_mode: String,
    pub worktree: bool,
    /// When true, later validate (P1-2) may require a terminal `role: inspect` task.
    /// Absent in old plans → false (serde default).
    #[serde(default)]
    pub require_inspect: bool,
    pub tasks: Vec<TaskIR>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskIR {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub group: Option<String>,
    pub provider: String,
    /// print | bg | auto
    pub mode: String,
    pub prompt: String,
    pub acceptance: Option<String>,
    pub timeout_secs: Option<u64>,
    pub worktree: Option<bool>,
    /// Opaque to host; validated by provider.
    pub provider_opts: serde_json::Value,
    /// When true, task is optional — confirm screen lets the user opt in/out.
    /// Title should contain 「（可选）」so the choice is obvious in lists.
    #[serde(default)]
    pub optional: bool,
    /// Whether to run this task. Missing field deserializes as false;
    /// `materialize_selected_tasks` forces required tasks on and drops
    /// unselected optional ones. Optional tasks stay off until the user checks.
    #[serde(default)]
    pub include: bool,
    /// Collaboration role (scout|implement|integrate|inspect). Optional for back-compat.
    #[serde(default)]
    pub role: Option<TaskRole>,
    /// Writable / readonly / forbid path globs. Optional for back-compat.
    #[serde(default)]
    pub scope: Option<TaskScope>,
    /// Required on-disk artifact paths after the task completes (relative to project).
    #[serde(default)]
    pub outputs: Vec<String>,
}

/// Ensure optional tasks have a clear title marker; required tasks stay as-is.
pub fn normalize_optional_title(title: &str, optional: bool) -> String {
    let t = title.trim();
    if !optional {
        return t.to_string();
    }
    let lower = t.to_ascii_lowercase();
    if t.contains("可选") || lower.contains("optional") {
        t.to_string()
    } else if t.is_empty() {
        "（可选）".into()
    } else {
        format!("{t}（可选）")
    }
}

/// Detect optional intent from a free-form title (planner / heading split).
pub fn title_looks_optional(title: &str) -> bool {
    let t = title.trim();
    let lower = t.to_ascii_lowercase();
    t.contains("可选") || lower.contains("optional") || lower.contains("(opt)")
}

/// True when a heading/title is document chrome — not a work package.
/// Used by Mode B planner (LLM reject + heuristic skip) so users who only
/// supply Markdown specs don't see Board / P0 / 修订历史 as runnable tasks.
pub fn title_is_meta_heading(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return true;
    }
    // Markdown table header / pipe row (e.g. "id | provider | role | …")
    let pipes = t.chars().filter(|c| *c == '|').count();
    if pipes >= 2 {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    let compact: String = lower
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '·' && *c != '•' && *c != '—' && *c != '-')
        .collect();

    // Bare structural labels (handoff template, TOC, plan chrome)
    const EXACT: &[&str] = &[
        "board",
        "timeline",
        "fragments",
        "graph",
        "tasks",
        "task",
        "toc",
        "目录",
        "overview",
        "summary",
        "notes",
        "readme",
        "前言",
        "全文",
        "附录",
        "附录a",
        "附录b",
        "appendix",
        "protocol",
        "一句话",
        "0.一句话",
    ];
    if EXACT.iter().any(|k| compact == *k || lower == *k) {
        return true;
    }

    // Phrase / prefix patterns common in product-spec Markdown (not work orders)
    const NEEDLES: &[&str] = &[
        "修订历史",
        "revision history",
        "非目标",
        "non-goal",
        "nongoal",
        "成功标准",
        "决策树",
        "决策默认",
        "开放确认",
        "关联真源",
        "代码锚点",
        "现状锚点",
        "现状分析",
        "产品结论",
        "协作契约",
        "阶段切分",
        "架构落点",
        "风险与决策",
        "附录",
        "appendix",
        // Bare handoff/board as substring would false-positive real work titles
        // like「实现 handoff 归并」— those are EXACT-only above.
        "instructions for next",
        "open risks",
        "protocol",
        "geb",
        "勾选",
        "p0 —",
        "p1 —",
        "p2 —",
        "p0-",
        "p1-",
        "p2-",
        "§",
        // Phase banners from product plans (title without leading P0)
        "协议与示例",
        "host 硬保障",
        "硬保障（代码）",
        "检验员与分配",
        "分配体验",
        "文档 / 示例",
        "文档/示例",
    ];
    if NEEDLES
        .iter()
        .any(|n| lower.contains(n) || compact.contains(&n.replace(' ', "")))
    {
        return true;
    }
    // "…（按需）" / "…(按需)" stage banners without a work verb
    if (lower.contains("按需") || lower.contains("可选增强"))
        && !["实现", "落地", "修复", "新增", "编写", "接入", "改造", "测试", "验收"]
            .iter()
            .any(|c| lower.contains(c))
    {
        return true;
    }

    // Leading "N. " / "N " section numbers that are pure catalog titles
    // e.g. "6. 阶段切分与勾选" already hit 阶段切分; also "12. 修订历史"
    if looks_like_numbered_catalog_title(&lower) {
        return true;
    }

    // Stage-only labels: "P0 协议与示例" without a verb-ish work cue
    if is_stage_catalog_title(&lower) {
        return true;
    }

    false
}

fn looks_like_numbered_catalog_title(lower: &str) -> bool {
    let t = lower.trim();
    // "0. 一句话" / "8. 非目标" / "10. 成功标准"
    let rest = if let Some(r) = t.strip_prefix(|c: char| c.is_ascii_digit()) {
        let r = r.trim_start_matches(|c: char| c.is_ascii_digit());
        r.strip_prefix('.').or_else(|| r.strip_prefix('、')).unwrap_or(r).trim()
    } else {
        return false;
    };
    const CATALOG: &[&str] = &[
        "一句话",
        "产品结论",
        "现状",
        "协作契约",
        "端到端",
        "计划与配置",
        "阶段",
        "架构",
        "非目标",
        "风险",
        "成功标准",
        "决策",
        "修订历史",
        "附录",
        "拍板",
        "分配策略",
        "档位",
        "勾选",
        "示例",
        "配置",
        "主路径",
        "契约",
        "落点",
        "总览",
        "决策树",
        "决策默认",
    ];
    CATALOG.iter().any(|c| rest.starts_with(c) || rest.contains(c))
}

fn is_stage_catalog_title(lower: &str) -> bool {
    let t = lower.trim();
    let stage = t.starts_with("p0")
        || t.starts_with("p1")
        || t.starts_with("p2")
        || t.starts_with("m0")
        || t.starts_with("m1")
        || t.starts_with("m2")
        || t.starts_with("m3")
        || t.starts_with("m4")
        || t.starts_with("m5")
        || t.starts_with("d0")
        || t.starts_with("d1")
        || t.starts_with("d2")
        || t.starts_with("d3")
        || t.starts_with("d4")
        || t.starts_with("d5");
    if !stage {
        return false;
    }
    // Allow real work titles like "P0 实现 handoff 归并" (has action-ish length + 实现)
    let work_cues = ["实现", "落地", "修复", "新增", "编写", "接入", "改造", "测试", "验收"];
    if work_cues.iter().any(|c| t.contains(c)) {
        return false;
    }
    // Short stage banners: "p0 — 协议与示例（文档 / 示例为主）"
    true
}

/// Drop unselected optional tasks and rewrite depends_on for execution.
pub fn materialize_selected_tasks(mut ir: PlanIR) -> Result<PlanIR> {
    for t in &mut ir.tasks {
        if !t.optional {
            t.include = true;
        }
        if t.optional {
            t.title = normalize_optional_title(&t.title, true);
        }
    }
    let drop: HashSet<String> = ir
        .tasks
        .iter()
        .filter(|t| !t.include)
        .map(|t| t.id.clone())
        .collect();
    ir.tasks.retain(|t| t.include);
    if ir.tasks.is_empty() {
        bail!("没有选中任何任务：请至少保留一项必选，或勾选可选项后再开始");
    }
    for t in &mut ir.tasks {
        t.depends_on.retain(|d| !drop.contains(d));
    }
    // Role defaults already applied at load_plan; re-apply is idempotent if IR
    // was built in-memory (planner / tests) without going through load_plan.
    materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}

/// Apply per-role default opts / scope after adapter parse (P2-1).
///
/// Currently only `role: inspect`:
/// - strip business-mutation tools (`Edit` / `MultiEdit` / `NotebookEdit`) unless
///   `provider_opts.allow_business_write: true`
/// - if `allowed_tools` empty/missing after strip → `INSPECT_DEFAULT_ALLOWED_TOOLS`
/// - ensure Write is present (VERDICT/ISSUES)
/// - empty `scope.paths` → `[.cco-out/inspect/**]`
/// - inject `INSPECT_SYSTEM_PROMPT` into `append_system_prompt` (idempotent)
///
/// Non-inspect roles and plans without `role` are untouched (legacy back-compat).
/// Call sites: [`load_plan`], [`materialize_selected_tasks`].
pub fn materialize_role_defaults(plan: &mut PlanIR) {
    for task in &mut plan.tasks {
        if task.role == Some(TaskRole::Inspect) {
            materialize_inspect_task(task);
        }
    }
}

fn materialize_inspect_task(task: &mut TaskIR) {
    let allow_business_write = task
        .provider_opts
        .get("allow_business_write")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ── allowed_tools (Claude / fake; codex ignores) ─────────────────
    if !allow_business_write {
        let tools = normalize_inspect_allowed_tools(task.provider_opts.get("allowed_tools"));
        task.provider_opts["allowed_tools"] = serde_json::json!(tools);
    }

    // ── scope.paths writable whitelist ───────────────────────────────
    let need_default_paths = match task.scope.as_ref() {
        None => true,
        Some(s) => s.paths.is_empty(),
    };
    if need_default_paths {
        let mut scope = task.scope.take().unwrap_or_default();
        scope.paths = vec![INSPECT_DEFAULT_WRITE_SCOPE.to_string()];
        task.scope = Some(scope);
    }

    // ── system prompt segment ────────────────────────────────────────
    inject_inspect_system_prompt(&mut task.provider_opts, allow_business_write);
}

fn normalize_inspect_allowed_tools(raw: Option<&serde_json::Value>) -> Vec<String> {
    let mut tools: Vec<String> = match raw {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .filter(|t| {
                !INSPECT_STRIP_TOOLS
                    .iter()
                    .any(|b| t.eq_ignore_ascii_case(b))
            })
            .collect(),
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|t| {
                !t.is_empty()
                    && !INSPECT_STRIP_TOOLS
                        .iter()
                        .any(|b| t.eq_ignore_ascii_case(b))
            })
            .collect(),
        _ => Vec::new(),
    };

    if tools.is_empty() {
        return INSPECT_DEFAULT_ALLOWED_TOOLS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }

    // Ensure report write + basic read are always available for the gate.
    for required in ["Read", "Write"] {
        if !tools.iter().any(|t| t.eq_ignore_ascii_case(required)) {
            tools.push(required.to_string());
        }
    }
    tools
}

fn inject_inspect_system_prompt(opts: &mut serde_json::Value, allow_business_write: bool) {
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(INSPECT_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let segment = if allow_business_write {
        format!(
            "{INSPECT_SYSTEM_PROMPT} Note: allow_business_write=true — prefer still writing only under `.cco-out/inspect/**`; do not silently rework features."
        )
    } else {
        INSPECT_SYSTEM_PROMPT.to_string()
    };
    let merged = if existing.trim().is_empty() {
        segment
    } else {
        format!("{existing}\n\n{segment}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    Pause,
    Continue,
    Retry,
}

impl Default for OnFailure {
    fn default() -> Self {
        Self::Pause
    }
}

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
        // ── 4. codex + mode=bg ───────────────────────────────────────────
        for t in &self.tasks {
            if t.provider.eq_ignore_ascii_case("codex") && t.mode.eq_ignore_ascii_case("bg") {
                bail!(
                    "task {}: codex does not support mode=bg (use print/exec)",
                    t.id
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
                    // inspect may only be followed by another inspect (chain);
                    // any business / unscoped role downstream = non-terminal.
                    match s.role {
                        Some(TaskRole::Inspect) => {}
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
            // At least one inspect must be a sink (no successors) — reachable end-gate.
            let has_sink = inspect_ids.iter().any(|id| {
                successors
                    .get(id)
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            });
            if !has_sink {
                bail!(
                    "require_inspect=true but no terminal inspect sink \
                     (every inspect has successors)"
                );
            }
        }

        Ok(())
    }
}

/// True when the DAG has at least two tasks that can be ready in the same wave
/// (neither is a transitive ancestor of the other).
fn dag_has_parallel_wave(tasks: &[TaskIR]) -> bool {
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
fn transitive_ancestors(tasks: &[TaskIR]) -> HashMap<String, HashSet<String>> {
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
fn scope_glob_prefix(glob: &str) -> Option<String> {
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
fn scope_paths_overlap(a: &str, b: &str) -> bool {
    match (scope_glob_prefix(a), scope_glob_prefix(b)) {
        (None, _) | (_, None) => true, // universal ∩ anything
        (Some(pa), Some(pb)) => {
            pa == pb || pa.starts_with(&format!("{pb}/")) || pb.starts_with(&format!("{pa}/"))
        }
    }
}

/// First overlapping path pair across two path lists, if any.
fn first_overlapping_paths<'a>(
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

/// True when the document is machine-structured and can skip AI planning.
///
/// - `cco-plan/v1` / `serial-prompts/v0` → structured (parse-only / skip-plan)
/// - `raw-single` → prose / unknown → default Mode B plan job
pub fn is_structured_adapter(adapter: &str) -> bool {
    matches!(adapter, "cco-plan/v1" | "serial-prompts/v0")
}

/// Peek adapter without full PlanIR parse (for CLI `run` routing).
pub fn peek_adapter(project_root: &Path, plan_path: &Path) -> Result<String> {
    let abs = resolve_plan_path(project_root, plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    Ok(detect_adapter(&abs, &text))
}

/// Detect adapter and load PlanIR.
pub fn load_plan(
    project_root: &Path,
    plan_path: &Path,
    adapter_hint: Option<&str>,
    config: &Config,
) -> Result<PlanIR> {
    let abs = resolve_plan_path(project_root, plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;

    let adapter = adapter_hint
        .map(|s| s.to_string())
        .unwrap_or_else(|| detect_adapter(&abs, &text));

    let mut plan = match adapter.as_str() {
        "cco-plan/v1" => adapters::cco_v1::parse(&abs, &text, config)?,
        "serial-prompts/v0" => adapters::serial_prompts::parse(&abs, &text, config)?,
        "raw-single" => adapters::raw_single::parse(&abs, &text, config)?,
        other => bail!("unknown adapter: {other}"),
    };
    plan.adapter = adapter;
    plan.source_path = abs;
    // P2-1: role=inspect default opts/scope before validate (require_inspect gate).
    materialize_role_defaults(&mut plan);
    plan.validate()?;
    Ok(plan)
}

pub fn resolve_plan_path(project_root: &Path, plan_path: &Path) -> Result<PathBuf> {
    let p = if plan_path.is_absolute() {
        plan_path.to_path_buf()
    } else {
        project_root.join(plan_path)
    };
    let canon = p
        .canonicalize()
        .with_context(|| format!("plan path not found: {}", p.display()))?;
    Ok(canon)
}

fn detect_adapter(path: &Path, text: &str) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let trimmed = text.trim_start();

    // YAML/JSON: only these are machine plan schemas (optional, not default).
    if matches!(ext.as_str(), "yaml" | "yml" | "json") {
        if text.contains("cco-plan/v1") || trimmed.starts_with("schema:") {
            return "cco-plan/v1".into();
        }
    }

    // Markdown with YAML frontmatter at the *start* only (not body examples).
    if trimmed.starts_with("---") {
        if let Some(rest) = trimmed.strip_prefix("---") {
            if let Some(end) = rest.find("\n---") {
                let front = &rest[..end];
                if front.contains("schema: cco-plan/v1") {
                    return "cco-plan/v1".into();
                }
            }
        }
    }

    // Pure yaml document starting with schema (no extension / .txt edge cases)
    if trimmed.starts_with("schema: cco-plan/v1") {
        return "cco-plan/v1".into();
    }

    // Default for documents: md (and anything else) is a plan *document*.
    // - multi-task prompt sections → serial-prompts/v0
    // - otherwise whole file is one prompt → raw-single
    // Never treat "schema: cco-plan/v1" appearing mid-document as schema.
    if text.contains("并行组")
        || text.contains("| id |")
        || (text.contains("## Tasks") && text.contains("### "))
    {
        return "serial-prompts/v0".into();
    }
    "raw-single".into()
}

/// List candidate plan files under project.
pub fn list_plans(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let candidates = [
        project_root.join("plans"), // chat-plan-builder 落盘优先目录
        project_root.join("docs/serial-plans"),
        project_root.join("docs/plans"),
        project_root.join("docs"),
        project_root.join(".cco"),
    ];
    for dir in candidates {
        if !dir.is_dir() {
            continue;
        }
        for entry in walkdir_shallow(&dir, 3)? {
            let name = entry
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.ends_with(".md")
                || name.ends_with(".yaml")
                || name.ends_with(".yml")
                || name == "plan.md"
            {
                // plans/ 下全部 .md 都算计划；其它目录仍要求文件名含 plan/prompt
                let under_plans = dir.ends_with("plans") || dir.ends_with("serial-plans");
                if name.contains("plan")
                    || name.contains("prompt")
                    || name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || under_plans
                {
                    out.push(entry);
                }
            }
        }
    }
    // 项目根 cco-plan-*.md（无 plans/ 时聊天落盘）
    if let Ok(rd) = std::fs::read_dir(project_root) {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_file() {
                continue;
            }
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if name.starts_with("cco-plan-") && name.ends_with(".md") {
                out.push(p);
            }
        }
    }
    out.sort();
    out.dedup();
    // Prefer markdown plan documents first; yaml/json are optional structured forms.
    out.sort_by(|a, b| {
        let rank = |p: &Path| {
            match p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "md" => 0,
                "yaml" | "yml" => 1,
                "json" => 2,
                _ => 3,
            }
        };
        rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
    });
    Ok(out)
}

fn walkdir_shallow(root: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    fn rec(dir: &Path, depth: usize, max: usize, out: &mut Vec<PathBuf>) -> Result<()> {
        if depth > max {
            return Ok(());
        }
        for ent in std::fs::read_dir(dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.is_dir() {
                rec(&p, depth + 1, max, out)?;
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    rec(root, 1, max_depth, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
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
}
