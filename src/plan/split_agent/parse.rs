//! Parse model text into CcoSplitJob (cco-split/v1).
//!
//! [INPUT]: model raw text · SplitRequest metadata
//! [OUTPUT]: CcoSplitJob after soft_accept
//! [POS]: plan/split_agent
//! [PROTOCOL]: 失败返回人话 Err；不静默空壳成功

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::domain::plan::{
    soft_accept_split, CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask, CcoTaskKind,
    CcoTaskStatus, CCO_SPLIT_SCHEMA, PLANNER_MAX_TASKS,
};
use crate::ports::split_agent::SplitRequest;

#[derive(Debug, Deserialize)]
struct AgentDoc {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    max_parallel: Option<usize>,
    #[serde(default)]
    tasks: Vec<AgentTask>,
}

#[derive(Debug, Deserialize)]
struct AgentTask {
    #[serde(default, alias = "task_id")]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    done_when: Option<String>,
    #[serde(default)]
    plan_ref: Option<String>,
    #[serde(default)]
    can_parallel: Option<bool>,
}

/// Extract JSON object from raw model output (fence or bare).
pub fn extract_json_object(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.is_empty() {
        bail!("拆分 Agent 输出为空");
    }
    // ```json ... ``` or ``` ... ```
    if let Some(start) = t.find("```") {
        let after = &t[start + 3..];
        let after = after
            .strip_prefix("json")
            .or_else(|| after.strip_prefix("JSON"))
            .unwrap_or(after);
        let after = after.trim_start_matches(|c: char| c == '\r' || c == '\n' || c == ' ');
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if block.starts_with('{') {
                return Ok(block.to_string());
            }
        }
    }
    // First balanced { ... }
    if let Some(i) = t.find('{') {
        if let Some(slice) = balanced_object(&t[i..]) {
            return Ok(slice.to_string());
        }
    }
    bail!("拆分 Agent 输出中找不到 JSON 对象")
}

fn balanced_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse agent text into a soft-accepted CcoSplitJob.
pub fn parse_agent_output(raw: &str, req: &SplitRequest) -> Result<CcoSplitJob> {
    let json = extract_json_object(raw)?;
    let doc: AgentDoc =
        serde_json::from_str(&json).with_context(|| "解析 cco-split JSON 失败")?;

    if let Some(s) = doc.schema.as_deref() {
        let s = s.trim();
        if !s.is_empty() && s != CCO_SPLIT_SCHEMA && s != "cco-split/v1" {
            bail!("不支持的 schema: {s}（需要 {CCO_SPLIT_SCHEMA}）");
        }
    }
    if doc.tasks.is_empty() {
        bail!("拆分 Agent 未返回任何任务");
    }
    if doc.tasks.len() > PLANNER_MAX_TASKS {
        // soft_accept also caps; pre-warn in err path only if absurd
    }

    let title = doc
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            req.plan_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("plan")
                .to_string()
        });
    let max_parallel = doc
        .max_parallel
        .unwrap_or(req.max_parallel)
        .clamp(1, 32);

    let mut tasks = Vec::with_capacity(doc.tasks.len());
    for (i, t) in doc.tasks.into_iter().enumerate() {
        let task_id = t
            .id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("t{}", i + 1));
        let title = t
            .title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("步骤 {}", i + 1));
        let body = t
            .body
            .map(|s| s.trim_end().to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| t.summary.clone().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| title.clone());
        let summary = t
            .summary
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let optional = t.optional;
        let enabled = if optional {
            t.enabled.unwrap_or(false)
        } else {
            t.enabled.unwrap_or(true)
        };
        let kind = CcoTaskKind::parse(t.kind.as_deref().unwrap_or("do"));
        let mut meta = serde_json::Map::new();
        if let Some(cp) = t.can_parallel {
            meta.insert("can_parallel".into(), serde_json::json!(cp));
        }
        tasks.push(CcoSplitTask {
            task_id,
            ord: i as i32,
            title,
            summary,
            body,
            depends_on: t.depends_on,
            wave: 0,
            enabled,
            optional,
            done_when: t.done_when.filter(|s| !s.trim().is_empty()),
            plan_ref: t.plan_ref.filter(|s| !s.trim().is_empty()),
            kind,
            status: CcoTaskStatus::Pending,
            provider: None,
            role: None,
            scope_paths: vec![],
            meta_json: if meta.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(meta))
            },
        });
    }

    let mut job = CcoSplitJob {
        job_id: req.job_id.clone(),
        project: req.project.clone(),
        plan_path: req.plan_path.clone(),
        status: CcoSplitStatus::Ready,
        title,
        max_parallel,
        source: CcoSplitSource::Llm,
        error: None,
        run_id: None,
        created_at: req.created_at.clone(),
        updated_at: req.updated_at.clone(),
        tasks,
    };
    let notes = soft_accept_split(&mut job);
    if job.tasks.is_empty() {
        bail!("soft_accept 后任务为空");
    }
    let _ = notes; // caller may log
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn req() -> SplitRequest {
        SplitRequest {
            job_id: "job-1".into(),
            project: PathBuf::from("/p"),
            plan_path: PathBuf::from("docs/x.md"),
            plan_abs: PathBuf::from("/p/docs/x.md"),
            plan_md: "# x".into(),
            max_parallel: 2,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        }
    }

    #[test]
    fn parse_fenced_json() {
        let raw = r#"
Here is the plan:
```json
{
  "schema": "cco-split/v1",
  "title": "demo",
  "max_parallel": 2,
  "tasks": [
    {"id": "a", "title": "做 A", "body": "完成 A", "depends_on": [], "optional": false, "enabled": true, "kind": "do"},
    {"id": "b", "title": "做 B", "summary": "B 一步", "depends_on": ["a"], "optional": true, "kind": "check", "can_parallel": false}
  ]
}
```
"#;
        let job = parse_agent_output(raw, &req()).unwrap();
        assert_eq!(job.tasks.len(), 2);
        assert_eq!(job.tasks[0].task_id, "a");
        assert_eq!(job.tasks[1].wave, 1);
        assert!(job.tasks[1].optional);
        assert!(!job.tasks[1].enabled);
        assert_eq!(job.source, CcoSplitSource::Llm);
    }

    #[test]
    fn parse_bare_json() {
        let raw = r#"{"schema":"cco-split/v1","title":"t","tasks":[{"id":"t1","title":"一步","body":"做"}]} "#;
        let job = parse_agent_output(raw, &req()).unwrap();
        assert_eq!(job.tasks.len(), 1);
        assert_eq!(job.tasks[0].title, "一步");
    }

    #[test]
    fn empty_tasks_err() {
        let raw = r#"{"schema":"cco-split/v1","tasks":[]}"#;
        assert!(parse_agent_output(raw, &req()).is_err());
    }
}
