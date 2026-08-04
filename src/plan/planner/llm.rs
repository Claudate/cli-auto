//! LLM planner path: Claude CLI print + stream-json JSON extraction.
//!
//! [INPUT]: PlanJob · Config · ClaudeProvider
//! [OUTPUT]: build_llm_plan · parse_llm_plan_output · read_planner_cost
//! [POS]: planner 子模块；ai mode 首选路径
//! note: digest 模式（regression/greenfield）进 system prompt；sanitize_task_deps 去假依赖
//! note: max_parallel 仅为调度上限，prompt 禁止为凑波次加 depends_on
//! note: finish_plan_job 再跑 critic_plan_tasks（回归改标题/钉 prompt）；本文件 parse 仅 sanitize
//! note: 可选第二跳 LLM critic：`CCO_PLANNER_CRITIC=1`（默认关；失败则保留规则校对）
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::config::Config;
use crate::plan::adapters::raw_single::default_provider_opts;
use crate::plan::{OnFailure, PlanIR, TaskIR, PLANNER_MAX_BUDGET_USD, PLANNER_MAX_TASKS};

use super::job::{append_log, apply_worker_defaults, job_dir, PlanJob, PlanJobStatus};
// PlanJob used when persisting digest_mode mid-plan

/// Call Claude CLI (print) to produce a cco-plan/v1 JSON task graph.
pub(super) fn build_llm_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    use crate::runtime::provider::{
        claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
    };

    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let source_text =
        std::fs::read_to_string(&abs).with_context(|| format!("read plan {}", abs.display()))?;
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
    append_log(config, &job.job_id, &format!("planner CLI bin = {bin}"));

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
    let digest = super::digest::build_plan_digest(&source_text);
    append_log(
        config,
        &job.job_id,
        &format!(
            "plan digest mode={} landed={} phases={}",
            digest.mode.as_str(),
            digest.landed_hint,
            digest.phase_lines.len()
        ),
    );
    // Persist mode early so UI can show regression/greenfield while still planning.
    if let Ok(mut j) = PlanJob::load(config, &job.job_id) {
        j.digest_mode = Some(digest.mode.as_str().to_string());
        j.updated_at = Utc::now();
        let _ = j.save(config);
    }
    let prompt = planner_system_prompt_with_memory(
        Some(config),
        &job.project,
        &source_text,
        max_parallel,
        &digest,
    );
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;
    append_log(config, &job.job_id, "starting intelligent planner…");

    let planner_task = TaskIR {
        id: "__planner__".into(),
        title: "plan split".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        verify_cmd: None,
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
        tags: vec![],
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
                        // Touch job.updated_at so UI/reaper know this planning is alive.
                        if let Ok(mut j) = PlanJob::load(config, &job.job_id) {
                            if matches!(j.status, PlanJobStatus::Planning) {
                                j.updated_at = Utc::now();
                                let _ = j.save(config);
                            }
                        }
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
            let err = result
                .error
                .unwrap_or_else(|| "planner worker failed".into());
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
    // P2-4/5: tags may still override soft defaults after worker paint.
    crate::plan::apply_tag_routing(&mut ir);
    crate::plan::materialize_role_defaults(&mut ir);
    ir.adapter = "planner-ai-llm".into();
    ir.source_path = abs;
    // Soft accept: order / waves / optional for UI+run — auto-fix collab strictness
    // instead of discarding the whole LLM graph (scope overlap → heuristic waste).
    let soft_notes = crate::domain::plan::soften_plan_for_accept(&mut ir);
    for n in &soft_notes {
        append_log(config, &job.job_id, &format!("soften: {n}"));
    }
    if let Err(e) = ir.validate() {
        // Still broken after soften → caller falls back to heuristic.
        append_log(
            config,
            &job.job_id,
            &format!("LLM plan still invalid after soften ({e:#})"),
        );
        return Err(e);
    }
    append_log(
        config,
        &job.job_id,
        &format!(
            "LLM plan ok: {} tasks{}",
            ir.tasks.len(),
            if soft_notes.is_empty() {
                String::new()
            } else {
                format!(" (softened {})", soft_notes.len())
            }
        ),
    );
    Ok(ir)
}

/// Gate for optional second-pass LLM critic (default off).
/// On if `config.default.planner_critic_enabled` **or** env `CCO_PLANNER_CRITIC=1|true|yes`.
pub(super) fn llm_critic_enabled(config: &Config) -> bool {
    if config.default.planner_critic_enabled {
        return true;
    }
    std::env::var("CCO_PLANNER_CRITIC")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// Outcome of optional LLM critic for job persistence / UI.
#[derive(Debug, Clone, Default)]
pub(super) struct LlmCriticOutcome {
    pub used: bool,
    pub report: super::digest::CriticReport,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
}

/// Plan modes that promise a local/fast split — never start a second Claude critic.
/// (Settings may still show critic enabled; that only applies to `ai`.)
pub(super) fn plan_mode_skips_llm_critic(plan_mode: &str) -> bool {
    matches!(
        plan_mode.trim().to_ascii_lowercase().as_str(),
        "fast" | "heuristic" | "parse" | "fake" | "direct"
    )
}

/// Optional second-pass LLM critic: only drop bad edges + notes.
/// Soft-fail: any error is logged and ignored (rule critic already ran).
pub(super) fn run_optional_llm_critic(
    config: &Config,
    job: &PlanJob,
    ir: &mut crate::plan::PlanIR,
    mode: super::digest::PlanModeKind,
) -> LlmCriticOutcome {
    use super::digest::{
        apply_llm_critic_patch, parse_llm_critic_patch, tasks_skeleton_json, CriticReport,
    };

    if plan_mode_skips_llm_critic(&job.plan_mode) {
        append_log(
            config,
            &job.job_id,
            &format!(
                "LLM critic skipped (plan_mode={}; local/fast path)",
                job.plan_mode
            ),
        );
        return LlmCriticOutcome::default();
    }
    if !llm_critic_enabled(config) {
        return LlmCriticOutcome::default();
    }
    if job.provider.eq_ignore_ascii_case("fake") {
        append_log(config, &job.job_id, "LLM critic skipped (fake provider)");
        return LlmCriticOutcome::default();
    }

    let skeleton = tasks_skeleton_json(&ir.tasks);
    let prompt = format!(
        r#"你是 cco 规划校对员。下面是已生成的任务 DAG 骨架（mode={mode}）。
只做两件事：1）标出应删除的假依赖边 2）给 0～3 条中文 notes。
禁止新增/删除任务，禁止改写 prompt 全文。

输出**仅一个 JSON 对象**（不要 Markdown 解释）：
{{
  "remove_edges": [{{"task": "t5", "deps": ["t3"]}}],
  "notes": ["可选说明"]
}}

规则：
- 无真实产物/接口耦合的 depends_on 应删除
- regression 模式：正交阶段（如 meta vs failover）默认无边
- 不确定就不要删边

任务骨架：
{skeleton}
"#,
        mode = mode.as_str(),
        skeleton = skeleton,
    );

    let started = std::time::Instant::now();
    match run_short_claude_print(config, job, &prompt, "__critic__") {
        Ok((raw, cost)) => {
            let duration_ms = started.elapsed().as_millis() as u64;
            let _ = std::fs::write(
                job_dir(config, &job.job_id).join("llm_critic_raw.txt"),
                &raw,
            );
            let (report, note) = match parse_llm_critic_patch(&raw) {
                Some(patch) => {
                    let report = apply_llm_critic_patch(&mut ir.tasks, &patch);
                    (
                        report,
                        format!(
                            "LLM critic ok: dropped edges, notes applied; cost={:?} ms={}",
                            cost, duration_ms
                        ),
                    )
                }
                None => (
                    CriticReport::default(),
                    format!(
                        "LLM critic: no valid JSON patch; keep rule critic only; cost={:?} ms={}",
                        cost, duration_ms
                    ),
                ),
            };
            append_log(config, &job.job_id, &note);
            if let Some(c) = cost {
                append_log(
                    config,
                    &job.job_id,
                    &format!("LLM critic cost_usd={c:.4} duration_ms={duration_ms}"),
                );
            } else {
                append_log(
                    config,
                    &job.job_id,
                    &format!("LLM critic duration_ms={duration_ms}"),
                );
            }
            LlmCriticOutcome {
                used: true,
                report,
                cost_usd: cost,
                duration_ms: Some(duration_ms),
            }
        }
        Err(e) => {
            append_log(
                config,
                &job.job_id,
                &format!("LLM critic skipped (error): {e:#}"),
            );
            LlmCriticOutcome::default()
        }
    }
}

/// Short Claude print call (shared by optional critic).
/// Returns `(stdout, cost_usd)`.
fn run_short_claude_print(
    config: &Config,
    job: &PlanJob,
    prompt: &str,
    task_id: &str,
) -> Result<(String, Option<f64>)> {
    use crate::runtime::provider::{
        claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
    };

    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = job_dir(config, &job.job_id).join("llm_work");
    let task_dir = work.join("tasks").join(task_id);
    std::fs::create_dir_all(&task_dir)?;
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    std::fs::write(task_dir.join("prompt.md"), prompt)?;

    let planner_task = TaskIR {
        id: task_id.into(),
        title: "plan critic".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt: prompt.to_string(),
        verify_cmd: None,
        acceptance: None,
        timeout_secs: Some(180),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            "max_turns": 3,
            "max_budget_usd": 0.35,
            "permission_mode": "dontAsk",
            "allowed_tools": [],
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
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
        .context("tokio for llm critic")?;

    rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&planner_task)?;
        let handle = provider.start(&planner_task, &ctx).await?;
        let mut ticks = 0u32;
        let mut timed_out = false;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    ticks += 1;
                    // ~2 min — must soft-fail well before planning hard timeout (5 min).
                    if ticks > 300 {
                        timed_out = true;
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => break,
            }
        }
        if timed_out {
            // Kill the hung Claude so hard-timeout reaper / next split are not blocked.
            let _ = provider.stop(&handle).await;
            if let Some(pid) = handle.pid {
                super::job::kill_pid_best_effort(pid);
            }
            bail!("llm critic timeout");
        }
        let result = provider.collect(&handle).await?;
        let cost = result.cost_usd;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            // Failed/stopped workers may still leave a live child briefly — best-effort stop.
            let _ = provider.stop(&handle).await;
            let err = result
                .error
                .unwrap_or_else(|| "critic worker failed".into());
            bail!("critic worker not done: {err}");
        }
        Ok((stdout, cost))
    })
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

/// Planner system prompt with optional project memory context (P2-2 · context only).
pub(super) fn planner_system_prompt_with_memory(
    config: Option<&Config>,
    project: &Path,
    source: &str,
    max_parallel: usize,
    digest: &super::digest::PlanDigest,
) -> String {
    let max_parallel = max_parallel.max(1);
    let max_tasks = PLANNER_MAX_TASKS;
    let digest_block = super::digest::format_digest_for_prompt(digest);
    let mode = digest.mode.as_str();
    let mode_rules: String = match digest.mode {
        super::digest::PlanModeKind::Regression => r#"
## 模式 = regression（文档声明已落地 / 勾选多已完成）— 硬约束
- 每个任务 = **回归验证 + 仅 blocking 残差才改代码**；title 用「回归验证 …」「核对 …」「补残差 …」，**禁止**「实现完整 H0–H4 / 从零落地」。
- 每条 prompt **第一行**写：`文档声明相关阶段已落地。默认只读验证；仅 ISSUES 中 severity=blocking 才改代码。`
- 正交能力（如 meta 过滤 vs failover）**默认无 depends_on**；禁止按章节编号串成假链。
- 任务数宜 3–8；最后一波可加 **检验员（可选）** 对照 S*/勾选表。
"#
        .to_string(),
        super::digest::PlanModeKind::Audit => r#"
## 模式 = audit — 硬约束
- 以只读检验 / 对照勾选为主；避免大段业务实现任务。
- 产出应含 VERDICT/ISSUES 类检验步骤。
"#
        .to_string(),
        super::digest::PlanModeKind::Mixed => r#"
## 模式 = mixed — 硬约束
- 已勾选项 → 回归验证；未勾选项 → 可实施工作包；二者在 title/prompt 中区分。
- 禁止把已完成项再拆成「从零实现」。
"#
        .to_string(),
        super::digest::PlanModeKind::Greenfield => {
            let stack = crate::domain::chat::planner_greenfield_stack_blurb();
            format!(
                r#"
## 模式 = greenfield — 硬约束
- 提炼可落地的实施工作包（调研 → 并行实现 → 整合 → 检验），3–12 个为宜。
- 依赖只能来自真实产物/接口耦合，禁止章节顺序当依赖。
{stack}
"#
            )
        }
    };
    // Cap source length so huge specs don't drown the instruction rules.
    let source = if source.chars().count() > 28_000 {
        let head: String = source.chars().take(24_000).collect();
        format!("{head}\n\n…(文档过长，已截断；请按全文意图提炼工作包，勿按目录切任务)…\n")
    } else {
        source.to_string()
    };
    let base = format!(
        r#"你是 cco 编排器的「规划器」。用户只会提供 **Markdown 计划**（不会写 YAML）。你的产出是给确认屏看的**可执行工作包 DAG**，不是文档目录。

项目路径: {project}
用户选择的 max_parallel: {max_parallel}
说明：**max_parallel 只是调度并发上限**，用来限制同一时刻跑几步；**禁止**为了「凑波次」而人为增加 depends_on。先按真实依赖建图，再自然形成波次。

## 文档摘要（规则抽取，优先于全文臆测）
{digest_block}
判定 mode = **{mode}**
{mode_rules}

## 什么叫「任务」（必须）
- 每个 task = 一个 worker 能独立做完的**工作包**：有动词（实现/修复/核对/回归/改造/验收/检验…）、有范围、有完成标志。
- title 用中文短句，例如「回归验证 H0 入口」「实现 handoff 归并」「检验员终检」。
- prompt 自包含：目标、可改路径、禁止事项、验收、最后一行 `CCO_DONE ok`。
- 每个 prompt 写清 **plan_ref**（对应阶段/勾选 ID）。若有依赖，写明 **依赖原因**（产物路径/接口），并在正文提到被依赖的 task id。

## 绝对禁止（违反则整次规划失败）
- **禁止**把文档结构当成任务：Board / Timeline / Fragments / 目录 / TOC / 附录 / 修订历史 / 非目标 / 成功标准 / 决策树 / PROTOCOL / 关联真源 / 代码锚点。
- **禁止**用 Markdown 表格表头当 title（例如含多个 `|` 的 `id | provider | role | …`）。
- **禁止**只用阶段标签当 title：单独的 P0/P1/P2/M0–M5/D0–D5、「阶段切分」「勾选表」——阶段名只能出现在 prompt 引用里，title 必须是工作内容。
- **禁止**「1 波把全文所有章节并行」：先提炼工作包，再拆 3–8 个有**真实**依赖的包。
- **禁止**无理由 depends_on（编号顺序 / 同一文档章节 / 为凑 max_parallel 加边）。无真实耦合 → depends_on: []。
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
      "provider": "claude",
      "role": "implement",
      "tags": [],
      "scope": {{ "paths": ["src/**"], "readonly": [], "forbid": [] }},
      "outputs": [],
      "prompt": "给执行 worker 的完整中文说明，自包含，结尾要求输出 CCO_DONE ok"
    }}
  ]
}}
3. 任务数 2–{max_tasks}（硬上限 {max_tasks}）；能并行的不要串成一条长链。
4. id 仅用 [a-z0-9_-]，稳定且唯一。
5. depends_on 只能引用已有 id，无环；每条边应在 prompt 中可辩护。
6. 每个 prompt 必须自包含（worker 看不到其它任务对话）；单任务 prompt 勿过长。
7. max_parallel 字段必须等于 {max_parallel}。
8. **可选项（用户自选）**：
   - 主路径/必做：`optional: false`（默认）。
   - 增强、打磨、文档润色、非阻塞 polish、计划写明「可选/可选项」的步骤：`optional: true`。
   - 可选项的 **title 必须**带「（可选）」后缀（例：`缓存层（可选）`），让用户在确认列表一眼看出可勾选。
   - 必做项标题不要写「可选」。
   - 至少保留 1 个必做任务；不要把全部标成 optional。
   - 其他任务的 depends_on 尽量不要只依赖 optional 任务（optional 可能被用户关掉）。
9. 若文档像「落地/回归某方案」：默认最后一波含 **检验/验收** 任务（只读检查 + 写结论，不写大段业务代码），可 optional。
10. **多 CLI 协作字段（P2-5，建议填写；缺省可省略）**：
   - `provider`：`claude` | `codex` | `fake`。缺省 = 配置默认引擎。
   - `role`：`scout` | `implement` | `integrate` | `inspect`（`closeout` 一般由 host 注入，勿手写兼差）。检验/验收写 `inspect`；主实现写 `implement`。
   - **inspect ≠ closeout**：巡检任务 title/prompt **禁止**「并回写台账 / commit / 勾选进度」；只对照计划写 VERDICT/ISSUES。台账关账另步或交 host。
   - `tags`：短标签数组，供路由（例：`["frontend"]`、`["codex"]`、`["inspect"]`）。可空。
   - `scope`：`paths` 可写白名单 glob；并行 implement 的 paths **不应相交**。inspect 可只写 inspect 输出目录。
   - `outputs`：完成后必须存在的相对路径（inspect 建议含 `.cco-out/inspect/VERDICT.md`）。
   - 混用 claude+codex 时：implement 任务尽量设 `worktree` 语义（prompt 写清独立目录）；**不要**给 codex 设 bg 模式。

## 执行闭环（P-loop，必须遵守）
- **计划是唯一勾选真源**：拆分与巡检都对照计划 § 阶段 / S* / V*，不另造第二清单。
- 每个工作包 prompt 写清：**plan_ref**、改哪些路径、不做哪些、完成标志、验收可否降级（默认否）。
- 落地/回归任务须要求 worker 在 progress 写 `plan_ref → 证据`；**禁止**静默把成功标准改弱。
- **缺资源默认补齐**（真图/素材）：搜索图库或生成落盘并改路径；禁止仅用几何 SVG 顶「真实感」或把标准改成「非 placehold」。
- 若有检验任务：prompt 要求 `GATE.json` + `VERDICT.md`（Result: PASS|FAIL）+ `ISSUES.md`（severity=blocking|map|residual|out-of-scope、plan_ref、fix_wp）。手点/录像/未 commit/未引用脚手架 CSS=**residual 且 GATE pass**；真功能缺口与**计划意图静默降级**=blocking + fix_wp，由 rework 补齐。
- **禁止**存在 blocking/map 时写 PASS；map（L1/L2 不同构）默认 blocking。
- 回补优先用户可见缺口；检验员默认不改业务代码。

## 用户计划文档（Markdown）
{source}
"#,
        project = project.display(),
        max_parallel = max_parallel,
        max_tasks = max_tasks,
        digest_block = digest_block,
        mode = mode,
        mode_rules = mode_rules,
        source = source,
    );
    // P2-2: pin/summary as context only — does not rewrite route or auto-confirm.
    if let Some(cfg) = config {
        let mem = crate::app::memory::prompt_context(cfg, project);
        if !mem.is_empty() {
            return format!("{base}\n{mem}");
        }
    }
    base
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
    /// P2-5: per-task engine hint from planner JSON.
    #[serde(default)]
    pub(super) provider: Option<String>,
    /// P2-5: collaboration role (scout|implement|integrate|inspect).
    #[serde(default)]
    pub(super) role: Option<crate::plan::TaskRole>,
    /// P2-5: path contract.
    #[serde(default)]
    pub(super) scope: Option<crate::plan::TaskScope>,
    /// P2-5: required artifact paths.
    #[serde(default)]
    pub(super) outputs: Vec<String>,
    /// P2-4: free-form routing tags.
    #[serde(default)]
    pub(super) tags: Vec<String>,
}

pub(super) fn parse_llm_plan_output(
    raw: &str,
    source_path: &Path,
    config: &Config,
) -> Result<PlanIR> {
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
    if doc.tasks.len() > PLANNER_MAX_TASKS {
        bail!(
            "LLM plan has too many tasks ({} > max {PLANNER_MAX_TASKS})",
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
            // P2-5: accept planner-provided provider; fall back to config default.
            let task_provider = t
                .provider
                .as_ref()
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| matches!(s.as_str(), "claude" | "codex" | "fake"))
                .unwrap_or_else(|| provider.clone());
            let task_opts = if task_provider == provider {
                opts.clone()
            } else {
                default_provider_opts(config, &task_provider)
            };
            // Empty scope object → treat as absent.
            let scope = t.scope.and_then(|s| {
                if s.paths.is_empty() && s.readonly.is_empty() && s.forbid.is_empty() {
                    None
                } else {
                    Some(s)
                }
            });
            // Infer inspect role from title/tags when model forgot the field.
            let mut role = t.role;
            let tags = t.tags;
            if role.is_none() {
                let lower = title.to_ascii_lowercase();
                if lower.contains("检验")
                    || lower.contains("验收")
                    || lower.contains("inspect")
                    || tags.iter().any(|x| x.eq_ignore_ascii_case("inspect"))
                {
                    role = Some(crate::plan::TaskRole::Inspect);
                }
            }
            Some(TaskIR {
                id: t.id,
                title,
                depends_on: t.depends_on,
                group: t.group,
                provider: task_provider,
                mode: config.default.default_mode.clone(),
                prompt,
                verify_cmd: None,
                acceptance: None,
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: task_opts,
                optional,
                // Optional tasks default off — user opts in on confirm screen.
                include: !optional,
                role,
                scope,
                outputs: t.outputs,
                tags,
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
    let keep: std::collections::HashSet<String> = tasks.iter().map(|t| t.id.clone()).collect();
    let mut tasks = tasks;
    for t in &mut tasks {
        t.depends_on.retain(|d| keep.contains(d));
    }
    // Drop unmotivated edges (no id/title mention / no depend reason in prompt).
    super::digest::sanitize_task_deps(&mut tasks);
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

#[cfg(test)]
mod llm_critic_gate_tests {
    use super::super::digest::PlanModeKind;
    use super::super::job::{job_dir, PlanJob, PlanJobStatus};
    use super::{plan_mode_skips_llm_critic, run_optional_llm_critic};
    use crate::config::Config;
    use crate::plan::{OnFailure, PlanIR, TaskIR};
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn plan_mode_skips_llm_critic_for_local_modes() {
        for m in [
            "fast",
            "heuristic",
            "parse",
            "fake",
            "direct",
            "FAST",
            " Parse ",
        ] {
            assert!(plan_mode_skips_llm_critic(m), "mode={m}");
        }
        assert!(!plan_mode_skips_llm_critic("ai"));
        assert!(!plan_mode_skips_llm_critic(""));
    }

    #[test]
    fn run_optional_llm_critic_skips_on_fast_even_if_setting_on() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.state_root = dir.path().join("state");
        cfg.default.planner_critic_enabled = true;
        std::fs::create_dir_all(&cfg.state_root).unwrap();

        let job_id = "plan-fast-skip-critic";
        let job = PlanJob {
            job_id: job_id.into(),
            status: PlanJobStatus::Planning,
            project: dir.path().to_path_buf(),
            plan_path: PathBuf::from("idea.md"),
            plan_mode: "fast".into(),
            provider: "claude".into(),
            exec_mode: "print".into(),
            error: None,
            run_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            plan_name: None,
            task_count: None,
            max_parallel: Some(2),
            adapter: None,
            planner_cost_usd: None,
            digest_mode: None,
            critic_summary: None,
            critic_edges_removed: None,
            critic_titles_rewritten: None,
            critic_prompts_tagged: None,
            critic_notes: vec![],
            critic_llm_used: None,
            critic_llm_cost_usd: None,
            critic_llm_ms: None,
            grain_hint: None,
            clarify_depth: None,
            revision_notes: None,
            effort: None,
        };
        job.save(&cfg).unwrap();

        let mut ir = PlanIR {
            schema: "cco-plan/v1".into(),
            name: "n".into(),
            adapter: "planner-ai-heuristic".into(),
            source_path: PathBuf::from("idea.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![TaskIR {
                id: "t1".into(),
                title: "A".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "p\nCCO_DONE ok".into(),
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
            }],
        };
        let out = run_optional_llm_critic(&cfg, &job, &mut ir, PlanModeKind::Greenfield);
        assert!(!out.used);
        assert!(out.duration_ms.is_none());
        // No __critic__ task dir should be created when skipped by plan_mode.
        let critic = job_dir(&cfg, job_id).join("llm_work/tasks/__critic__");
        assert!(
            !critic.exists(),
            "critic dir should not be created on fast skip"
        );
        let log =
            std::fs::read_to_string(job_dir(&cfg, job_id).join("planner.log")).unwrap_or_default();
        assert!(
            log.contains("LLM critic skipped") && log.contains("fast"),
            "log={log}"
        );
    }
}
