//! [INPUT]: 依赖 worker stdout/stderr 文本（Claude stream-json NDJSON / 纯文本）
//! [OUTPUT]: 对外提供 LogEvent、parse_worker_logs、events_to_plain、read_text_tail
//! [POS]: runtime 的日志语义层，供 services 桌面 live / planner 视图消费
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md

use serde::{Deserialize, Serialize};

/// 单条可展示日志事件（机器相 stdout 的语义相）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEvent {
    pub id: String,
    /// meta | message | tool_use | tool_result | result | error | stderr | raw_line
    pub kind: String,
    /// stdout | stderr | system
    pub stream: String,
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// info | warn | error | success
    pub level: String,
}

const DEFAULT_MAX_EVENTS: usize = 200;
const DETAIL_CAP: usize = 4_000;
const SUMMARY_CAP: usize = 240;

/// 解析 worker 输出为事件列表。
/// stdout：逐行语义化；stderr：折叠为 1 条摘要（避免粉红卡片墙）。
pub fn parse_worker_logs(stdout: &str, stderr: &str, max_events: usize) -> Vec<LogEvent> {
    let cap = if max_events == 0 {
        DEFAULT_MAX_EVENTS
    } else {
        max_events
    };
    let mut events = Vec::new();
    let mut idx = 0usize;

    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        // 跳过 tail 截断标记行（不当正文事件）
        if is_truncation_marker(line) {
            continue;
        }
        idx += 1;
        let ev = parse_stdout_line(idx, line);
        // 过滤几乎无信息的 meta 噪音（可选保留 init）
        if ev.kind == "meta" && ev.summary == "…" {
            continue;
        }
        events.push(ev);
    }

    // stderr 折叠：一条摘要 + detail 里放尾部若干行
    let stderr_lines: Vec<&str> = stderr
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty() && !is_truncation_marker(l))
        .collect();
    if !stderr_lines.is_empty() {
        idx += 1;
        let n = stderr_lines.len();
        let tail_n = 40.min(n);
        let tail = stderr_lines[n - tail_n..].join("\n");
        let first = stderr_lines.first().copied().unwrap_or("");
        let level = if stderr_lines.iter().any(|l| looks_like_error(l)) {
            "warn"
        } else {
            "info"
        };
        events.push(LogEvent {
            id: format!("e{idx}"),
            kind: "stderr".into(),
            stream: "stderr".into(),
            title: format!("stderr · {n} 行"),
            summary: truncate(first, SUMMARY_CAP),
            detail: Some(truncate(&tail, DETAIL_CAP.max(8000))),
            level: level.into(),
        });
    }

    // 只保留尾部 cap 条（stdout 事件优先：cap 用在合并后）
    if events.len() > cap {
        events = events.split_off(events.len() - cap);
    }
    events
}

fn is_truncation_marker(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("… (truncated")
        || t.starts_with("... (truncated")
        || t.contains("(truncated,") && t.contains("bytes total)")
}

/// 从事件生成可读纯文本（复制用）。
pub fn events_to_plain(events: &[LogEvent]) -> String {
    let mut out = String::new();
    for e in events {
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!("[{}] {} — {}\n", e.kind, e.title, e.summary),
        );
        if let Some(d) = &e.detail {
            for line in d.lines().take(12) {
                let _ = std::fmt::Write::write_fmt(&mut out, format_args!("    {}\n", line));
            }
        }
    }
    out
}

/// 从全文抽一行错误摘要。
pub fn error_summary_from(events: &[LogEvent], fallback: Option<&str>) -> Option<String> {
    if let Some(e) = events.iter().rev().find(|e| e.level == "error" || e.kind == "error") {
        return Some(truncate(&format!("{}: {}", e.title, e.summary), SUMMARY_CAP));
    }
    fallback.map(|s| truncate(s, SUMMARY_CAP))
}

fn parse_stdout_line(idx: usize, line: &str) -> LogEvent {
    let id = format!("e{idx}");
    // 跳过我们自己写的 start banner
    if line.starts_with('[') && line.contains("starting claude") {
        return LogEvent {
            id,
            kind: "meta".into(),
            stream: "system".into(),
            title: "启动".into(),
            summary: truncate(line, SUMMARY_CAP),
            detail: None,
            level: "info".into(),
        };
    }

    let trimmed = line.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return event_from_json(id, &v, trimmed);
    }

    // 非 JSON 纯文本
    let level = if looks_like_error(trimmed) {
        "error"
    } else {
        "info"
    };
    LogEvent {
        id,
        kind: if level == "error" {
            "error".into()
        } else {
            "raw_line".into()
        },
        stream: "stdout".into(),
        title: if level == "error" { "错误" } else { "输出" }.into(),
        summary: truncate(trimmed, SUMMARY_CAP),
        detail: if trimmed.len() > SUMMARY_CAP {
            Some(truncate(trimmed, DETAIL_CAP))
        } else {
            None
        },
        level: level.into(),
    }
}

fn event_from_json(id: String, v: &serde_json::Value, raw: &str) -> LogEvent {
    let ty = v
        .get("type")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ty.as_str() {
        "system" | "meta" => {
            let sub = v
                .get("subtype")
                .and_then(|x| x.as_str())
                .unwrap_or("system");
            LogEvent {
                id,
                kind: "meta".into(),
                stream: "system".into(),
                title: "系统".into(),
                summary: truncate(sub, SUMMARY_CAP),
                detail: None,
                level: "info".into(),
            }
        }
        "assistant" | "message" => {
            // Prefer nested tool_use blocks when present (Claude stream-json).
            if let Some((title, summary, detail, is_err)) = extract_tool_result(v) {
                // extract_tool_result also returns tool_use blocks from assistant content
                let kind = if title.starts_with("结果") {
                    "tool_result"
                } else if summary == "工具调用" || v
                    .pointer("/message/content")
                    .and_then(|c| c.as_array())
                    .map(|a| {
                        a.iter().any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                    })
                    .unwrap_or(false)
                    || v
                        .get("content")
                        .and_then(|c| c.as_array())
                        .map(|a| {
                            a.iter()
                                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                        })
                        .unwrap_or(false)
                {
                    // distinguish tool_use vs tool_result by block type
                    let is_use = v
                        .pointer("/message/content")
                        .and_then(|c| c.as_array())
                        .or_else(|| v.get("content").and_then(|c| c.as_array()))
                        .map(|a| {
                            a.iter()
                                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                        })
                        .unwrap_or(false)
                        && !title.starts_with("结果");
                    if is_use {
                        "tool_use"
                    } else {
                        "tool_result"
                    }
                } else {
                    "message"
                };
                return LogEvent {
                    id,
                    kind: kind.into(),
                    stream: "stdout".into(),
                    title,
                    summary,
                    detail,
                    level: if is_err {
                        "error".into()
                    } else if kind == "tool_result" {
                        "success".into()
                    } else {
                        "info".into()
                    },
                };
            }
            let text = extract_text_content(v);
            LogEvent {
                id,
                kind: "message".into(),
                stream: "stdout".into(),
                title: "助手".into(),
                summary: truncate(&text, SUMMARY_CAP),
                detail: if text.len() > SUMMARY_CAP {
                    Some(truncate(&text, DETAIL_CAP))
                } else {
                    None
                },
                level: "info".into(),
            }
        }
        "content_block_start" | "content_block_delta" | "content_block_stop" => {
            // 流式增量：尽量抽 text / tool name
            if let Some(name) = v
                .pointer("/content_block/name")
                .and_then(|x| x.as_str())
                .or_else(|| v.pointer("/delta/name").and_then(|x| x.as_str()))
            {
                return LogEvent {
                    id,
                    kind: "tool_use".into(),
                    stream: "stdout".into(),
                    title: name.into(),
                    summary: "工具调用".into(),
                    detail: None,
                    level: "info".into(),
                };
            }
            let text = v
                .pointer("/delta/text")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if text.is_empty() {
                return LogEvent {
                    id,
                    kind: "meta".into(),
                    stream: "stdout".into(),
                    title: ty,
                    summary: "…".into(),
                    detail: None,
                    level: "info".into(),
                };
            }
            LogEvent {
                id,
                kind: "message".into(),
                stream: "stdout".into(),
                title: "助手".into(),
                summary: truncate(text, SUMMARY_CAP),
                detail: None,
                level: "info".into(),
            }
        }
        "user" => {
            // 常含 tool_result
            if let Some((title, summary, detail, is_err)) = extract_tool_result(v) {
                return LogEvent {
                    id,
                    kind: "tool_result".into(),
                    stream: "stdout".into(),
                    title,
                    summary,
                    detail,
                    level: if is_err { "error".into() } else { "success".into() },
                };
            }
            let text = extract_text_content(v);
            LogEvent {
                id,
                kind: "message".into(),
                stream: "stdout".into(),
                title: "用户/工具回传".into(),
                summary: truncate(&text, SUMMARY_CAP),
                detail: if text.len() > SUMMARY_CAP {
                    Some(truncate(&text, DETAIL_CAP))
                } else {
                    None
                },
                level: "info".into(),
            }
        }
        "tool_use" | "tool_call" => {
            let name = v
                .get("name")
                .or_else(|| v.pointer("/tool_use/name"))
                .or_else(|| v.pointer("/tool_call/function/name"))
                .and_then(|x| x.as_str())
                .unwrap_or("tool");
            let input = v
                .get("input")
                .or_else(|| v.pointer("/tool_use/input"))
                .or_else(|| v.pointer("/tool_call/function/arguments"))
                .map(|x| {
                    if x.is_string() {
                        x.as_str().unwrap_or("").to_string()
                    } else {
                        x.to_string()
                    }
                })
                .unwrap_or_default();
            let summary = first_line_summary(&input);
            LogEvent {
                id,
                kind: "tool_use".into(),
                stream: "stdout".into(),
                title: name.into(),
                summary,
                detail: if input.len() > 80 {
                    Some(truncate(&input, DETAIL_CAP))
                } else {
                    None
                },
                level: "info".into(),
            }
        }
        "tool_result" => {
            let content = v
                .get("content")
                .map(|c| {
                    if c.is_string() {
                        c.as_str().unwrap_or("").to_string()
                    } else {
                        c.to_string()
                    }
                })
                .unwrap_or_default();
            let is_err = v
                .get("is_error")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
                || looks_like_error(&content);
            LogEvent {
                id,
                kind: "tool_result".into(),
                stream: "stdout".into(),
                title: "工具结果".into(),
                summary: truncate(&content, SUMMARY_CAP),
                detail: if content.len() > SUMMARY_CAP {
                    Some(truncate(&content, DETAIL_CAP))
                } else {
                    None
                },
                level: if is_err { "error".into() } else { "success".into() },
            }
        }
        "result" => {
            let sub = v
                .get("subtype")
                .and_then(|x| x.as_str())
                .unwrap_or("result");
            let cost = v
                .get("total_cost_usd")
                .or_else(|| v.get("cost_usd"))
                .and_then(|x| x.as_f64());
            let is_err = sub.contains("error")
                || v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false)
                || v.get("error").is_some();
            let mut summary = sub.to_string();
            if let Some(c) = cost {
                summary = format!("{sub} · ${c:.4}");
            }
            if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
                summary = format!("{summary} · {err}");
            }
            // result 常带最终文本
            let result_text = v
                .get("result")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            LogEvent {
                id,
                kind: "result".into(),
                stream: "stdout".into(),
                title: if is_err { "失败" } else { "完成" }.into(),
                summary: truncate(&summary, SUMMARY_CAP),
                detail: if result_text.is_empty() {
                    None
                } else {
                    Some(truncate(&result_text, DETAIL_CAP))
                },
                level: if is_err { "error".into() } else { "success".into() },
            }
        }
        "error" => {
            let msg = v
                .get("error")
                .or_else(|| v.get("message"))
                .map(|x| {
                    if x.is_string() {
                        x.as_str().unwrap_or("").to_string()
                    } else {
                        x.to_string()
                    }
                })
                .unwrap_or_else(|| "error".into());
            LogEvent {
                id,
                kind: "error".into(),
                stream: "stdout".into(),
                title: "错误".into(),
                summary: truncate(&msg, SUMMARY_CAP),
                detail: Some(truncate(raw, DETAIL_CAP)),
                level: "error".into(),
            }
        }
        // Codex / 其他 agent 常见字段
        "item" | "event" | "response" | "agent_message" => {
            let text = extract_text_content(v);
            let summary = if text.is_empty() {
                truncate(raw, SUMMARY_CAP)
            } else {
                truncate(&text, SUMMARY_CAP)
            };
            LogEvent {
                id,
                kind: "message".into(),
                stream: "stdout".into(),
                title: if ty.is_empty() { "事件".into() } else { ty },
                summary,
                detail: None,
                level: "info".into(),
            }
        }
        _ => {
            // 有 type 但不认识，或无 type 的 JSON
            if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
                if v.get("input").is_some() || v.get("arguments").is_some() {
                    return event_from_json(
                        id,
                        &serde_json::json!({"type":"tool_use","name":name,"input":v.get("input").cloned().unwrap_or(serde_json::Value::Null)}),
                        raw,
                    );
                }
            }
            let text = extract_text_content(v);
            if !text.is_empty() {
                return LogEvent {
                    id,
                    kind: "message".into(),
                    stream: "stdout".into(),
                    title: if ty.is_empty() { "JSON".into() } else { ty },
                    summary: truncate(&text, SUMMARY_CAP),
                    detail: if text.len() > SUMMARY_CAP {
                        Some(truncate(&text, DETAIL_CAP))
                    } else {
                        None
                    },
                    level: "info".into(),
                };
            }
            LogEvent {
                id,
                kind: "raw_line".into(),
                stream: "stdout".into(),
                title: if ty.is_empty() { "JSON".into() } else { ty },
                summary: truncate(raw, SUMMARY_CAP),
                detail: if raw.len() > SUMMARY_CAP {
                    Some(truncate(raw, DETAIL_CAP))
                } else {
                    None
                },
                level: "info".into(),
            }
        }
    }
}

fn extract_text_content(v: &serde_json::Value) -> String {
    // message.content 可能是 string 或 array of blocks
    if let Some(s) = v.get("content").and_then(|c| c.as_str()) {
        return s.to_string();
    }
    if let Some(s) = v.pointer("/message/content").and_then(|c| c.as_str()) {
        return s.to_string();
    }
    if let Some(arr) = v
        .get("content")
        .and_then(|c| c.as_array())
        .or_else(|| v.pointer("/message/content").and_then(|c| c.as_array()))
    {
        let mut parts = Vec::new();
        for block in arr {
            let bty = block.get("type").and_then(|x| x.as_str()).unwrap_or("");
            match bty {
                "text" => {
                    if let Some(t) = block.get("text").and_then(|x| x.as_str()) {
                        parts.push(t.to_string());
                    }
                }
                "tool_use" => {
                    let name = block.get("name").and_then(|x| x.as_str()).unwrap_or("tool");
                    parts.push(format!("→ {name}"));
                }
                "tool_result" => {
                    let t = block
                        .get("content")
                        .map(|c| {
                            if c.is_string() {
                                c.as_str().unwrap_or("").to_string()
                            } else {
                                c.to_string()
                            }
                        })
                        .unwrap_or_default();
                    parts.push(truncate(&t, 120));
                }
                _ => {
                    if let Some(t) = block.get("text").and_then(|x| x.as_str()) {
                        parts.push(t.to_string());
                    }
                }
            }
        }
        return parts.join("\n");
    }
    if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    if let Some(s) = v.get("result").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    String::new()
}

fn extract_tool_result(v: &serde_json::Value) -> Option<(String, String, Option<String>, bool)> {
    let arr = v
        .get("content")
        .and_then(|c| c.as_array())
        .or_else(|| v.pointer("/message/content").and_then(|c| c.as_array()))?;
    for block in arr {
        let bty = block.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if bty != "tool_result" {
            continue;
        }
        let content = block
            .get("content")
            .map(|c| {
                if c.is_string() {
                    c.as_str().unwrap_or("").to_string()
                } else {
                    c.to_string()
                }
            })
            .unwrap_or_default();
        let is_err = block
            .get("is_error")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
            || looks_like_error(&content);
        let tool_use_id = block
            .get("tool_use_id")
            .and_then(|x| x.as_str())
            .unwrap_or("tool");
        return Some((
            format!("结果·{tool_use_id}"),
            truncate(&content, SUMMARY_CAP),
            if content.len() > SUMMARY_CAP {
                Some(truncate(&content, DETAIL_CAP))
            } else {
                None
            },
            is_err,
        ));
    }
    // assistant content with tool_use only
    for block in arr {
        if block.get("type").and_then(|x| x.as_str()) == Some("tool_use") {
            let name = block.get("name").and_then(|x| x.as_str()).unwrap_or("tool");
            let input = block
                .get("input")
                .map(|x| x.to_string())
                .unwrap_or_default();
            return Some((
                name.into(),
                first_line_summary(&input),
                if input.len() > 80 {
                    Some(truncate(&input, DETAIL_CAP))
                } else {
                    None
                },
                false,
            ));
        }
    }
    None
}

fn first_line_summary(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return "…".into();
    }
    // 尝试 JSON 里抽 path/file/command
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
        for key in ["path", "file_path", "file", "command", "pattern", "query"] {
            if let Some(p) = v.get(key).and_then(|x| x.as_str()) {
                return truncate(p, SUMMARY_CAP);
            }
        }
    }
    truncate(t, SUMMARY_CAP)
}

fn looks_like_error(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.contains("error")
        || l.contains("failed")
        || l.contains("traceback")
        || l.contains("exception")
        || l.contains("panic")
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\r', "");
    if s.chars().count() <= max {
        return s;
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Floor a byte index to the nearest previous UTF-8 char boundary.
/// Never returns past `s.len()`; `0` is always a boundary.
pub fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Compact a long log string for live IPC: keep a line-aligned tail under
/// `soft_cap` **bytes**, never slicing mid-char (CJK/emoji safe).
pub fn compact_text_tail(full: &str, soft_cap: usize, marker: &str) -> String {
    if full.len() <= soft_cap {
        return full.to_string();
    }
    let start = floor_char_boundary(full, full.len().saturating_sub(soft_cap));
    let slice = &full[start..];
    if let Some(pos) = slice.find('\n') {
        format!("{marker}{}", &slice[pos + 1..])
    } else {
        format!("{marker}{slice}")
    }
}

/// 行边界 tail：避免半截 JSON。
pub fn read_text_tail(path: &std::path::Path, max_bytes: usize) -> (String, u64) {
    let meta_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if meta_len == 0 {
        return (String::new(), 0);
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(max_bytes);
            let slice = &bytes[start..];
            // lossy decode already yields valid UTF-8; still floor if we drop
            // a leading incomplete sequence from a mid-file byte cut.
            let mut text = String::from_utf8_lossy(slice).into_owned();
            if start > 0 {
                if let Some(pos) = text.find('\n') {
                    text = text[pos + 1..].to_string();
                }
                text = format!("… (truncated, {meta_len} bytes total)\n{text}");
            }
            (text, meta_len)
        }
        Err(_) => (String::new(), meta_len),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_stream_json() -> String {
        // Prefer repo fixture (tests/fixtures/claude-stream-json.ndjson); fall back to inline.
        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-stream-json.ndjson"),
            PathBuf::from("tests/fixtures/claude-stream-json.ndjson"),
        ];
        for p in &candidates {
            if let Ok(s) = std::fs::read_to_string(p) {
                return s;
            }
        }
        r#"
{"type":"system","subtype":"init","session_id":"sess-fixture-1"}
{"type":"assistant","message":{"content":[{"type":"text","text":"I'll inspect the project layout first."}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01","name":"Read","input":{"path":"src/main.rs"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_01","content":"fn main() {}"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_02","name":"Bash","input":{"command":"cargo test -q"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_02","is_error":true,"content":"error: no tests"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Tests are missing; I'll add a smoke test."}]}}
{"type":"result","subtype":"success","result":"Added smoke test.","total_cost_usd":0.0421}
not-json-plain-line that should become raw_line
{"type":"unknown_kind","payload":{"x":1}}
"#
        .to_string()
    }

    #[test]
    fn parses_claude_stream_json_happy_path() {
        let stdout = r#"
{"type":"system","subtype":"init"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Looking at the repo"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"path":"src/main.rs"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"1","content":"fn main() {}"}]}}
{"type":"result","subtype":"success","total_cost_usd":0.0123,"result":"done"}
"#;
        let events = parse_worker_logs(stdout, "", 50);
        assert!(events.iter().any(|e| e.kind == "message"));
        assert!(events.iter().any(|e| e.kind == "tool_use" && e.title == "Read"));
        assert!(events.iter().any(|e| e.kind == "tool_result"));
        assert!(events.iter().any(|e| e.kind == "result" && e.level == "success"));
    }

    #[test]
    fn parses_fixture_stream_json_file() {
        let stdout = fixture_stream_json();
        let events = parse_worker_logs(&stdout, "stderr boom\n", 100);
        // AI interaction kinds present
        assert!(
            events.iter().any(|e| e.kind == "message"),
            "expected assistant message; got {:?}",
            events.iter().map(|e| &e.kind).collect::<Vec<_>>()
        );
        assert!(events.iter().any(|e| e.kind == "tool_use" && e.title == "Read"));
        assert!(events.iter().any(|e| e.kind == "tool_use" && e.title == "Bash"));
        assert!(events.iter().any(|e| e.kind == "tool_result"));
        assert!(events.iter().any(|e| e.kind == "result" && e.level == "success"));
        // plain + unknown → raw_line (never drop lines)
        assert!(
            events.iter().any(|e| e.kind == "raw_line"),
            "plain/unknown lines should become raw_line"
        );
        // stderr collapsed to 1
        assert_eq!(events.iter().filter(|e| e.kind == "stderr").count(), 1);
        // tool_use summary should mention path / command
        let read = events
            .iter()
            .find(|e| e.kind == "tool_use" && e.title == "Read")
            .expect("Read tool_use");
        assert!(
            read.summary.contains("main.rs") || read.detail.as_deref().unwrap_or("").contains("main.rs"),
            "Read summary/detail should mention path: {:?}",
            read
        );
    }

    #[test]
    fn read_text_tail_line_aligned() {
        let dir = std::env::temp_dir().join(format!(
            "cco-log-tail-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("stdout.json");
        // Build multi-line file with known first complete line after cut.
        let mut body = String::new();
        body.push_str("AAAA_partial_should_drop\n");
        for i in 0..40 {
            body.push_str(&format!("{{\"type\":\"system\",\"subtype\":\"n{i}\"}}\n"));
        }
        std::fs::write(&path, &body).unwrap();
        // max_bytes cuts into first line → must drop partial, keep complete NDJSON
        let cut = body.len().saturating_sub(body.len() / 3);
        let (text, total) = read_text_tail(&path, cut);
        assert_eq!(total, body.len() as u64);
        assert!(text.contains("(truncated"));
        // No half-open JSON at first content line after marker
        let after = text
            .lines()
            .skip_while(|l| l.starts_with('…') || l.starts_with("..."))
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        if !after.is_empty() {
            assert!(
                after.starts_with('{') || after.starts_with("…"),
                "first content line should be complete: {after:?}"
            );
            if after.starts_with('{') {
                assert!(serde_json::from_str::<serde_json::Value>(after).is_ok());
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn floor_char_boundary_and_compact_text_tail_cjk_safe() {
        let s = "中文✅路径";
        // Every byte offset must floor without panic and stay a boundary.
        for i in 0..=s.len() {
            let b = floor_char_boundary(s, i);
            assert!(s.is_char_boundary(b), "i={i} → {b}");
            assert!(b <= i.min(s.len()));
        }
        let long = "TOOL Edit /tmp/中文/计划.md\n已更新 ✅\n".repeat(500);
        assert!(long.len() > 6_000);
        for cap in [1usize, 2, 3, 7, 13, 599, 6000, long.len() - 1] {
            let out = compact_text_tail(&long, cap, "… (compact)\n");
            assert!(out.starts_with("… (compact)\n") || out == long);
            // Ensure we never introduced replacement chars from a bad cut.
            assert!(!out.contains('\u{FFFD}'));
        }
        assert_eq!(compact_text_tail("短", 6000, "…\n"), "短");
    }

    #[test]
    fn events_to_plain_includes_kinds() {
        let events = parse_worker_logs(&fixture_stream_json(), "", 50);
        let plain = events_to_plain(&events);
        assert!(plain.contains("tool_use") || plain.contains("Read") || plain.contains("message"));
    }

    #[test]
    fn raw_lines_and_stderr() {
        let events = parse_worker_logs("hello plain\n", "boom error\n", 50);
        assert!(events.iter().any(|e| e.kind == "raw_line"));
        let se = events.iter().find(|e| e.kind == "stderr").expect("stderr collapsed");
        assert!(se.title.contains("1 行"));
        assert_eq!(se.level, "warn");
    }

    #[test]
    fn stderr_collapsed_not_per_line() {
        let mut err = String::new();
        for i in 0..20 {
            err.push_str(&format!("warn line {i}\n"));
        }
        let events = parse_worker_logs("{\"type\":\"result\",\"subtype\":\"success\"}\n", &err, 50);
        let stderr_n = events.iter().filter(|e| e.kind == "stderr").count();
        assert_eq!(stderr_n, 1);
    }

    #[test]
    fn caps_events() {
        let mut s = String::new();
        for i in 0..100 {
            s.push_str(&format!("{{\"type\":\"system\",\"subtype\":\"n{i}\"}}\n"));
        }
        let events = parse_worker_logs(&s, "", 10);
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn never_drops_unrecognized_json_or_text() {
        let stdout = "{\"type\":\"totally_new\",\"x\":1}\nplain text line\n";
        let events = parse_worker_logs(stdout, "", 20);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.kind == "raw_line" || e.kind == "meta"));
    }
}
