//! Structured VERDICT / Result line parser (host gate — no prose guessing).

use super::super::types::InspectVerdict;

/// Parse raw VERDICT text into Pass / Fail / Unknown.
///
/// **Contract (host gate — no prose guessing):**
/// 1. Prefer structured result lines only:
///    `Result: PASS|FAIL`, `Result: **PASS**`, `VERDICT: …`, `VERDICT=…`,
///    bare first token `PASS` / `FAIL` on its own line.
/// 2. Scan the **head** of the file (first ~40 non-empty lines) for the first
///    structured result; that wins.
/// 3. **Never** whole-body scan for bare `FAIL` / `PASS` words — reinspect prose
///    often says「P1b 可选 FAIL 不阻塞」and must not flip a structured PASS
///    (wros check-p0-acceptance · 2026-07-24).
pub fn parse_verdict_text(text: &str) -> InspectVerdict {
    let mut seen = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        seen += 1;
        if let Some(v) = structured_verdict_on_line(t) {
            return v;
        }
        // Skip title / meta lines; stop after head window.
        if seen >= 40 {
            break;
        }
    }
    InspectVerdict::Unknown
}

/// Structured result on a single line (markdown bold/code stripped for match).
///
/// Accepts only:
/// - bare `PASS` / `FAIL` (optional trailing `.` / `。`)
/// - bare grade with delimiter: `FAIL: reason` / `PASS | ok` / `FAIL — …`
/// - keyed: `Result: PASS` / `VERDICT=FAIL` (anywhere on the line after strip)
///
/// Rejects prose like `PASS was hoped` or `P1b optional FAIL` (no key, not bare grade).
fn structured_verdict_on_line(line: &str) -> Option<InspectVerdict> {
    // Collapse markdown emphasis so `Result: **PASS**` / `**Result: PASS**` share path.
    let stripped: String = line
        .chars()
        .filter(|c| *c != '*' && *c != '`' && *c != '_')
        .collect();
    let t = stripped.trim();
    if t.is_empty() {
        return None;
    }
    let upper = t.to_ascii_uppercase();

    // Bare grade alone.
    if matches!(
        upper.as_str(),
        "FAIL" | "FAIL." | "FAIL。" | "PASS" | "PASS." | "PASS。"
    ) {
        return if upper.starts_with("FAIL") {
            Some(InspectVerdict::Fail)
        } else {
            Some(InspectVerdict::Pass)
        };
    }

    // Bare grade + delimiter (not space-separated prose).
    for (grade, verdict) in [
        ("FAIL", InspectVerdict::Fail),
        ("PASS", InspectVerdict::Pass),
    ] {
        if upper == grade {
            return Some(verdict);
        }
        for sep in [":", "|", "—", "–", " -"] {
            let prefix = format!("{grade}{sep}");
            if upper.starts_with(&prefix) {
                return Some(verdict);
            }
        }
    }

    // Keyed forms: Result / VERDICT (first key occurrence on the line).
    for key in [
        "RESULT:",
        "RESULT =",
        "RESULT=",
        "VERDICT:",
        "VERDICT =",
        "VERDICT=",
    ] {
        if let Some(idx) = upper.find(key) {
            let after = upper[idx + key.len()..].trim_start();
            let token = after
                .split(|c: char| {
                    c.is_whitespace()
                        || c == '|'
                        || c == ','
                        || c == ';'
                        || c == '—'
                        || c == '–'
                        || c == '('
                        || c == '（'
                })
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| {
                    c == '.'
                        || c == '。'
                        || c == ':'
                        || c == '*'
                        || c == '`'
                        || c == '"'
                        || c == '\''
                });
            if token == "FAIL" {
                return Some(InspectVerdict::Fail);
            }
            if token == "PASS" {
                return Some(InspectVerdict::Pass);
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
        // Structured VERDICT= line wins; prose "PASS was hoped" is ignored.
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
        assert_eq!(parse_verdict_text("Result: PASS\nok"), InspectVerdict::Pass);
        assert_eq!(
            parse_verdict_text("Result: **PASS**\n\n- role: inspect"),
            InspectVerdict::Pass
        );
    }

    /// Contract: structured Result: PASS must not be flipped by body prose mentioning FAIL.
    #[test]
    fn structured_pass_not_flipped_by_prose_fail() {
        let text = r#"# VERDICT · check-p0-acceptance

Result: **PASS**

- role: inspect
- plan_ref: P0

## Summary

P1b 可选 FAIL 不计入 P0 blocking。
open: blocking=0 · residual=4 → Result: PASS

| 门 | 结果 |
| W1 | PASS |
"#;
        assert_eq!(parse_verdict_text(text), InspectVerdict::Pass);
        // No Result/VERDICT key → Unknown (do not whole-body guess).
        assert_eq!(
            parse_verdict_text("smoke failed\nsee logs\nFAIL path"),
            InspectVerdict::Unknown
        );
    }
}
