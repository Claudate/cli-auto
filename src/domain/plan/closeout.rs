//! Inject sys-closeout + strip inspect ledger duties (Ensure E1).
//!
//! [INPUT]: PlanIR · auto_closeout flag · optional checklist paste
//! [OUTPUT]: inject_closeout_task (idempotent) · strip helpers
//! [POS]: domain/plan — pure; no Config IO
//! [PROTOCOL]: 触发条件变更须同步 materialize 单测（role=None 图）

use super::system_ids::is_system_post_task;
use super::types::{
    PlanIR, TaskIR, TaskRole, TaskScope, CLOSEOUT_DEFAULT_FORBID, CLOSEOUT_DEFAULT_WRITE_SCOPE,
    CLOSEOUT_SYSTEM_PROMPT, CLOSEOUT_SYSTEM_PROMPT_MARKER, SYS_CLOSEOUT_ID,
};

/// Heuristic: title/prompt looks like a terminal inspect/gate task.
pub fn looks_like_inspect_gate(task: &TaskIR) -> bool {
    if task.role == Some(TaskRole::Inspect) {
        return true;
    }
    let hay = format!("{} {}", task.title, task.prompt).to_ascii_lowercase();
    let tokens = [
        "门禁",
        "验收",
        "巡检",
        "inspect",
        "gates",
        "verdict",
        "对照计划",
    ];
    tokens
        .iter()
        .any(|t| hay.contains(&t.to_ascii_lowercase()) || hay.contains(t))
}

/// True when title/prompt still asks inspect to rewrite ledger / commit.
pub fn inspect_has_closeout_duty(task: &TaskIR) -> bool {
    let hay = format!("{} {}", task.title, task.prompt);
    let tokens = [
        "回写台账",
        "并回写",
        "勾选",
        "§9",
        "gap-audit",
        "commit",
        "回写进度",
        "更新台账",
    ];
    tokens.iter().any(|t| hay.contains(t))
}

/// Whether host should inject `sys-closeout` for this plan.
pub fn should_inject_closeout(plan: &PlanIR, auto_closeout: bool) -> bool {
    if !auto_closeout {
        return false;
    }
    if plan
        .tasks
        .iter()
        .any(|t| t.role == Some(TaskRole::Closeout) || t.id == SYS_CLOSEOUT_ID)
    {
        return false;
    }
    let business = business_tasks(plan);
    if business.is_empty() {
        return false;
    }
    let has_gate = plan.require_inspect || plan.tasks.iter().any(|t| looks_like_inspect_gate(t));
    has_gate
}

fn business_tasks(plan: &PlanIR) -> Vec<&TaskIR> {
    plan.tasks
        .iter()
        .filter(|t| {
            t.role != Some(TaskRole::Inspect)
                && t.role != Some(TaskRole::Closeout)
                && !is_system_post_task(&t.id)
                && t.id != SYS_CLOSEOUT_ID
        })
        .collect()
}

/// Inject closeout task and rewire inspect depends_on. Idempotent.
///
/// DAG: `[business…] → sys-closeout → [inspect / E3]`
///
/// `checklist_paste` is optional host table text embedded into closeout/inspect prompts.
pub fn inject_closeout_task(plan: &mut PlanIR, auto_closeout: bool, checklist_paste: Option<&str>) {
    if !should_inject_closeout(plan, auto_closeout) {
        // Still strip dual-duty wording when closeout already present or inject skipped
        // but inspect still carries ledger language.
        strip_all_inspect_closeout_duty(plan, checklist_paste);
        return;
    }

    strip_all_inspect_closeout_duty(plan, checklist_paste);

    let business_ids: Vec<String> = business_tasks(plan)
        .into_iter()
        .map(|t| t.id.clone())
        .collect();
    if business_ids.is_empty() {
        return;
    }

    let provider = plan.default_provider.clone();
    let mode = plan.default_mode.clone();
    let opts = plan
        .tasks
        .iter()
        .find(|t| t.role != Some(TaskRole::Inspect))
        .map(|t| t.provider_opts.clone())
        .unwrap_or_else(|| serde_json::json!({}));

    let checklist_block = checklist_paste.unwrap_or("（见 plan.checklist.json / 计划成功标准）");
    let prompt = format!(
        "{CLOSEOUT_SYSTEM_PROMPT}\n\n\
         ## 主机勾选清单（有证据才勾 ✅）\n\
         {checklist_block}\n\n\
         ## 必做\n\
         1. 读/跑已有 acceptance 证据（smoke、进度文件、测试输出）。\n\
         2. 绿 → 回写 ledger/map 类勾选、README 进度句、acceptance 索引断链。\n\
         3. 可 `git add` 相关文档 + `.cco-out/progress/**` 并 commit（信息含 plan_ref）。\n\
         4. 不绿 → **禁止**勾 ✅；只写 `.cco-out/progress/` 说明缺口。\n\
         5. **禁止**改业务源码（src/** 等）凑绿。\n\n\
         全部完成后最后一行：CCO_DONE ok\n"
    );

    let closeout = TaskIR {
        id: SYS_CLOSEOUT_ID.into(),
        title: "回写台账与验收索引（有证据才勾）".into(),
        depends_on: business_ids,
        group: Some("ensure".into()),
        provider,
        mode,
        prompt,
        verify_cmd: None,
        acceptance: Some("台账/索引与证据对齐；无证据不勾".into()),
        timeout_secs: None,
        worktree: None, // Inherit plan.worktree (multi-provider parallel requires it)
        provider_opts: opts,
        optional: false,
        include: true,
        role: Some(TaskRole::Closeout),
        scope: Some(TaskScope {
            paths: CLOSEOUT_DEFAULT_WRITE_SCOPE
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            readonly: vec!["**".into()],
            forbid: CLOSEOUT_DEFAULT_FORBID
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }),
        outputs: vec![
            ".cco-out/progress/SUMMARY.md".into(),
            ".cco-out/progress/CLOSEOUT.md".into(),
        ],
        tags: vec!["closeout".into(), "ensure".into()],
    };

    plan.tasks.push(closeout);

    // Only true inspect tasks wait on closeout. Do **not** rewire heuristic
    // gate-like business titles (「验收…」) — that would cycle: closeout→task→closeout.
    for t in &mut plan.tasks {
        if t.id == SYS_CLOSEOUT_ID {
            continue;
        }
        if t.role == Some(TaskRole::Inspect) {
            if !t.depends_on.iter().any(|d| d == SYS_CLOSEOUT_ID) {
                t.depends_on.push(SYS_CLOSEOUT_ID.into());
            }
        }
    }

    // Ensure closeout system prompt marker on provider_opts.
    if let Some(t) = plan.tasks.iter_mut().find(|t| t.id == SYS_CLOSEOUT_ID) {
        inject_closeout_system_prompt(&mut t.provider_opts);
    }
}

fn strip_all_inspect_closeout_duty(plan: &mut PlanIR, checklist_paste: Option<&str>) {
    let paste = checklist_paste.map(|s| s.to_string()).unwrap_or_default();
    for t in &mut plan.tasks {
        if t.role == Some(TaskRole::Inspect) || looks_like_inspect_gate(t) {
            strip_inspect_closeout_duty(t, &paste);
        }
    }
}

/// Rewrite inspect title/prompt so closeout is not dual-homed on the gate.
pub fn strip_inspect_closeout_duty(task: &mut TaskIR, checklist_paste: &str) {
    if inspect_has_closeout_duty(task) {
        // Soften title: drop 「并回写台账」 style dual duty.
        let mut title = task.title.clone();
        for bad in ["并回写台账", "并回写", "回写台账", "更新台账", "与 commit"] {
            title = title.replace(bad, "");
        }
        title = title
            .replace("（）", "")
            .replace("()", "")
            .trim()
            .trim_matches(|c| c == '·' || c == '-' || c == '—' || c == ' ')
            .to_string();
        if title.is_empty() || title.len() < 2 {
            title = "对照计划验收（只审计）".into();
        } else if !title.contains("验收") && !title.to_ascii_lowercase().contains("inspect") {
            title = format!("{title}（只验收对照）");
        }
        task.title = title;
    }

    // Append audit-only discipline + checklist (idempotent marker).
    const MARKER: &str = "CCO ensure E1/E3:";
    if !task.prompt.contains(MARKER) {
        let extra = format!(
            "\n\n{MARKER} 你只做对照清单审计与 VERDICT/ISSUES。\
             关账/回写台账/commit 由 `sys-closeout` 负责；**禁止**改业务源码凑 PASS。\n\
             ## 主机勾选清单\n\
             {checklist}\n",
            checklist = if checklist_paste.trim().is_empty() {
                "（见 plan.checklist.json）"
            } else {
                checklist_paste
            }
        );
        task.prompt.push_str(&extra);
    }
}

fn inject_closeout_system_prompt(opts: &mut serde_json::Value) {
    let existing = opts
        .get("append_system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if existing.contains(CLOSEOUT_SYSTEM_PROMPT_MARKER) {
        return;
    }
    let merged = if existing.trim().is_empty() {
        CLOSEOUT_SYSTEM_PROMPT.to_string()
    } else {
        format!("{existing}\n\n{CLOSEOUT_SYSTEM_PROMPT}")
    };
    opts["append_system_prompt"] = serde_json::json!(merged);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::types::OnFailure;
    use std::path::PathBuf;

    fn task(id: &str, role: Option<TaskRole>, title: &str, deps: &[&str]) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: title.into(),
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: format!("do {id}"),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role,
            scope: None,
            outputs: vec![],
            tags: vec![],
        }
    }

    fn plan(tasks: Vec<TaskIR>) -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "t".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("p.md"),
            max_parallel: 4,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "fake".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks,
        }
    }

    #[test]
    fn inject_on_role_none_business_plus_inspect() {
        // wros shape: implement role=None + terminal inspect dual-duty title
        let mut ir = plan(vec![
            task("t1", None, "实现 A", &[]),
            task("t2", None, "实现 B", &["t1"]),
            task(
                "t7-p0-gates",
                Some(TaskRole::Inspect),
                "门禁验收并回写台账",
                &["t2"],
            ),
        ]);
        inject_closeout_task(&mut ir, true, Some("| plan_ref | 台账 |"));
        assert!(
            ir.tasks.iter().any(|t| t.id == SYS_CLOSEOUT_ID),
            "closeout injected"
        );
        let co = ir.tasks.iter().find(|t| t.id == SYS_CLOSEOUT_ID).unwrap();
        assert_eq!(co.role, Some(TaskRole::Closeout));
        assert!(co.depends_on.iter().any(|d| d == "t1"));
        assert!(co.depends_on.iter().any(|d| d == "t2"));
        let insp = ir.tasks.iter().find(|t| t.id == "t7-p0-gates").unwrap();
        assert!(insp.depends_on.iter().any(|d| d == SYS_CLOSEOUT_ID));
        assert!(
            !insp.title.contains("回写台账"),
            "title stripped: {}",
            insp.title
        );
        assert!(insp.prompt.contains("CCO ensure E1/E3:"));
    }

    #[test]
    fn inject_idempotent() {
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Implement), "A", &[]),
            task("t7", Some(TaskRole::Inspect), "巡检", &[]),
        ]);
        inject_closeout_task(&mut ir, true, None);
        inject_closeout_task(&mut ir, true, None);
        assert_eq!(
            ir.tasks.iter().filter(|t| t.id == SYS_CLOSEOUT_ID).count(),
            1
        );
    }

    #[test]
    fn auto_closeout_off_skips() {
        let mut ir = plan(vec![
            task("t1", Some(TaskRole::Implement), "A", &[]),
            task("t7", Some(TaskRole::Inspect), "巡检", &[]),
        ]);
        inject_closeout_task(&mut ir, false, None);
        assert!(!ir.tasks.iter().any(|t| t.id == SYS_CLOSEOUT_ID));
    }

    #[test]
    fn no_gate_no_inject_even_if_auto() {
        let mut ir = plan(vec![task("t1", Some(TaskRole::Implement), "A", &[])]);
        inject_closeout_task(&mut ir, true, None);
        assert!(!ir.tasks.iter().any(|t| t.id == SYS_CLOSEOUT_ID));
    }
}
