//! Claude output parsing: agent id, stream-json result helpers.
//!
//! [INPUT]: CLI stdout/stderr text · agents JSON
//! [OUTPUT]: agent_id · lenient JSON
//! [POS]: claude provider 解析；D4 自 claude.rs 抽出
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md

use regex::Regex;

/// Parse agent id from `backgrounded · 895cb666 (...)` or similar.
pub fn parse_agent_id(text: &str) -> Option<String> {
    let patterns = [
        r"(?i)backgrounded\s*[·•\-:]?\s*([a-f0-9]{6,})",
        r"(?i)agent\s+id[:\s]+([a-f0-9]{6,})",
        r"(?i)session[:\s]+([a-f0-9]{6,})",
        r"\b([a-f0-9]{8})\b",
    ];
    for p in patterns {
        if let Ok(re) = Regex::new(p) {
            if let Some(c) = re.captures(text) {
                return Some(c[1].to_string());
            }
        }
    }
    None
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::provider::claude::poll_bg::find_agent_state;

    #[test]
    fn parses_backgrounded_line() {
        let t = "backgrounded · 895cb666 (idle — send a prompt to start)";
        assert_eq!(parse_agent_id(t).as_deref(), Some("895cb666"));
    }

    #[test]
    fn find_agent_state_works() {
        let v = serde_json::json!([
            {"id": "abc12345", "state": "running"},
            {"id": "895cb666", "status": "done"}
        ]);
        assert_eq!(find_agent_state(&v, "895cb666").as_deref(), Some("done"));
    }
}
