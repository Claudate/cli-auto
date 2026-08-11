//! ISSUES body → graded rows (P-loop §3.4.3).

use super::super::types::{IssueSeverity, ParsedIssue};

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
        // (`### R1 · …` / `### O1 · …` / `### issue_id=R1`), or single-line
        // severity bullets (not field-only under an existing block).
        let starts_block = t.starts_with("- id:")
            || t.starts_with("-id:")
            || t.starts_with("## I-")
            || t.starts_with("### I-")
            || is_issue_id_field_heading(t)
            || (t.starts_with('-') || t.starts_with('*'))
                && (t[1..].trim_start().starts_with("I-")
                    || t[1..].trim_start().starts_with("R")
                    || t[1..].trim_start().starts_with("O"))
                && is_issue_heading_token(
                    t[1..].trim_start().split_whitespace().next().unwrap_or(""),
                )
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
            if extract_issue_id_from_heading_line(l).is_some() {
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
            is_issue_heading_token(first) || bare.starts_with("id:") || t.starts_with("- id:")
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
            .or_else(|| extract_kv(&block, "issue_id"))
            .or_else(|| {
                block.lines().next().and_then(|l| {
                    // `### I-1` / `### R1 · title` / `### issue_id=R1` / `- O2 · …`
                    if let Some(id) = extract_issue_id_from_heading_line(l) {
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
    if lower.contains("## 空集") || lower.contains("empty-set") || lower.contains("空集确认")
    {
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

/// Heading line for a single ISSUE block (`### I-1`, `### R1 · …`, `### O2 · …`,
/// `### issue_id=R1`).
fn is_issue_heading_line(t: &str) -> bool {
    if is_issue_id_field_heading(t) {
        return true;
    }
    let s = t.trim_start_matches('#').trim();
    let first = s
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(|c: char| c == '·' || c == ':' || c == ',' || c == '—');
    is_issue_heading_token(first)
}

/// `### issue_id=R1` / `- issue_id: R2` / `issue_id=B6` style headings (common in reinspect).
fn is_issue_id_field_heading(t: &str) -> bool {
    extract_issue_id_from_heading_line(t).is_some()
}

fn extract_issue_id_from_heading_line(line: &str) -> Option<String> {
    let s = line
        .trim()
        .trim_start_matches('#')
        .trim_start_matches(|c: char| c == '-' || c == '*' || c == '•')
        .trim();
    let lower = s.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("issue_id=") {
        Some(r)
    } else if let Some(r) = lower.strip_prefix("issue_id:") {
        Some(r)
    } else if let Some(r) = lower.strip_prefix("issue_id ") {
        Some(r)
    } else {
        None
    }?;
    // Map back to original slice length for the value after the key.
    let key_len = s.len() - rest.len();
    let val = s[key_len..]
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| {
            c == '`' || c == '*' || c == '"' || c == '\'' || c == '·' || c == ',' || c == ':'
        });
    if val.is_empty() {
        return None;
    }
    // Prefer classic I-/R/O tokens; otherwise keep the token (B6, R1, …).
    Some(val.to_string())
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
    let cut = s.find(['（', '(', '【', '[', '—', '–']).unwrap_or(s.len());
    s[..cut].trim()
}

fn severity_from_token(token: &str) -> Option<IssueSeverity> {
    let token = strip_severity_trailing_note(token)
        .trim()
        .trim_matches(|c: char| {
            c == '`'
                || c == '*'
                || c == '"'
                || c == '\''
                || c == '_'
                || c == '。'
                || c == '.'
                || c == '；'
                || c == ';'
        })
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
///
/// **Must not** match prose like「存在 severity=blocking 或 severity=map」— that
/// falsely fail-closed residual ISSUES when the whole preamble was one block
/// (wros reinspect-r1 · 2026-07-24).
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
        // Field-only: key at line start (after bullet/bold strip). No mid-prose find.
        let rest = if let Some(r) = s.strip_prefix("severity=") {
            Some(r)
        } else if let Some(r) = s.strip_prefix("severity:") {
            Some(r)
        } else if let Some(r) = s.strip_prefix("severity ") {
            // `severity residual` rare form — only when next token is a grade
            Some(r)
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
    // Fix A: OutOfScope must only be a *declared grade* — line-start heading / note
    // like `## out-of-scope`, `- out-of-scope`, `范围外：…` — never mid-prose.
    // Symptom prose like「feat-a (out of scope)」describes a scope *violation* and
    // must fail-closed to Blocking, not be treated as a residual grade.
    if lower.lines().any(|l| {
        let t = l
            .trim()
            .trim_start_matches(|c: char| c == '#' || c == '-' || c == '*' || c == '•')
            .trim_start();
        let compact = t
            .replace('-', " ")
            .replace('_', " ")
            .split_whitespace()
            .collect::<String>();
        t.starts_with("oos") || compact.starts_with("范围外") || compact.starts_with("outofscope")
    }) {
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
#[path = "issues_tests.rs"]
mod tests;
