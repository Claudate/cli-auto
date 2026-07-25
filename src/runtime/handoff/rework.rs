//! Rework plan builder · residual accept · inspect loop view (A1-5 adapter).
//!
//! Strategy constants (REWORK_MAX_ROUNDS · MAP whitelist) come from domain::inspect.
//! Path IO and PlanIR materialize stay here.
//!
//! [INPUT]: PlanIR · ParsedIssue · RunState · project_root
//! [OUTPUT]: rework PlanIR · handoff residual note · InspectLoopView
//! [POS]: runtime/handoff
//! [PROTOCOL]: rework 仍开新 run；不改原 DAG inspect 终端语义

use std::path::Path;

use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::inspect::{
    all_blocking_are_docs_closeout, can_start_rework, count_blocking_issues, count_residual_issues,
    parse_issues_text, parse_verdict_text, InspectVerdict, IssueSeverity, ParsedIssue,
    INSPECT_ISSUES_REL, INSPECT_VERDICT_REL, MAP_REWORK_PATH_WHITELIST, REWORK_MAX_ROUNDS,
};
use crate::plan::{PlanIR, TaskIR, TaskRole};
use crate::state::{RunState, RunStatus};

use super::inspect_io::{load_parsed_inspect_issues, read_inspect_verdict};
use super::model::Handoff;

/// Count prior rework waves recorded under project `.cco-out/rework/` or handoff timeline.
pub fn count_rework_rounds(project_root: &Path, run_dir: &Path) -> u32 {
    let rework_dir = project_root.join(".cco-out/rework");
    let mut n = 0u32;
    if rework_dir.is_dir() {
        if let Ok(rd) = std::fs::read_dir(&rework_dir) {
            for ent in rd.flatten() {
                let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
                if name.starts_with("round") && (name.ends_with(".md") || name.ends_with(".json"))
                {
                    n += 1;
                }
            }
        }
    }
    if n == 0 {
        // Fallback: handoff timeline markers
        if let Ok(h) = Handoff::load(run_dir) {
            n = h
                .timeline
                .iter()
                .filter(|l| l.contains("rework_wave") || l.contains("REWORK_WAVE"))
                .count() as u32;
        }
    }
    n
}

/// Build a sequential rework PlanIR: one implement (or map-scoped) task + terminal inspect.
///
/// Does **not** attach rework as downstream of inspect in the original DAG (inspect stays terminal);
/// starts a **new run** wave that pastes ISSUES + plan_ref (R-rework-2).
pub fn build_rework_plan(
    base: &PlanIR,
    issues: &[ParsedIssue],
    round: u32,
    source_run_id: &str,
) -> Result<PlanIR> {
    if issues.is_empty() {
        bail!("no ISSUES to rework");
    }
    let blocking: Vec<&ParsedIssue> = issues
        .iter()
        .filter(|i| i.severity.is_blocking_for_gate())
        .collect();
    let target: Vec<&ParsedIssue> = if blocking.is_empty() {
        // Still allow rework of residual if user explicitly requested (rare).
        issues.iter().collect()
    } else {
        blocking
    };

    let only_map = target.iter().all(|i| i.severity == IssueSeverity::Map);
    let provider = base.default_provider.clone();
    let mode = base.default_mode.clone();
    let opts = base
        .tasks
        .first()
        .map(|t| t.provider_opts.clone())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut issues_body = String::new();
    for i in &target {
        issues_body.push_str(&format!(
            "### {}\n- severity: {}\n- plan_ref: {}\n- path: {}\n- symptom: {}\n- fix_wp: {}\n\n```\n{}\n```\n\n",
            i.id,
            i.severity.as_str(),
            i.plan_ref,
            i.path,
            i.symptom,
            i.fix_wp,
            i.raw
        ));
    }

    let scope_paths: Vec<String> = if only_map {
        MAP_REWORK_PATH_WHITELIST
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        let mut paths: Vec<String> = vec![
            ".cco-out/progress/**".into(),
            ".cco-out/rework/**".into(),
        ];
        for i in &target {
            if i.path != "n/a" && !i.path.is_empty() {
                paths.push(i.path.clone());
            }
        }
        // Broad implement fallback when paths unknown — worker still bound by prompt.
        if paths.len() <= 2 {
            paths.push("**".into());
        }
        paths
    };

    let rework_id = format!("rework-r{round}");
    let inspect_id = format!("reinspect-r{round}");
    let title = if only_map {
        format!("回补地图指针（第 {round} 轮）")
    } else {
        format!("回补阻塞遗漏（第 {round} 轮）")
    };

    let rework_prompt = format!(
        "你是回补实现者（rework wave），不是检验员。\n\
         来源 run: {source_run_id}\n\
         轮次: {round}/{REWORK_MAX_ROUNDS}\n\n\
         ## 必须粘贴的 ISSUES 原文（禁止空话「再检查一下」）\n\
         {issues_body}\n\
         ## 任务\n\
         1. 按每条 fix_wp / plan_ref 修改代码或允许的文档路径。\n\
         2. map / docs-closeout 类仅改 GEB/文档指针、台账勾选、README 进度、acceptance 索引（CLAUDE.md、docs/**、.cco-out/progress/**）。\n\
         3. 每完成一条在 `.cco-out/progress/SUMMARY.md` 追加：`plan_ref → 证据`。\n\
         4. 写 `.cco-out/rework/ROUND-{round}.md`：改了什么、对应 ISSUE id。\n\
         5. 文档/台账修复完成后：按需 `git add` 相关 md 与 progress，并 `git commit`（信息含 ISSUE id 与 plan_ref）。\n\
         6. 不要扩大范围；非目标不实现；禁止无证据勾 ✅。\n\n\
         全部完成后最后一行：CCO_DONE ok\n"
    );

    let inspect_prompt = format!(
        "你是检验员（inspect），二次巡检（回补后）。\n\
         对照上轮 ISSUES 与计划勾选，只验下列项是否已清：\n\
         {issues_body}\n\
         ## 必做\n\
         1. 写出计划勾选对照表（plan_ref | PASS|FAIL|SKIP|DEGRADED | 证据）。\n\
         2. 写入 `.cco-out/inspect/VERDICT.md`：首行 **Result: PASS** 或 **Result: FAIL**。\n\
         3. 写入 `.cco-out/inspect/ISSUES.md`：每条含 severity=blocking|map|residual|out-of-scope、plan_ref、path、symptom、fix_wp。\n\
         4. **禁止**在存在未处理 blocking/map 时写 PASS。\n\
         5. residual 可附录；不得伪装成「没问题」。\n\
         6. 默认不改业务代码；只写 `.cco-out/inspect/**`。\n\n\
         最后一行：CCO_DONE ok\n"
    );

    let rework_task = TaskIR {
        id: rework_id.clone(),
        title,
        depends_on: vec![],
        group: Some(format!("rework-{round}")),
        provider: provider.clone(),
        mode: mode.clone(),
        prompt: rework_prompt,
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: opts.clone(),
        optional: false,
        include: true,
        role: Some(TaskRole::Implement),
        scope: Some(crate::plan::TaskScope {
            paths: scope_paths,
            readonly: vec![],
            forbid: if only_map {
                vec!["src/**".into(), "web/**".into(), "src-tauri/**".into()]
            } else {
                vec![]
            },
        }),
        outputs: vec![
            format!(".cco-out/rework/ROUND-{round}.md"),
            ".cco-out/progress/SUMMARY.md".into(),
        ],
        tags: vec!["rework".into()],
    };

    let inspect_task = TaskIR {
        id: inspect_id,
        title: format!("回补后巡检（第 {round} 轮）"),
        depends_on: vec![rework_id],
        group: Some(format!("rework-{round}")),
        provider: provider.clone(),
        mode: mode.clone(),
        prompt: inspect_prompt,
        verify_cmd: None,
        acceptance: None,
        timeout_secs: None,
        worktree: Some(false),
        provider_opts: opts,
        optional: false,
        include: true,
        role: Some(TaskRole::Inspect),
        scope: Some(crate::plan::TaskScope {
            paths: vec![INSPECT_VERDICT_REL.into(), ".cco-out/inspect/**".into()],
            readonly: vec!["**".into()],
            forbid: vec![],
        }),
        outputs: vec![INSPECT_VERDICT_REL.into(), INSPECT_ISSUES_REL.into()],
        tags: vec!["inspect".into(), "rework".into()],
    };

    let mut ir = PlanIR {
        schema: "cco-plan/v1".into(),
        name: format!("{}-rework-r{round}", base.name),
        adapter: "rework-wave".into(),
        source_path: base.source_path.clone(),
        max_parallel: 1,
        on_failure: base.on_failure,
        retry_max: 0,
        default_provider: provider,
        default_mode: mode,
        worktree: base.worktree,
        require_inspect: true,
        tasks: vec![rework_task, inspect_task],
    };
    crate::plan::materialize_role_defaults(&mut ir);
    ir.validate()?;
    Ok(ir)
}

/// Append ACCEPTED_RESIDUAL note to handoff open_risks (P-loop Q7). Does not flip run status.
pub fn accept_residual_on_handoff(plan: &PlanIR, state: &RunState, note: &str) -> Result<()> {
    let mut h = super::lifecycle::load_or_init(plan, state)?;
    h.updated = Utc::now();
    let line = if note.trim().is_empty() {
        format!(
            "ACCEPTED_RESIDUAL: user accepted remaining open risks at {}",
            Utc::now().to_rfc3339()
        )
    } else {
        format!(
            "ACCEPTED_RESIDUAL: {} ({})",
            note.trim().chars().take(300).collect::<String>(),
            Utc::now().to_rfc3339()
        )
    };
    if !h
        .open_risks
        .iter()
        .any(|r| r.starts_with("ACCEPTED_RESIDUAL:"))
    {
        h.open_risks.push(line.clone());
    } else {
        // refresh note
        h.open_risks
            .retain(|r| !r.starts_with("ACCEPTED_RESIDUAL:"));
        h.open_risks.push(line.clone());
    }
    h.push_timeline(format!(
        "{} · accepted_residual · {}",
        Utc::now().to_rfc3339(),
        note.chars().take(80).collect::<String>()
    ));
    h.instructions_for_next = format!(
        "- {line}\n- blocking items were explicitly accepted; do not treat as pure PASS\n"
    );
    h.save(&state.run_dir)
}

/// Snapshot for desktop / live view (P-loop L2 + Ensure E3/E4).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InspectLoopView {
    pub verdict: Option<String>,
    pub blocking_count: usize,
    pub residual_count: usize,
    pub issue_preview: Vec<String>,
    pub can_rework: bool,
    pub rework_round: u32,
    pub rework_max: u32,
    pub accepted_residual: bool,
    pub require_inspect: bool,
    /// When host auto-started a rework wave from this run (Ensure E3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_rework_run_id: Option<String>,
    /// Ensure phase hint for UI: audit | closeout | reinspect | rework.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ensure_phase: Option<String>,
    /// True when all current blocking ISSUES are docs-closeout (DTO only; UI must not re-classify).
    #[serde(default)]
    pub docs_closeout_only: bool,
}

/// Build inspect-loop summary from project inspect products + handoff.
pub fn inspect_loop_view(
    plan: Option<&PlanIR>,
    state: &RunState,
    project_root: &Path,
) -> InspectLoopView {
    let require_inspect = plan.map(|p| p.require_inspect).unwrap_or(false);
    let rework_round = count_rework_rounds(project_root, &state.run_dir);
    let mut view = InspectLoopView {
        require_inspect,
        rework_round,
        rework_max: REWORK_MAX_ROUNDS,
        ..Default::default()
    };

    // Prefer role=inspect task; else conventional paths.
    let inspect_task = plan.and_then(|p| {
        p.tasks
            .iter()
            .rev()
            .find(|t| t.role == Some(TaskRole::Inspect))
    });

    let work_dir = state.project_root.as_path();
    let verdict = if let Some(t) = inspect_task {
        read_inspect_verdict(t, work_dir, project_root)
    } else {
        // Conventional path only
        let path = project_root.join(INSPECT_VERDICT_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_verdict_text(&t))
                .unwrap_or(InspectVerdict::Unknown)
        } else {
            InspectVerdict::Unknown
        }
    };
    view.verdict = match verdict {
        InspectVerdict::Pass => Some("PASS".into()),
        InspectVerdict::Fail => Some("FAIL".into()),
        InspectVerdict::Unknown => {
            if project_root.join(INSPECT_VERDICT_REL).is_file() {
                Some("UNKNOWN".into())
            } else {
                None
            }
        }
    };

    let parsed = if let Some(t) = inspect_task {
        load_parsed_inspect_issues(t, work_dir, project_root)
    } else {
        let path = project_root.join(INSPECT_ISSUES_REL);
        if path.is_file() {
            std::fs::read_to_string(&path)
                .map(|t| parse_issues_text(&t))
                .unwrap_or_default()
        } else {
            vec![]
        }
    };
    view.blocking_count = count_blocking_issues(&parsed);
    view.residual_count = count_residual_issues(&parsed);
    view.issue_preview = parsed
        .iter()
        .take(8)
        .map(|i| {
            format!(
                "{} severity={} {}",
                i.id,
                i.severity.as_str(),
                i.symptom.chars().take(100).collect::<String>()
            )
        })
        .collect();

    if let Ok(h) = Handoff::load(&state.run_dir) {
        view.accepted_residual = h
            .open_risks
            .iter()
            .any(|r| r.starts_with("ACCEPTED_RESIDUAL:"));
        if view.issue_preview.is_empty() {
            view.issue_preview = h
                .open_risks
                .iter()
                .filter(|r| r.contains("ISSUES[") || r.contains("REWORK_HOOK"))
                .take(6)
                .cloned()
                .collect();
        }
    }

    let run_is_terminal = matches!(
        state.status,
        RunStatus::Paused | RunStatus::Failed | RunStatus::Completed | RunStatus::Aborted
    );
    view.can_rework = can_start_rework(
        verdict,
        view.blocking_count,
        require_inspect,
        view.accepted_residual,
        rework_round,
        run_is_terminal,
        view.verdict.as_deref(),
    );
    view.docs_closeout_only =
        view.blocking_count > 0 && all_blocking_are_docs_closeout(&parsed);

    // Ensure E3 marker written by app::run::ensure_loop.
    let marker = state.run_dir.join("auto_rework.json");
    if marker.is_file() {
        if let Some(v) = std::fs::read_to_string(&marker)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            view.auto_rework_run_id = v
                .get("auto_rework_run_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            view.ensure_phase = v
                .get("ensure_phase")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some("rework".into()));
        }
    } else if plan
        .map(|p| p.tasks.iter().any(|t| t.role == Some(TaskRole::Closeout)))
        .unwrap_or(false)
    {
        view.ensure_phase = Some("closeout".into());
    } else if view.verdict.is_some() {
        view.ensure_phase = Some("audit".into());
    }

    view
}
