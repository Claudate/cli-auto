//! Extract cco-split JSON from model / CLI stdout.
//!
//! [INPUT]: raw model text (plain · fenced · stream-json NDJSON)
//! [OUTPUT]: JSON object string or Err
//! [POS]: plan/split_agent — pure; no soft_accept
//! [PROTOCOL]: 变更时更新此头部；与 parse.rs 同模块

use anyhow::{bail, Result};

/// Extract cco-split JSON from model / CLI stdout.
///
/// Claude print uses `--output-format stream-json` (NDJSON). The plan lives in the
/// final `{"type":"result","result":"…"}` string (often fenced ```json), **not** in
/// the first `{` on the stream (system/init events). Same pitfall as planner LLM.
pub fn extract_json_object(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.is_empty() {
        bail!("拆分 Agent 输出为空");
    }

    // 1) stream-json NDJSON → last type=result envelope
    if let Some(s) = extract_from_stream_json(t) {
        return Ok(s);
    }

    // 2) fenced ```json … ``` in plain text
    if let Some(s) = extract_fenced_json(t) {
        if looks_like_split_json(&s) {
            return Ok(s);
        }
    }

    // 3) multiline / bare: first complete balanced object that looks like cco-split
    //    (must run before per-line scan — first line of pretty JSON is incomplete)
    if let Some(i) = t.find('{') {
        if let Some(slice) = balanced_object(&t[i..]) {
            if looks_like_split_json(slice) || slice.contains("\"tasks\"") {
                return Ok(slice.to_string());
            }
        }
    }

    // 4) any *complete* single-line object (NDJSON-ish bare plan line)
    for line in t.lines().rev() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Some(complete) = balanced_object(line) else {
            continue;
        };
        // incomplete line (e.g. `{"tasks":[`) must not win
        if complete.len() != line.len() {
            continue;
        }
        if looks_like_split_json(complete) {
            return Ok(complete.to_string());
        }
    }

    // 5) fenced without lookalike check; largest brace span last resort
    if let Some(s) = extract_fenced_json(t) {
        return Ok(s);
    }
    if let (Some(start), Some(end)) = (t.find('{'), t.rfind('}')) {
        if end > start {
            let slice = &t[start..=end];
            if looks_like_split_json(slice) {
                return Ok(slice.to_string());
            }
        }
    }
    bail!("拆分 Agent 输出中找不到 cco-split JSON 对象（若走了 stream-json，需含 type=result）")
}

fn looks_like_split_json(s: &str) -> bool {
    let t = s.trim();
    if !t.starts_with('{') {
        return false;
    }
    // cco-split/v1 or has tasks array
    (t.contains("cco-split") || t.contains("\"tasks\""))
        && (t.contains("\"schema\"") || t.contains("\"title\"") || t.contains("\"id\""))
}

fn extract_from_stream_json(raw: &str) -> Option<String> {
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let is_result = v
            .get("type")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t == "result");
        if !is_result {
            // assistant message content may already embed the plan
            if let Some(s) = split_json_from_value(&v) {
                return Some(s);
            }
            continue;
        }
        if let Some(s) = split_json_from_value(&v) {
            return Some(s);
        }
    }
    None
}

fn split_json_from_value(v: &serde_json::Value) -> Option<String> {
    // Nested plan object
    if looks_like_split_value(v) {
        return serde_json::to_string(v).ok();
    }
    // result: string (fenced or bare JSON)
    if let Some(s) = v.get("result").and_then(|r| r.as_str()) {
        if let Some(f) = extract_fenced_json(s) {
            return Some(f);
        }
        let st = s.trim();
        if looks_like_split_json(st) {
            return Some(st.to_string());
        }
        if let Some(i) = st.find('{') {
            if let Some(slice) = balanced_object(&st[i..]) {
                if looks_like_split_json(slice) {
                    return Some(slice.to_string());
                }
            }
        }
    }
    // result: object
    if let Some(obj) = v.get("result") {
        if looks_like_split_value(obj) {
            return serde_json::to_string(obj).ok();
        }
    }
    // message.content[].text (assistant events)
    if let Some(content) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content.iter().rev() {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                if let Some(f) = extract_fenced_json(text) {
                    if looks_like_split_json(&f) {
                        return Some(f);
                    }
                }
            }
        }
    }
    None
}

fn looks_like_split_value(v: &serde_json::Value) -> bool {
    v.get("tasks").and_then(|t| t.as_array()).is_some()
        || v.get("schema")
            .and_then(|s| s.as_str())
            .is_some_and(|s| s.contains("cco-split"))
}

fn extract_fenced_json(text: &str) -> Option<String> {
    let t = text.trim();
    let start = t.find("```")?;
    let after = &t[start + 3..];
    let after = after
        .strip_prefix("json")
        .or_else(|| after.strip_prefix("JSON"))
        .unwrap_or(after);
    let after = after.trim_start_matches(|c: char| c == '\r' || c == '\n' || c == ' ');
    let end = after.find("```")?;
    let block = after[..end].trim();
    if block.starts_with('{') {
        Some(block.to_string())
    } else {
        None
    }
}

fn balanced_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_multiline_not_truncated_at_first_line() {
        let raw = r#"{"schema":"cco-split/v1","title":"T","tasks":[
          {"id":"t1","title":"A","body":"do"}
        ]}"#;
        let s = extract_json_object(raw).unwrap();
        assert!(s.contains("t1"));
        assert!(s.contains("]"));
    }

    #[test]
    fn stream_json_prefers_result_envelope_not_init() {
        // First brace is system/init; plan lives in type=result.result (fenced).
        let raw = r#"{"type":"system","subtype":"init","session_id":"x"}
{"type":"assistant","message":{"content":[{"type":"text","text":"thinking…"}]}}
{"type":"result","result":"```json\n{\"schema\":\"cco-split/v1\",\"title\":\"S\",\"tasks\":[{\"id\":\"a\",\"title\":\"做 A\",\"body\":\"完成\"}]}\n```"}
"#;
        let s = extract_json_object(raw).unwrap();
        assert!(s.contains("cco-split/v1"), "got {s}");
        assert!(s.contains("\"id\":\"a\"") || s.contains("做 A"), "got {s}");
    }

    #[test]
    fn incomplete_first_line_does_not_win() {
        let raw = r#"{"schema":"cco-split/v1","title":"T","tasks":[
          {"id":"z","title":"Z","body":"z"}
        ]}"#;
        let s = extract_json_object(raw).unwrap();
        assert!(s.contains("\"id\":\"z\"") || s.contains("\"z\""));
    }
}
