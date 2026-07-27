//! session-digest/v1 pure checks (built-in chat compression cache).
//!
//! [INPUT]: raw YAML / fence body string
//! [OUTPUT]: accept / reject reasons · prompt block for next turn
//! [POS]: domain/chat — **no** fs / path join / confirm
//! [PROTOCOL]: field rules mirror docs/contracts/session-digest.md

/// Soft cap for host-stored digest text (chars).
pub const SESSION_DIGEST_SOFT_MAX_CHARS: usize = 12_000;

/// True when body looks like a minimally acceptable session-digest/v1 document.
///
/// Checks are intentionally string-level (no YAML crate in domain): schema line,
/// non-empty goal, and at least one of constraints/decisions/dont structure.
/// Full decision.rejected enforcement stays in the model prompt + docs contract;
/// host only rejects empty / prose-only blobs.
pub fn session_digest_looks_valid(raw: &str) -> bool {
    session_digest_reject_reason(raw).is_none()
}

/// Human-readable reject reason, or `None` if acceptable enough to store.
pub fn session_digest_reject_reason(raw: &str) -> Option<&'static str> {
    let t = raw.trim();
    if t.is_empty() {
        return Some("empty");
    }
    if t.chars().count() > SESSION_DIGEST_SOFT_MAX_CHARS {
        return Some("too_long");
    }
    let lower = t.to_ascii_lowercase();
    if !lower.contains("session-digest/v1") && !lower.contains("schema: session-digest/v1") {
        // allow `schema: session-digest/v1` or bare version mention in first lines
        let head: String = t.lines().take(8).collect::<Vec<_>>().join("\n");
        let head_l = head.to_ascii_lowercase();
        if !head_l.contains("session-digest") {
            return Some("missing_schema");
        }
    }
    let has_goal = t.lines().any(|l| {
        let s = l.trim();
        s.starts_with("goal:") && s.len() > "goal:".len() && s["goal:".len()..].trim().len() > 0
            || s == "goal: |"
            || s == "goal: >"
    });
    if !has_goal {
        return Some("missing_goal");
    }
    let has_structure = t.contains("constraints:")
        || t.contains("decisions:")
        || t.contains("dont:")
        || t.contains("open:");
    if !has_structure {
        return Some("missing_structure");
    }
    None
}

/// Truncate for storage (char-based).
pub fn truncate_session_digest(raw: &str) -> String {
    let t = raw.trim();
    let n = t.chars().count();
    if n <= SESSION_DIGEST_SOFT_MAX_CHARS {
        return t.to_string();
    }
    format!(
        "{}…",
        t.chars()
            .take(SESSION_DIGEST_SOFT_MAX_CHARS.saturating_sub(1))
            .collect::<String>()
    )
}

/// Block injected into the next chat turn (after pins/summary). Not a second system novel.
pub fn format_session_digest_prompt_block(yaml: &str) -> String {
    let body = truncate_session_digest(yaml);
    if body.is_empty() {
        return String::new();
    }
    format!(
        "--- 会话压缩状态（session-digest/v1 · 内置 · 每轮维护 · 非开跑指令）---\n\
先遵守下列 goal / constraints / decisions.rejected / dont；细节按 source 与 artifacts 回查对话。\n\
禁止用本块触发分配计划之外的业务开跑。\n\n{body}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_minimal_example_shape() {
        let y = r#"
schema: session-digest/v1
updated_at: "2026-07-27T00:00:00Z"
goal: "落地内置压缩"
constraints:
  - id: C1
    text: "confirm 唯一开跑"
    source: "CLAUDE.md"
dont:
  - id: X1
    text: "禁止旁路 confirm"
"#;
        assert!(session_digest_looks_valid(y));
        assert!(session_digest_reject_reason(y).is_none());
    }

    #[test]
    fn rejects_prose_only() {
        assert_eq!(
            session_digest_reject_reason("大致按以前来"),
            Some("missing_schema")
        );
    }

    #[test]
    fn rejects_schema_without_goal() {
        let y = "schema: session-digest/v1\nconstraints:\n  - id: C1\n    text: x\n";
        assert_eq!(session_digest_reject_reason(y), Some("missing_goal"));
    }
}
