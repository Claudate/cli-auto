//! Lightweight plan document digest + rule critic for Mode B planner.
//!
//! [INPUT]: plan markdown text · proposed TaskIR graph
//! [OUTPUT]: PlanDigest · mode · sanitize_task_deps · critic_plan_tasks
//! [POS]: planner 子模块；llm prompt + finish_plan_job 消费
//! note: critic 为确定性规则（无第二跳 LLM）；回归模式改标题/钉 prompt/清假依赖
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/plan/CLAUDE.md

use crate::plan::{TaskIR, TaskRole};

/// How the planner should treat the source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlanModeKind {
    /// Header / checklist says already landed — verify + residual only.
    Regression,
    /// From-scratch delivery.
    Greenfield,
    /// Inspect / audit only.
    Audit,
    /// Mixed signals.
    Mixed,
}

impl PlanModeKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            PlanModeKind::Regression => "regression",
            PlanModeKind::Greenfield => "greenfield",
            PlanModeKind::Audit => "audit",
            PlanModeKind::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PlanDigest {
    pub mode: PlanModeKind,
    pub title: String,
    pub landed_hint: bool,
    pub phase_lines: Vec<String>,
    pub non_goal_lines: Vec<String>,
    pub success_lines: Vec<String>,
    pub char_count: usize,
}

/// Build a short structural digest from plan markdown (no LLM).
pub(super) fn build_plan_digest(text: &str) -> PlanDigest {
    let char_count = text.chars().count();
    let title = first_h1(text).unwrap_or_else(|| "（无标题）".into());
    let head = text.chars().take(2_500).collect::<String>();

    // Prefer the document's own status line over mentions of other closed work.
    // e.g. "未实施 … 不阻塞 D0–D4 已闭环" must NOT count as this doc landed.
    let status_line = text
        .lines()
        .take(40)
        .map(str::trim)
        .find(|l| {
            l.contains("状态") && (l.contains("已") || l.contains("未") || l.contains("定稿"))
        })
        .unwrap_or("")
        .to_string();
    let status_open = !status_line.is_empty()
        && (status_line.contains("未实施")
            || status_line.contains("待排期")
            || status_line.contains("待落地")
            || status_line.contains("未完成")
            || (status_line.contains("定稿")
                && !status_line.contains("已落地")
                && !status_line.contains("已闭环")
                && !status_line.contains("主线已落地")));
    let status_landed = !status_line.is_empty()
        && !status_open
        && (status_line.contains("已落地")
            || status_line.contains("主线已落地")
            || status_line.contains("已闭环")
            || status_line.contains("全 PASS")
            || status_line.contains("全绿"));

    // Fallback when no 状态 line: only strong whole-doc claims, not cross-refs.
    let body_landed = status_line.is_empty()
        && (head.contains("**已落地**")
            || head.contains("主线已落地")
            || head.contains("H0–H4 已落地")
            || head.contains("H0-H4 已落地")
            || head.to_ascii_lowercase().contains("already landed"));

    let landed_hint = status_landed || body_landed;

    let mut phase_lines: Vec<String> = Vec::new();
    let mut non_goal_lines: Vec<String> = Vec::new();
    let mut success_lines: Vec<String> = Vec::new();
    let mut in_non_goals = false;
    let mut in_success = false;

    for line in text.lines().take(400) {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // section switches
        if t.starts_with("## ") {
            let h = t.trim_start_matches('#').trim();
            in_non_goals = h.contains("非目标") || h.to_ascii_lowercase().contains("non-goal");
            in_success = h.contains("成功")
                || h.contains("验收")
                || h.starts_with("S") && h.contains("标准");
            if h.starts_with('H')
                || h.contains("阶段")
                || h.starts_with("### H")
                || (h.len() <= 40 && (h.contains("H0") || h.contains("H1")))
            {
                if phase_lines.len() < 12 {
                    phase_lines.push(t.chars().take(100).collect());
                }
            }
            continue;
        }
        if t.starts_with("### H") || t.starts_with("###H") {
            if phase_lines.len() < 12 {
                phase_lines.push(t.chars().take(100).collect());
            }
            continue;
        }
        // checklist lines that look like phase ids
        if (t.starts_with("- [")
            || t.starts_with("* [")
            || t.starts_with("- [x]")
            || t.starts_with("- [X]"))
            && (t.contains("H0")
                || t.contains("H1")
                || t.contains("H2")
                || t.contains("H3")
                || t.contains("H4")
                || t.contains("G0")
                || t.contains("U0")
                || t.contains("L0"))
        {
            if phase_lines.len() < 16 {
                phase_lines.push(t.chars().take(120).collect());
            }
        }
        if in_non_goals && (t.starts_with('-') || t.starts_with('*')) && non_goal_lines.len() < 8 {
            non_goal_lines.push(t.chars().take(100).collect());
        }
        if in_success
            && (t.starts_with('-') || t.starts_with('*') || t.starts_with('|'))
            && success_lines.len() < 10
        {
            success_lines.push(t.chars().take(100).collect());
        }
    }

    // checked ratio for phase-like lines
    let checked = phase_lines
        .iter()
        .filter(|l| l.contains("[x]") || l.contains("[X]"))
        .count();
    let unchecked = phase_lines.iter().filter(|l| l.contains("[ ]")).count();
    let mostly_checked = checked >= 3 && checked > unchecked;

    let mode = if head.contains("只读")
        && (head.contains("检验") || head.contains("巡检") || head.contains("audit"))
    {
        PlanModeKind::Audit
    } else if status_open {
        // Explicit open status wins over cross-references to other closed work.
        PlanModeKind::Greenfield
    } else if landed_hint || mostly_checked {
        PlanModeKind::Regression
    } else if phase_lines.is_empty() && !landed_hint {
        PlanModeKind::Greenfield
    } else if landed_hint && unchecked > checked {
        PlanModeKind::Mixed
    } else {
        PlanModeKind::Greenfield
    };

    PlanDigest {
        mode,
        title,
        landed_hint: landed_hint && !status_open,
        phase_lines,
        non_goal_lines,
        success_lines,
        char_count,
    }
}

fn first_h1(text: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let s = rest.trim();
            if !s.is_empty() {
                return Some(s.chars().take(80).collect());
            }
        }
    }
    None
}

/// Compact text block for planner system prompt.
pub(super) fn format_digest_for_prompt(d: &PlanDigest) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "mode: {}\nlanded_hint: {}\ntitle: {}\nchars: {}\n",
        d.mode.as_str(),
        d.landed_hint,
        d.title,
        d.char_count
    ));
    if !d.phase_lines.is_empty() {
        out.push_str("phases_or_checks:\n");
        for l in &d.phase_lines {
            out.push_str("  ");
            out.push_str(l);
            out.push('\n');
        }
    }
    if !d.non_goal_lines.is_empty() {
        out.push_str("non_goals:\n");
        for l in &d.non_goal_lines {
            out.push_str("  ");
            out.push_str(l);
            out.push('\n');
        }
    }
    if !d.success_lines.is_empty() {
        out.push_str("success_hints:\n");
        for l in &d.success_lines {
            out.push_str("  ");
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// Drop dependency edges that look like "document order" without real coupling signals.
/// Keeps edges when the dependent task prompt mentions the dependency id/title, or
/// when depend_reason-like text exists in the prompt (「依赖原因」「等待产物」).
pub(super) fn sanitize_task_deps(tasks: &mut [TaskIR]) {
    let ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    let titles: Vec<(String, String)> = tasks
        .iter()
        .map(|t| (t.id.clone(), t.title.clone()))
        .collect();

    for t in tasks.iter_mut() {
        let prompt_l = t.prompt.to_ascii_lowercase();
        let prompt_raw = t.prompt.clone();
        t.depends_on.retain(|dep| {
            if !ids.iter().any(|id| id == dep) {
                return false;
            }
            // Always keep if prompt cites the dep id
            if prompt_raw.contains(dep) {
                return true;
            }
            // Keep if prompt cites dependency title (short titles only)
            if let Some((_, title)) = titles.iter().find(|(id, _)| id == dep) {
                if title.chars().count() >= 4 && prompt_raw.contains(title) {
                    return true;
                }
            }
            // Keep if explicit reason markers exist near depend language
            if prompt_raw.contains("依赖原因")
                || prompt_raw.contains("等待产物")
                || prompt_raw.contains("depends on")
                || prompt_l.contains("blocked by")
            {
                // Only keep if this dep id is listed somewhere in depends-related lines
                return prompt_raw.lines().any(|line| {
                    let l = line.trim();
                    (l.contains("依赖") || l.contains("depend") || l.contains("等待"))
                        && l.contains(dep.as_str())
                });
            }
            // Regression-style orthogonal phases: drop bare edges with no mention
            false
        });
    }
}

/// Result of rule-based critic pass over a proposed task graph.
#[derive(Debug, Clone, Default)]
pub(super) struct CriticReport {
    pub edges_removed: usize,
    pub titles_rewritten: usize,
    pub prompts_tagged: usize,
    pub notes: Vec<String>,
}

impl CriticReport {
    pub(super) fn summary_line(&self) -> String {
        let mut parts = Vec::new();
        if self.edges_removed > 0 {
            parts.push(format!("去掉 {} 条可疑依赖", self.edges_removed));
        }
        if self.titles_rewritten > 0 {
            parts.push(format!("改写 {} 个标题为回归验证", self.titles_rewritten));
        }
        if self.prompts_tagged > 0 {
            parts.push(format!("钉入 {} 条只读提示", self.prompts_tagged));
        }
        for n in &self.notes {
            if !parts
                .iter()
                .any(|p| p.contains(n.as_str()) || n.contains(p.as_str()))
            {
                parts.push(n.clone());
            }
        }
        if parts.is_empty() {
            "critic：无需改动".into()
        } else {
            format!("critic：{}", parts.join(" · "))
        }
    }
}

const REGRESSION_BANNER: &str =
    "文档声明相关阶段已落地。默认只读验证；仅 ISSUES 中 severity=blocking 才改代码。";

fn looks_like_implement_title(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() {
        return false;
    }
    // Already verify-oriented
    if t.contains("回归")
        || t.contains("验证")
        || t.contains("核对")
        || t.contains("检验")
        || t.contains("巡检")
        || t.contains("终检")
        || t.contains("只读")
    {
        return false;
    }
    t.contains("实现")
        || t.contains("落地")
        || t.contains("从零")
        || t.contains("完整实施")
        || t.contains("全量实施")
        || (t.contains("补齐") && !t.contains("残差"))
}

fn rewrite_regression_title(title: &str) -> String {
    let t = title.trim();
    let stripped = t
        .trim_start_matches("实现")
        .trim_start_matches("落地")
        .trim_start_matches("从零")
        .trim_start_matches("完整")
        .trim_start_matches("全量")
        .trim_start_matches('·')
        .trim_start_matches(' ')
        .trim_start_matches('：')
        .trim_start_matches(':')
        .trim();
    let body = if stripped.is_empty() { t } else { stripped };
    format!("回归验证 · {body}")
}

fn is_inspect_like(t: &TaskIR) -> bool {
    if t.role == Some(TaskRole::Inspect) {
        return true;
    }
    if t.id.contains("inspect") || t.id.starts_with("sys-post-inspect") {
        return true;
    }
    let title = &t.title;
    title.contains("检验")
        || title.contains("巡检")
        || title.contains("终检")
        || title.to_ascii_lowercase().contains("inspect")
        || title.contains("VERDICT")
}

/// Deterministic critic: sanitize deps + regression title/prompt hygiene + inspect note.
/// Idempotent enough to run on every finish_plan_job path.
pub(super) fn critic_plan_tasks(tasks: &mut [TaskIR], mode: PlanModeKind) -> CriticReport {
    let mut report = CriticReport::default();
    let before: usize = tasks.iter().map(|t| t.depends_on.len()).sum();
    sanitize_task_deps(tasks);
    let after: usize = tasks.iter().map(|t| t.depends_on.len()).sum();
    report.edges_removed = before.saturating_sub(after);

    let regressionish = matches!(mode, PlanModeKind::Regression | PlanModeKind::Mixed);
    if regressionish {
        for t in tasks.iter_mut() {
            if t.id.starts_with("sys-post-") {
                continue;
            }
            if looks_like_implement_title(&t.title) {
                t.title = rewrite_regression_title(&t.title);
                report.titles_rewritten += 1;
            }
            if !t.prompt.contains("已落地")
                && !t.prompt.contains(REGRESSION_BANNER)
                && !t.prompt.contains("severity=blocking")
            {
                t.prompt = format!("{REGRESSION_BANNER}\n\n{}", t.prompt.trim_start());
                report.prompts_tagged += 1;
            }
        }
    }

    let has_inspect = tasks.iter().any(is_inspect_like);
    if !has_inspect && matches!(mode, PlanModeKind::Regression | PlanModeKind::Audit) {
        report
            .notes
            .push("未检测到检验尾波（可在设置开启「拆分后附加：任务巡检」）".into());
    }

    report
}

/// Parse mode string from job.digest_mode.
pub(super) fn mode_from_str(s: &str) -> PlanModeKind {
    match s.trim().to_ascii_lowercase().as_str() {
        "regression" => PlanModeKind::Regression,
        "audit" => PlanModeKind::Audit,
        "mixed" => PlanModeKind::Mixed,
        _ => PlanModeKind::Greenfield,
    }
}

/// Compact task list for optional second-pass LLM critic (ids/titles/deps only).
pub(super) fn tasks_skeleton_json(tasks: &[TaskIR]) -> String {
    let items: Vec<serde_json::Value> = tasks
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "depends_on": t.depends_on,
                "optional": t.optional,
            })
        })
        .collect();
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into())
}

/// Patch from optional LLM critic. Only remove_edges + notes (no full replan).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub(super) struct LlmCriticPatch {
    #[serde(default)]
    pub remove_edges: Vec<LlmCriticEdgeDrop>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub(super) struct LlmCriticEdgeDrop {
    /// Task that currently depends on `deps`.
    #[serde(alias = "task_id", alias = "from")]
    pub task: String,
    /// Dependency ids to drop from that task.
    #[serde(default, alias = "drop", alias = "deps_drop")]
    pub deps: Vec<String>,
}

/// Apply a second-pass critic patch (deterministic; unit-tested).
/// Only drops edges that still exist; never invents new tasks.
pub(super) fn apply_llm_critic_patch(tasks: &mut [TaskIR], patch: &LlmCriticPatch) -> CriticReport {
    let mut report = CriticReport::default();
    for drop in &patch.remove_edges {
        let Some(task) = tasks.iter_mut().find(|t| t.id == drop.task) else {
            continue;
        };
        let before = task.depends_on.len();
        if drop.deps.is_empty() {
            // Empty deps list means "drop all unmotivated" — skip; rule critic already did that.
            continue;
        }
        task.depends_on
            .retain(|d| !drop.deps.iter().any(|x| x == d));
        report.edges_removed += before.saturating_sub(task.depends_on.len());
    }
    for n in &patch.notes {
        let t = n.trim();
        if !t.is_empty() && !report.notes.iter().any(|x| x == t) {
            report.notes.push(format!("LLM校对：{t}"));
        }
    }
    report
}

/// Parse critic JSON object from free LLM text (fenced or raw).
pub(super) fn parse_llm_critic_patch(raw: &str) -> Option<LlmCriticPatch> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Try direct parse first
    if let Ok(p) = serde_json::from_str::<LlmCriticPatch>(s) {
        return Some(p);
    }
    // Extract first {…} balanced block
    let bytes = s.as_bytes();
    let start = s.find('{')?;
    let mut depth = 0i32;
    let mut end = None;
    for (i, b) in bytes.iter().enumerate().skip(start) {
        match *b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let slice = &s[start..=end];
    serde_json::from_str(slice).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_landed_header_is_regression() {
        let md = r#"# chat-home

> 状态：**H0–H4 已落地**（终检 t9：S1–S8 全 PASS）

## 5. 阶段

### H0 — 入口
- [x] done

### H1 — 右轨
- [x] done
"#;
        let d = build_plan_digest(md);
        assert_eq!(d.mode, PlanModeKind::Regression);
        assert!(d.landed_hint);
    }

    #[test]
    fn digest_open_feature_is_greenfield() {
        let md = r#"# 新功能：导出 PDF

## 目标
做一个导出按钮。

## 任务大纲
- 实现导出
"#;
        let d = build_plan_digest(md);
        assert_eq!(d.mode, PlanModeKind::Greenfield);
    }

    #[test]
    fn sanitize_drops_unmentioned_deps() {
        let mut tasks = vec![
            TaskIR {
                id: "t1".into(),
                title: "回归 H0".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "do h0\nCCO_DONE ok".into(),
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
            },
            TaskIR {
                id: "t2".into(),
                title: "回归 H4".into(),
                depends_on: vec!["t1".into()],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                // no mention of t1
                prompt: "do h4 failover only\nCCO_DONE ok".into(),
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
            },
        ];
        sanitize_task_deps(&mut tasks);
        assert!(tasks[1].depends_on.is_empty());
    }

    #[test]
    fn sanitize_keeps_mentioned_deps() {
        let mut tasks = vec![
            TaskIR {
                id: "t1".into(),
                title: "写接口".into(),
                depends_on: vec![],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "api\nCCO_DONE ok".into(),
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
            },
            TaskIR {
                id: "t2".into(),
                title: "接前端".into(),
                depends_on: vec!["t1".into()],
                group: None,
                provider: "claude".into(),
                mode: "print".into(),
                prompt: "依赖 t1 的接口路径；对接 UI\nCCO_DONE ok".into(),
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
            },
        ];
        sanitize_task_deps(&mut tasks);
        assert_eq!(tasks[1].depends_on, vec!["t1".to_string()]);
    }

    fn sample_task(id: &str, title: &str, deps: &[&str], prompt: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: title.into(),
            depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
            group: None,
            provider: "claude".into(),
            mode: "print".into(),
            prompt: prompt.into(),
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
        }
    }

    #[test]
    fn critic_regression_rewrites_implement_title_and_tags_prompt() {
        let mut tasks = vec![
            sample_task(
                "t1",
                "实现 H0 入口路由",
                &[],
                "改 web/js/plan.js\nCCO_DONE ok",
            ),
            // prompt must not contain the substring "t1" (sanitize keeps edges that cite dep id)
            sample_task(
                "t2",
                "回归验证 H4 failover",
                &["t1"],
                "only h4 failover path; orthogonal to entry routing\nCCO_DONE ok",
            ),
        ];
        let r = critic_plan_tasks(&mut tasks, PlanModeKind::Regression);
        assert!(
            r.titles_rewritten >= 1,
            "title rewrite: {:?}",
            tasks[0].title
        );
        assert!(
            tasks[0].title.contains("回归验证"),
            "got {}",
            tasks[0].title
        );
        assert!(
            tasks[0].prompt.contains("已落地") || tasks[0].prompt.contains("blocking"),
            "prompt tagged"
        );
        // unmotivated edge dropped
        assert!(tasks[1].depends_on.is_empty());
        assert!(r.edges_removed >= 1);
        // no inspect → note
        assert!(r.notes.iter().any(|n| n.contains("检验")));
    }

    #[test]
    fn critic_greenfield_does_not_rewrite_implement_titles() {
        let mut tasks = vec![sample_task(
            "t1",
            "实现导出 PDF",
            &[],
            "add export\nCCO_DONE ok",
        )];
        let r = critic_plan_tasks(&mut tasks, PlanModeKind::Greenfield);
        assert_eq!(r.titles_rewritten, 0);
        assert_eq!(tasks[0].title, "实现导出 PDF");
        assert!(!tasks[0].prompt.contains("已落地"));
    }

    #[test]
    fn golden_landed_chat_home_header_is_regression() {
        // Shape of docs/archive/chat-home-plan-cli-2026-07-19.md header
        let md = r#"# cco 聊天主窗 · 计划可改 · CLI 不卡死

> 状态：**方案已定稿 · H0–H4 已落地**（终检 t9：S1–S8 全 PASS；**不阻塞** D0–D4）

## 5. 阶段切分与勾选

### H0 — 入口路由
- [x] selectProject 路由

### H1 — 聊天右轨
- [x] plan-rail

### H2 — 已执行标识
- [x] meta

## 6. 非目标
- 不新建 Scheduler
"#;
        let d = build_plan_digest(md);
        assert_eq!(
            d.mode,
            PlanModeKind::Regression,
            "landed header → regression"
        );
        assert!(d.landed_hint);
    }

    /// Golden: chat-home-shaped H0–H4 regression DAG after critic.
    /// Orthogonal H2 (meta) must not stay blocked on H4 (failover) without reason.
    #[test]
    fn golden_chat_home_regression_dag_after_critic() {
        let mut tasks = vec![
            sample_task(
                "t1",
                "实现 H0 入口路由",
                &[],
                "verify entry route\nCCO_DONE ok",
            ),
            sample_task(
                "t2",
                "实现 H1 右轨与全文",
                &[],
                "verify plan rail\nCCO_DONE ok",
            ),
            sample_task(
                "t3",
                "落地 H2 已执行 meta",
                &["t1"], // unmotivated serial edge
                "meta filter only; no entry dependency\nCCO_DONE ok",
            ),
            sample_task(
                "t4",
                "实现 H3 stall 可见",
                &["t2"],
                "stall UI only\nCCO_DONE ok",
            ),
            sample_task(
                "t5",
                "实现 H4 failover",
                &["t3"], // classic bad edge: H4 waited on H2 meta
                "provider failover only\nCCO_DONE ok",
            ),
            sample_task(
                "t6",
                "检验员终检 S1–S8（可选）",
                &["t1", "t2", "t3", "t4", "t5"],
                "plan_ref S1-S8; write VERDICT\n依赖原因：等待 t1 t2 t3 t4 t5\nCCO_DONE ok",
            ),
        ];
        // Make t6 optional + inspect-like title already
        tasks[5].optional = true;
        tasks[5].include = false;

        let r = critic_plan_tasks(&mut tasks, PlanModeKind::Regression);
        // Implement titles → 回归验证
        for t in &tasks[..5] {
            assert!(
                t.title.contains("回归验证") || t.title.contains("检验"),
                "expected regression title, got {}",
                t.title
            );
            assert!(
                t.prompt.contains("已落地") || t.prompt.contains("blocking"),
                "expected banner on {}",
                t.id
            );
        }
        // Bad edges without id mention dropped
        assert!(tasks[2].depends_on.is_empty(), "t3 should not wait on t1");
        assert!(tasks[3].depends_on.is_empty(), "t4 should not wait on t2");
        assert!(tasks[4].depends_on.is_empty(), "t5 should not wait on t3");
        // Inspect-like present → no missing-inspect note required, but edges_removed > 0
        assert!(r.edges_removed >= 3, "report={r:?}");
        assert!(r.titles_rewritten >= 3, "report={r:?}");
        // t6 keeps deps that mention task ids in prompt
        assert!(
            tasks[5].depends_on.len() >= 3,
            "inspect should keep motivated deps: {:?}",
            tasks[5].depends_on
        );
    }

    /// Read real docs/*.md from the workspace (CARGO_MANIFEST_DIR) and lock digest modes.
    #[test]
    fn golden_real_docs_digest_modes() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cases: &[(&str, PlanModeKind)] = &[
            // 已落地 / 已闭环
            (
                "docs/archive/chat-home-plan-cli-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/system-post-tasks-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/ux-plan-mgmt-attach-ttl-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/chat-plan-builder-2026-07-18.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/plan-execute-inspect-rework-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/plan-mgmt-to-exec-flow-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/chat-utf8-fence-panic-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/archive/ux-simple-mainpath-2026-07-17.md",
                PlanModeKind::Regression,
            ),
            (
                "docs/product-mode-b-ai-planner.md",
                PlanModeKind::Regression,
            ),
            // U0–U2 已落地 → regression
            (
                "docs/archive/chat-ux-focus-2026-07-19.md",
                PlanModeKind::Regression,
            ),
            // P0–P1 全绿 · P2 主线已落地 → regression（状态句含「已落地」）
            (
                "docs/multi-cli-collaboration-2026-07-18.md",
                PlanModeKind::Regression,
            ),
        ];
        for (rel, want) in cases {
            let path = root.join(rel);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let d = build_plan_digest(&text);
            assert_eq!(
                d.mode, *want,
                "{rel}: got {:?} landed={} title={}",
                d.mode, d.landed_hint, d.title
            );
        }
    }

    #[test]
    fn apply_llm_critic_patch_drops_named_edges_and_notes() {
        let mut tasks = vec![
            sample_task("t1", "A", &[], "a"),
            sample_task("t2", "B", &["t1"], "b only"),
        ];
        let patch = LlmCriticPatch {
            remove_edges: vec![LlmCriticEdgeDrop {
                task: "t2".into(),
                deps: vec!["t1".into()],
            }],
            notes: vec!["t2 与 t1 正交".into()],
        };
        let r = apply_llm_critic_patch(&mut tasks, &patch);
        assert!(tasks[1].depends_on.is_empty());
        assert_eq!(r.edges_removed, 1);
        assert!(r.notes.iter().any(|n| n.contains("正交")));
    }

    #[test]
    fn parse_llm_critic_patch_from_fenced_json() {
        let raw = r#"说明如下
```json
{"remove_edges":[{"task":"t5","deps":["t3"]}],"notes":["H4 不依赖 H2"]}
```
"#;
        let p = parse_llm_critic_patch(raw).expect("parse");
        assert_eq!(p.remove_edges.len(), 1);
        assert_eq!(p.remove_edges[0].task, "t5");
        assert_eq!(p.remove_edges[0].deps, vec!["t3".to_string()]);
    }

    #[test]
    fn golden_open_feature_stays_greenfield_and_keeps_motivated_edge() {
        let md = r#"# 导出 PDF

## 目标
给报告页加导出按钮。

## 任务大纲
- 写导出 API
- 接前端按钮
"#;
        let d = build_plan_digest(md);
        assert_eq!(d.mode, PlanModeKind::Greenfield);

        let mut tasks = vec![
            sample_task("t1", "实现导出 API", &[], "api endpoint\nCCO_DONE ok"),
            sample_task(
                "t2",
                "接前端导出按钮",
                &["t1"],
                "依赖 t1 的接口路径；UI button\nCCO_DONE ok",
            ),
        ];
        let r = critic_plan_tasks(&mut tasks, d.mode);
        assert_eq!(r.titles_rewritten, 0);
        assert_eq!(tasks[1].depends_on, vec!["t1".to_string()]);
        assert!(!tasks[0].prompt.contains("已落地"));
    }
}
