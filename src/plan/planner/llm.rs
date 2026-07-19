//! LLM planner path: Claude CLI print + stream-json JSON extraction.
//!
//! [INPUT]: PlanJob · Config · ClaudeProvider
//! [OUTPUT]: build_llm_plan · parse_llm_plan_output · read_planner_cost
//! [POS]: planner 子模块；ai mode 首选路径
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::config::Config;
use crate::plan::adapters::raw_single::default_provider_opts;
use crate::plan::{OnFailure, PlanIR, TaskIR, MAX_TASKS, PLANNER_MAX_BUDGET_USD};

use super::job::{append_log, apply_worker_defaults, job_dir, PlanJob};

/// Call Claude CLI (print) to produce a cco-plan/v1 JSON task graph.
pub(super) fn build_llm_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    use crate::runtime::provider::{
        claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
    };

    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let source_text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    // Cap source size for prompt budget
    let source_text = if source_text.len() > 40_000 {
        format!(
            "{}…\n\n[truncated, {} bytes total]",
            &source_text[..40_000],
            source_text.len()
        )
    } else {
        source_text
    };

    // GUI/.app 往往没有 shell PATH：必须走与 ProviderRegistry 相同的解析。
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin.clone(), extra);
    append_log(
        config,
        &job.job_id,
        &format!("planner CLI bin = {bin}"),
    );

    let work = job_dir(config, &job.job_id).join("llm_work");
    let task_dir = work.join("tasks").join("__planner__");
    std::fs::create_dir_all(&task_dir)?;
    // Reused fixed dir: clear stale completion so poll waits for this planner run.
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");

    let max_parallel = job
        .max_parallel
        .unwrap_or(config.default.max_parallel)
        .clamp(1, 32);
    let prompt = planner_system_prompt(&job.project, &source_text, max_parallel);
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;
    append_log(config, &job.job_id, "starting Claude LLM planner (print)…");

    let planner_task = TaskIR {
        id: "__planner__".into(),
        title: "plan split".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        acceptance: None,
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            "max_turns": 8,
            "max_budget_usd": PLANNER_MAX_BUDGET_USD,
            "permission_mode": "dontAsk",
            "allowed_tools": [],
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
    };

    let ctx = StartCtx {
        run_id: job.job_id.clone(),
        project_root: job.project.clone(),
        work_dir: job.project.clone(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for llm planner")?;

    let (raw_out, planner_cost) = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&planner_task)?;
        let handle = provider.start(&planner_task, &ctx).await?;
        append_log(
            config,
            &job.job_id,
            "claude CLI started (print); waiting for planner JSON…",
        );
        // Poll until done (max ~10 min already in task timeout)
        let mut ticks = 0u32;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    ticks += 1;
                    // 每 ~4s 写心跳，供桌面轮询展示，避免“假死”
                    if ticks % 10 == 0 {
                        let stdout_bytes = std::fs::metadata(&handle.stdout_path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        append_log(
                            config,
                            &job.job_id,
                            &format!(
                                "claude still running… {}s, stdout ~{} bytes",
                                ticks * 400 / 1000,
                                stdout_bytes
                            ),
                        );
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => {
                    break;
                }
            }
        }
        let result = provider.collect(&handle).await?;
        let cost = result.cost_usd;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result.error.unwrap_or_else(|| "planner worker failed".into());
            bail!("planner worker not done: {err}\n{stdout}");
        }
        Ok::<(String, Option<f64>), anyhow::Error>((stdout, cost))
    })?;

    // Persist planner cost for job.json / report split (P1-5).
    if let Some(c) = planner_cost {
        let _ = std::fs::write(
            job_dir(config, &job.job_id).join("planner_cost.json"),
            serde_json::to_string_pretty(&serde_json::json!({ "cost_usd": c }))?,
        );
        // Also patch in-memory job if caller reloads after finish.
        if let Ok(mut j) = PlanJob::load(config, &job.job_id) {
            j.planner_cost_usd = Some(c);
            j.updated_at = Utc::now();
            let _ = j.save(config);
        }
        append_log(
            config,
            &job.job_id,
            &format!("planner cost_usd={c:.4} (cap {PLANNER_MAX_BUDGET_USD})"),
        );
    }

    append_log(
        config,
        &job.job_id,
        &format!("LLM raw output {} bytes", raw_out.len()),
    );
    // Keep a copy for debug
    let _ = std::fs::write(job_dir(config, &job.job_id).join("llm_raw.txt"), &raw_out);

    let mut ir = parse_llm_plan_output(&raw_out, &abs, config)?;
    apply_worker_defaults(&mut ir, &job.provider, &job.exec_mode);
    ir.adapter = "planner-ai-llm".into();
    ir.source_path = abs;
    ir.validate()?;
    append_log(
        config,
        &job.job_id,
        &format!("LLM plan ok: {} tasks", ir.tasks.len()),
    );
    Ok(ir)
}

/// Read planner cost written by LLM path (if any).
pub(super) fn read_planner_cost(config: &Config, job_id: &str) -> Option<f64> {
    let path = job_dir(config, job_id).join("planner_cost.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("cost_usd")
        .and_then(|x| x.as_f64())
        .or_else(|| v.get("planner_cost_usd").and_then(|x| x.as_f64()))
}

pub(super) fn planner_system_prompt(project: &Path, source: &str, max_parallel: usize) -> String {
    let max_parallel = max_parallel.max(1);
    let max_tasks = MAX_TASKS;
    // Cap source length so huge specs don't drown the instruction rules.
    let source = if source.chars().count() > 28_000 {
        let head: String = source.chars().take(24_000).collect();
        format!("{head}\n\n…(文档过长，已截断；请按全文意图提炼工作包，勿按目录切任务)…\n")
    } else {
        source.to_string()
    };
    format!(
        r#"你是 cco 编排器的「规划器」。用户只会提供 **Markdown 计划**（不会写 YAML）。你的产出是给确认屏看的**可执行工作包 DAG**，不是文档目录。

项目路径: {project}
用户选择的 max_parallel（必须原样写入 JSON，并据此设计 depends_on 波次）: {max_parallel}

## 什么叫「任务」（必须）
- 每个 task = 一个 worker 能独立做完的**工作包**：有动词（实现/修复/新增/改造/验收/检验…）、有范围、有完成标志。
- title 用中文短句，例如「实现 handoff 归并」「接入混跑 worktree 校验」「检验员终检」。
- prompt 自包含：目标、可改路径、禁止事项、验收、最后一行 `CCO_DONE ok`。

## 绝对禁止（违反则整次规划失败）
- **禁止**把文档结构当成任务：Board / Timeline / Fragments / 目录 / TOC / 附录 / 修订历史 / 非目标 / 成功标准 / 决策树 / PROTOCOL / 关联真源 / 代码锚点。
- **禁止**用 Markdown 表格表头当 title（例如含多个 `|` 的 `id | provider | role | …`）。
- **禁止**只用阶段标签当 title：单独的 P0/P1/P2/M0–M5/D0–D5、「阶段切分」「勾选表」——阶段名只能出现在 prompt 引用里，title 必须是工作内容。
- **禁止**「1 波把全文所有章节并行」：若文档是产品方案/契约/总账，先**提炼要落地的实施项**，再拆 3–8 个有依赖的工作包（调研 → 并行实现 → 整合 → 检验）。
- 用户文档里的示例 YAML/代码块是**说明**，不要逐段复制成空壳任务。

## 输出要求（必须遵守）
1. 只输出 **一个** JSON 对象，不要 Markdown 解释，不要用 ``` 包裹以外的多余文字（若必须用代码块，仅一个 json 代码块）。
2. JSON schema:
{{
  "schema": "cco-plan/v1",
  "name": "短名称",
  "max_parallel": {max_parallel},
  "on_failure": "pause",
  "tasks": [
    {{
      "id": "t1",
      "title": "中文短标题（工作包，不是章节名）",
      "depends_on": [],
      "optional": false,
      "prompt": "给执行 worker 的完整中文说明，自包含，结尾要求输出 CCO_DONE ok"
    }}
  ]
}}
3. 任务数 2–{max_tasks}（硬上限 {max_tasks}）；能并行的不要串成一条长链；同一波最多约 {max_parallel} 个无依赖任务。
4. id 仅用 [a-z0-9_-]，稳定且唯一。
5. depends_on 只能引用已有 id，无环。
6. 每个 prompt 必须自包含（worker 看不到其它任务对话）；单任务 prompt 勿过长。
7. max_parallel 字段必须等于 {max_parallel}。
8. **可选项（用户自选）**：
   - 主路径/必做：`optional: false`（默认）。
   - 增强、打磨、文档润色、非阻塞 polish、计划写明「可选/可选项」的步骤：`optional: true`。
   - 可选项的 **title 必须**带「（可选）」后缀（例：`缓存层（可选）`），让用户在确认列表一眼看出可勾选。
   - 必做项标题不要写「可选」。
   - 至少保留 1 个必做任务；不要把全部标成 optional。
   - 其他任务的 depends_on 尽量不要只依赖 optional 任务（optional 可能被用户关掉）。
9. 若文档像「落地某方案」：默认最后一波含 **检验/验收** 任务（只读检查 + 写结论，不写大段业务代码）。

## 执行闭环（P-loop，必须遵守）
- **计划是唯一勾选真源**：拆分与巡检都对照计划 § 阶段 / S* / V*，不另造第二清单。
- 每个工作包 prompt 写清：**plan_ref**（对应哪些勾选 ID）、改哪些路径、不做哪些、完成标志、验收可否降级（默认否）。
- 落地任务须要求 worker 在 progress 写 `plan_ref → 证据`；**禁止**静默把成功标准改弱。
- 若有检验任务：prompt 要求产出计划勾选对照表 + `.cco-out/inspect/VERDICT.md`（Result: PASS|FAIL）+ `ISSUES.md`（severity=blocking|map|residual|out-of-scope、plan_ref、fix_wp）。
- **禁止**存在 blocking/map 时写 PASS；map（L1/L2 不同构）默认 blocking。
- 回补由后续 rework 波做；检验员默认不改业务代码。

## 用户计划文档（Markdown）
{source}
"#,
        project = project.display(),
        max_parallel = max_parallel,
        max_tasks = max_tasks,
        source = source,
    )
}

#[derive(Debug, Deserialize)]
pub(super) struct LlmPlanDoc {
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) max_parallel: Option<usize>,
    #[serde(default)]
    pub(super) on_failure: Option<String>,
    pub(super) tasks: Vec<LlmTask>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LlmTask {
    pub(super) id: String,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) depends_on: Vec<String>,
    pub(super) prompt: String,
    #[serde(default)]
    pub(super) group: Option<String>,
    #[serde(default)]
    pub(super) optional: bool,
}

pub(super) fn parse_llm_plan_output(raw: &str, source_path: &Path, config: &Config) -> Result<PlanIR> {
    let json_str = extract_json_object(raw).context("LLM 输出中未找到 JSON 对象")?;
    let doc: LlmPlanDoc = serde_json::from_str(&json_str).with_context(|| {
        format!(
            "parse planner JSON: {}",
            &json_str.chars().take(200).collect::<String>()
        )
    })?;
    if doc.tasks.is_empty() {
        bail!("LLM plan has no tasks");
    }
    if doc.tasks.len() > MAX_TASKS {
        bail!(
            "LLM plan has too many tasks ({} > max {MAX_TASKS})",
            doc.tasks.len()
        );
    }
    let name = doc
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("planned")
                .to_string()
        });
    let provider = config.default.default_provider.clone();
    let opts = default_provider_opts(config, &provider);
    let on_failure = match doc.on_failure.as_deref().unwrap_or("pause") {
        "continue" => OnFailure::Continue,
        "retry" => OnFailure::Retry,
        _ => OnFailure::Pause,
    };
    use crate::plan::{normalize_optional_title, title_is_meta_heading, title_looks_optional};
    let mut dropped_meta = Vec::new();
    let tasks: Vec<TaskIR> = doc
        .tasks
        .into_iter()
        .filter_map(|t| {
            let raw_title = t
                .title
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| t.id.clone());
            if title_is_meta_heading(&raw_title) {
                dropped_meta.push(raw_title);
                return None;
            }
            let optional = t.optional || title_looks_optional(&raw_title);
            let title = normalize_optional_title(&raw_title, optional);
            let mut prompt = t.prompt;
            if !prompt.contains("CCO_DONE") {
                prompt.push_str("\n\n全部完成后在最后一行输出：CCO_DONE ok\n");
            }
            Some(TaskIR {
                id: t.id,
                title,
                depends_on: t.depends_on,
                group: t.group,
                provider: provider.clone(),
                mode: config.default.default_mode.clone(),
                prompt,
                acceptance: None,
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: opts.clone(),
                optional,
                // Optional tasks default off — user opts in on confirm screen.
                include: !optional,
                role: None,
                scope: None,
                outputs: vec![],
            })
        })
        .collect();

    if tasks.is_empty() {
        bail!(
            "LLM 计划任务均为文档目录/表头类标题（已丢弃 {} 项），无法执行；请换「工作说明」Markdown 或重新规划",
            dropped_meta.len()
        );
    }
    // Drop depends_on edges that pointed at removed meta tasks.
    let keep: std::collections::HashSet<String> =
        tasks.iter().map(|t| t.id.clone()).collect();
    let mut tasks = tasks;
    for t in &mut tasks {
        t.depends_on.retain(|d| keep.contains(d));
    }
    if !dropped_meta.is_empty() {
        tracing::warn!(
            dropped = ?dropped_meta,
            kept = tasks.len(),
            "planner dropped meta-heading tasks from LLM output"
        );
    }

    Ok(PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "planner-ai-llm".into(),
        source_path: source_path.to_path_buf(),
        // Prefer document value; finish_plan_job may still override with split-time choice.
        max_parallel: doc
            .max_parallel
            .unwrap_or(config.default.max_parallel)
            .clamp(1, 32),
        on_failure,
        retry_max: 0,
        default_provider: provider,
        default_mode: config.default.default_mode.clone(),
        worktree: false,
        require_inspect: false,
        tasks,
    })
}

/// Pull a cco-plan JSON object out of planner worker stdout.
///
/// Claude print mode uses `--output-format stream-json`, so stdout is NDJSON:
/// system/assistant/user events plus a final `{"type":"result","result":"…"}`.
/// The plan lives in `result` (often a fenced ```json block), **not** in the
/// first `{…}` on the stream (that is usually the init event without `tasks`).
pub(super) fn extract_json_object(raw: &str) -> Option<String> {
    // 1) stream-json: final result envelope → result string / nested object
    if let Some(s) = extract_from_stream_json(raw) {
        return Some(s);
    }

    // 2) fenced ```json … ``` (plain text or embedded in a larger blob)
    if let Some(s) = extract_fenced_json(raw) {
        if looks_like_plan_json(&s) {
            return Some(s);
        }
    }

    // 3) any NDJSON / standalone line that is itself a plan object
    for line in raw.lines().rev() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        if looks_like_plan_json(line) {
            return Some(line.to_string());
        }
        // line may be an envelope with nested result/text
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(s) = plan_json_from_value(&v) {
                return Some(s);
            }
        }
    }

    // 4) last resort: fenced block even if schema check failed, then outer braces
    if let Some(s) = extract_fenced_json(raw) {
        return Some(s);
    }
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end > start {
        let slice = &raw[start..=end];
        // Prefer a plan-shaped sub-object if the giant slice is stream noise
        if looks_like_plan_json(slice) {
            return Some(slice.to_string());
        }
        if let Some(s) = find_plan_object_in_text(slice) {
            return Some(s);
        }
        Some(slice.to_string())
    } else {
        None
    }
}

pub(super) fn extract_from_stream_json(raw: &str) -> Option<String> {
    // Prefer last successful result line (stream-json convention).
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let is_result = v
            .get("type")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t == "result");
        if !is_result {
            // Also accept assistant text payloads that already embed the plan.
            if let Some(s) = plan_json_from_value(&v) {
                return Some(s);
            }
            continue;
        }
        if let Some(s) = plan_json_from_value(&v) {
            return Some(s);
        }
    }
    None
}

pub(super) fn plan_json_from_value(v: &serde_json::Value) -> Option<String> {
    // Direct plan object
    if looks_like_plan_value(v) {
        return serde_json::to_string(v).ok();
    }
    // result: "…json…" | { plan }
    if let Some(r) = v.get("result") {
        if let Some(s) = r.as_str() {
            if let Some(plan) = coerce_plan_text(s) {
                return Some(plan);
            }
        } else if looks_like_plan_value(r) {
            return serde_json::to_string(r).ok();
        }
    }
    // assistant message.content[].text
    if let Some(msg) = v.get("message") {
        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
            for part in content.iter().rev() {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if let Some(plan) = coerce_plan_text(text) {
                        return Some(plan);
                    }
                }
            }
        }
    }
    None
}

pub(super) fn coerce_plan_text(text: &str) -> Option<String> {
    if let Some(s) = extract_fenced_json(text) {
        if looks_like_plan_json(&s) || s.contains("\"tasks\"") {
            return Some(s);
        }
    }
    let trimmed = text.trim();
    if looks_like_plan_json(trimmed) {
        return Some(trimmed.to_string());
    }
    find_plan_object_in_text(text)
}

pub(super) fn extract_fenced_json(raw: &str) -> Option<String> {
    let mut search = raw;
    // Prefer the last ```json fence (final answer), not an earlier example.
    let mut best: Option<String> = None;
    while let Some(idx) = search.find("```") {
        let after = &search[idx + 3..];
        let after = after
            .strip_prefix("json")
            .or_else(|| after.strip_prefix("JSON"))
            .unwrap_or(after);
        let after = after.trim_start_matches(|c| c == '\n' || c == '\r' || c == ' ');
        if let Some(end) = after.find("```") {
            let block = after[..end].trim();
            if block.starts_with('{') {
                best = Some(block.to_string());
            }
            search = &after[end + 3..];
        } else {
            break;
        }
    }
    best
}

pub(super) fn looks_like_plan_json(s: &str) -> bool {
    let t = s.trim();
    if !(t.starts_with('{') && t.contains("\"tasks\"")) {
        return false;
    }
    match serde_json::from_str::<serde_json::Value>(t) {
        Ok(v) => looks_like_plan_value(&v),
        Err(_) => false,
    }
}

pub(super) fn looks_like_plan_value(v: &serde_json::Value) -> bool {
    v.get("tasks")
        .and_then(|t| t.as_array())
        .is_some_and(|a| !a.is_empty())
}

/// Scan text for a balanced `{…}` that deserializes as a plan with tasks.
pub(super) fn find_plan_object_in_text(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        if let Some(end) = find_matching_brace(bytes, i) {
            let slice = &text[i..=end];
            if looks_like_plan_json(slice) {
                return Some(slice.to_string());
            }
            i += 1;
        } else {
            break;
        }
    }
    None
}

pub(super) fn find_matching_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
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
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}
