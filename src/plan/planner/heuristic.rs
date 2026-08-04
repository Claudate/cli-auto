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
use crate::plan::{OnFailure, PlanIR, TaskIR, TaskRole, PLANNER_MAX_TASKS};

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
        let dep_note = if deps.is_empty() {
            String::new()
        } else {
            format!("\n依赖原因：等待产物来自 {}\n", deps.join("、"))
        };
        TaskIR {
            id: id.into(),
            title: title.clone(),
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
            group: Some(group.into()),
            provider: job.provider.clone(),
            mode: job.exec_mode.clone(),
            prompt: format!(
                "【模拟任务 {id}】{title}\n来源计划: {src_hint}\n{dep_note}\n{body}\n\n完成后输出一行: CCO_DONE ok\n"
            ),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: Some(120),
            worktree: Some(false),
            provider_opts: opts.clone(),
            optional,
            include: !optional,
            role: None,
            scope: None,
            outputs: vec![],
        tags: vec![],
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
    let raw =
        std::fs::read_to_string(&abs).with_context(|| format!("read plan {}", abs.display()))?;
    // Always drop prior split-summary write-back before any path — bare `### 波次 1`
    // junk must never compete with real #### P0-1 / A1 work packages.
    let text = strip_cco_split_summary_region(&raw);
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("planned")
        .to_string();
    let opts = default_provider_opts(config, &job.provider);

    let mut force_serial = false;

    // **Universal first path**: #### task ids (P0-1 / A1 / U1-1…), any doc shape.
    // Must not wait for looks_like_spec_document — landing plans with few chrome
    // needles used to fall through to ### 波次 1 summary stubs.
    // S0/S2: do NOT force_serial here — prefer 依赖 table / max_parallel batches.
    let mut from_task_ids = false;
    let task_id_phases = extract_task_id_headings(&text);
    let sections = if task_id_phases.len() >= 1 {
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic: {} #### task-id package(s) from plan (primary path)",
                task_id_phases.len()
            ),
        );
        from_task_ids = true;
        trim_phase_bodies(task_id_phases)
    } else {
        let mut sections = split_sections(&text);
        append_log(
            config,
            &job.job_id,
            &format!("heuristic found {} section(s)", sections.len()),
        );

        // Prefer ##-only boundaries when ### made the graph too fine-grained.
        if sections.len() > PLANNER_MAX_TASKS {
            let coarse = split_sections_level(&text, /*include_h3=*/ false);
            if coarse.len() > 1 && coarse.len() <= PLANNER_MAX_TASKS {
                append_log(
                    config,
                    &job.job_id,
                    &format!(
                        "heuristic coarsened to {} ## section(s) (was {}, max {PLANNER_MAX_TASKS})",
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

        // Spec / contract docs without #### task ids:
        // 1) Prefer work phases (W0/W1… / 波次 / 阶段) carved from the plan.
        // 2) If phase extract misses → diagnose + recover from headings.
        // 3) Meta template only when pure chrome.
        if looks_like_spec_document(&text) {
            let phases = extract_work_phases(&text);
            if !phases.is_empty() {
                append_log(
                    config,
                    &job.job_id,
                    &format!(
                        "heuristic: product/spec MD → {} work phase(s) from plan (not meta template)",
                        phases.len()
                    ),
                );
                force_serial = true;
                phases
            } else {
                let diag = diagnose_phase_extraction_miss(&text, sections.len());
                append_log(
                    config,
                    &job.job_id,
                    &format!("heuristic: work-phase extract failed — {diag}"),
                );
                let recovered = recover_actionable_sections(&text);
                if !recovered.is_empty() {
                    append_log(
                        config,
                        &job.job_id,
                        &format!(
                            "heuristic: recovered {} task(s) from plan headings (solved extract miss; not meta template)",
                            recovered.len()
                        ),
                    );
                    force_serial = true;
                    recovered
                } else {
                    append_log(
                        config,
                        &job.job_id,
                        &format!(
                            "heuristic: no recoverable work structure ({diag}) → last-resort work-order template"
                        ),
                    );
                    force_serial = true;
                    work_order_template_from_spec(&text)
                }
            }
        } else if sections.len() <= 1 {
            force_serial = true;
            chunk_prose(&text, 3)
        } else {
            // Still prefer wave slices with substance over bare heading soup.
            let waves = extract_wave_headings(&text);
            if waves.len() >= 2 {
                append_log(
                    config,
                    &job.job_id,
                    &format!("heuristic: {} wave slice(s) from plan", waves.len()),
                );
                force_serial = true;
                trim_phase_bodies(waves)
            } else {
                sections
            }
        }
    };

    let sections = if sections.len() > PLANNER_MAX_TASKS {
        let merged = merge_sections(sections, PLANNER_MAX_TASKS);
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic merged into {} task(s) to satisfy max {PLANNER_MAX_TASKS}",
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
    // #### packages: honor 依赖 column when present; else max_parallel batches (S2).
    let max_parallel = job
        .max_parallel
        .unwrap_or(config.default.max_parallel)
        .clamp(1, 32);

    let (table_deps, has_dep_info) = if from_task_ids {
        crate::domain::plan::cco_split::resolve_deps_from_sections(&sections)
    } else {
        (vec![], false)
    };
    let use_table_deps = from_task_ids && has_dep_info;
    let use_batch_parallel = from_task_ids && !has_dep_info;
    if use_table_deps {
        append_log(
            config,
            &job.job_id,
            "heuristic: #### packages → depends from plan 依赖 column (not serial chain)",
        );
        force_serial = false;
    } else if use_batch_parallel {
        append_log(
            config,
            &job.job_id,
            &format!(
                "heuristic: #### packages → no 依赖 column; batch by max_parallel={max_parallel}"
            ),
        );
        force_serial = false;
    }

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
        let depends_on = if use_table_deps {
            table_deps.get(i).cloned().unwrap_or_default()
        } else {
            let wave = i / wave_size;
            if wave == 0 {
                vec![]
            } else {
                // Barrier: wait for the previous wave so at most wave_size run together.
                let start = (wave - 1) * wave_size;
                let end = wave * wave_size;
                (start..end).map(|j| format!("t{}", j + 1)).collect()
            }
        };
        let optional = crate::plan::title_looks_optional(title);
        let title = crate::plan::normalize_optional_title(title, optional);
        let title = crate::domain::plan::cco_split::display_title(&title);
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
        let acceptance = crate::domain::plan::cco_split::parse_done_when(body);
        let dep_note = if depends_on.is_empty() {
            String::new()
        } else {
            format!(
                "\n## 依赖原因\n等待前置步骤产物：{}\n",
                depends_on.join("、")
            )
        };
        // W2-3: no worker-identity scaffold (desk + exec share human work orders).
        let prompt = if dep_note.is_empty() {
            format!("{body}\n\n完成后输出一行: CCO_DONE ok\n")
        } else {
            format!("{body}{dep_note}\n完成后输出一行: CCO_DONE ok\n")
        };
        tasks.push(TaskIR {
            id,
            title,
            depends_on,
            group: Some(format!("G{}", i + 1)),
            provider: job.provider.clone(),
            mode: job.exec_mode.clone(),
            prompt,
            verify_cmd: acceptance
                .as_ref()
                .filter(|s| crate::domain::plan::is_runnable_verify(s))
                .cloned(),
            acceptance,
            timeout_secs: None,
            worktree: Some(false),
            provider_opts: opts.clone(),
            optional,
            include: !optional,
            role,
            scope,
            outputs,
            tags: vec![],
        });
    }

    // Recompute group by topo wave for display (after depends set).
    if use_table_deps || use_batch_parallel {
        // group filled after validate via soft_accept waves; keep G{i} ok for now
    }

    // P-loop L1: spec work-order / inspect tail → require_inspect so Unknown≡FAIL.
    let require_inspect =
        last_is_inspect || tasks.iter().any(|t| t.role == Some(TaskRole::Inspect));

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
    crate::plan::apply_tag_routing(&mut ir);
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
                .or_else(|| trimmed.strip_prefix("## ").map(|r| r.trim().to_string()))
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
        // Landing / 派工 plans (PilotDeck / nondev) — fewer classic chrome needles
        "任务表",
        "完成定义",
        "落地实施",
        "勾选只认",
        "波次 p0",
        "波次 p1",
        "实施真源",
    ] {
        if lower.contains(needle) {
            score += 1;
        }
    }
    // Many ## headings + long body → catalog-like
    let h2 = text
        .lines()
        .filter(|l| l.trim_start().starts_with("## "))
        .count();
    if h2 >= 8 {
        score += 2;
    }
    if text.len() > 8_000 {
        score += 1;
    }
    // #### task-id packages strongly imply a 派工 plan even when chrome score is low.
    if extract_task_id_headings(text).len() >= 2 {
        score += 2;
    }
    score >= 3
}

/// Explain why `extract_work_phases` returned empty — for planner log / operators.
/// Does not invent tasks; diagnosis only.
fn diagnose_phase_extraction_miss(text: &str, leftover_heading_sections: usize) -> String {
    let task_ids = extract_task_id_headings(text);
    let mut h3 = split_sections_level(text, /*include_h3=*/ true);
    h3.retain(|(t, b)| !b.trim().is_empty() && !crate::plan::title_is_meta_heading(t));
    let phase_like: Vec<_> = h3
        .iter()
        .filter(|(t, _)| title_looks_like_work_phase(t))
        .map(|(t, _)| t.clone())
        .collect();
    let wave_like: Vec<_> = h3
        .iter()
        .filter(|(t, _)| title_looks_like_wave_slice(t))
        .map(|(t, _)| t.clone())
        .collect();
    let non_meta_n = h3.len();
    let meta_only = leftover_heading_sections == 0 && non_meta_n == 0;

    let mut bits = Vec::new();
    if meta_only {
        bits.push("document is mostly chrome (no non-meta heading with body)".into());
    }
    if !task_ids.is_empty() {
        bits.push(format!(
            "found {} #### task-id heading(s) but extract discarded them (unexpected)",
            task_ids.len()
        ));
    } else {
        // Count #### lines that did not pass title_looks_like_task_id
        let mut hash4 = 0usize;
        let mut hash4_non_task = 0usize;
        for line in text.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("#### ") {
                hash4 += 1;
                if !title_looks_like_task_id(rest.trim()) {
                    hash4_non_task += 1;
                }
            }
        }
        if hash4 > 0 {
            bits.push(format!(
                "{hash4} #### heading(s), {hash4_non_task} not matching task-id pattern (A1/B2/U1-1…)"
            ));
        } else {
            bits.push("no #### task-id headings (A1/B2/…)".into());
        }
    }
    if phase_like.is_empty() {
        bits.push("no W0/阶段/窗-style phase titles".into());
    } else {
        bits.push(format!(
            "{} phase-like title(s) present but not selected: {}",
            phase_like.len(),
            phase_like
                .into_iter()
                .take(4)
                .collect::<Vec<_>>()
                .join(" · ")
        ));
    }
    if wave_like.is_empty() {
        bits.push("no ### 波次 / Wave slices".into());
    } else {
        bits.push(format!(
            "{} wave-like title(s): {}",
            wave_like.len(),
            wave_like
                .into_iter()
                .take(3)
                .collect::<Vec<_>>()
                .join(" · ")
        ));
    }
    if non_meta_n > 0 {
        bits.push(format!(
            "{non_meta_n} non-meta heading section(s) available for recovery"
        ));
    }
    bits.join("; ")
}

/// After phase extract fails: use remaining plan headings as tasks (solve, don't abandon).
/// Drops meta chrome; only keeps sections that look like real work (title or body).
/// Pure product chrome (一句话 / 账本 / 非目标 only) returns empty so caller can use
/// last-resort meta template — still after logging the failure reason.
fn recover_actionable_sections(text: &str) -> Vec<(String, String)> {
    let mut sections = split_sections_level(text, /*include_h3=*/ true);
    sections.retain(|(title, body)| {
        !body.trim().is_empty()
            && !crate::plan::title_is_meta_heading(title)
            && !title_is_recover_chrome(title)
    });

    let workish: Vec<_> = sections
        .iter()
        .filter(|(t, b)| {
            title_looks_like_work_phase(t)
                || title_looks_like_wave_slice(t)
                || title_looks_like_task_id(t)
                || title_looks_like_implement_heading(t)
                || section_body_looks_actionable(b)
        })
        .cloned()
        .collect();
    if !workish.is_empty() {
        return trim_phase_bodies(workish);
    }

    // Coarser ## with actionable body (no ###).
    let mut coarse = split_sections_level(text, /*include_h3=*/ false);
    coarse.retain(|(title, body)| {
        !body.trim().is_empty()
            && !crate::plan::title_is_meta_heading(title)
            && !title_is_recover_chrome(title)
            && (title_looks_like_work_phase(title)
                || title_looks_like_implement_heading(title)
                || section_body_looks_actionable(body))
    });
    if !coarse.is_empty() {
        return trim_phase_bodies(coarse);
    }
    Vec::new()
}

/// Body suggests implementable work (not a table-only chrome blurb).
fn section_body_looks_actionable(body: &str) -> bool {
    let b = body.trim();
    if b.chars().count() < 40 {
        return false;
    }
    // Pipe-heavy tables alone (Board / 勾选表) are not work packages.
    let lines: Vec<&str> = b.lines().filter(|l| !l.trim().is_empty()).collect();
    let pipe_lines = lines.iter().filter(|l| l.matches('|').count() >= 2).count();
    if !lines.is_empty() && pipe_lines * 2 >= lines.len() {
        return false;
    }
    for needle in [
        "改法",
        "文件",
        "完成定义",
        "自测",
        "依赖",
        "**ID**",
        "sessionEntry",
        "index.html",
        "web/js",
        "src/",
        "- [ ]",
        "- [x]",
        "实现",
        "修复",
        "落地",
        "confirm_start",
        "resolveEntryRoute",
    ] {
        if b.contains(needle) {
            return true;
        }
    }
    false
}

/// Extra chrome titles that must not be recovered as work (even if not in title_is_meta).
fn title_is_recover_chrome(title: &str) -> bool {
    let t = title.trim();
    let lower = t.to_ascii_lowercase();
    t.contains("账本")
        || t.contains("一句话")
        || t.contains("为什么做")
        || t.contains("问题陈述")
        || lower.contains("overview")
        || (t.contains("阶段") && t.contains("勾选"))
}

/// Verb / implement-style section titles when not W/A1 patterned.
fn title_looks_like_implement_heading(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() || crate::plan::title_is_meta_heading(t) {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    // Numbered chrome like "3.5 账本" is NOT implement work.
    if t.contains("账本")
        || lower.contains("board")
        || lower.contains("timeline")
        || t.contains("勾选表")
        || t.contains("决策")
    {
        return false;
    }
    for needle in [
        "实现",
        "修复",
        "改造",
        "接入",
        "落地",
        "拆分",
        "确认",
        "路由",
        "顶栏",
        "入口",
        "聊天",
        "结果",
        "验收",
        "回归",
        "implement",
        "fix",
        "add ",
        "wire",
        "refactor",
    ] {
        if t.contains(needle) || lower.contains(needle) {
            return true;
        }
    }
    // "步骤 3 · 改入口" ok; bare "3.5 账本" / "6. 阶段切分" alone is not.
    if t.contains("步骤") || t.contains("任务") {
        return true;
    }
    false
}

/// Pull real work windows from a landing / phase plan (W0/W1…, 阶段, 窗, #### A1).
/// Returns empty when the doc has no actionable phases — caller diagnoses + recovers,
/// and only then may use the meta work-order template (product chrome only).
pub(super) fn extract_work_phases(text: &str) -> Vec<(String, String)> {
    // Strip prior split-summary write-back first — it invents bare `### 波次 1` junk
    // that used to beat real #### P0-1 tasks when meta chrome swallowed task ids.
    let text = strip_cco_split_summary_region(text);
    let text = text.as_str();
    // 0) Task-id headings first (#### A1 · … / #### P0-1 · … / #### U1-1 · …).
    // Landing/派工 plans write implementation as #### tasks under ### 波次 — not W0 windows.
    // Without this, looks_like_spec_document docs fall into the meta 4-wave template
    // (读懂目标 → 拆包 → 落地 → 巡检) and ignore the real checklist.
    let task_ids = extract_task_id_headings(text);
    if task_ids.len() >= 2 {
        return trim_phase_bodies(task_ids);
    }
    if task_ids.len() == 1 {
        return trim_phase_bodies(task_ids);
    }

    // 1) Prefer ### (W0/W1 under ## 分期) then ##; drop meta chrome.
    let mut candidates = split_sections_level(text, /*include_h3=*/ true);
    candidates.retain(|(title, body)| {
        !body.trim().is_empty()
            && !crate::plan::title_is_meta_heading(title)
            && title_looks_like_work_phase(title)
    });
    if candidates.len() >= 2 {
        return trim_phase_bodies(candidates);
    }
    // Coarser: ## only that look like work (e.g. "## 阶段 1 · 实现 handoff").
    let mut coarse = split_sections_level(text, /*include_h3=*/ false);
    coarse.retain(|(title, body)| {
        !body.trim().is_empty()
            && !crate::plan::title_is_meta_heading(title)
            && title_looks_like_work_phase(title)
    });
    if coarse.len() >= 2 {
        return trim_phase_bodies(coarse);
    }
    // Single explicit work window still better than meta template when present.
    if candidates.len() == 1 {
        return trim_phase_bodies(candidates);
    }
    if coarse.len() == 1 {
        return trim_phase_bodies(coarse);
    }

    // 2) ### 波次 A/B/C as coarser work slices when no #### task ids and no W-windows.
    let waves = extract_wave_headings(text);
    if waves.len() >= 2 {
        return trim_phase_bodies(waves);
    }
    if waves.len() == 1 {
        return trim_phase_bodies(waves);
    }
    Vec::new()
}

/// `#### A1 · title` / `#### B2 · …` / `#### U1-1 · …` / `#### PR1 · …` / `#### S0 · …` / `#### P0-1 · …`
/// Body = lines until next #### or ### / ## heading.
/// Skips `<!-- cco-split-summary -->` regions (prior bad splits written back into the plan).
fn extract_task_id_headings(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur_body = String::new();
    let mut in_split_summary = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.contains("cco-split-summary:start") {
            // Flush open task before junk region.
            if let Some(prev) = cur_title.take() {
                let body = cur_body.trim().to_string();
                if !body.is_empty() {
                    out.push((prev, body));
                }
                cur_body.clear();
            }
            in_split_summary = true;
            continue;
        }
        if trimmed.contains("cco-split-summary:end") {
            in_split_summary = false;
            continue;
        }
        if in_split_summary {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            let title = rest.trim().to_string();
            if let Some(prev) = cur_title.take() {
                let body = cur_body.trim().to_string();
                if !body.is_empty() {
                    out.push((prev, body));
                }
                cur_body.clear();
            } else {
                cur_body.clear();
            }
            // Domain SoT: work task ids are never meta (P0-1 · …, A1 · …).
            if title_looks_like_task_id(&title) {
                cur_title = Some(title);
            } else {
                cur_title = None;
            }
            continue;
        }
        // Higher-level headings close the current #### task body.
        if trimmed.starts_with("### ") || trimmed.starts_with("## ") {
            if let Some(prev) = cur_title.take() {
                let body = cur_body.trim().to_string();
                if !body.is_empty() {
                    out.push((prev, body));
                }
                cur_body.clear();
            }
            continue;
        }
        if cur_title.is_some() {
            cur_body.push_str(line);
            cur_body.push('\n');
        }
    }
    if let Some(prev) = cur_title {
        let body = cur_body.trim().to_string();
        if !body.is_empty() {
            out.push((prev, body));
        }
    }
    out
}

/// True for landing-plan task titles: `A1 · …`, `P0-1 · …`, `U1-1 ·`, `S0 ·`, `PR1 ·`.
fn title_looks_like_task_id(title: &str) -> bool {
    crate::plan::looks_like_work_task_id(title)
}

/// `### 波次 A — …` / `### 波次 P0 — …` as coarse implementation slices.
/// Ignores prior split-summary junk (`### 波次 1` with only a checkbox line).
fn extract_wave_headings(text: &str) -> Vec<(String, String)> {
    // Prefer body above cco-split-summary so a previous bad write-back cannot win.
    let text = strip_cco_split_summary_region(text);
    let mut sections = split_sections_level(&text, /*include_h3=*/ true);
    sections.retain(|(title, body)| {
        !body.trim().is_empty()
            && !crate::plan::title_is_meta_heading(title)
            && title_looks_like_wave_slice(title)
            && !wave_body_is_summary_junk(body)
    });
    // Prefer real 派工 waves (`波次 P0 — …`) over bare `波次 1` stubs.
    let rich: Vec<_> = sections
        .iter()
        .filter(|(t, _)| wave_title_has_substance(t))
        .cloned()
        .collect();
    if rich.len() >= 2 {
        return rich;
    }
    sections
}

/// Drop `<!-- cco-split-summary:start -->…end -->` so heuristic never sees prior empty waves.
fn strip_cco_split_summary_region(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skip = false;
    for line in text.lines() {
        let t = line.trim();
        if t.contains("cco-split-summary:start") {
            skip = true;
            continue;
        }
        if t.contains("cco-split-summary:end") {
            skip = false;
            continue;
        }
        if skip {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn wave_title_has_substance(title: &str) -> bool {
    let t = title.trim();
    // `波次 P0 — …` / `波次 A — …` / `Wave 1 · …` vs bare `波次 1`
    if t.contains('—') || t.contains('–') || t.contains('·') || t.contains(':') || t.contains('：')
    {
        return true;
    }
    // letter after 波次 / Wave (P0, A, B…)
    let lower = t.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("波次")
        .or_else(|| lower.strip_prefix("wave"))
    {
        let rest = rest.trim();
        return rest
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic())
            .unwrap_or(false);
    }
    t.chars().count() > 8
}

/// Bodies that are only a single checkbox / empty — from split-summary write-back.
fn wave_body_is_summary_junk(body: &str) -> bool {
    let b = body.trim();
    if b.is_empty() {
        return true;
    }
    // only checkbox lines and whitespace
    let meaningful: Vec<_> = b
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("<!--"))
        .collect();
    if meaningful.is_empty() {
        return true;
    }
    if meaningful.len() <= 2
        && meaningful.iter().all(|l| {
            l.starts_with("- [")
                || l.starts_with("* [")
                || l.starts_with("- [x]")
                || l.starts_with("- [ ]")
                || *l == "- [ ]"
                || l.starts_with("- ☐")
                || l.starts_with("☐")
        })
    {
        return true;
    }
    // no nested #### task and very short → junk summary
    !b.contains("#### ") && b.chars().count() < 80
}

fn title_looks_like_wave_slice(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    // 波次 A / 波次 P0 / Wave A / Wave 1
    if t.contains("波次") {
        return true;
    }
    if lower.starts_with("wave ") || lower.starts_with("wave") {
        return lower.chars().any(|c| c.is_ascii_digit())
            || lower.contains(" a")
            || lower.contains(" b")
            || lower.contains(" c")
            || lower.contains(" d")
            || lower.ends_with('a')
            || lower.ends_with('b')
            || lower.ends_with('c')
            || lower.ends_with('d');
    }
    false
}

/// W0 / 阶段1 / 窗 / Phase 2 / Sprint — actionable implementation slices.
fn title_looks_like_work_phase(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    // W0 · W1 · W2 (Chinese landing plans) / Phase N / Sprint N / 阶段 N / 第N窗
    // W0 / w0 / W10 at start or after separators
    let w_phase = regex_is_work_window(t);
    if w_phase {
        return true;
    }
    if lower.contains("phase ")
        || lower.starts_with("phase")
        || lower.contains("sprint")
        || t.contains("阶段")
        || t.contains("分期")
        || t.contains("里程碑")
        || t.contains("迭代")
        || t.contains("交付窗")
        || t.contains("实现窗")
        || (t.contains("窗") && (t.contains("W") || t.chars().any(|c| c.is_ascii_digit())))
    {
        // Exclude pure status / freeze tables
        if t.contains("冻结面") || t.contains("状态表") || t.contains("修订") {
            return false;
        }
        return true;
    }
    // Checkbox-heavy body sections titled with verbs often are work packages
    // when parent was a phase heading — already filtered by title_is_meta.
    false
}

fn regex_is_work_window(title: &str) -> bool {
    // Match W0 / W1 / W10 / w0 at word-ish boundaries without pulling in "Board".
    let chars: Vec<char> = title.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i + 1 < n {
        let c = chars[i];
        let is_w = c == 'W' || c == 'w';
        if is_w && chars[i + 1].is_ascii_digit() {
            let prev_ok = i == 0
                || chars[i - 1].is_whitespace()
                || matches!(
                    chars[i - 1],
                    '·' | '•' | '—' | '-' | '/' | '|' | '（' | '(' | '【' | '['
                );
            if prev_ok {
                // consume digits
                let mut j = i + 1;
                while j < n && chars[j].is_ascii_digit() {
                    j += 1;
                }
                // require at least one digit and not part of a longer token like "Write"
                let next_ok = j >= n
                    || chars[j].is_whitespace()
                    || matches!(
                        chars[j],
                        '·' | '•'
                            | '—'
                            | '-'
                            | '/'
                            | '|'
                            | '）'
                            | ')'
                            | '】'
                            | ']'
                            | '：'
                            | ':'
                            | '，'
                            | ','
                    )
                    || !chars[j].is_ascii_alphabetic();
                if next_ok {
                    return true;
                }
            }
        }
        i += 1;
    }
    // 阶段 0 / 阶段1 / 第 1 阶段
    if title.contains("阶段") {
        return title.chars().any(|c| c.is_ascii_digit())
            || title.contains("一")
            || title.contains("二")
            || title.contains("三")
            || title.contains("四")
            || title.contains("五");
    }
    false
}

fn trim_phase_bodies(mut sections: Vec<(String, String)>) -> Vec<(String, String)> {
    // Cap body size so worker prompts stay readable; full plan is still on disk.
    const MAX_CHARS: usize = 3_500;
    for (_, body) in sections.iter_mut() {
        if body.chars().count() > MAX_CHARS {
            let clipped: String = body.chars().take(MAX_CHARS).collect();
            *body = format!("{clipped}\n\n…（正文已截断；请读计划全文对应章节，勿只依赖摘录）");
        }
        // Prepend a short host contract so workers implement the phase, not re-plan.
        let contract = "\
按**本阶段/本窗**交付，不要另起「读范围→拆包→巡检」元流程。\n\
- 对照本段清单与完成判据 / Acceptance 实现或验收。\n\
- 不做范围外与非目标；不把修订历史/Board 当任务。\n\
- 有验收命令则跑通；改代码须可回看证据。\n\n\
--- 本阶段说明 ---\n";
        *body = format!("{contract}{body}");
    }
    // Cap task count for planner limit.
    if sections.len() > PLANNER_MAX_TASKS {
        sections = merge_sections(sections, PLANNER_MAX_TASKS);
    }
    sections
}

/// When the MD is pure product chrome (no recoverable work structure), emit the
/// meta work-order (scope → breakdown → implement → inspect).
/// See `docs/plan-execute-inspect-rework-2026-07-19.md` §3 / L0.
/// **Last resort only** — caller must have logged why extract+recover failed.
/// **Do not** use this when phases/recovery already found real work.
fn work_order_template_from_spec(text: &str) -> Vec<(String, String)> {
    let excerpt: String = text.chars().take(2_500).collect();
    let reason = diagnose_phase_extraction_miss(text, 0);
    vec![
        (
            "读懂目标与范围".into(),
            format!(
                "【规划器说明 · 最后手段】未能从计划中识别可派工标题，原因：{reason}。\n\
                 这不是「忽略你的任务表」的成功路径；若文档里其实有 #### A1 / W0 / 波次 等任务，\
                 请改标题后点「重新拆分」，或检查规划日志。\n\n\
                 阅读下列计划/方案摘要，对齐**计划勾选真源**（§ 阶段表 / 成功标准 S* / 验证 V*）。\n\
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
             **缺资源默认补齐**：缺真图/素材时 → 搜索可溯源图库（Unsplash/Pexels/Pixabay）或生成后**下载落盘**并改引用路径；禁止仅用几何 SVG 顶「真实感商品/场景图」、禁止改验收定义过关。\n\
             范围外需求：写入 progress「拒做 + 归属非目标」。\n\
             遵守文档边界；不要扩大范围。"
                .into(),
        ),
        (
            "专门巡检对照计划".into(),
            "你是**检验员**（独立波，不是实现者自测）。对照**计划勾选**与 progress 证据：\n\
             1. 产出计划勾选对照表（嵌 VERDICT 或 CHECKLIST）：\n\
                | plan_ref | status(PASS|FAIL|SKIP|DEGRADED) | evidence | 备注 |\n\
             2. 写入 `.cco-out/inspect/GATE.json`：`result=pass|fail` + blocking/map/residual 计数。\n\
             3. 写入 `.cco-out/inspect/VERDICT.md`：首行 `Result: PASS` 或 `Result: FAIL`。\n\
             4. 写入 `.cco-out/inspect/ISSUES.md`：每条含\n\
                id / severity=blocking|map|residual|out-of-scope / plan_ref / path / symptom / fix_wp\n\
                · 手点/录像/未 commit/未引用脚手架 CSS = **residual**（不得 blocking；仅 residual 时 GATE pass）\n\
                · 真功能缺口 / **计划意图静默降级**（例：真实感图→仅插画 SVG）= **blocking** + 可执行 fix_wp（缺图写「搜图落盘改路径」）\n\
             5. **禁止**存在未处理 blocking/map 时写 PASS；仅 residual 时必须 GATE pass。\n\
             6. residual（手点/录像/未 commit/死 CSS/可选）可 PASS 附录；map 默认 blocking。\n\
             7. 验收被静默降级 → DEGRADED + severity=blocking（除非计划写明允许）。\n\
             8. 默认不改业务代码；只写 `.cco-out/inspect/**`。真缺口由 rework 波补齐，不甩给用户。\n\
             9. **禁止**回写台账/勾选/commit（关账由 host `sys-closeout` 或落地波负责）。\n\
             10. 用户可见类勾选禁止仅用字符串扫描/HTTP 200 结案；rework 优先用户可见缺口，卫生项不得顶替。\n\
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
