//! P1-4: plan-level acceptance quality + checklist parse.

use serde::Serialize;

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
        if t.starts_with("## ") || (t.starts_with("##") && !t.starts_with("###") && t.len() > 2) {
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
    fn mixed_stub_and_real_is_filled() {
        // One real line is enough → filled (not pure stub).
        let md = "## 验收\n- [ ] …\n- [ ] 用户能完成一次完整结账\n";
        assert_eq!(acceptance_quality(md), AcceptanceQuality::Filled);
    }
}

