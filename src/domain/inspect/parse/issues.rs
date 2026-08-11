//! ISSUES body → graded rows (P-loop §3.4.3).
//!
//! Split by pure-function boundary (arch soft ≤400):
//! - [`super::parsers`] — severity / ID / empty-set / extract helpers
//!
//! [PROTOCOL]: 变更时更新 domain/inspect/CLAUDE.md 与 parse/mod.rs

use super::super::types::{IssueSeverity, ParsedIssue};
use super::parsers;

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
        let severity_field_only = parsers::is_severity_field_only_line(t);
        // Block starts: classic `- id:` / `### I-1`, residual/oos headings
        // (`### R1 · …` / `### O1 · …` / `### issue_id=R1`), or single-line
        // severity bullets (not field-only under an existing block).
        let starts_block = t.starts_with("- id:")
            || t.starts_with("-id:")
            || t.starts_with("## I-")
            || t.starts_with("### I-")
            || parsers::is_issue_id_field_heading(t)
            || (t.starts_with('-') || t.starts_with('*'))
                && (t[1..].trim_start().starts_with("I-")
                    || t[1..].trim_start().starts_with("R")
                    || t[1..].trim_start().starts_with("O"))
                && parsers::is_issue_heading_token(
                    t[1..].trim_start().split_whitespace().next().unwrap_or(""),
                )
            || (t.starts_with('#') && parsers::is_issue_heading_line(t))
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
            if parsers::extract_issue_id_from_heading_line(l).is_some() {
                return true;
            }
            let t = l.trim().trim_start_matches('#').trim();
            let bare = t
                .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
                .trim();
            let first = bare
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(|c: char| c == '·' || c == ':' || c == ',');
            parsers::is_issue_heading_token(first)
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
        let has_field_sev = parsers::parse_severity_field_only(&block).is_some();
        if !has_issue_id && !has_field_sev && first.starts_with('#') {
            continue;
        }
        // Empty-set confirmation lines (`- **blocking**: 无`) are not ISSUES.
        if !has_issue_id && parsers::is_empty_set_confirmation_block(&block) {
            continue;
        }
        let severity = parsers::parse_severity_token(&block).unwrap_or(IssueSeverity::Blocking);
        let id = parsers::extract_kv(&block, "id")
            .or_else(|| parsers::extract_kv(&block, "issue_id"))
            .or_else(|| {
                block.lines().next().and_then(|l| {
                    // `### I-1` / `### R1 · title` / `### issue_id=R1` / `- O2 · …`
                    if let Some(id) = parsers::extract_issue_id_from_heading_line(l) {
                        return Some(id);
                    }
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
                    if parsers::is_issue_heading_token(token) {
                        Some(token.to_string())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| format!("I-{}", i + 1));
        let plan_ref = parsers::extract_kv(&block, "plan_ref").unwrap_or_default();
        let path = parsers::extract_kv(&block, "path")
            .or_else(|| parsers::extract_kv(&block, "file"))
            .unwrap_or_else(|| "n/a".into());
        let symptom = parsers::extract_kv(&block, "symptom").unwrap_or_else(|| {
            block
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect()
        });
        let fix_wp = parsers::extract_kv(&block, "fix_wp")
            .or_else(|| parsers::extract_kv(&block, "suggestion"))
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

// Re-export helpers so issues_tests can `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use super::parsers::{
    extract_issue_id_from_heading_line, extract_kv, is_empty_set_confirmation_block,
    is_issue_heading_line, is_issue_heading_token, is_issue_id_field_heading,
    is_severity_field_only_line, parse_severity_field_only, parse_severity_token,
    severity_from_token, strip_severity_trailing_note,
};

#[cfg(test)]
#[path = "issues_tests.rs"]
mod tests;
