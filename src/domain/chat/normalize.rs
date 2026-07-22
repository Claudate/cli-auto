//! Plan markdown normalize / local structure fill (G0 / G0b local).
//! P1-4: plan-level acceptance quality (`acceptance_quality`).
//! P2-1: plan acceptance checklist parse (`parse_acceptance_checklist`) + verification assemble.

use serde::Serialize;

use super::title::extract_title_from_md;

/// Normalize plan markdown before disk write (G0).
/// - Unify newlines
/// - If essentially one line, insert breaks before `##` / `###` headings
pub fn normalize_plan_markdown(md: &str) -> String {
    let mut s = md.replace("\r\n", "\n").replace('\r', "\n");
    let nl = s.matches('\n').count();
    if nl <= 1 && s.chars().count() > 60 {
        // Recover jammed single-line structure for Mode B + human read.
        s = s.replace("### ", "\n\n### ");
        s = s.replace("## ", "\n\n## ");
        s = s.trim().to_string();
        // Ensure H1 is followed by blank line when next is ##
        if let Some(rest) = s.strip_prefix("# ") {
            if let Some(pos) = rest.find("\n\n##") {
                let title = &rest[..pos];
                let body = &rest[pos..];
                s = format!("# {}\n{}", title.trim_end(), body);
            } else if !rest.contains('\n') {
                // still one line after ## inject failed (no ##) — keep as is
            }
        }
    }
    // Guarantee trailing newline
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// How filled the plan-level acceptance section is (P1-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceQuality {
    /// Real criteria under 验收 / acceptance / 成功标准.
    Filled,
    /// Section exists but body is only placeholders (`- [ ] …` / 「请补充」).
    Stub,
    /// No acceptance-like H2.
    Missing,
}

/// One-line human note for confirm UI (`None` when filled).
pub fn acceptance_hint(q: AcceptanceQuality) -> Option<&'static str> {
    match q {
        AcceptanceQuality::Filled => None,
        AcceptanceQuality::Stub => {
            Some("计划验收仍是占位，建议写清「怎样算做完」再开始（仍可确认）")
        }
        AcceptanceQuality::Missing => {
            Some("计划里还没有验收/成功标准，建议补充后再开始（仍可确认）")
        }
    }
}

/// True when acceptance is stub or missing (confirm yellow bar).
pub fn acceptance_is_stub(q: AcceptanceQuality) -> bool {
    !matches!(q, AcceptanceQuality::Filled)
}

/// Detect plan-level acceptance quality (pure; no IO).
///
/// Section aliases (H2 only): `## 验收` · `## acceptance` · `## 成功标准`
/// (and parenthetical suffixes like `## 成功标准（怎样算做完）`).
///
/// Stub = section body only has empty/ellipsis checkboxes or 「请补充」-class placeholders.
pub fn acceptance_quality(md: &str) -> AcceptanceQuality {
    let s = md.replace("\r\n", "\n").replace('\r', "\n");
    let Some(body) = extract_acceptance_body(&s) else {
        return AcceptanceQuality::Missing;
    };
    if body_is_stub(&body) {
        AcceptanceQuality::Stub
    } else {
        AcceptanceQuality::Filled
    }
}

// ─── P2-1: acceptance checklist + verification ─────────────────────────────

/// One plan-level acceptance checklist line (structure only; no LLM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanChecklistItem {
    pub text: String,
    /// Checkbox checked when the line had `[x]` / `[X]`; false for bare bullets / `[ ]`.
    pub checked: bool,
    /// True when the line used a markdown checkbox marker.
    pub has_checkbox: bool,
}

/// Task-level acceptance / done_when row (optional secondary list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskAcceptanceItem {
    pub task_id: String,
    pub text: String,
}

/// Where the result-desk verification block drew its primary story from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSource {
    /// Real inspect product present — inspect is authoritative; plan list is sidebar.
    Inspect,
    /// No usable inspect; plan wrote acceptance items that were not auto-checked.
    PlanOnly,
    /// Neither inspect product nor plan checklist.
    None,
}

/// One inspect-side item (issue preview / residual), not auto-matched to plan lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationItemStatus {
    Pass,
    Fail,
    Unknown,
    Skipped,
}

/// Inspect-side verification row (issue text + status).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationItem {
    pub text: String,
    pub status: VerificationItemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

/// Side-by-side plan checklist vs inspect snapshot (P2-1 live / report DTO).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerificationView {
    pub source: VerificationSource,
    /// Plan-level checklist lines under 验收 / 成功标准 / acceptance.
    #[serde(default)]
    pub plan_items: Vec<PlanChecklistItem>,
    /// Count of plan-level checklist lines (same as `plan_items.len()`).
    pub plan_count: usize,
    /// Task-level acceptance / done_when (optional; may be empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_items: Vec<TaskAcceptanceItem>,
    /// Inspect-side rows when `source == Inspect` (issues / residuals).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<VerificationItem>,
    /// Human note e.g. 「计划写了 N 条验收，本轮未自动对照」.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_count: Option<usize>,
}

/// Parse plan-level acceptance checklist lines (structure only; no LLM).
///
/// Reads the same section aliases as [`acceptance_quality`]. Skips stub-only lines
/// (`…` / 「请补充」) so empty shells do not inflate the count.
pub fn parse_acceptance_checklist(md: &str) -> Vec<PlanChecklistItem> {
    let s = md.replace("\r\n", "\n").replace('\r', "\n");
    let Some(body) = extract_acceptance_body(&s) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in body.lines() {
        if let Some(item) = parse_checklist_line(line) {
            // Drop pure stubs so N reflects real criteria.
            if line_is_stub_content(item.text.trim()) {
                continue;
            }
            out.push(item);
        }
    }
    out
}

/// Collect non-empty task acceptance / done_when strings into rows.
pub fn collect_task_acceptance_items<'a, I>(tasks: I) -> Vec<TaskAcceptanceItem>
where
    I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
{
    let mut out = Vec::new();
    for (id, acc) in tasks {
        let Some(raw) = acc else { continue };
        let t = raw.trim();
        if t.is_empty() || line_is_stub_content(t) {
            continue;
        }
        // Multi-line acceptance → one item per non-empty line (still structure-only).
        let mut any = false;
        for line in t.lines() {
            let lt = line.trim();
            if lt.is_empty() {
                continue;
            }
            // Strip leading list/checkbox markers when present.
            let text = strip_list_prefix(lt);
            if text.is_empty() || line_is_stub_content(text) {
                continue;
            }
            out.push(TaskAcceptanceItem {
                task_id: id.to_string(),
                text: text.to_string(),
            });
            any = true;
        }
        if !any {
            out.push(TaskAcceptanceItem {
                task_id: id.to_string(),
                text: t.lines().next().unwrap_or(t).trim().to_string(),
            });
        }
    }
    out
}

/// Inputs for pure verification assembly (no IO / no inspect re-parse).
#[derive(Debug, Clone, Default)]
pub struct VerificationInputs {
    pub plan_items: Vec<PlanChecklistItem>,
    pub task_items: Vec<TaskAcceptanceItem>,
    /// True when a real inspect product is available (PASS/FAIL or blocking with product).
    pub has_real_inspect: bool,
    pub blocking_count: usize,
    pub residual_count: usize,
    /// Issue preview lines from inspect_loop (authoritative when inspect present).
    pub issue_preview: Vec<String>,
    /// True when plan required inspect but no real product (pending / unclear).
    pub inspect_pending: bool,
}

/// Assemble plan checklist vs inspect side-by-side view (pure).
///
/// Rules (P2-1):
/// 1. Real inspect → `source=inspect`; plan list is sidebar; inspect issues in `items`.
/// 2. No inspect but plan wrote N criteria → `source=plan_only` + note
///    「计划写了 N 条验收，本轮未自动对照」.
/// 3. Neither → `source=none`.
pub fn build_verification(input: VerificationInputs) -> VerificationView {
    let plan_count = input.plan_items.len();
    let task_n = input.task_items.len();
    let total_planish = plan_count + task_n;

    if input.has_real_inspect {
        let mut items = Vec::new();
        for line in &input.issue_preview {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            // Heuristic status from issue text (no LLM): blocking/fail markers → fail.
            let lower = t.to_ascii_lowercase();
            let status = if lower.contains("severity")
                || lower.contains("fail")
                || lower.contains("missing")
                || lower.contains("遗漏")
                || lower.contains("阻塞")
            {
                VerificationItemStatus::Fail
            } else {
                VerificationItemStatus::Unknown
            };
            items.push(VerificationItem {
                text: t.chars().take(200).collect(),
                status,
                task_id: None,
            });
        }
        let plan_note = if total_planish > 0 {
            Some(format!(
                "原计划写了 {total_planish} 条验收（巡检为准 · 清单仅作对照）"
            ))
        } else {
            None
        };
        return VerificationView {
            source: VerificationSource::Inspect,
            plan_items: input.plan_items,
            plan_count,
            task_items: input.task_items,
            items,
            plan_note,
            blocking_count: Some(input.blocking_count),
            residual_count: Some(input.residual_count),
        };
    }

    if total_planish > 0 {
        let note = if input.inspect_pending {
            format!("计划写了 {total_planish} 条验收，本轮未自动对照（巡检尚未产出结论）")
        } else {
            format!("计划写了 {total_planish} 条验收，本轮未自动对照")
        };
        return VerificationView {
            source: VerificationSource::PlanOnly,
            plan_items: input.plan_items,
            plan_count,
            task_items: input.task_items,
            items: Vec::new(),
            plan_note: Some(note),
            blocking_count: None,
            residual_count: None,
        };
    }

    VerificationView {
        source: VerificationSource::None,
        plan_items: Vec::new(),
        plan_count: 0,
        task_items: Vec::new(),
        items: Vec::new(),
        plan_note: None,
        blocking_count: None,
        residual_count: None,
    }
}

fn parse_checklist_line(line: &str) -> Option<PlanChecklistItem> {
    let t = line.trim();
    if t.is_empty() || t == "---" || t == "***" || t == "___" {
        return None;
    }
    if t.starts_with("<!--") || (t.starts_with('<') && t.ends_with('>')) {
        return None;
    }
    // Skip nested headings inside the section.
    if t.starts_with('#') {
        return None;
    }

    let (has_checkbox, checked, rest) = if let Some(r) = t
        .strip_prefix("- [x]")
        .or_else(|| t.strip_prefix("- [X]"))
        .or_else(|| t.strip_prefix("* [x]"))
        .or_else(|| t.strip_prefix("* [X]"))
        .or_else(|| t.strip_prefix("+ [x]"))
        .or_else(|| t.strip_prefix("+ [X]"))
    {
        (true, true, r.trim())
    } else if let Some(r) = t
        .strip_prefix("- [ ]")
        .or_else(|| t.strip_prefix("* [ ]"))
        .or_else(|| t.strip_prefix("+ [ ]"))
    {
        (true, false, r.trim())
    } else if let Some(r) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        (false, false, r.trim())
    } else if let Some(pos) = t.find(". ") {
        let (n, rest) = t.split_at(pos);
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            (false, false, rest.trim_start_matches('.').trim())
        } else {
            return None;
        }
    } else {
        // Non-list prose under acceptance still counts as one criterion.
        (false, false, t)
    };

    if rest.is_empty() {
        return None;
    }
    Some(PlanChecklistItem {
        text: rest.to_string(),
        checked,
        has_checkbox,
    })
}

fn strip_list_prefix(s: &str) -> &str {
    if let Some(r) = s
        .strip_prefix("- [ ]")
        .or_else(|| s.strip_prefix("- [x]"))
        .or_else(|| s.strip_prefix("- [X]"))
        .or_else(|| s.strip_prefix("* [ ]"))
        .or_else(|| s.strip_prefix("* [x]"))
        .or_else(|| s.strip_prefix("+ [ ]"))
    {
        return r.trim();
    }
    if let Some(r) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
    {
        return r.trim();
    }
    s
}

fn is_acceptance_h2(line: &str) -> bool {
    let t = line.trim();
    // Exactly H2 (`## …`) — not H1/H3.
    let rest = if let Some(r) = t.strip_prefix("## ") {
        r
    } else if t.starts_with("##") && !t.starts_with("###") {
        t.trim_start_matches('#').trim()
    } else {
        return false;
    };
    let head = rest
        .split(['（', '(', '·', '—', '–', '-', ':', '：'])
        .next()
        .unwrap_or(rest);
    let head = head.trim().to_lowercase();
    head == "验收"
        || head == "acceptance"
        || head == "成功标准"
        || head == "success criteria"
        || head == "success criterion"
        || head.starts_with("验收")
        || head.starts_with("acceptance")
        || head.starts_with("成功标准")
}

fn extract_acceptance_body(md: &str) -> Option<String> {
    let lines: Vec<&str> = md.lines().collect();
    let mut start: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if is_acceptance_h2(line) {
            start = Some(i + 1);
            break;
        }
    }
    let start = start?;
    let mut end = lines.len();
    for (j, line) in lines.iter().enumerate().skip(start) {
        let t = line.trim();
        // Next H2 (not H3) ends the section.
        if t.starts_with("## ")
            || (t.starts_with("##") && !t.starts_with("###") && t.len() > 2)
        {
            end = j;
            break;
        }
    }
    Some(lines[start..end].join("\n"))
}

fn body_is_stub(body: &str) -> bool {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t == "---" || t == "***" || t == "___" {
            continue;
        }
        // Ignore pure HTML comments / tags.
        if t.starts_with("<!--") || (t.starts_with('<') && t.ends_with('>')) {
            continue;
        }
        if !line_is_stub_content(t) {
            return false;
        }
    }
    // Empty section body (heading only) → stub.
    true
}

fn line_is_stub_content(t: &str) -> bool {
    // Strip common list / checkbox prefixes.
    let mut s = t;
    if let Some(rest) = s
        .strip_prefix("- [ ]")
        .or_else(|| s.strip_prefix("- [x]"))
        .or_else(|| s.strip_prefix("- [X]"))
        .or_else(|| s.strip_prefix("* [ ]"))
        .or_else(|| s.strip_prefix("* [x]"))
        .or_else(|| s.strip_prefix("+ [ ]"))
    {
        s = rest.trim();
    } else if let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
    {
        s = rest.trim();
    } else if let Some(pos) = s.find(". ") {
        let (n, rest) = s.split_at(pos);
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            s = rest.trim_start_matches('.').trim();
        }
    }

    if s.is_empty() {
        return true;
    }
    // Ellipsis / middle-dot only.
    if s.chars()
        .all(|c| matches!(c, '.' | '…' | '·' | '．' | ' ' | '\t'))
    {
        return true;
    }
    let lower = s.to_lowercase();
    if lower.contains("请补充")
        || lower.contains("待补充")
        || lower.contains("待填写")
        || lower.contains("待完善")
        || lower == "tbd"
        || lower == "todo"
        || lower == "待定"
        || lower == "…"
        || lower == "..."
        || lower == "……"
    {
        return true;
    }
    // structure_plan_markdown placeholders like "（请补充 1～3 句…）".
    if (s.starts_with('（') || s.starts_with('(')) && s.contains("请补充") {
        return true;
    }
    false
}

/// G0b local: ensure draft has short H1 + core sections (no CLI).
/// Idempotent when already structured; fills missing headings only.
pub fn structure_plan_markdown(md: &str) -> String {
    let mut s = normalize_plan_markdown(md);
    let lower = s.to_lowercase();
    let has_h1 = s.lines().any(|l| {
        let t = l.trim();
        t.starts_with("# ") || (t.starts_with('#') && !t.starts_with("##"))
    });
    if !has_h1 {
        let title = extract_title_from_md(&s).unwrap_or_else(|| "聊天生成计划".into());
        s = format!("# {title}\n\n{s}");
    }
    // Re-extract short title and rewrite first H1 if wall-like
    if let Some(title) = extract_title_from_md(&s) {
        if let Some(rest_start) = s.find('\n') {
            let rest = &s[rest_start..];
            s = format!("# {title}{rest}");
        } else {
            s = format!("# {title}\n");
        }
    }
    let mut missing = Vec::new();
    if !lower.contains("## 目标") && !lower.contains("## goal") {
        missing.push("## 目标\n（请补充 1～3 句目标）\n");
    }
    if !lower.contains("## 范围") && !lower.contains("## scope") {
        missing.push("## 范围\n- 做：…\n- 不做：…\n");
    }
    if !lower.contains("## 任务") && !lower.contains("## tasks") {
        missing.push("## 任务大纲\n### T1 · （可执行标题）\n- 说明：…\n- 验收：…\n");
    }
    // P1-4: `## 成功标准` counts as acceptance section (structure alias).
    if !lower.contains("## 验收")
        && !lower.contains("## acceptance")
        && !lower.contains("## 成功标准")
        && !lower.contains("## success criteria")
    {
        missing.push("## 验收（整计划）\n- [ ] …\n");
    }
    if !missing.is_empty() {
        s = s.trim_end().to_string();
        s.push_str("\n\n---\n\n");
        s.push_str(&missing.join("\n"));
    }
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod acceptance_quality_tests {
    use super::*;

    #[test]
    fn missing_when_no_acceptance_section() {
        let md = "# 计划\n\n## 目标\n做点事\n\n## 范围\n- 做：A\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Missing);
        assert!(acceptance_is_stub(AcceptanceQuality::Missing));
        assert!(acceptance_hint(AcceptanceQuality::Missing).is_some());
    }

    #[test]
    fn stub_empty_checkbox_ellipsis() {
        let md = "# 计划\n\n## 验收（整计划）\n- [ ] …\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Stub);
        assert!(acceptance_is_stub(AcceptanceQuality::Stub));
    }

    #[test]
    fn stub_please_fill_placeholder() {
        let md = "# 计划\n\n## 验收\n- [ ] 请补充验收条件\n- 待填写\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Stub);
    }

    #[test]
    fn stub_heading_only() {
        let md = "# 计划\n\n## 成功标准\n\n## 任务\n- 做\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Stub);
    }

    #[test]
    fn filled_real_criteria() {
        let md = r#"# 出海落地页

## 目标
做出可上线落地页

## 成功标准（怎样算做完）

- [ ] 主标题 + 副标题说清「给谁 · 解决什么」
- [ ] 至少 3 个利益点
- [ ] 主 CTA 清晰

## 建议步骤
1. 定受众
"#;
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Filled);
        assert!(!acceptance_is_stub(AcceptanceQuality::Filled));
        assert!(acceptance_hint(AcceptanceQuality::Filled).is_none());
    }

    #[test]
    fn filled_acceptance_english() {
        let md = "# Plan\n\n## Acceptance\n- [ ] Login works with SSO\n- API returns 200\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Filled);
    }

    #[test]
    fn structure_respects_success_criteria_alias() {
        let thin = "# 登录优化\n\n## 成功标准\n- [ ] 登录可过\n\n做快点\n";
        let out = structure_plan_markdown(thin);
        // Must not append a second stub 验收 section.
        let n_acc = out.matches("## 验收").count();
        assert_eq!(n_acc, 0, "should not inject ## 验收 when 成功标准 present:\n{out}");
        assert!(out.contains("## 成功标准"), "got:\n{out}");
        assert_eq!(acceptance_quality(&out), AcceptanceQuality::Filled);
    }

    #[test]
    fn structure_injects_stub_acceptance_when_missing() {
        let thin = "# 登录优化\n\n做快点\n";
        let out = structure_plan_markdown(thin);
        assert!(out.contains("## 验收"), "got:\n{out}");
        assert_eq!(acceptance_quality(&out), AcceptanceQuality::Stub);
    }

    #[test]
    fn mixed_stub_and_real_is_filled() {
        // One real line is enough → filled (not pure stub).
        let md = "## 验收\n- [ ] …\n- [ ] 用户能完成一次完整结账\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Filled);
    }
}

#[cfg(test)]
mod checklist_verification_tests {
    use super::*;

    #[test]
    fn parse_checklist_real_items_skips_stubs() {
        let md = r#"# 出海落地页

## 成功标准（怎样算做完）

- [ ] 主标题 + 副标题说清「给谁 · 解决什么」
- [x] 至少 3 个利益点
- [ ] …
- [ ] 请补充
- 主 CTA 清晰

## 建议步骤
1. 定受众
"#;
        let items = parse_acceptance_checklist(md);
        assert_eq!(items.len(), 3, "got: {items:?}");
        assert_eq!(items[0].text, "主标题 + 副标题说清「给谁 · 解决什么」");
        assert!(!items[0].checked);
        assert!(items[0].has_checkbox);
        assert_eq!(items[1].text, "至少 3 个利益点");
        assert!(items[1].checked);
        assert_eq!(items[2].text, "主 CTA 清晰");
        assert!(!items[2].has_checkbox);
    }

    #[test]
    fn parse_checklist_empty_when_missing_section() {
        let md = "# 计划\n\n## 目标\n做点事\n";
        assert!(parse_acceptance_checklist(md).is_empty());
    }

    #[test]
    fn parse_checklist_numbered_and_english() {
        let md = "# Plan\n\n## Acceptance\n1. Login works with SSO\n2. API returns 200\n";
        let items = parse_acceptance_checklist(md);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "Login works with SSO");
        assert_eq!(items[1].text, "API returns 200");
    }

    #[test]
    fn collect_task_acceptance_filters_empty() {
        let rows = collect_task_acceptance_items([
            ("t1", Some("file exists")),
            ("t2", Some("")),
            ("t3", None),
            ("t4", Some("…")),
            ("t5", Some("- [ ] 写完 report.md\n- [ ] 有 VERDICT")),
        ]);
        assert_eq!(rows.len(), 3, "got: {rows:?}");
        assert_eq!(rows[0].task_id, "t1");
        assert_eq!(rows[0].text, "file exists");
        assert_eq!(rows[1].task_id, "t5");
        assert_eq!(rows[1].text, "写完 report.md");
        assert_eq!(rows[2].task_id, "t5");
        assert_eq!(rows[2].text, "有 VERDICT");
    }

    #[test]
    fn build_verification_inspect_authoritative() {
        let plan_items = parse_acceptance_checklist(
            "## 验收\n- [ ] 主 CTA 清晰\n- [ ] 至少 3 个利益点\n",
        );
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![],
            has_real_inspect: true,
            blocking_count: 1,
            residual_count: 0,
            issue_preview: vec!["I-1 severity=blocking missing CTA".into()],
            inspect_pending: false,
        });
        assert_eq!(v.source, VerificationSource::Inspect);
        assert_eq!(v.plan_count, 2);
        assert_eq!(v.items.len(), 1);
        assert_eq!(v.items[0].status, VerificationItemStatus::Fail);
        assert!(
            v.plan_note
                .as_deref()
                .unwrap_or("")
                .contains("巡检为准"),
            "got: {:?}",
            v.plan_note
        );
    }

    #[test]
    fn build_verification_plan_only_note() {
        let plan_items =
            parse_acceptance_checklist("## 成功标准\n- [ ] 登录可过\n- [ ] 结账可过\n");
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![TaskAcceptanceItem {
                task_id: "t1".into(),
                text: "file exists".into(),
            }],
            has_real_inspect: false,
            blocking_count: 0,
            residual_count: 0,
            issue_preview: vec![],
            inspect_pending: false,
        });
        assert_eq!(v.source, VerificationSource::PlanOnly);
        assert_eq!(v.plan_count, 2);
        assert!(v.items.is_empty());
        assert_eq!(
            v.plan_note.as_deref(),
            Some("计划写了 3 条验收，本轮未自动对照")
        );
    }

    #[test]
    fn build_verification_none_when_empty() {
        let v = build_verification(VerificationInputs::default());
        assert_eq!(v.source, VerificationSource::None);
        assert_eq!(v.plan_count, 0);
        assert!(v.plan_note.is_none());
    }

    #[test]
    fn build_verification_pending_inspect_wording() {
        let plan_items = parse_acceptance_checklist("## 验收\n- [ ] 有 VERDICT\n");
        let v = build_verification(VerificationInputs {
            plan_items,
            task_items: vec![],
            has_real_inspect: false,
            blocking_count: 0,
            residual_count: 0,
            issue_preview: vec![],
            inspect_pending: true,
        });
        assert_eq!(v.source, VerificationSource::PlanOnly);
        assert!(
            v.plan_note
                .as_deref()
                .unwrap_or("")
                .contains("巡检尚未产出"),
            "got: {:?}",
            v.plan_note
        );
    }
}
