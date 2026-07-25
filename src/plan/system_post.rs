//! System post-tasks appended after Mode B planning (not from the planner).
//!
//! [INPUT]: PlanIR · Config.post_inspect / post_git_push / post_open_pr
//! [OUTPUT]: inject_system_post_tasks · fixed ids inspect / git-push / open-pr
//! [POS]: plan/ 侧路；finish_plan_job 写 proposed 前调用
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md
//!
//! 规则：
//! - 不参与任务拆解；固定 id / 文案 / 依赖边
//! - 总是 `optional: true`；功能开启时 `include: true`（确认屏默认勾选）
//! - 功能关闭：不注入；若图上已有同 id 则剥离（避免旧图残留）
//! - 链：业务 → inspect（门禁）→ git-push → open-pr（S-PR，默认关）
//! - 扩展：在本文件增 make_* + 开关即可

use serde_json::json;

use crate::config::Config;
use crate::domain::plan::{
    normalize_optional_title, PlanIR, TaskIR, TaskRole, TaskScope, MAX_TASKS,
};
// Domain-owned ids + predicate (A1); re-export for plan::system_post callers.
pub use crate::domain::plan::{
    is_system_post_task, SYS_POST_GIT_PUSH_ID, SYS_POST_INSPECT_ID, SYS_POST_OPEN_PR_ID,
};

const SYS_GROUP: &str = "系统收尾";

/// Append or strip system post-tasks according to config switches.
///
/// Call **after** planner output, **before** `write_proposed` / validate.
/// Idempotent: re-running replaces same-id tasks rather than duplicating.
pub fn inject_system_post_tasks(ir: &mut PlanIR, config: &Config) {
    // Strip previous system post-tasks so toggles / re-plan stay clean.
    ir.tasks.retain(|t| !is_system_post_task(&t.id));

    let want_pr = config.default.post_open_pr_enabled;
    // Open-PR needs a pushed branch; force Push when PR is on.
    let want_push = config.default.post_git_push_enabled || want_pr;
    // Push (or PR) forces inspect gate — 先巡检通过才提交 / 开 PR
    let want_inspect = config.default.post_inspect_enabled || want_push;
    if !want_inspect && !want_push && !want_pr {
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
            business_deps.clone()
        };
        ir.tasks.push(make_git_push_task(ir, &deps));
        budget = budget.saturating_sub(1);
    }

    if want_pr && budget > 0 {
        // Prefer after push (branch on remote); else after inspect; else business.
        let deps = if ir.tasks.iter().any(|t| t.id == SYS_POST_GIT_PUSH_ID) {
            vec![SYS_POST_GIT_PUSH_ID.to_string()]
        } else if ir.tasks.iter().any(|t| t.id == SYS_POST_INSPECT_ID) {
            vec![SYS_POST_INSPECT_ID.to_string()]
        } else {
            business_deps
        };
        ir.tasks.push(make_open_pr_task(ir, &deps));
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
- `.cco-out/inspect/GATE.json` — 机器门：`{{"schema":"cco-inspect-gate/v1","result":"pass"|"fail","blocking":N,"map":N,"residual":N}}`
- `.cco-out/inspect/VERDICT.md` — 一行总判：`Result: PASS` / `Result: FAIL` + 简述
- `.cco-out/inspect/ISSUES.md` — 问题列表（无则写「无」）
  - 每条必须含：`severity=blocking|map|residual|out-of-scope`、plan_ref、path、symptom、fix_wp

## 严重度（写错会卡死整轮 · 必须遵守）
- **blocking**：功能/验收未落地、红测、编译失败、主路径不可用 → `result=fail` + blocking≥1
- **map**：台账/索引/L1 不同构 → map≥1（默认挡关账，走 closeout/回补）
- **residual（不挡 PASS）**：真书手点/30s 录像/截图未做、工作区未 commit、gitignore 卫生、可选 polish
- **禁止**把 residual 写成 blocking；仅 residual 时 **GATE result=pass**、blocking=0、residual=N

## 硬规则
1. **业务源码只读**；禁止为「刷绿」改应用代码
2. 只允许写入 `.cco-out/inspect/**`
3. 真阻塞遗漏 → VERDICT=FAIL + ISSUES；**可修的 blocking 写清 fix_wp**，交给回补波补齐（不是甩给用户）
4. 仅 residual → **必须 PASS**（附录 ISSUES），禁止 FAIL 卡轮
5. 本任务由 cco 设置「拆分后附加：任务巡检」注入；用户可在确认屏取消勾选

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
        // H0-3A: human criteria stay in prompt; host gate = outputs paths only.
        verify_cmd: None,
        acceptance: None,
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
        // H0-3A: no fake shell; worker prompt carries criteria (outputs empty by design).
        verify_cmd: None,
        acceptance: None,
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

fn make_open_pr_task(ir: &PlanIR, depends_on: &[String]) -> TaskIR {
    let title = normalize_optional_title("自动开 PR（系统）", true);
    let after_push = depends_on.iter().any(|d| d == SYS_POST_GIT_PUSH_ID);
    let after_inspect = depends_on.iter().any(|d| d == SYS_POST_INSPECT_ID);
    let chain_note = if after_push {
        "本任务依赖 `sys-post-git-push`（分支应已 push）。"
    } else if after_inspect {
        "本任务依赖巡检；未挂 Push 时请先确认当前分支已 push 到 origin。"
    } else {
        "请先确认当前分支已 push 到 origin，再开 PR。"
    };
    let prompt = format!(
        r#"# 自动开 Pull Request（系统收尾 · 非业务拆解）

你是 cco **系统内置**收尾任务（S-PR）：在业务{chain_suffix}完成后，用本机 **GitHub CLI `gh`** 开一个 PR。

{chain}

## 前置检查（任一失败 → 跳过，不要硬开）
1. `command -v gh` 可用；`gh auth status` 已登录（否则输出 `CCO_PR_SKIPPED reason=gh_not_ready`）
2. 当前目录是 git 仓库；有 `origin` remote（无则 `CCO_PR_SKIPPED reason=no_origin`）
3. 当前分支 **不是** 默认主干（main/master）——在主干上不要开 PR（`CCO_PR_SKIPPED reason=on_default_branch`）
4. 当前分支相对默认分支 **有 commits**；无差异 → `CCO_PR_SKIPPED reason=no_diff`
5. 若已有同 head 的 **open** PR：输出已有 URL + `CCO_PR_OK reused=1`，**不要**再 create

## 目标（检查通过后）
1. 确认 upstream 已 push（若 push 任务刚跑过应已有；否则 `git push -u origin HEAD`，**禁止** `--force`）
2. 用 `gh pr create` 开 PR：
   - base = 仓库默认分支（`gh repo view --json defaultBranchRef -q .defaultBranchRef.name`）
   - head = 当前分支
   - title：简短说明本计划做了什么（可用计划名「{name}」+ 一句摘要；≤72 字）
   - body：2–6 行中文或英文：背景 / 改动要点 / 如何自测；**不要**贴密钥、token、`.env`
3. 成功后输出一行：`CCO_PR_OK url=<pr_url>`
4. 失败：输出 `CCO_PR_SKIPPED reason=…` + 错误摘要，**不要**循环重试

## 硬规则（安全）
1. **禁止** `git push --force` / `--force-with-lease` / `gh pr merge` / 自动 merge
2. **禁止**改 `git config` 全局项；不要改用户 name/email
3. **禁止**把密钥、cookie、私钥、`.env` 内容写入 PR 描述
4. 仅本机已安装且已登录的 `gh`；**不要**调用未授权远程 HTTP API 旁路
5. 本任务由 cco 设置「拆分后附加：自动开 PR」注入；用户可在确认屏取消勾选
6. **默认关**：未开设置时不会出现本任务

计划名：{name}
"#,
        name = ir.name,
        chain = chain_note,
        chain_suffix = if after_push {
            "、巡检与 Push"
        } else if after_inspect {
            "与巡检"
        } else {
            ""
        },
    );
    TaskIR {
        id: SYS_POST_OPEN_PR_ID.into(),
        title,
        depends_on: depends_on.to_vec(),
        group: Some(SYS_GROUP.into()),
        provider: ir.default_provider.clone(),
        mode: ir.default_mode.clone(),
        prompt,
        // H0-3A: no fake shell; worker prompt carries CCO_PR_* markers.
        verify_cmd: None,
        acceptance: None,
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: json!({}),
        optional: true,
        include: true,
        role: Some(TaskRole::Integrate),
        scope: None,
        outputs: vec![],
        tags: vec!["system".into(), "pr".into()],
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
                verify_cmd: None,
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
        assert!(!cfg.default.post_open_pr_enabled);
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
        // H0-3A: no Chinese (or any) acceptance string that would fake-shell
        assert!(ir.tasks[1].acceptance.is_none());
        assert!(ir.tasks[2].acceptance.is_none());
        assert!(
            !ir.tasks[1].outputs.is_empty(),
            "inspect still has path outputs gate"
        );
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
    fn open_pr_only_auto_adds_inspect_and_push() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_open_pr_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 4, "inspect+push+pr");
        assert_eq!(ir.tasks[1].id, SYS_POST_INSPECT_ID);
        assert_eq!(ir.tasks[2].id, SYS_POST_GIT_PUSH_ID);
        assert_eq!(ir.tasks[3].id, SYS_POST_OPEN_PR_ID);
        assert!(ir.tasks[3].optional && ir.tasks[3].include);
        assert_eq!(
            ir.tasks[3].depends_on,
            vec![SYS_POST_GIT_PUSH_ID.to_string()]
        );
        assert!(
            ir.tasks[3].prompt.contains("CCO_PR_OK")
                && ir.tasks[3].prompt.contains("gh pr create")
                && ir.tasks[3].prompt.contains("禁止"),
            "pr prompt must document gh create + safety"
        );
        assert!(ir.require_inspect);
        ir.validate().expect("inspect→push→pr chain valid");
    }

    #[test]
    fn reinject_is_idempotent() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_inspect_enabled = true;
        cfg.default.post_open_pr_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(
            ir.tasks.iter().filter(|t| t.id == SYS_POST_INSPECT_ID).count(),
            1
        );
        assert_eq!(
            ir.tasks
                .iter()
                .filter(|t| t.id == SYS_POST_OPEN_PR_ID)
                .count(),
            1
        );
    }

    #[test]
    fn turning_off_strips_previous() {
        let mut ir = sample_ir();
        let mut cfg = Config::default();
        cfg.default.post_inspect_enabled = true;
        cfg.default.post_open_pr_enabled = true;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 4);
        cfg.default.post_inspect_enabled = false;
        cfg.default.post_open_pr_enabled = false;
        inject_system_post_tasks(&mut ir, &cfg);
        assert_eq!(ir.tasks.len(), 1);
        assert!(!ir.tasks.iter().any(|t| is_system_post_task(&t.id)));
    }
}
