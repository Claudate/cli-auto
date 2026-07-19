//! Fake demo DAG + heading/paragraph heuristic splitter.
//!
//! [INPUT]: PlanJob · Config · plan source text
//! [OUTPUT]: build_fake_plan · build_heuristic_ai_plan
//! [POS]: planner 子模块；ai fallback / fake mode
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::plan::adapters::raw_single::default_provider_opts;
use crate::plan::{OnFailure, PlanIR, TaskIR, TaskRole, MAX_TASKS};

use super::job::{append_log, PlanJob};

pub(super) fn build_fake_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("demo")
        .to_string();
    let opts = default_provider_opts(config, &job.provider);
    let src_hint = job.plan_path.display().to_string();

    let mk = |id: &str, title: &str, deps: Vec<&str>, group: &str, body: &str, optional: bool| {
        let title = crate::plan::normalize_optional_title(title, optional);
        TaskIR {
            id: id.into(),
            title: title.clone(),
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
            group: Some(group.into()),
            provider: job.provider.clone(),
            mode: job.exec_mode.clone(),
            prompt: format!(
                "【模拟任务 {id}】{title}\n来源计划: {src_hint}\n\n{body}\n\n完成后输出一行: CCO_DONE ok\n"
            ),
            acceptance: None,
            timeout_secs: Some(120),
            worktree: Some(false),
            provider_opts: opts.clone(),
            optional,
            include: !optional,
            role: None,
            scope: None,
            outputs: vec![],
        }
    };

    let ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name: format!("{name}-fake"),
        adapter: "planner-fake".into(),
        source_path: abs,
        max_parallel: job
            .max_parallel
            .unwrap_or(config.default.max_parallel)
            .clamp(1, 32),
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: job.provider.clone(),
        default_mode: job.exec_mode.clone(),
        worktree: false,
        require_inspect: false,
        tasks: vec![
            mk(
                "t1",
                "调研与范围",
                vec![],
                "G1",
                "阅读计划意图，列出 3 条范围说明（模拟，无需改仓库）。",
                false,
            ),
            mk(
                "t2",
                "脚手架",
                vec![],
                "G1",
                "描述将创建的文件清单（模拟，无需真实写入）。",
                false,
            ),
            mk(
                "t3",
                "实现与集成",
                vec!["t1", "t2"],
                "G2",
                "在 t1/t2 完成后做集成说明（模拟）。",
                false,
            ),
            mk(
                "t4",
                "验收摘要",
                vec!["t3"],
                "G3",
                "输出验收检查表（模拟）。",
                false,
            ),
            mk(
                "t5",
                "文档润色",
                vec!["t4"],
                "G4",
                "可选：整理 README 要点（模拟）。用户未勾选则不跑。",
                true,
            ),
        ],
    };
    ir.validate()?;
    // tiny delay so UI can show planning state if polled mid-flight (sync path still fine)
    let _ = Duration::from_millis(1);
    Ok(ir)
}

/// Split markdown-ish text into tasks by `##` / `###` headings; sequential deps.
pub(super) fn build_heuristic_ai_plan(config: &Config, job: &PlanJob) -> Result<PlanIR> {
    let abs = crate::plan::resolve_plan_path(&job.project, &job.plan_path)?;
    let text = std::fs::read_to_string(&abs)
        .with_context(|| format!("read plan {}", abs.display()))?;
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("planned")
        .to_string();
    let opts = default_provider_opts(config, &job.provider);

    let mut sections = split_sections(&text);
    append_log(
        config,
        &job.job_id,
        &format!("heuristic found {} section(s)", sections.len()),
    );

    // Prefer ##-only boundaries when ### made the graph too fine-grained.
    if sections.len() > MAX_TASKS {
        let coarse = split_sections_level(&text, /*include_h3=*/ false);
        if coarse.len() > 1 && coarse.len() <= MAX_TASKS {
            append_log(
                config,
                &job.job_id,
                &format!(
                    "heuristic coarsened to {} ## section(s) (was {}, max {MAX_TASKS})",
                    coarse.len(),
                    sections.len()
                ),
            );
            sections = coarse;
        }
    }

    // Drop document-chrome headings (Board / P0 勾选 / 修订历史 / 表头…).
    let before_meta = sections.len();
    sections.retain(|(title, _)| !crate::plan::title_is_meta_heading(title));
    if sections.len() != before_meta {
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic dropped {} meta heading(s); {} work-like section(s) left",
                before_meta - sections.len(),
                sections.len()
            ),
        );
    }

    // Spec / contract docs: NEVER TOC-split, even if some ## survived meta filter
    // (e.g. "1. 产品结论" / "分配策略档位" still look like section titles, not work).
    let mut force_serial = false;
    let sections = if looks_like_spec_document(&text) {
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic: product/spec MD ({} leftover heading section(s)) → work-order template",
                sections.len()
            ),
        );
        force_serial = true;
        work_order_template_from_spec(&text)
    } else if sections.len() <= 1 {
        // Fall back to chunking long prose into up to 3 sequential tasks.
        force_serial = true;
        chunk_prose(&text, 3)
    } else {
        sections
    };

    let sections = if sections.len() > MAX_TASKS {
        let merged = merge_sections(sections, MAX_TASKS);
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic merged into {} task(s) to satisfy max {MAX_TASKS}",
                merged.len()
            ),
        );
        merged
    } else {
        sections
    };

    if sections.is_empty() {
        bail!("计划文档为空，无法拆分");
    }

    // Wave-sized parallel groups from split-time concurrency (not a full serial chain).
    // Work-order templates / prose chunks stay sequential so the user sees a pipeline.
    let max_parallel = job
        .max_parallel
        .unwrap_or(config.default.max_parallel)
        .clamp(1, 32);
    let wave_size = if force_serial { 1 } else { max_parallel };

    let mut tasks = Vec::new();
    let n_sections = sections.len();
    // Spec work-order last wave is always the dedicated inspect (P-loop L0/L1).
    let last_is_inspect = force_serial
        && n_sections >= 2
        && looks_like_spec_document(&text)
        && sections
            .last()
            .map(|(t, _)| {
                let lower = t.to_ascii_lowercase();
                t.contains("巡检")
                    || t.contains("检验")
                    || lower.contains("inspect")
                    || lower.contains("verdict")
            })
            .unwrap_or(false);

    for (i, (title, body)) in sections.iter().enumerate() {
        let id = format!("t{}", i + 1);
        let wave = i / wave_size;
        let depends_on = if wave == 0 {
            vec![]
        } else {
            // Barrier: wait for the previous wave so at most wave_size run together.
            let start = (wave - 1) * wave_size;
            let end = wave * wave_size;
            (start..end).map(|j| format!("t{}", j + 1)).collect()
        };
        let optional = crate::plan::title_looks_optional(title);
        let title = crate::plan::normalize_optional_title(title, optional);
        let is_inspect = last_is_inspect && i + 1 == n_sections;
        let (role, outputs, scope) = if is_inspect {
            (
                Some(TaskRole::Inspect),
                vec![
                    ".cco-out/inspect/VERDICT.md".into(),
                    ".cco-out/inspect/ISSUES.md".into(),
                ],
                Some(crate::plan::TaskScope {
                    paths: vec![".cco-out/inspect/**".into()],
                    readonly: vec!["**".into()],
                    forbid: vec![],
                }),
            )
        } else {
            (None, vec![], None)
        };
        let prompt = format!(
            "你是执行任务 `{id}`（{title}）的 worker。\n\
             项目根目录即当前工作目录。\n\
             依据下列说明完成工作；不要做范围外改动。\n\n\
             ## 任务说明\n{body}\n\n\
             全部完成后在最后一行输出：CCO_DONE ok\n"
        );
        tasks.push(TaskIR {
            id,
            title,
            depends_on,
            group: Some(format!("G{}", wave + 1)),
            provider: job.provider.clone(),
            mode: job.exec_mode.clone(),
            prompt,
            acceptance: None,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: opts.clone(),
            optional,
            include: !optional,
            role,
            scope,
            outputs,
        });
    }

    // P-loop L1: spec work-order / inspect tail → require_inspect so Unknown≡FAIL.
    let require_inspect = last_is_inspect
        || tasks.iter().any(|t| t.role == Some(TaskRole::Inspect));

    let mut ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name,
        adapter: "planner-ai-heuristic".into(),
        source_path: abs,
        max_parallel,
        on_failure: OnFailure::Pause,
        retry_max: 0,
        default_provider: job.provider.clone(),
        default_mode: job.exec_mode.clone(),
        worktree: false,
        require_inspect,
        tasks,
    };
    crate::plan::materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}

pub(super) fn split_sections(text: &str) -> Vec<(String, String)> {
    split_sections_level(text, /*include_h3=*/ true)
}

/// Split markdown by `##` (and optionally `###`) headings.
pub(super) fn split_sections_level(text: &str, include_h3: bool) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur_body = String::new();
    // When true, current heading is document chrome — do not accumulate body as a task.
    let mut skipping_meta = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        let heading = if include_h3 {
            trimmed
                .strip_prefix("### ")
                .map(|r| r.trim().to_string())
                .or_else(|| {
                    trimmed
                        .strip_prefix("## ")
                        .map(|r| r.trim().to_string())
                })
        } else {
            trimmed
                .strip_prefix("## ")
                .filter(|r| !r.starts_with('#')) // don't treat ### as ##
                .map(|r| r.trim().to_string())
        };

        if let Some(t) = heading {
            // Flush any open work section before switching.
            if let Some(prev) = cur_title.take() {
                sections.push((prev, cur_body.trim().to_string()));
                cur_body.clear();
            } else if !skipping_meta && !cur_body.trim().is_empty() {
                sections.push(("前言".into(), cur_body.trim().to_string()));
                cur_body.clear();
            } else {
                cur_body.clear();
            }

            if crate::plan::title_is_meta_heading(&t) {
                skipping_meta = true;
                cur_title = None;
                continue;
            }
            skipping_meta = false;
            cur_title = Some(t);
        } else if !skipping_meta {
            cur_body.push_str(line);
            cur_body.push('\n');
        }
    }
    if let Some(t) = cur_title {
        if !crate::plan::title_is_meta_heading(&t) {
            sections.push((t, cur_body.trim().to_string()));
        }
    } else if !skipping_meta && !cur_body.trim().is_empty() {
        sections.push(("全文".into(), cur_body.trim().to_string()));
    }
    sections.retain(|(title, b)| !b.is_empty() && !crate::plan::title_is_meta_heading(title));
    sections
}

/// Product / architecture / contract Markdown (not a short work-order).
pub(super) fn looks_like_spec_document(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let mut score = 0u32;
    for needle in [
        "非目标",
        "修订历史",
        "成功标准",
        "关联真源",
        "protocol",
        "handoff",
        "workerprovider",
        "planir",
        "阶段切分",
        "架构落点",
        "决策默认",
        "d5 池",
        "不排期则不碰",
        "schema: cco-plan",
        "[protocol]",
        "geb 入口",
        "附录 a",
        "附录 b",
    ] {
        if lower.contains(needle) {
            score += 1;
        }
    }
    // Many ## headings + long body → catalog-like
    let h2 = text.lines().filter(|l| l.trim_start().starts_with("## ")).count();
    if h2 >= 8 {
        score += 2;
    }
    if text.len() > 8_000 {
        score += 1;
    }
    score >= 3
}

/// When the MD is a spec, emit a sequential work DAG aligned to the plan-loop
/// (scope → breakdown with plan_ref → implement with evidence → inspect checklist).
/// See `docs/plan-execute-inspect-rework-2026-07-19.md` §3 / L0.
fn work_order_template_from_spec(text: &str) -> Vec<(String, String)> {
    let excerpt: String = text.chars().take(2_500).collect();
    vec![
        (
            "读懂目标与范围".into(),
            format!(
                "阅读下列计划/方案摘要，对齐**计划勾选真源**（§ 阶段表 / 成功标准 S* / 验证 V*）。\n\
                 用中文写入 `.cco-out/scope/SUMMARY.md`：\n\
                 - 目标\n\
                 - 范围内 / 范围外（引用计划非目标）\n\
                 - 必做勾选 ID 列表（F0/U0/S1…）\n\
                 - 验收标准（可判定命令或产物）\n\
                 不要修改业务代码。\n\n\
                 --- 文档摘录 ---\n{excerpt}\n--- 摘录结束 ---\n\
                 若工作区可读完整计划文件，优先读全文再总结。"
            ),
        ),
        (
            "拆出可执行工作包".into(),
            "根据 `.cco-out/scope/SUMMARY.md` 与计划勾选，列出 3–6 个可派工工作包。\n\
             每个工作包 **必须** 含：\n\
             - WP-id · 标题（动词开头：实现/修复/新增/验收…）\n\
             - **plan_ref**：对应计划勾选 ID（§x / S* / V*，可多对一）\n\
             - 改哪些路径（或「只读验证 + 重编」）\n\
             - 不做哪些（引用计划非目标；禁止 Board/非目标/修订历史/PROTOCOL 当包）\n\
             - 完成标志（测试命令、文件存在、勾选回写）\n\
             - 验收可否降级：默认 **否**；若可，写清等价条件与「降级后是否阻塞巡检」\n\
             硬规则：每个必做 plan_ref ≥1 个 WP；验收/重编/GEB 若计划要求不得静默省略。\n\
             写入 `.cco-out/work-breakdown/SUMMARY.md`。"
                .into(),
        ),
        (
            "按工作包落地".into(),
            "按 `.cco-out/work-breakdown/SUMMARY.md` 从第一个必做包开始实现。\n\
             每完成一项必须在 `.cco-out/progress/SUMMARY.md` 追加：\n\
             `plan_ref → 证据路径或命令`（禁止只写「完成了」）。\n\
             **禁止**把计划成功标准改写成更弱定义而不记 ISSUES（静默降级默认 **blocking**）。\n\
             范围外需求：写入 progress「拒做 + 归属非目标」。\n\
             遵守文档边界；不要扩大范围。"
                .into(),
        ),
        (
            "专门巡检对照计划".into(),
            "你是**检验员**（独立波，不是实现者自测）。对照**计划勾选**与 progress 证据：\n\
             1. 产出计划勾选对照表（嵌 VERDICT 或 CHECKLIST）：\n\
                | plan_ref | status(PASS|FAIL|SKIP|DEGRADED) | evidence | 备注 |\n\
             2. 写入 `.cco-out/inspect/VERDICT.md`：首行 `Result: PASS` 或 `Result: FAIL`。\n\
             3. 写入 `.cco-out/inspect/ISSUES.md`：每条含\n\
                id / severity=blocking|map|residual|out-of-scope / plan_ref / path / symptom / fix_wp\n\
             4. **禁止**存在未处理 blocking/map 时写 PASS。\n\
             5. residual（可选/不排期）可 PASS 附录；map（L1/L2 不同构）默认 blocking。\n\
             6. 验收被静默降级 → DEGRADED + severity=blocking（除非计划写明允许）。\n\
             7. 默认不改业务代码；只写 `.cco-out/inspect/**`。回补由 rework 波做。\n\
             跑仓库已有测试/检查（若有）。"
                .into(),
        ),
    ]
}

/// Fold many sections into at most `max` tasks by joining consecutive bodies.
/// When `sections.len() > max`, produces exactly `max` groups with roughly even size.
pub(super) fn merge_sections(sections: Vec<(String, String)>, max: usize) -> Vec<(String, String)> {
    if sections.is_empty() || max == 0 {
        return sections;
    }
    if sections.len() <= max {
        return sections;
    }
    let n = sections.len();
    let mut out: Vec<(String, String)> = Vec::with_capacity(max);
    for i in 0..max {
        let start = i * n / max;
        let end = (i + 1) * n / max;
        let part = &sections[start..end];
        if part.is_empty() {
            continue;
        }
        let title = if part.len() == 1 {
            part[0].0.clone()
        } else {
            format!("{} 等 {} 项", part[0].0, part.len())
        };
        let mut body = String::new();
        for (j, (t, b)) in part.iter().enumerate() {
            if j > 0 {
                body.push_str("\n\n");
            }
            if part.len() > 1 {
                body.push_str("## ");
                body.push_str(t);
                body.push('\n');
            }
            body.push_str(b);
        }
        out.push((title, body));
    }
    out
}

pub(super) fn chunk_prose(text: &str, max_parts: usize) -> Vec<(String, String)> {
    let paras: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with("---"))
        .collect();
    if paras.is_empty() {
        let t = text.trim();
        if t.is_empty() {
            return vec![];
        }
        return vec![("全文任务".into(), t.to_string())];
    }
    let n = max_parts.min(paras.len()).max(1);
    let chunk = (paras.len() + n - 1) / n;
    let mut out = Vec::new();
    for (i, part) in paras.chunks(chunk).enumerate() {
        if i >= max_parts {
            break;
        }
        let body = part.join("\n\n");
        let title = first_line_title(part[0], i);
        out.push((title, body));
    }
    out
}

pub(super) fn first_line_title(para: &str, idx: usize) -> String {
    let line = para.lines().next().unwrap_or("任务").trim();
    let cleaned = line
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(40)
        .collect::<String>();
    if cleaned.is_empty() {
        format!("任务 {}", idx + 1)
    } else {
        cleaned
    }
}
