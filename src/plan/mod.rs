//! Plan loading, adapters, and PlanIR.

pub mod adapters;
pub mod planner;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;

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
        Ok(())
    }
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
                if name.contains("plan")
                    || name.contains("prompt")
                    || name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || dir.ends_with("serial-plans")
                {
                    out.push(entry);
                }
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
}
