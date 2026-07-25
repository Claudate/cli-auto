//! Parse model text into CcoSplitJob (cco-split/v1).
//!
//! [INPUT]: model raw text · SplitRequest metadata
//! [OUTPUT]: CcoSplitJob after soft_accept
//! [POS]: plan/split_agent
//! [PROTOCOL]: 失败返回人话 Err；不静默空壳成功

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde::Deserializer;

use super::extract::extract_json_object;
use crate::domain::plan::{
    soft_accept_split, CcoSplitJob, CcoSplitSource, CcoSplitStatus, CcoSplitTask, CcoTaskKind,
    CcoTaskStatus, CCO_SPLIT_SCHEMA, PLANNER_MAX_TASKS,
};
use crate::ports::split_agent::SplitRequest;

/// LLM often emits string fields as arrays (or the reverse). Coerce both shapes
/// so `invalid type: sequence, expected a string` does not kill a whole split.
fn deserialize_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(value_to_opt_string(&v))
}

fn deserialize_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(value_to_string_vec(&v))
}

fn value_to_opt_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|x| match x {
                    serde_json::Value::String(s) => {
                        let t = s.trim();
                        if t.is_empty() {
                            None
                        } else {
                            Some(t.to_string())
                        }
                    }
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    serde_json::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                // Join checklist-like arrays into one done_when / body block.
                Some(parts.join("\n"))
            }
        }
        serde_json::Value::Object(_) => {
            // Refuse opaque objects as free text (too lossy).
            None
        }
    }
}

fn value_to_string_vec(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::Null => vec![],
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                vec![]
            } else if t.contains(',') {
                // "a, b, c" → list (common LLM slip for depends_on)
                t.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            } else {
                vec![t.to_string()]
            }
        }
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|x| match x {
                serde_json::Value::String(s) => {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                }
                serde_json::Value::Number(n) => Some(n.to_string()),
                // Nested array: flatten one level (["t1", ["t2"]] rare but seen)
                serde_json::Value::Array(inner) => {
                    let joined: Vec<String> = inner
                        .iter()
                        .filter_map(|y| y.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .collect();
                    if joined.is_empty() {
                        None
                    } else {
                        Some(joined.join(","))
                    }
                }
                _ => None,
            })
            .flat_map(|s| {
                if s.contains(',') && !s.contains('/') && !s.contains('.') {
                    // depends_on "t1,t2" flattened earlier as single string with comma
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect::<Vec<_>>()
                } else {
                    vec![s]
                }
            })
            .collect(),
        serde_json::Value::Number(n) => vec![n.to_string()],
        serde_json::Value::Bool(b) => vec![b.to_string()],
        serde_json::Value::Object(_) => vec![],
    }
}

#[derive(Debug, Deserialize)]
struct AgentDoc {
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    schema: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    title: Option<String>,
    #[serde(default)]
    max_parallel: Option<usize>,
    #[serde(default)]
    tasks: Vec<AgentTask>,
}

#[derive(Debug, Deserialize)]
struct AgentTask {
    #[serde(
        default,
        alias = "task_id",
        deserialize_with = "deserialize_opt_string"
    )]
    id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    summary: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    body: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_vec")]
    depends_on: Vec<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    done_when: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    verify_cmd: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string")]
    plan_ref: Option<String>,
    #[serde(default)]
    can_parallel: Option<bool>,
    /// File ownership for parallel waves (Q2).
    #[serde(
        default,
        alias = "scope",
        deserialize_with = "deserialize_string_vec"
    )]
    scope_paths: Vec<String>,
}

/// Trim · drop empty · dedupe · collapse `.`/`..` · reject absolute / drive paths.
fn normalize_scope_paths(raw: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for s in raw {
        let mut p = s.trim().replace('\\', "/");
        while p.starts_with("./") {
            p = p[2..].to_string();
        }
        p = p.trim_matches('/').to_string();
        if p.is_empty() {
            continue;
        }
        // Absolute / drive / home — too broad or non-repo; drop.
        if p.starts_with('/')
            || p.starts_with('~')
            || (p.len() >= 2 && p.as_bytes()[1] == b':' && p.as_bytes()[0].is_ascii_alphabetic())
        {
            continue;
        }
        // Collapse `.` / `..` (drop path if `..` escapes repo root).
        let mut stack: Vec<&str> = Vec::new();
        let mut escaped = false;
        for seg in p.split('/') {
            if seg.is_empty() || seg == "." {
                continue;
            }
            if seg == ".." {
                if stack.pop().is_none() {
                    escaped = true;
                    break;
                }
                continue;
            }
            stack.push(seg);
        }
        if escaped || stack.is_empty() {
            continue;
        }
        let p = stack.join("/");
        if !out.iter().any(|x: &String| x == &p) {
            out.push(p);
        }
    }
    out
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
        let scope_paths = normalize_scope_paths(t.scope_paths);
        // Humanize title/summary (never worker noise from model slip).
        let title = crate::domain::plan::cco_split::display_title(&title);
        let summary = if summary.is_empty()
            || crate::domain::plan::cco_split::is_worker_noise_line(&summary)
        {
            crate::domain::plan::cco_split::human_summary(
                &title,
                &body,
                t.done_when.as_deref(),
            )
        } else {
            summary
        };
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
            verify_cmd: t
                .verify_cmd
                .filter(|s| crate::domain::plan::is_runnable_verify(s)),
            plan_ref: t.plan_ref.filter(|s| !s.trim().is_empty()),
            kind,
            status: CcoTaskStatus::Pending,
            provider: None,
            role: None,
            scope_paths,
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
            grain_hint: None,
            effort: None,
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
    {"id": "a", "title": "做 A", "body": "完成 A", "depends_on": [], "optional": false, "enabled": true, "kind": "do", "scope_paths": ["web/a.js"]},
    {"id": "b", "title": "做 B", "summary": "B 一步", "depends_on": ["a"], "optional": true, "kind": "check", "can_parallel": false, "scope_paths": ["web/b.js"]}
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
        assert_eq!(job.tasks[0].scope_paths, vec!["web/a.js".to_string()]);
        assert_eq!(job.tasks[1].scope_paths, vec!["web/b.js".to_string()]);
    }

    #[test]
    fn parse_bare_json() {
        let raw = r#"{"schema":"cco-split/v1","title":"t","tasks":[{"id":"t1","title":"一步","body":"做"}]} "#;
        let job = parse_agent_output(raw, &req()).unwrap();
        assert_eq!(job.tasks.len(), 1);
        assert_eq!(job.tasks[0].title, "一步");
    }

    #[test]
    fn parse_pretty_multiline_bare_json() {
        // First line alone is incomplete (`"tasks":[`); must use balanced_object on full text.
        let raw = r#"{"schema":"cco-split/v1","title":"T","tasks":[
          {"id":"t1","title":"写入口","body":"实现 main","depends_on":[],"kind":"do"},
          {"id":"t2","title":"补测","body":"单测","depends_on":["t1"],"kind":"check"}
        ]}"#;
        let job = parse_agent_output(raw, &req()).unwrap();
        assert_eq!(job.tasks.len(), 2);
        assert_eq!(job.tasks[1].depends_on, vec!["t1".to_string()]);
    }

    #[test]
    fn empty_tasks_err() {
        let raw = r#"{"schema":"cco-split/v1","tasks":[]}"#;
        assert!(parse_agent_output(raw, &req()).is_err());
    }

    /// 2026-07-24: model emitted string fields as arrays → hard fail entire split.
    /// Coerce sequence→string and string→list so desk can accept the graph.
    #[test]
    fn coerce_string_fields_from_arrays() {
        let raw = r#"{
          "schema": "cco-split/v1",
          "title": "demo",
          "tasks": [
            {
              "id": "t1",
              "title": "做 A",
              "body": ["完成 A 的实现", "写验收说明"],
              "done_when": ["A 可编译", "有单测"],
              "depends_on": "ghost-missing",
              "scope_paths": "web/a.js",
              "kind": "do"
            },
            {
              "id": "t2",
              "title": "做 B",
              "body": "完成 B",
              "depends_on": "t1, ghost-missing",
              "scope_paths": ["web/b.js", "web/c.js"],
              "kind": "check"
            }
          ]
        }"#;
        let job = parse_agent_output(raw, &req()).expect("coerced parse");
        assert_eq!(job.tasks.len(), 2);
        assert!(job.tasks[0].body.contains("完成 A"));
        assert!(
            job.tasks[0]
                .done_when
                .as_deref()
                .unwrap_or("")
                .contains("可编译"),
            "done_when array joined"
        );
        // ghost edges pruned by soft_accept; string depends_on still accepted as list
        assert!(job.tasks[0].depends_on.is_empty(), "missing deps pruned");
        assert_eq!(job.tasks[0].scope_paths, vec!["web/a.js".to_string()]);
        // string "t1, ghost-missing" → [t1, ghost] then ghost pruned → [t1]
        assert_eq!(job.tasks[1].depends_on, vec!["t1".to_string()]);
        assert_eq!(job.tasks[1].scope_paths.len(), 2);
    }

    #[test]
    fn fixture_dual_audience_parses_with_scope_and_parallel() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/cco_split/dual_audience_pilotdeck_sample.json"
        );
        let raw = std::fs::read_to_string(path).expect("fixture");
        let job = parse_agent_output(&raw, &req()).unwrap();
        assert_eq!(job.tasks.len(), 4);
        assert!(job.tasks[0].scope_paths.iter().any(|p| p.contains("result")));
        assert!(job.tasks[0].depends_on.is_empty());
        assert!(job.tasks[1].depends_on.is_empty());
        // t1∥t2 → wave 0 both; t3 waits t2
        assert_eq!(job.tasks[0].wave, 0);
        assert_eq!(job.tasks[1].wave, 0);
        assert!(job.tasks[2].depends_on.iter().any(|d| d == "t2"));
        assert!(!job.tasks[0].summary.contains("worker"));
    }

    #[test]
    fn fixture_shell_chrome_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/cco_split/shell_chrome_sample.json"
        );
        let raw = std::fs::read_to_string(path).expect("fixture");
        let job = parse_agent_output(&raw, &req()).unwrap();
        // W1-4: ≥6 steps, a4 depends a3, scopes non-empty, independent work not force-chained.
        assert!(
            job.tasks.len() >= 6,
            "shell-chrome golden needs ≥6 tasks, got {}",
            job.tasks.len()
        );
        let a1 = job.tasks.iter().find(|t| t.task_id == "a1").unwrap();
        let a2 = job.tasks.iter().find(|t| t.task_id == "a2").unwrap();
        let a3 = job.tasks.iter().find(|t| t.task_id == "a3").unwrap();
        let a4 = job.tasks.iter().find(|t| t.task_id == "a4").unwrap();
        let a5 = job.tasks.iter().find(|t| t.task_id == "a5").unwrap();
        let b1 = job.tasks.iter().find(|t| t.task_id == "b1").unwrap();
        assert!(a4.depends_on.iter().any(|d| d == "a3"));
        for t in [&a1, &a2, &a3, &a4, &a5, &b1] {
            assert!(
                !t.scope_paths.is_empty(),
                "{} scope_paths empty",
                t.task_id
            );
        }
        // W1-4: independent work packages must not chain each other.
        // (a1 may serialize with a4 via shared index.html — a4 is outside this set.)
        let independent = ["a1", "a2", "a5", "b1"];
        for &id in &independent {
            let t = job.tasks.iter().find(|t| t.task_id == id).unwrap();
            for &other in &independent {
                if other == id {
                    continue;
                }
                assert!(
                    !t.depends_on.iter().any(|d| d == other),
                    "{id} must not depend on {other}, got {:?}",
                    t.depends_on
                );
            }
        }
        // a4 wave after a3
        assert!(a4.wave > a3.wave, "a4 wave {} must be after a3 {}", a4.wave, a3.wave);
        // body keeps work-order labels (no worker scaffold on golden)
        assert!(a1.body.contains("【做什么】") || a1.body.contains("去掉顶栏"));
    }

    #[test]
    fn scope_overlap_serializes_in_soft_accept() {
        let raw = r#"{
  "schema":"cco-split/v1","title":"ov","max_parallel":2,
  "tasks":[
    {"id":"x","title":"改同一文件 A","body":"a","depends_on":[],"scope_paths":["web/index.html"]},
    {"id":"y","title":"改同一文件 B","body":"b","depends_on":[],"scope_paths":["web/index.html"]}
  ]
}"#;
        let job = parse_agent_output(raw, &req()).unwrap();
        let x = job.tasks.iter().find(|t| t.task_id == "x").unwrap();
        let y = job.tasks.iter().find(|t| t.task_id == "y").unwrap();
        assert!(
            y.depends_on.iter().any(|d| d == "x"),
            "overlapping scope must serialize y after x, got {:?}",
            y.depends_on
        );
        assert_ne!(
            x.wave, y.wave,
            "same-file tasks must not share a wave after soft_accept"
        );
    }

    #[test]
    fn normalize_drops_absolute_and_dotdot_scope() {
        let raw = r#"{
  "schema":"cco-split/v1","title":"n","tasks":[
    {"id":"t1","title":"一步","body":"做","scope_paths":[
      "/etc/passwd","web/js/foo.js","../secrets","web/js/../js/bar.js","","  web/js/foo.js  "
    ]}
  ]
}"#;
        let job = parse_agent_output(raw, &req()).unwrap();
        let paths = &job.tasks[0].scope_paths;
        assert!(paths.iter().all(|p| !p.starts_with('/')));
        assert!(paths.iter().all(|p| !p.contains("..")));
        assert!(paths.iter().any(|p| p == "web/js/foo.js"));
        // web/js/../js/bar.js → web/js/bar.js
        assert!(
            paths.iter().any(|p| p == "web/js/bar.js"),
            "collapsed bar path, got {paths:?}"
        );
        // ../secrets escapes → dropped; foo.js deduped
        assert_eq!(
            paths.iter().filter(|p| *p == "web/js/foo.js").count(),
            1
        );
        assert!(!paths.iter().any(|p| p.contains("secrets")));
    }

    #[test]
    fn stream_json_result_envelope_not_first_brace() {
        // Repro: ModelSplitAgent CLI stdout is NDJSON; plan is only in type=result.
        // Old extract took first `{` (system event) → "未返回任何任务".
        let result_body = r#"```json
{
  "schema": "cco-split/v1",
  "title": "demo",
  "max_parallel": 2,
  "tasks": [
    {"id": "p0-1", "title": "结果台费用", "body": "做费用", "depends_on": [], "scope_paths": ["web/js/features/result/**"]},
    {"id": "p0-2", "title": "report 标题", "body": "做人话标题", "depends_on": [], "scope_paths": ["src/report/**"]}
  ]
}
```"#;
        let escaped = serde_json::to_string(result_body).unwrap();
        let raw = format!(
            r#"{{"type":"system","subtype":"init","session_id":"x"}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"thinking..."}}]}}}}
{{"type":"result","subtype":"success","is_error":false,"result":{escaped}}}
"#
        );
        let job = parse_agent_output(&raw, &req()).expect("must parse stream-json result");
        assert_eq!(job.tasks.len(), 2);
        assert_eq!(job.tasks[0].task_id, "p0-1");
        assert!(job.tasks[0]
            .scope_paths
            .iter()
            .any(|p| p.contains("result")));
    }
}
