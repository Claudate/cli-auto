//! System post-tasks appended after Mode B planning (not from the planner).
//!
//! [INPUT]: PlanIR · Config.post_inspect_enabled / post_git_push_enabled
//! [OUTPUT]: inject_system_post_tasks · fixed ids sys-post-inspect / sys-post-git-push
//! [POS]: plan/ 侧路；finish_plan_job 写 proposed 前调用
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md
//!
//! 规则：
//! - 不参与任务拆解；固定 id / 文案 / 依赖边
//! - 总是 `optional: true`；功能开启时 `include: true`（确认屏默认勾选）
//! - 功能关闭：不注入；若图上已有同 id 则剥离（避免旧图残留）
//! - 扩展：在 FEATURES 表增一项即可

use serde_json::json;

use crate::config::Config;
use crate::domain::plan::{
    normalize_optional_title, PlanIR, TaskIR, TaskRole, TaskScope, MAX_TASKS,
};
// Domain-owned ids + predicate (A1); re-export for plan::system_post callers.
pub use crate::domain::plan::{is_system_post_task, SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID};

const SYS_GROUP: &str = "系统收尾";

/// Append or strip system post-tasks according to config switches.
///
/// Call **after** planner output, **before** `write_proposed` / validate.
/// Idempotent: re-running replaces same-id tasks rather than duplicating.
pub fn inject_system_post_tasks(ir: &mut PlanIR, config: &Config) {
    // Strip previous system post-tasks so toggles / re-plan stay clean.
    ir.tasks.retain(|t| !is_system_post_task(&t.id));

    let want_push = config.default.post_git_push_enabled;
    // 开启 Push 时强制附带巡检门禁（先巡检通过才提交）
    let want_inspect = config.default.post_inspect_enabled || want_push;
    if !want_inspect && !want_push {
        return;
    }

    let business_ids: Vec<String> = ir
        .tasks
        .iter()
        .filter(|t| !is_system_post_task(&t.id))
        .map(|t| t.id.clone())
        .collect();

    // Need at least one business task as dependency anchor; otherwise attach
    // with empty depends (still valid for a lone system task plan).
    let business_deps = business_ids.clone();

    let slots = MAX_TASKS.saturating_sub(ir.tasks.len());
    let mut budget = slots;

    if want_inspect && budget > 0 {
        ir.tasks.push(make_inspect_task(ir, &business_deps));
        ir.require_inspect = true;
        budget = budget.saturating_sub(1);
    }

    if want_push && budget > 0 {
        // After inspect if present (inspect already waits on all business);
        // otherwise after all business tasks.
        let deps = if want_inspect
            || ir.tasks.iter().any(|t| t.id == SYS_POST_INSPECT_ID)
        {
            vec![SYS_POST_INSPECT_ID.to_string()]
        } else {
            business_deps
        };
        ir.tasks.push(make_git_push_task(ir, &deps));
    }

    crate::plan::materialize_role_defaults(ir);
}

fn make_inspect_task(ir: &PlanIR, depends_on: &[String]) -> TaskIR {
    let title = normalize_optional_title("任务巡检（系统）", true);
    let prompt = format!(
        r#"# 任务巡检（系统收尾 · 非业务拆解）

你是 cco **系统内置**巡检任务（`role=inspect`），**不是**实现者。

## 对照
- 原计划文档与本次已执行工作包
- 成功标准 / 验收勾选（计划内 § 或任务说明）

## 必须产出（仅可写报告目录）
- `.cco-out/inspect/VERDICT.md` — 一行总判：`PASS` / `FAIL` / `SKIP` + 简述
- `.cco-out/inspect/ISSUES.md` — 问题列表（无则写「无」）
  - 每条尽量含：严重度（blocking / non-blocking）、位置、复现/证据、建议

## 硬规则
1. **业务源码只读**；禁止为「刷绿」改应用代码
2. 只允许写入 `.cco-out/inspect/**`
3. 有阻塞遗漏 → VERDICT=FAIL，写清 ISSUES；不要假装 PASS
4. 本任务由 cco 设置「拆分后附加：任务巡检」注入；用户可在确认屏取消勾选

计划名：{name}
"#,
        name = ir.name
    );
    TaskIR {
        id: SYS_POST_INSPECT_ID.into(),
        title,
        depends_on: depends_on.to_vec(),
        group: Some(SYS_GROUP.into()),
        provider: ir.default_provider.clone(),
        mode: ir.default_mode.clone(),
        prompt,
        acceptance: Some(
            "存在 .cco-out/inspect/VERDICT.md 与 ISSUES.md；阻塞项必须 FAIL".into(),
        ),
        timeout_secs: Some(900),
        worktree: Some(false),
        provider_opts: json!({}),
        optional: true,
        include: true, // feature on → default checked
        role: Some(TaskRole::Inspect),
        scope: Some(TaskScope {
            paths: vec![".cco-out/inspect/**".into()],
            readonly: vec![],
            forbid: vec![],
        }),
        outputs: vec![
            ".cco-out/inspect/VERDICT.md".into(),
            ".cco-out/inspect/ISSUES.md".into(),
        ],
        tags: vec!["inspect".into(), "system".into()],
    }
}

fn make_git_push_task(ir: &PlanIR, depends_on: &[String]) -> TaskIR {
    let title = normalize_optional_title("代码提交 Push（系统）", true);
    let after_inspect = depends_on.iter().any(|d| d == SYS_POST_INSPECT_ID);
    let gate_block = if after_inspect {
        r#"
## 前置门禁（硬 · 先巡检通过才提交）
1. **先读** `.cco-out/inspect/VERDICT.md`（本任务依赖 `sys-post-inspect`，应已存在）
2. 仅当总判为 **PASS**（或明确等价：通过 / OK）时，才允许 `git add` / `commit` / `push`
3. 若为 **FAIL** / **SKIP** / 文件缺失 / 无法判定：
   - **禁止**任何 commit 或 push
   - 输出一行：`CCO_PUSH_SKIPPED reason=inspect_not_pass` + 摘录 VERDICT 首行
   - 任务以「已按门禁跳过提交」结束（不要为刷绿去改业务代码）
4. 若 ISSUES 含 blocking 项即使 VERDICT 写了 PASS，也视为未通过，跳过提交
"#
    } else {
        r#"
## 前置说明
当前未挂接系统巡检任务。仍先快速自检：`git status`；有明显未完成/失败痕迹时不要强行 push。
（推荐在设置中同时开启「任务巡检」，以便强制「先巡检通过再提交」。）
"#
    };
    let prompt = format!(
        r#"# 代码提交与 Push（系统收尾 · 非业务拆解）

你是 cco **系统内置**收尾任务，在业务任务{inspect_note}完成后执行。
{gate}

## 目标（仅门禁通过后）
1. 查看 `git status` / `git diff`（含 untracked）
2. 若有与本计划相关的变更：
   - `git add` 相关文件（**不要** add 密钥、`.env`、大二进制、无关垃圾）
   - 写清晰 commit message（中文或英文均可；首行 ≤72 字；说明「做了什么 / 为何」）
   - `git commit`
   - `git push` 到当前分支的 upstream（无 upstream 时：说明并尝试 `git push -u origin HEAD`，失败则报告原因）
3. 若工作区干净：在输出写明「无变更可提交」，**不要**空 commit
4. 成功 push 后输出一行：`CCO_PUSH_OK`

## 硬规则
1. **禁止** `git push --force` / `--force-with-lease`（除非用户在计划中明文要求）
2. **禁止**改 `git config` 全局项；不要改用户 name/email
3. 冲突 / 鉴权失败 → 停止并写清错误，不要循环重试
4. 本任务由 cco 设置「拆分后附加：代码提交 Push」注入；用户可在确认屏取消勾选
5. **巡检未通过绝不提交**（见前置门禁）

计划名：{name}
"#,
        name = ir.name,
        inspect_note = if after_inspect {
            "与巡检"
        } else {
            ""
        },
        gate = gate_block,
    );
    TaskIR {
        id: SYS_POST_GIT_PUSH_ID.into(),
        title,
        depends_on: depends_on.to_vec(),
        group: Some(SYS_GROUP.into()),
        provider: ir.default_provider.clone(),
        mode: ir.default_mode.clone(),
        prompt,
        acceptance: Some("有变更则已 commit+push，或明确说明无变更/失败原因".into()),
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: json!({}),
        optional: true,
        include: true,
        role: Some(TaskRole::Integrate),
        scope: None,
        outputs: vec![],
        tags: vec!["system".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::plan::{OnFailure, PlanIR, TaskIR};
    use std::path::PathBuf;

    fn sample_ir() -> PlanIR {
        PlanIR {
            schema: "cco-plan/v1".into(),
            name: "demo".into(),
            adapter: "test".into(),
            source_path: PathBuf::from("plans/demo.md"),
            max_parallel: 2,
            on_failure: OnFailure::Pause,
            retry_max: 0,
            default_provider: "claude".into(),
            default_mode: "print".into(),
            worktree: false,
            require_inspect: false,
            tasks: vec![TaskIR {
                id: "t1".into(),
                title: "实现".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "do work".into(),
                acceptance: None,
                timeout_secs: None,
                worktree: Some(false),
                provider_opts: json!({}),
                optional: false,
                include: true,
                role: None,
                scope: None,
                outputs: vec![],
            tags: vec![],
            }],
        }
    }

    #[test]
    fn off_by_default_injects_nothing() {
        let mut ir = sample_ir();
        let cfg = Config::default();
        assert!(!cfg.default.post_inspect_enabled);
        assert!(!cfg.default.post_git_push_enabled);
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 1);
        assert!(!ir.require_inspect);
    }

    #[test]
    fn both_on_appends_inspect_then_push() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_inspect_enabled = true;
        cfg.default.post_git_push_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 3);
        assert_eq!(ir.tasks[1].id, SYS_POST_INSPECT_ID);
        assert_eq!(ir.tasks[2].id, SYS_POST_GIT_PUSH_ID);
        assert!(ir.tasks[1].optional && ir.tasks[1].include);
        assert!(ir.tasks[2].optional && ir.tasks[2].include);
        assert_eq!(ir.tasks[1].depends_on, vec!["t1".to_string()]);
        assert_eq!(ir.tasks[2].depends_on, vec![SYS_POST_INSPECT_ID.to_string()]);
        assert!(ir.require_inspect);
        assert!(ir.tasks[1].role == Some(TaskRole::Inspect));
        // Push prompt must gate on VERDICT PASS
        assert!(
            ir.tasks[2].prompt.contains("PASS")
                && ir.tasks[2].prompt.contains("VERDICT")
                && ir.tasks[2]
                    .prompt
                    .contains("CCO_PUSH_SKIPPED"),
            "push must require inspect pass before commit"
        );
        ir.validate().expect("inspect→sys-post-push allowed by collab rules");
    }

    #[test]
    fn push_only_auto_adds_inspect_gate() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        // 只开 push：仍注入 inspect，push 依赖 inspect
        cfg.default.post_git_push_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 3);
        assert_eq!(ir.tasks[1].id, SYS_POST_INSPECT_ID);
        assert_eq!(ir.tasks[2].id, SYS_POST_GIT_PUSH_ID);
        assert_eq!(
            ir.tasks[2].depends_on,
            vec![SYS_POST_INSPECT_ID.to_string()]
        );
        assert!(ir.require_inspect);
        ir.validate().unwrap();
    }

    #[test]
    fn reinject_is_idempotent() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_inspect_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(
            ir.tasks.iter().filter(|t| t.id == SYS_POST_INSPECT_ID).count(),
            1
        );
    }

    #[test]
    fn turning_off_strips_previous() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_inspect_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 2);
        cfg.default.post_inspect_enabled = false;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 1);
        assert!(!ir.tasks.iter().any(|t| t.id == SYS_POST_INSPECT_ID));
    }
}
