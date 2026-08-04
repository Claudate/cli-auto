//! [CCO_HANDOFF] prompt prefix injection (P1-5 · A1-5 adapter).
//!
//! [INPUT]: TaskIR · run_dir handoff.json
//! [OUTPUT]: prompt prefix block; with_handoff_prefix idempotent wrap
//! [POS]: runtime/handoff
//! [PROTOCOL]: marker 常量勿静默改；缺失 handoff 仍注入 identity shell

use std::path::Path;

use crate::plan::TaskIR;

use super::model::{
    role_str, Handoff, HANDOFF_PROMPT_CLOSE, HANDOFF_PROMPT_OPEN, PREFIX_SUMMARY_CHARS,
};

/// Build the `[CCO_HANDOFF]…[/CCO_HANDOFF]` block for a task about to start.
///
/// Short summary only: identity + scope + outputs + Board table + depends_on
/// Fragments. Missing handoff file → empty Board/Fragments shell (no panic).
pub fn build_prompt_prefix(task: &TaskIR, run_dir: &Path) -> String {
    let role = role_str(task.role).unwrap_or_else(|| "-".into());
    let (paths, forbid) = match &task.scope {
        Some(s) => (s.paths.join(", "), s.forbid.join(", ")),
        None => (String::new(), String::new()),
    };
    let paths = if paths.is_empty() { "-".into() } else { paths };
    let forbid = if forbid.is_empty() {
        "-".into()
    } else {
        forbid
    };
    let deps = if task.depends_on.is_empty() {
        "-".into()
    } else {
        task.depends_on.join(", ")
    };
    let outputs = if task.outputs.is_empty() {
        "-".into()
    } else {
        task.outputs.join(", ")
    };
    let ledger = Handoff::path_md(run_dir).display().to_string();

    let handoff = if Handoff::path_json(run_dir).exists() {
        Handoff::load(run_dir).ok()
    } else {
        None
    };

    let mut body = String::new();
    body.push_str(HANDOFF_PROMPT_OPEN);
    body.push('\n');
    body.push_str(&format!(
        "你是 task={} provider={} role={}\n",
        task.id, task.provider, role
    ));
    body.push_str(&format!("scope.paths={paths}\n"));
    body.push_str(&format!("scope.forbid={forbid}\n"));
    body.push_str(&format!("必读: Board + Fragments(depends_on: {deps})\n"));
    body.push_str(&format!("全局账本: {ledger}\n"));
    body.push_str(&format!("你的 outputs: {outputs}\n"));
    body.push_str("完成后最后一行: CCO_DONE ok\n");
    // Observation-only mid-step progress (desktop parses these lines; not a second DAG).
    if wants_step_progress(task) {
        body.push_str("\n## 进度标记（观察用 · 必遵守）\n");
        body.push_str("1. 开工先列 3–7 条短清单（来自【自测】/怎样算做完），每条一行：`CCO_STEP todo: 简述`\n");
        body.push_str(
            "2. 同一时刻只推进一条：开始时 `CCO_STEP start: 简述`，完成时 `CCO_STEP done: 简述`\n",
        );
        body.push_str("3. 标记写在 stdout 普通文本行即可；不要为此改业务代码结构\n");
        body.push_str("4. 全部小步完成后仍以最后一行 `CCO_DONE ok` 收尾\n");
    }
    // H3-2: shallow discipline for integrate/inspect (no auto git merge).
    if matches!(
        task.role,
        Some(crate::plan::TaskRole::Integrate) | Some(crate::plan::TaskRole::Inspect)
    ) {
        body.push_str("\n## 拼在一起怎么验（纪律）\n");
        body.push_str("1. 先读各步 SUMMARY / Fragments，再下总判\n");
        body.push_str("2. 有失败或未完成的任务，不要装成全部成功\n");
        body.push_str("3. 合并/巡检后对照原计划验收；host 不会自动 git merge\n");
    }

    // Short Board table (status snapshot only).
    body.push_str("\n## Board\n");
    body.push_str("| id | provider | role | status | scope | outputs | notes |\n");
    body.push_str("|----|----------|------|--------|-------|---------|-------|\n");
    if let Some(h) = &handoff {
        if h.board.is_empty() {
            body.push_str("| - | - | - | - | - | - | (empty) |\n");
        } else {
            for r in &h.board {
                let r_role = r.role.as_deref().unwrap_or("-");
                let outs = if r.outputs.is_empty() {
                    "-".into()
                } else {
                    r.outputs.join(", ")
                };
                let scope = if r.scope.is_empty() {
                    "-"
                } else {
                    r.scope.as_str()
                };
                let notes: String = if r.notes.is_empty() {
                    "-".into()
                } else {
                    r.notes.chars().take(80).collect()
                };
                body.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | {} |\n",
                    r.id, r.provider, r_role, r.status, scope, outs, notes
                ));
            }
        }
    } else {
        body.push_str("| - | - | - | - | - | - | (no handoff yet) |\n");
    }

    // Only depends_on Fragments (not full ledger).
    body.push_str("\n## Fragments (depends_on)\n");
    if task.depends_on.is_empty() {
        body.push_str("_none_\n");
    } else if let Some(h) = &handoff {
        let mut any = false;
        for dep in &task.depends_on {
            if let Some(f) = h.fragments.get(dep) {
                any = true;
                body.push_str(&format!("### {dep}\n"));
                body.push_str(&format!(
                    "- status: {} · provider: {}\n",
                    f.status, f.provider
                ));
                if !f.summary.is_empty() {
                    let s: String = f.summary.chars().take(PREFIX_SUMMARY_CHARS).collect();
                    body.push_str(&format!("- summary: {s}\n"));
                }
                if !f.artifacts.is_empty() {
                    body.push_str(&format!("- artifacts: {}\n", f.artifacts.join(", ")));
                }
                if !f.risks.is_empty() {
                    body.push_str(&format!("- risks: {}\n", f.risks.join("; ")));
                }
            } else {
                body.push_str(&format!("### {dep}\n- (no fragment yet)\n"));
                any = true;
            }
        }
        if !any {
            body.push_str("_none_\n");
        }
    } else {
        for dep in &task.depends_on {
            body.push_str(&format!("### {dep}\n- (no handoff yet)\n"));
        }
    }

    body.push_str(HANDOFF_PROMPT_CLOSE);
    body.push('\n');
    body
}

/// Implement / Integrate / role-unset business tasks get mid-step CCO_STEP markers.
/// Scout / Inspect / Closeout / system post skip (different job shape).
fn wants_step_progress(task: &TaskIR) -> bool {
    if task.id.starts_with("sys-post-") || task.id == "sys-closeout" {
        return false;
    }
    match task.role {
        Some(crate::plan::TaskRole::Scout)
        | Some(crate::plan::TaskRole::Inspect)
        | Some(crate::plan::TaskRole::Closeout) => false,
        Some(crate::plan::TaskRole::Implement) | Some(crate::plan::TaskRole::Integrate) | None => {
            true
        }
    }
}

/// Prepend handoff summary to the business prompt. Idempotent if already wrapped.
///
/// On missing/corrupt handoff: still inject identity shell (never panics).
pub fn with_handoff_prefix(prompt: &str, task: &TaskIR, run_dir: &Path) -> String {
    if prompt.contains(HANDOFF_PROMPT_OPEN) {
        return prompt.to_string();
    }
    let prefix = build_prompt_prefix(task, run_dir);
    if prompt.trim().is_empty() {
        prefix
    } else {
        format!("{prefix}\n{prompt}")
    }
}

#[cfg(test)]
mod step_progress_tests {
    use super::*;
    use crate::plan::{TaskIR, TaskRole};

    fn bare(id: &str, role: Option<TaskRole>) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "fake".into(),
            mode: "print".into(),
            prompt: "x".into(),
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

    #[test]
    fn implement_gets_cco_step_contract() {
        let t = bare("t1", Some(TaskRole::Implement));
        let p = build_prompt_prefix(&t, Path::new("/tmp/no-run"));
        assert!(p.contains("CCO_STEP"), "{p}");
        assert!(p.contains("CCO_DONE ok"));
    }

    #[test]
    fn inspect_skips_cco_step_contract() {
        let t = bare("sys-post-inspect", Some(TaskRole::Inspect));
        let p = build_prompt_prefix(&t, Path::new("/tmp/no-run"));
        assert!(!p.contains("CCO_STEP todo"), "{p}");
    }
}
