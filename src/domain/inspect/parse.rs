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
        let starts_block = t.starts_with("- id:")
            || t.starts_with("-id:")
            || t.starts_with("## I-")
            || t.starts_with("### I-")
            || (t.starts_with("- I-") || t.starts_with("* I-"))
            || (t.starts_with('-') && t.contains("severity="))
            || (t.starts_with('-') && t.contains("severity:"));
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
        ) || lower.starts_with("# issues")
            || lower == "## residual"
            || lower == "## blocking"
        {
            // Section headers alone are not issues; content under them is.
            if block.lines().count() <= 1 && (lower.starts_with('#') || lower.starts_with("##")) {
                continue;
            }
        }
        let severity = parse_severity_token(&block).unwrap_or(IssueSeverity::Blocking);
        let id = extract_kv(&block, "id")
            .or_else(|| {
                block.lines().next().and_then(|l| {
                    let t = l
                        .trim()
                        .trim_start_matches('-')
                        .trim_start_matches('*')
                        .trim();
                    if t.starts_with('I') && t.contains('-') {
                        Some(t.split_whitespace().next().unwrap_or(t).to_string())
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

fn parse_severity_token(block: &str) -> Option<IssueSeverity> {
    let lower = block.to_ascii_lowercase();
    // severity=… or severity: … (trim so "severity: residual" works)
    for key in ["severity=", "severity:"] {
        if let Some(idx) = lower.find(key) {
            let rest = lower[idx + key.len()..].trim_start();
            let token = rest
                .split(|c: char| c.is_whitespace() || c == ',' || c == '|' || c == ';')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(|c| c == '`' || c == '*' || c == '"' || c == '\'');
            if token.is_empty() {
                continue;
            }
            return Some(match token {
                "blocking" | "block" | "p0" => IssueSeverity::Blocking,
                "map" | "geb" => IssueSeverity::Map,
                "residual" | "non-blocking" | "nonblocking" | "optional" => {
                    IssueSeverity::Residual
                }
                "out-of-scope" | "outofscope" | "oos" => IssueSeverity::OutOfScope,
                _ => IssueSeverity::Blocking,
            });
        }
    }
    // Chinese / informal (whole-block hints only when no explicit severity=)
    if lower.contains("地图") || lower.contains("geb 指针") || lower.contains("l1/l2") {
        return Some(IssueSeverity::Map);
    }
    if lower.contains("residual") || lower.contains("不阻塞") || lower.contains("可选残留") {
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
    fn parse_issues_fail_closed_without_severity() {
        let parsed = parse_issues_text("- missing plan pointer in CLAUDE.md\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].severity, IssueSeverity::Blocking);
    }
}
