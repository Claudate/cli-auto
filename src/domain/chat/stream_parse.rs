//! Extract assistant prose from Claude stream-json / plain stdout (pure).

use serde_json::Value;

/// Summarize the terminal stream-json `result` line for diagnostics.
pub fn stream_result_summary(raw: &str) -> String {
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("result") {
            continue;
        }
        let subtype = v
            .get("subtype")
            .and_then(|s| s.as_str())
            .unwrap_or("result");
        let mut parts = vec![format!("subtype={subtype}")];
        if let Some(errs) = v.get("errors").and_then(|e| e.as_array()) {
            let joined: Vec<&str> = errs.iter().filter_map(|x| x.as_str()).collect();
            if !joined.is_empty() {
                parts.push(format!("errors={}", joined.join("; ")));
            }
        }
        if let Some(n) = v.get("num_turns").and_then(|x| x.as_u64()) {
            parts.push(format!("turns={n}"));
        }
        if let Some(sr) = v.get("stop_reason").and_then(|x| x.as_str()) {
            parts.push(format!("stop={sr}"));
        }
        return format!(" · {}", parts.join(", "));
    }
    String::new()
}

/// Extract human-readable assistant text from stream-json / plain stdout.
pub fn extract_assistant_text(raw: &str) -> String {
    // 1) Prefer last successful stream-json `result.result` (string or nested text).
    //    Error envelopes (error_max_turns / is_error) fall through to assistant prose.
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "result" {
            continue;
        }
        let is_err = v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false)
            || v.get("subtype")
                .and_then(|s| s.as_str())
                .is_some_and(|s| {
                    s.eq_ignore_ascii_case("error")
                        || s.starts_with("error_")
                        || s.eq_ignore_ascii_case("error_max_turns")
                        || s.eq_ignore_ascii_case("error_max_budget_usd")
                });
        if let Some(s) = v.get("result").and_then(|r| r.as_str()) {
            if !s.trim().is_empty() {
                return s.to_string();
            }
        }
        // Some builds nest the final text under content[].text on the result line.
        if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
            let mut parts = Vec::new();
            for part in content {
                if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                        if !t.trim().is_empty() {
                            parts.push(t.to_string());
                        }
                    }
                }
            }
            if !parts.is_empty() {
                return parts.join("\n");
            }
        }
        if is_err {
            // Fall through to assistant deltas / plain text below.
            break;
        }
    }

    // 2) Collect assistant message text (full blocks + streaming deltas).
    let mut block_texts: Vec<String> = Vec::new();
    let mut delta_buf = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "assistant" {
            // Flush any in-flight deltas before a full assistant block.
            if !delta_buf.trim().is_empty() {
                block_texts.push(std::mem::take(&mut delta_buf));
            }
            let mut parts = Vec::new();
            if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                for part in content {
                    if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                parts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
                for part in content {
                    if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                            if !t.trim().is_empty() {
                                parts.push(t.to_string());
                            }
                        }
                    }
                }
            }
            if !parts.is_empty() {
                block_texts.push(parts.join("\n"));
            }
        } else if ty == "content_block_delta" {
            if let Some(t) = v.pointer("/delta/text").and_then(|x| x.as_str()) {
                if !t.is_empty() {
                    delta_buf.push_str(t);
                }
            }
        } else if ty == "content_block_stop" || ty == "message_stop" {
            if !delta_buf.trim().is_empty() {
                block_texts.push(std::mem::take(&mut delta_buf));
            }
        }
    }
    if !delta_buf.trim().is_empty() {
        block_texts.push(delta_buf);
    }
    if !block_texts.is_empty() {
        // Prefer the longest complete prose block (final answer over short tool preambles).
        let best = block_texts
            .iter()
            .max_by_key(|s| s.chars().count())
            .cloned()
            .unwrap_or_default();
        if best.chars().count() >= 40 {
            return best;
        }
        // Short-only stream (e.g. max_turns cut mid-tool): return whatever we have.
        let joined = block_texts.join("\n\n");
        if !joined.trim().is_empty() {
            return joined;
        }
        return best;
    }

    // 3) Plain text fallback: strip pure-JSON NDJSON lines, keep non-JSON tails.
    let mut plain_parts: Vec<&str> = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('{') {
            continue;
        }
        plain_parts.push(t);
    }
    if !plain_parts.is_empty() {
        return plain_parts.join("\n");
    }
    String::new()
}
