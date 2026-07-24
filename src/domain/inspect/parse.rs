//! Pure VERDICT / ISSUES text parsers (A1-5).
//!
//! [INPUT]: raw product file body strings
//! [OUTPUT]: InspectVerdict · Vec<ParsedIssue>
//! [POS]: domain/inspect — no filesystem
//! [PROTOCOL]: 解析语义变更须同步 tests + plan-execute-inspect 契约说明

use super::types::{InspectVerdict, IssueSeverity, ParsedIssue};

/// Parse raw VERDICT text: first clear PASS/FAIL wins (line-oriented, then whole body).
pub fn parse_verdict_text(text: &str) -> InspectVerdict {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Prefer first meaningful line: "FAIL" / "PASS" or "VERDICT: FAIL"
        let upper = t.to_ascii_uppercase();
        // Word-boundary style: avoid matching FAIL inside longer tokens poorly.
        if upper == "FAIL"
            || upper.starts_with("FAIL ")
            || upper.starts_with("FAIL:")
            || upper.starts_with("FAIL|")
            || upper.contains("VERDICT=FAIL")
            || upper.contains("VERDICT: FAIL")
            || upper.contains("VERDICT:FAIL")
            || upper.contains("RESULT: FAIL")
            || upper.contains("RESULT:FAIL")
            || upper.contains("**RESULT: FAIL**")
        {
            return InspectVerdict::Fail;
        }
        if upper == "PASS"
            || upper.starts_with("PASS ")
            || upper.starts_with("PASS:")
            || upper.starts_with("PASS|")
            || upper.contains("VERDICT=PASS")
            || upper.contains("VERDICT: PASS")
            || upper.contains("VERDICT:PASS")
            || upper.contains("RESULT: PASS")
            || upper.contains("RESULT:PASS")
            || upper.contains("**RESULT: PASS**")
        {
            return InspectVerdict::Pass;
        }
        // First non-empty line had content but neither — keep scanning body below.
        break;
    }
    let upper = text.to_ascii_uppercase();
    // Whole-body fallback: FAIL takes precedence if both appear.
    let has_fail = upper.split_whitespace().any(|w| {
        w == "FAIL" || w.starts_with("FAIL:") || w.starts_with("FAIL|")
    }) || upper.contains("VERDICT=FAIL")
        || upper.contains("VERDICT: FAIL")
        || upper.contains("VERDICT:FAIL");
    let has_pass = upper.split_whitespace().any(|w| {
        w == "PASS" || w.starts_with("PASS:") || w.starts_with("PASS|")
    }) || upper.contains("VERDICT=PASS")
        || upper.contains("VERDICT: PASS")
        || upper.contains("VERDICT:PASS");
    if has_fail {
        InspectVerdict::Fail
    } else if has_pass {
        InspectVerdict::Pass
    } else {
        InspectVerdict::Unknown
    }
}

/// Parse ISSUES body into graded rows (P-loop §3.4.3).
///
/// Recognizes:
/// - `severity=blocking|map|residual|out-of-scope` (or `severity: …`)
/// - `plan_ref=` / `path=` / `fix_wp=` / `- id: I-*`
/// - Free-form bullets without severity → **blocking** (fail-closed for silent residual).
pub fn parse_issues_text(text: &str) -> Vec<ParsedIssue> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    let lower_all = trimmed.to_ascii_lowercase();
    if matches!(
        lower_all.as_str(),
        "无" | "none" | "n/a" | "na" | "no issues" | "no issue"
    ) {
        return vec![];
    }

    // Split into issue blocks: lines starting with `- id:` / `## I-` / `- I-` start a new block;
    // otherwise treat each non-empty bullet as its own issue.
    let mut blocks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in trimmed.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Field-only lines like `- **severity**: residual` must NOT open a new
        // block (otherwise `### I-1` alone becomes fail-closed Blocking).
        let severity_field_only = is_severity_field_only_line(t);
        // Block starts: classic `- id:` / `### I-1`, residual/oos headings
        // (`### R1 · …` / `### O1 · …`), or single-line severity bullets.
        let starts_block = t.starts_with("- id:")
            || t.starts_with("-id:")
            || t.starts_with("## I-")
            || t.starts_with("### I-")
            || (t.starts_with('-') || t.starts_with('*'))
                && (t[1..].trim_start().starts_with("I-")
                    || t[1..].trim_start().starts_with("R")
                    || t[1..].trim_start().starts_with("O"))
                && is_issue_heading_token(t[1..].trim_start().split_whitespace().next().unwrap_or(""))
            || (t.starts_with('#') && is_issue_heading_line(t))
            || (t.starts_with('-')
                && (t.contains("severity=") || t.contains("severity:"))
                && !severity_field_only);
        if starts_block && !cur.is_empty() {
            blocks.push(cur.trim().to_string());
            cur.clear();
        }
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(t);
    }
    if !cur.is_empty() {
        blocks.push(cur.trim().to_string());
    }

    // If nothing looked like multi-line blocks, fall back to per non-empty line.
    if blocks.len() == 1
        && !blocks[0].contains('\n')
        && trimmed.lines().filter(|l| !l.trim().is_empty()).count() > 1
    {
        blocks = trimmed
            .lines()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .filter(|t| {
                let lower = t.to_ascii_lowercase();
                lower != "无"
                    && lower != "none"
                    && lower != "n/a"
                    && lower != "na"
                    && !lower.starts_with("# ")
                    && lower != "# issues"
                    && lower != "## issues"
            })
            .map(|s| s.to_string())
            .collect();
    }

    let mut out = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        let lower = block.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "无" | "none" | "n/a" | "na" | "no issues" | "no issue"
        ) || lower == "## residual"
            || lower == "## blocking"
            || lower == "## out-of-scope"
            || lower == "## map"
        {
            // Section headers alone are not issues; content under them is.
            if block.lines().count() <= 1 {
                continue;
            }
        }
        // Title / section preamble is not an ISSUE. Free-form bullets without
        // severity still fail-closed → Blocking.
        let has_issue_id = block.lines().any(|l| {
            let t = l.trim().trim_start_matches('#').trim();
            let bare = t
                .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
                .trim();
            let first = bare
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(|c: char| c == '·' || c == ':' || c == ',');
            is_issue_heading_token(first)
                || bare.starts_with("id:")
                || t.starts_with("- id:")
        });
        let first = block
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        // Explicit field only — section titles like `## residual` must not promote preamble.
        let has_field_sev = parse_severity_field_only(&block).is_some();
        if !has_issue_id && !has_field_sev && first.starts_with('#') {
            continue;
        }
        // Empty-set confirmation lines (`- **blocking**: 无`) are not ISSUES.
        if !has_issue_id && is_empty_set_confirmation_block(&block) {
            continue;
        }
        let severity = parse_severity_token(&block).unwrap_or(IssueSeverity::Blocking);
        let id = extract_kv(&block, "id")
            .or_else(|| {
                block.lines().next().and_then(|l| {
                    // `### I-1` / `### R1 · title` / `- O2 · …` / `I-3`
                    let t = l
                        .trim()
                        .trim_start_matches('#')
                        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
                        .trim();
                    let token = t
                        .split_whitespace()
                        .next()
                        .unwrap_or(t)
                        .trim_end_matches(|c: char| c == '·' || c == ':' || c == ',');
                    if is_issue_heading_token(token) {
                        Some(token.to_string())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| format!("I-{}", i + 1));
        let plan_ref = extract_kv(&block, "plan_ref").unwrap_or_default();
        let path = extract_kv(&block, "path")
            .or_else(|| extract_kv(&block, "file"))
            .unwrap_or_else(|| "n/a".into());
        let symptom = extract_kv(&block, "symptom").unwrap_or_else(|| {
            block
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect()
        });
        let fix_wp = extract_kv(&block, "fix_wp")
            .or_else(|| extract_kv(&block, "suggestion"))
            .unwrap_or_else(|| format!("Fix {id}: {symptom}"));
        out.push(ParsedIssue {
            id,
            severity,
            plan_ref,
            path,
            symptom,
            fix_wp,
            raw: block,
        });
    }
    out
}

/// `- **blocking**: 无` / `- **map**: 无` empty-set footnotes under ISSUES.
fn is_empty_set_confirmation_block(block: &str) -> bool {
    let lower = block.to_ascii_lowercase();
    if lower.contains("## 空集") || lower.contains("empty-set") || lower.contains("空集确认") {
        return true;
    }
    // Single-line "no blocking" confirmations without I-*.
    let stripped: String = lower
        .chars()
        .filter(|c| *c != '*' && *c != '`' && *c != '_')
        .collect();
    let s = stripped.trim();
    let s = s.trim_start_matches(|c: char| c == '-' || c == '•').trim();
    (s.starts_with("blocking:") || s.starts_with("map:"))
        && (s.contains("无") || s.contains("none") || s.contains("no "))
}

/// True when the line is only a severity field under a multi-line ISSUE
/// (`- **severity**: residual`), not a single-line issue that embeds severity.
///
/// Parenthetical notes after the grade are still field-only, including fullwidth
/// Chinese notes like `out-of-scope（本波角色=静态 inspect）` — those must not
/// open a new block or fail-closed to Blocking.
fn is_severity_field_only_line(t: &str) -> bool {
    let stripped = t
        .trim()
        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
        .trim();
    let s: String = stripped
        .chars()
        .filter(|c| *c != '*' && *c != '`' && *c != '_')
        .collect();
    let s = s.trim().to_ascii_lowercase();
    let rest = if let Some(r) = s.strip_prefix("severity=") {
        r
    } else if let Some(r) = s.strip_prefix("severity:") {
        r
    } else {
        return false;
    };
    // Drop trailing notes before counting tokens.
    let token_core = strip_severity_trailing_note(rest.trim());
    let parts: Vec<&str> = token_core.split_whitespace().collect();
    // field-only: one severity token; extra keys ⇒ single-line issue row
    parts.len() == 1
        && severity_from_token(parts[0]).is_some()
        && !rest.contains("plan_ref")
        && !rest.contains("path=")
        && !rest.contains("fix_wp")
}

/// Heading line for a single ISSUE block (`### I-1`, `### R1 · …`, `### O2 · …`).
fn is_issue_heading_line(t: &str) -> bool {
    let s = t.trim_start_matches('#').trim();
    let first = s
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(|c: char| c == '·' || c == ':' || c == ',' || c == '—');
    is_issue_heading_token(first)
}

/// Token form of an ISSUE id/heading: `I-1`, `I1`, `R1`, `R-2`, `O1`, `O-3`.
fn is_issue_heading_token(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() {
        return false;
    }
    // Classic `I-*`
    if t.starts_with('I') && t.contains('-') {
        return true;
    }
    let mut chars = t.chars();
    let Some(c0) = chars.next() else {
        return false;
    };
    if !matches!(c0, 'I' | 'R' | 'O') {
        return false;
    }
    let rest: String = chars.collect();
    let rest = rest.trim_start_matches('-');
    rest.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Drop fullwidth/halfwidth parenthetical notes after a severity token.
/// `out-of-scope（本波…）` / `residual (optional)` → bare grade.
fn strip_severity_trailing_note(s: &str) -> &str {
    let cut = s
        .find(['（', '(', '【', '[', '—', '–'])
        .unwrap_or(s.len());
    s[..cut].trim()
}

fn severity_from_token(token: &str) -> Option<IssueSeverity> {
    let token = strip_severity_trailing_note(token)
        .trim()
        .trim_matches(|c: char| c == '`' || c == '*' || c == '"' || c == '\'' || c == '_' || c == '。' || c == '.' || c == '；' || c == ';')
        .to_ascii_lowercase();
    // Collapse common separators so `out_of_scope` / `out-of-scope` share path.
    let compact = token.replace('_', "-");
    match compact.as_str() {
        "blocking" | "block" | "p0" => Some(IssueSeverity::Blocking),
        "map" | "geb" => Some(IssueSeverity::Map),
        "residual" | "non-blocking" | "nonblocking" | "optional" => Some(IssueSeverity::Residual),
        "out-of-scope" | "outofscope" | "oos" => Some(IssueSeverity::OutOfScope),
        // Unknown grade: do NOT fail-closed here — caller treats missing grade as
        // free-form Blocking only when no severity field was present at all.
        _ => None,
    }
}

/// Explicit `severity=` / `severity:` / `**severity**: residual` fields only.
fn parse_severity_field_only(block: &str) -> Option<IssueSeverity> {
    let lower = block.to_ascii_lowercase();
    for line in lower.lines() {
        let t = line
            .trim()
            .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
            .trim();
        // Collapse markdown bold/code around the key: **severity**: residual
        let stripped: String = t
            .chars()
            .filter(|c| *c != '*' && *c != '`' && *c != '_')
            .collect();
        let s = stripped.trim();
        let rest = if let Some(r) = s.strip_prefix("severity=") {
            Some(r)
        } else if let Some(r) = s.strip_prefix("severity:") {
            Some(r)
        } else if let Some(r) = s.strip_prefix("severity ") {
            Some(r)
        } else if let Some(idx) = s.find("severity=") {
            Some(&s[idx + "severity=".len()..])
        } else if let Some(idx) = s.find("severity:") {
            Some(&s[idx + "severity:".len()..])
        } else {
            None
        };
        let Some(rest) = rest else {
            continue;
        };
        let token = rest
            .trim_start()
            .split(|c: char| c.is_whitespace() || c == ',' || c == '|' || c == ';')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|c| c == '`' || c == '*' || c == '"' || c == '\'' || c == '_');
        if token.is_empty() {
            continue;
        }
        // Empty-set footnotes: severity/key used as section label with 无
        if token == "无" || token == "none" || token == "no" {
            continue;
        }
        // Known grade (after stripping fullwidth notes) wins; unknown token
        // does not invent Blocking — keep scanning other lines.
        if let Some(sev) = severity_from_token(token) {
            return Some(sev);
        }
    }
    None
}

fn parse_severity_token(block: &str) -> Option<IssueSeverity> {
    if let Some(s) = parse_severity_field_only(block) {
        return Some(s);
    }
    let lower = block.to_ascii_lowercase();
    // Chinese / informal (whole-block hints only when no explicit severity=).
    // Do NOT match bare `## residual` section headers (no issue body).
    if lower.contains("地图") || lower.contains("geb 指针") || lower.contains("l1/l2") {
        return Some(IssueSeverity::Map);
    }
    if lower.contains("不阻塞") || lower.contains("可选残留") {
        return Some(IssueSeverity::Residual);
    }
    if lower.contains("范围外") || lower.contains("out of scope") {
        return Some(IssueSeverity::OutOfScope);
    }
    None
}

fn extract_kv(block: &str, key: &str) -> Option<String> {
    let lower_key = key.to_ascii_lowercase();
    for line in block.lines() {
        let t = line
            .trim()
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim();
        let lower = t.to_ascii_lowercase();
        for sep in [": ", "=", "："] {
            let pat = format!("{lower_key}{sep}");
            if let Some(rest) = lower.strip_prefix(&pat) {
                // Use original slice with same byte length prefix — prefer after first sep on line.
                if let Some(pos) = t.to_ascii_lowercase().find(sep) {
                    let val = t[pos + sep.len()..].trim();
                    if !val.is_empty() {
                        return Some(val.chars().take(300).collect());
                    }
                }
                let _ = rest;
            }
            // also allow `severity=blocking plan_ref=S5` mid-line
            if let Some(idx) = lower.find(&format!("{lower_key}{sep}")) {
                let after = &t[idx + lower_key.len() + sep.len()..];
                let val = after
                    .split_whitespace()
                    .next()
                    .unwrap_or(after)
                    .trim()
                    .trim_end_matches(',')
                    .to_string();
                if !val.is_empty() {
                    return Some(val.chars().take(300).collect());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_verdict_fail_and_pass() {
        assert_eq!(parse_verdict_text("FAIL\nreason"), InspectVerdict::Fail);
        assert_eq!(parse_verdict_text("PASS\nok"), InspectVerdict::Pass);
        assert_eq!(
            parse_verdict_text("VERDICT: FAIL — scope leak"),
            InspectVerdict::Fail
        );
        assert_eq!(parse_verdict_text("VERDICT=PASS"), InspectVerdict::Pass);
        assert_eq!(parse_verdict_text("maybe later"), InspectVerdict::Unknown);
        // FAIL wins when both present in body
        assert_eq!(
            parse_verdict_text("notes\nPASS was hoped\nbut VERDICT=FAIL overall"),
            InspectVerdict::Fail
        );
    }

    #[test]
    fn parse_verdict_result_prefix() {
        assert_eq!(
            parse_verdict_text("**Result: FAIL**\n\n| plan_ref |"),
            InspectVerdict::Fail
        );
        assert_eq!(
            parse_verdict_text("Result: PASS\nok"),
            InspectVerdict::Pass
        );
    }

    #[test]
    fn parse_issues_grades_severity() {
        let text = r#"
- id: I-1
  severity=map
  plan_ref: §8 GEB
  path: CLAUDE.md
  symptom: L1 still says 待验
  fix_wp: Update CLAUDE.md config row to F0+F1 closed

- id: I-2 severity=blocking plan_ref=S5 path=web/
  symptom: desktop Chinese path not verified
  fix_wp: Re-run GUI or mark DEGRADED only if plan allows

- id: I-3
  severity: residual
  plan_ref: F2
  symptom: optional polish
"#;
        let parsed = parse_issues_text(text);
        assert!(parsed.len() >= 3, "parsed={parsed:?}");
        let i1 = parsed.iter().find(|i| i.id.contains("I-1")).unwrap();
        assert_eq!(i1.severity, IssueSeverity::Map);
        assert!(i1.severity.is_blocking_for_gate());
        let i2 = parsed.iter().find(|i| i.id.contains("I-2")).unwrap();
        assert_eq!(i2.severity, IssueSeverity::Blocking);
        let i3 = parsed.iter().find(|i| i.id.contains("I-3")).unwrap();
        assert_eq!(i3.severity, IssueSeverity::Residual);
        assert!(!i3.severity.is_blocking_for_gate());
    }

    #[test]
    fn markdown_bold_severity_residual_not_blocking() {
        let text = r#"
### I-1
- **severity**: residual
- **plan_ref**: 验收
- **fix_wp**: polish
- **说明**: archive soft 历史表
"#;
        let parsed = parse_issues_text(text);
        assert!(!parsed.is_empty(), "parsed={parsed:?}");
        let i1 = parsed.iter().find(|i| i.id.contains("I-1")).unwrap();
        assert_eq!(i1.severity, IssueSeverity::Residual);
        assert!(!i1.severity.is_blocking_for_gate());
    }

    #[test]
    fn markdown_bold_severity_out_of_scope() {
        let text = "- **severity**: out-of-scope\n- **plan_ref**: 后置\n";
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::OutOfScope);
    }

    #[test]
    fn parse_issues_fail_closed_without_severity() {
        let parsed = parse_issues_text("- missing plan pointer in CLAUDE.md\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::Blocking);
    }

    #[test]
    fn real_t7_issues_markdown_all_non_blocking() {
        let text = r#"
# ISSUES · t7-inspect

plan_ref: docs/chat §验收
Result companion: VERDICT.md → **PASS**

## residual

### I-1
- **severity**: residual
- **plan_ref**: S2–S6
- **fix_wp**: polish
- **说明**: archive soft 历史表

### I-2
- **severity**: residual
- **plan_ref**: 死链
- **fix_wp**: polish

## out-of-scope

### I-4
- **severity**: out-of-scope
- **plan_ref**: 后置

## 空集确认

- **blocking**: 无
- **map**: 无
"#;
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 3, "parsed={parsed:?}");
        assert!(
            parsed.iter().all(|i| !i.severity.is_blocking_for_gate()),
            "parsed={parsed:?}"
        );
        assert!(parsed.iter().any(|i| i.id.contains("I-1")));
        assert!(parsed.iter().any(|i| i.id.contains("I-2")));
        assert!(parsed.iter().any(|i| i.id.contains("I-4")));
    }

    /// Regression: inspect often writes `### R1` residual + `out-of-scope（中文说明）`.
    /// Host used to fail-closed the oos line as Blocking → false P-loop gate fail.
    #[test]
    fn residual_r_headers_and_oos_fullwidth_note_not_blocking() {
        let text = r#"# ISSUES · t6 inspect

> Result 为 PASS：无 blocking / map。

## residual

### R1 · 场景「茶席」线标为 Lucide coffee 杯形
- **severity:** residual
- **plan_ref:** §做.4
- **fix_wp:** t4
- **描述:** 线标命名略西式

### R2 · ys-006 材质字面
- **severity:** residual
- **plan_ref:** §做.3
- **fix_wp:** t2

### R3 · 静态 href
- **severity:** residual
- **plan_ref:** 主路径
- **fix_wp:** t5

## blocking

（无）

## map

（无 · L1/L2 不同构未发现）

## out-of-scope

### O1 · 浏览器实机
- **severity:** out-of-scope（本波角色=静态 inspect；任务允许无浏览器时静态完成）
- **plan_ref:** 任务大纲 7
- **fix_wp:** 人工
- **描述:** 未跑 npm run dev
"#;
        let parsed = parse_issues_text(text);
        assert_eq!(parsed.len(), 4, "parsed={parsed:?}");
        assert!(
            parsed.iter().all(|i| !i.severity.is_blocking_for_gate()),
            "blocking false-positive: {parsed:?}"
        );
        assert_eq!(
            count_blocking_for_test(&parsed),
            0,
            "blocking_n must be 0 for residual+oos-only"
        );
        let o1 = parsed.iter().find(|i| i.id.starts_with('O')).unwrap();
        assert_eq!(o1.severity, IssueSeverity::OutOfScope);
        assert!(parsed.iter().filter(|i| i.severity == IssueSeverity::Residual).count() >= 3);
    }

    #[test]
    fn severity_token_strips_fullwidth_chinese_note() {
        assert_eq!(
            severity_from_token("out-of-scope（本波角色=静态 inspect）"),
            Some(IssueSeverity::OutOfScope)
        );
        assert_eq!(
            severity_from_token("residual (optional polish)"),
            Some(IssueSeverity::Residual)
        );
        // Unknown bare token does not invent Blocking at token layer.
        assert_eq!(severity_from_token("mystery-grade"), None);
    }

    fn count_blocking_for_test(issues: &[ParsedIssue]) -> usize {
        issues
            .iter()
            .filter(|i| i.severity.is_blocking_for_gate())
            .count()
    }
}
