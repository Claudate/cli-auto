//! cco-plan/v1 YAML adapter.
//!
//! [INPUT]: YAML/JSON 计划文本（schema: cco-plan/v1）
//! [OUTPUT]: PlanIR（含 depends_on / max_parallel / provider_opts / role / scope / outputs / require_inspect）
//! [POS]: 结构化计划首选适配器
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/adapters/CLAUDE.md

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::config::Config;
use crate::plan::adapters::raw_single::default_provider_opts;
use crate::plan::{OnFailure, PlanIR, TaskIR, TaskRole, TaskScope};

#[derive(Debug, Deserialize)]
struct FilePlan {
    #[serde(default = "default_schema")]
    schema: String,
    name: String,
    #[serde(default)]
    defaults: Defaults,
    #[serde(default)]
    groups: Vec<Group>,
    tasks: Vec<FileTask>,
    #[serde(default)]
    max_parallel: Option<usize>,
    #[serde(default)]
    on_failure: Option<String>,
    #[serde(default)]
    retry_max: Option<u32>,
    #[serde(default)]
    worktree: Option<bool>,
    #[serde(default)]
    default_provider: Option<String>,
    #[serde(default)]
    default_mode: Option<String>,
    #[serde(default)]
    providers: Option<serde_yaml::Value>,
    /// Plan-level flag: later validate may require a terminal inspect task.
    #[serde(default)]
    require_inspect: bool,
}

fn default_schema() -> String {
    "cco-plan/v1".into()
}

#[derive(Debug, Default, Deserialize)]
struct Defaults {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    worktree: Option<bool>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    max_turns: Option<u32>,
    #[serde(default)]
    max_budget_usd: Option<f64>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    providers: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct Group {
    id: String,
    #[serde(default)]
    tasks: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FileTask {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    prompt_file: Option<String>,
    #[serde(default)]
    acceptance: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    worktree: Option<bool>,
    #[serde(default)]
    provider_opts: Option<serde_yaml::Value>,
    /// Optional tasks require user opt-in on the confirm screen.
    #[serde(default)]
    optional: bool,
    /// Include in run; optional defaults to false when omitted.
    #[serde(default)]
    include: Option<bool>,
    /// Collaboration role (scout|implement|integrate|inspect). Absent → None.
    #[serde(default)]
    role: Option<TaskRole>,
    /// Path contract; absent or empty object both OK for old plans.
    #[serde(default)]
    scope: Option<TaskScope>,
    /// Required on-disk artifact paths (relative). Absent → empty.
    #[serde(default)]
    outputs: Vec<String>,
    /// Free-form tags for L1 routing (P2-4). Absent → empty.
    #[serde(default)]
    tags: Vec<String>,
}

pub fn parse(path: &Path, text: &str, config: &Config) -> Result<PlanIR> {
    let yaml_text = extract_yaml(text)?;
    let file: FilePlan = serde_yaml::from_str(&yaml_text).context("parse cco-plan/v1 yaml")?;
    if file.schema != "cco-plan/v1" && !file.schema.is_empty() {
        // allow missing via default
        if file.schema != "cco-plan/v1" {
            bail!("unsupported schema: {}", file.schema);
        }
    }

    let default_provider = file
        .default_provider
        .or(file.defaults.provider.clone())
        .unwrap_or_else(|| config.default.default_provider.clone());
    let default_mode = file
        .default_mode
        .or(file.defaults.mode.clone())
        .unwrap_or_else(|| config.default.default_mode.clone());
    let worktree = file
        .worktree
        .or(file.defaults.worktree)
        .unwrap_or(config.default.worktree);
    let max_parallel = file.max_parallel.unwrap_or(config.default.max_parallel);
    let on_failure = match file.on_failure.as_deref().unwrap_or("pause") {
        "continue" => OnFailure::Continue,
        "retry" => OnFailure::Retry,
        _ => OnFailure::Pause,
    };

    // group → task membership + group depends (flatten to task deps for M0/M1)
    let mut group_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut group_tasks: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for g in &file.groups {
        for tid in &g.tasks {
            group_of.insert(tid.clone(), g.id.clone());
            group_tasks.entry(g.id.clone()).or_default().push(tid.clone());
        }
    }
    // map group depends → task depends_on for tasks in dependent groups
    let mut extra_deps: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for g in &file.groups {
        if g.depends_on.is_empty() {
            continue;
        }
        let mut upstream_tasks = Vec::new();
        for ug in &g.depends_on {
            if let Some(ts) = group_tasks.get(ug) {
                upstream_tasks.extend(ts.iter().cloned());
            }
        }
        for tid in &g.tasks {
            extra_deps
                .entry(tid.clone())
                .or_default()
                .extend(upstream_tasks.clone());
        }
    }

    let plan_dir = path.parent().unwrap_or(Path::new("."));
    let mut tasks = Vec::new();

    for ft in &file.tasks {
        let provider = ft
            .provider
            .clone()
            .unwrap_or_else(|| default_provider.clone());
        let mode = ft.mode.clone().unwrap_or_else(|| default_mode.clone());
        let mut prompt = ft.prompt.clone().unwrap_or_default();
        if prompt.is_empty() {
            if let Some(pf) = &ft.prompt_file {
                let pp = plan_dir.join(pf);
                prompt = std::fs::read_to_string(&pp)
                    .with_context(|| format!("read prompt_file {}", pp.display()))?;
            }
        }
        if prompt.trim().is_empty() {
            bail!("task {} missing prompt / prompt_file", ft.id);
        }

        let mut depends = ft.depends_on.clone();
        if let Some(extra) = extra_deps.get(&ft.id) {
            for d in extra {
                if !depends.contains(d) {
                    depends.push(d.clone());
                }
            }
        }

        let group = ft
            .group
            .clone()
            .or_else(|| group_of.get(&ft.id).cloned());

        let mut opts = default_provider_opts(config, &provider);
        // merge defaults.providers.<name> and top-level providers and task opts
        merge_provider_bucket(&mut opts, file.providers.as_ref(), &provider);
        merge_provider_bucket(&mut opts, file.defaults.providers.as_ref(), &provider);
        // scalar defaults for claude-like
        if let Some(v) = file.defaults.max_turns {
            opts["max_turns"] = serde_json::json!(v);
        }
        if let Some(v) = file.defaults.max_budget_usd {
            opts["max_budget_usd"] = serde_json::json!(v);
        }
        if let Some(v) = &file.defaults.permission_mode {
            opts["permission_mode"] = serde_json::json!(v);
        }
        if let Some(v) = &file.defaults.allowed_tools {
            opts["allowed_tools"] = serde_json::json!(v);
        }
        if let Some(po) = &ft.provider_opts {
            merge_json(&mut opts, &yaml_to_json(po)?);
        }

        let raw_title = ft.title.clone().unwrap_or_else(|| ft.id.clone());
        let optional = ft.optional || crate::plan::title_looks_optional(&raw_title);
        let title = crate::plan::normalize_optional_title(&raw_title, optional);
        let include = ft.include.unwrap_or(!optional);
        // Empty scope object (all vecs empty) → treat as absent for cleaner IR.
        let scope = ft.scope.clone().and_then(|s| {
            if s.paths.is_empty() && s.readonly.is_empty() && s.forbid.is_empty() {
                None
            } else {
                Some(s)
            }
        });
        tasks.push(TaskIR {
            id: ft.id.clone(),
            title,
            depends_on: depends,
            group,
            provider,
            mode,
            prompt,
            acceptance: ft.acceptance.clone(),
            timeout_secs: ft.timeout_secs.or(file.defaults.timeout_secs),
            worktree: ft.worktree.or(Some(worktree)),
            provider_opts: opts,
            optional,
            include: if optional { include } else { true },
            role: ft.role,
            scope,
            outputs: ft.outputs.clone(),
            tags: ft.tags.clone(),
        });
    }

    Ok(PlanIR {
        schema: "cco-plan/v1".into(),
        name: file.name,
        adapter: "cco-plan/v1".into(),
        source_path: path.to_path_buf(),
        max_parallel,
        on_failure,
        retry_max: file.retry_max.unwrap_or(0),
        default_provider,
        default_mode,
        worktree,
        require_inspect: file.require_inspect,
        tasks,
    })
}

fn extract_yaml(text: &str) -> Result<String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with("---") {
        // frontmatter
        let rest = &trimmed[3..];
        if let Some(end) = rest.find("\n---") {
            return Ok(rest[..end].to_string());
        }
        bail!("unclosed YAML frontmatter");
    }
    // pure yaml/json
    Ok(text.to_string())
}

fn yaml_to_json(v: &serde_yaml::Value) -> Result<serde_json::Value> {
    Ok(simple_yaml_json(v))
}

fn simple_yaml_json(v: &serde_yaml::Value) -> serde_json::Value {
    match v {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::json!(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::json!(i)
            } else if let Some(u) = n.as_u64() {
                serde_json::json!(u)
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::json!(s),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(simple_yaml_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                obj.insert(key, simple_yaml_json(val));
            }
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(t) => simple_yaml_json(&t.value),
    }
}

fn merge_provider_bucket(
    opts: &mut serde_json::Value,
    bucket: Option<&serde_yaml::Value>,
    provider: &str,
) {
    let Some(bucket) = bucket else { return };
    if let serde_yaml::Value::Mapping(map) = bucket {
        for (k, v) in map {
            let key = k.as_str().unwrap_or("");
            if key == provider {
                merge_json(opts, &simple_yaml_json(v));
            }
        }
    }
}

fn merge_json(dst: &mut serde_json::Value, src: &serde_json::Value) {
    match (dst, src) {
        (serde_json::Value::Object(d), serde_json::Value::Object(s)) => {
            for (k, v) in s {
                let entry = d.entry(k.clone()).or_insert(serde_json::Value::Null);
                if entry.is_object() && v.is_object() {
                    merge_json(entry, v);
                } else {
                    *entry = v.clone();
                }
            }
        }
        (d, s) => *d = s.clone(),
    }
}
