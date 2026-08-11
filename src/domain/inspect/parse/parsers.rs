//! ISSUES parser helpers (pure functions for parsing text blocks, severity, IDs, etc.).
//!
//! Split from main parse_issues_text (arch soft ≤400).
//!
//! [PROTOCOL]: 变更时更新 domain/inspect/CLAUDE.md 与 parse/mod.rs

use super::super::types::IssueSeverity;

pub(crate) fn is_empty_set_confirmation_block(block: &str) -> bool {
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

pub(crate) fn is_severity_field_only_line(t: &str) -> bool {
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

pub(crate) fn is_issue_heading_line(t: &str) -> bool {
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

pub(crate) fn is_issue_id_field_heading(t: &str) -> bool {
    extract_issue_id_from_heading_line(t).is_some()
}

pub(crate) fn extract_issue_id_from_heading_line(line: &str) -> Option<String> {
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

pub(crate) fn is_issue_heading_token(token: &str) -> bool {
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

pub(crate) fn strip_severity_trailing_note(s: &str) -> &str {
    let cut = s.find(['（', '(', '【', '[', '—', '–']).unwrap_or(s.len());
    s[..cut].trim()
}

pub(crate) fn severity_from_token(token: &str) -> Option<IssueSeverity> {
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

pub(crate) fn parse_severity_field_only(block: &str) -> Option<IssueSeverity> {
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

pub(crate) fn parse_severity_token(block: &str) -> Option<IssueSeverity> {
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

pub(crate) fn extract_kv(block: &str, key: &str) -> Option<String> {
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
