//! Per-result-kind decoding → [`WorkerOutcome`]（平台输出契约层 T1/T2）.
//!
//! 收编 log_events / stream_parse 的散落硬编码：每个 decoder 统一回答三件事——
//! 「怎么判成功」「有没有执行动作证据」「人话错误」。shell_print adapter 按
//! [`ShellProfile::result_kind`] 单点 dispatch（不做 7 个 trait 实现）。
//!
//! [INPUT]: stdout NDJSON / one-shot JSON / plain text · meta · exit
//! [OUTPUT]: WorkerOutcome（done_marker · execution_evidence · error_hint …）
//! [POS]: runtime/provider/shell_print
//! [PROTOCOL]: 新 CLI = 挂 profile.result_kind + 这里加一个 decoder，禁止再往
//!             log_events/stream_parse 散落 if-branch。

use serde_json::Value;

use crate::ports::worker::{TaskStatus, WorkerOutcome};
use super::profiles::ResultKind;
use crate::runtime::provider::task_status_from_exit;

/// Entry point: dispatch by result kind (adapter calls this once per collect).
pub fn decode(kind: ResultKind, stdout: &str, meta: &Value, exit: Option<i32>) -> WorkerOutcome {
    match kind {
        ResultKind::CodexItemStream => decode_codex(stdout, meta, exit),
        ResultKind::CodewhaleExecStream => decode_codewhale(stdout, meta, exit),
        ResultKind::OneShotJson => decode_one_shot(stdout, meta, exit),
        ResultKind::Plain => decode_plain(stdout, meta, exit),
    }
}

/// codex `--json`: `item.started|item.completed` NDJSON.
///
/// 执行证据 = 至少发起过一次 `command_execution`。只有纯 `agent_message`
/// （无任何命令/工具）→ 无执行证据——这就是「找不到依赖就打印一句退出」的指纹。
///
/// 平台错误 = `turn.failed` / `error` 事件中包含 API 级错误指纹（404/429/auth/
/// reconnect/rate limit）——这不是任务逻辑失败，是中转/鉴权/endpoint 坏了。
fn decode_codex(stdout: &str, _meta: &Value, exit: Option<i32>) -> WorkerOutcome {
    let mut any_command = false;
    let mut any_item = false;
    let mut agent_messages = 0usize;
    let mut platform_err_msg: Option<String> = None;
    for line in stdout.lines() {
        let l = line.trim();
        if !l.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        // Detect platform-level errors from turn.failed / error events.
        if (ty == "turn.failed" || ty == "error") && platform_err_msg.is_none() {
            let msg = v
                .get("error")
                .and_then(|e| e.get("message").or(Some(e)))
                .and_then(|m| m.as_str())
                .or_else(|| v.get("message").and_then(|m| m.as_str()))
                .unwrap_or("");
            if is_platform_error_fingerprint(msg) {
                platform_err_msg = Some(extract_platform_error_hint(msg));
            }
            continue;
        }

        if ty != "item.started" && ty != "item.completed" {
            continue;
        }
        let Some(item) = v.get("item") else { continue; };
        any_item = true;
        match item.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "command_execution" => any_command = true,
            "agent_message" => agent_messages += 1,
            _ => {}
        }
    }
    let status = task_status_from_exit(exit);
    let execution_evidence = any_command;
    let done_marker = any_item || exit == Some(0);
    let empty_stdout = stdout.trim().is_empty();

    // Platform error takes precedence over spin hint.
    let (error_hint, platform_error) = if let Some(ref pe) = platform_err_msg {
        (Some(pe.clone()), true)
    } else {
        let hint = (!empty_stdout && agent_messages > 0 && !any_command)
            .then(|| "平台空转：codex 仅输出文本（agent_message），无任何命令/工具执行".to_string());
        (hint, false)
    };
    WorkerOutcome {
        status,
        done_marker,
        execution_evidence,
        empty_stdout,
        error_hint,
        platform_error,
        session_id: None,
        cost_usd: None,
    }
}

/// CodeWhale exec-stream NDJSON（`schema=codewhale.exec-stream`）。
fn decode_codewhale(stdout: &str, _meta: &Value, exit: Option<i32>) -> WorkerOutcome {
    let mut content = false;
    let mut tool = false;
    let mut done = false;
    for line in stdout.lines() {
        let l = line.trim();
        if !l.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v.get("schema").and_then(|t| t.as_str()) != Some("codewhale.exec-stream") {
            continue;
        }
        match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "content" => {
                let nonempty = v
                    .get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if nonempty {
                    content = true;
                }
            }
            "tool" | "tool_call" | "tool_use" | "command" => tool = true,
            "done" => done = true,
            _ => {}
        }
    }
    let status = task_status_from_exit(exit);
    let execution_evidence = content || tool;
    let done_marker = done || exit == Some(0);
    let empty_stdout = stdout.trim().is_empty();
    let error_hint = (done && !content && !tool)
        .then(|| "平台空转：CodeWhale 仅 metadata/done，无任何内容或工具执行".to_string());
    WorkerOutcome {
        status,
        done_marker,
        execution_evidence,
        empty_stdout,
        error_hint,
        platform_error: false,
        session_id: None,
        cost_usd: None,
    }
}

/// One-shot JSON 结果对象（gemini / qwen / codebuddy `-o json`）。
fn decode_one_shot(stdout: &str, _meta: &Value, exit: Option<i32>) -> WorkerOutcome {
    let trimmed = stdout.trim();
    let parsed = serde_json::from_str::<Value>(trimmed)
        .ok()
        .or_else(|| last_json_value(trimmed));
    let status = task_status_from_exit(exit);
    let has_result = parsed.as_ref().is_some_and(|v| {
        ["result", "text", "content", "output", "response", "answer", "message"]
            .iter()
            .any(|k| {
                v.get(*k).is_some_and(|x| {
                    !x.is_null() && !(x.is_string() && x.as_str().unwrap_or("").trim().is_empty())
                })
            })
    });
    let done_marker = parsed.is_some() || exit == Some(0);
    let empty_stdout = trimmed.is_empty();
    let execution_evidence = has_result;
    let error_hint = if parsed.is_some() && !has_result {
        Some("平台空转：one-shot 结果对象无任何结果字段".to_string())
    } else if empty_stdout && status == TaskStatus::Done {
        Some("平台空转：one-shot stdout 为空".to_string())
    } else {
        None
    };
    WorkerOutcome {
        status,
        done_marker,
        execution_evidence,
        empty_stdout,
        error_hint,
        platform_error: false,
        session_id: None,
        cost_usd: None,
    }
}

/// Plain 文本（kimi / copilot）：任何非空输出即弱证据。
fn decode_plain(stdout: &str, _meta: &Value, exit: Option<i32>) -> WorkerOutcome {
    let trimmed = stdout.trim();
    let status = task_status_from_exit(exit);
    let nonempty = !trimmed.is_empty();
    let execution_evidence = nonempty;
    let done_marker = nonempty || exit == Some(0);
    let empty_stdout = trimmed.is_empty();
    let error_hint = (empty_stdout && status == TaskStatus::Done)
        .then(|| "平台空转：plain stdout 为空".to_string());
    WorkerOutcome {
        status,
        done_marker,
        execution_evidence,
        empty_stdout,
        error_hint,
        platform_error: false,
        session_id: None,
        cost_usd: None,
    }
}

/// Fingerprints that distinguish platform/API errors from task-logic failures.
/// These indicate the provider's endpoint/auth is broken (not the task prompt).
fn is_platform_error_fingerprint(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("429") || m.contains("too many requests")
        || m.contains("rate limit")
        || m.contains("404 not found")
        || m.contains("不支持该 api")
        || m.contains("exceeded retry limit")
        || m.contains("reconnecting")
        || m.contains("unauthorized") || m.contains("401")
        || m.contains("authentication") || m.contains("auth")
        || m.contains("api key")
        || m.contains("forbidden") || m.contains("403")
        || m.contains("connection refused")
        || m.contains("timeout") && m.contains("connect")
}

/// Extract a short human-readable hint from a platform error message.
fn extract_platform_error_hint(msg: &str) -> String {
    let truncated: String = msg.chars().take(200).collect();
    format!("平台/API 错误（非任务逻辑）：{truncated}")
}

/// Last JSON object in a buffer (one-shot JSON wrapped by incidental lines).
fn last_json_value(raw: &str) -> Option<Value> {
    for line in raw.lines().rev().take(200) {
        let l = line.trim();
        if l.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                return Some(v);
            }
        }
    }
    None
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_empty_spin_no_execution_evidence() {
        // 金样：找不到依赖就打印一句退出（纯 agent_message，exit 0，无命令）→ 无执行证据。
        let raw = r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"找不到依赖。"}}
{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"已退出。"}}
"#;
        let o = decode(ResultKind::CodexItemStream, raw, &Value::Null, Some(0));
        assert_eq!(o.status, TaskStatus::Done);
        assert!(o.done_marker);
        assert!(!o.execution_evidence, "纯 agent_message 不应有执行证据");
        assert!(o.error_hint.is_some(), "应给出空转人话提示");
    }

    #[test]
    fn codex_command_execution_has_evidence() {
        let raw = r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"我看看项目。"}}
{"type":"item.completed","item":{"id":"i2","type":"command_execution","command":"/bin/zsh -lc pwd","exit_code":0,"status":"completed"}}
"#;
        let o = decode(ResultKind::CodexItemStream, raw, &Value::Null, Some(0));
        assert!(o.execution_evidence, "command_execution 应算执行证据");
        assert!(o.error_hint.is_none());
    }

    #[test]
    fn codewhale_empty_metadata_no_evidence() {
        let raw = r#"{"type":"metadata","meta":{"status":"completed"},"schema_version":1,"schema":"codewhale.exec-stream"}
{"type":"done","schema_version":1,"schema":"codewhale.exec-stream"}
"#;
        let o = decode(ResultKind::CodewhaleExecStream, raw, &Value::Null, Some(0));
        assert!(!o.execution_evidence, "仅 metadata/done 不应有执行证据");
        assert!(o.error_hint.is_some());
    }

    #[test]
    fn codewhale_content_has_evidence() {
        let raw = r#"{"type":"content","content":"搞定！","schema_version":1,"schema":"codewhale.exec-stream"}
{"type":"done","schema_version":1,"schema":"codewhale.exec-stream"}
"#;
        let o = decode(ResultKind::CodewhaleExecStream, raw, &Value::Null, Some(0));
        assert!(o.execution_evidence);
    }

    #[test]
    fn one_shot_empty_object_no_evidence() {
        let raw = "{}";
        let o = decode(ResultKind::OneShotJson, raw, &Value::Null, Some(0));
        assert!(o.done_marker);
        assert!(!o.execution_evidence, "空对象 + exit 0 不应有证据");
        assert!(o.error_hint.is_some());
    }

    #[test]
    fn one_shot_with_result_has_weak_evidence() {
        let raw = r#"{"result":"已完成重构"}"#;
        let o = decode(ResultKind::OneShotJson, raw, &Value::Null, Some(0));
        assert!(o.execution_evidence);
    }

    #[test]
    fn plain_empty_is_noop() {
        let o = decode(ResultKind::Plain, "  ", &Value::Null, Some(0));
        assert!(!o.execution_evidence);
        assert!(o.error_hint.is_some());
    }

    #[test]
    fn codex_platform_error_429_detected() {
        // 金样：codex 中转 429/404 → platform_error=true，error_hint 上浮真实错误。
        let raw = r#"{"type":"thread.started","thread_id":"01a00ec3"}
{"type":"turn.started"}
{"type":"error","message":"exceeded retry limit, last status: 429 Too Many Requests, request id: a2c72ffcdfbffa2f-IAD"}
{"type":"turn.failed","error":{"message":"exceeded retry limit, last status: 429 Too Many Requests, request id: a2c72ffcdfbffa2f-IAD"}}
"#;
        let o = decode(ResultKind::CodexItemStream, raw, &Value::Null, Some(1));
        assert!(o.platform_error, "429 应识别为平台错误");
        assert!(o.error_hint.is_some(), "应上浮真实错误提示");
        assert!(o.error_hint.as_deref().unwrap_or("").contains("429"));
    }

    #[test]
    fn codex_platform_error_404_detected() {
        let raw = r#"{"type":"turn.failed","error":{"message":"unexpected status 404 Not Found: 当前平台不支持该 API 路径, url: https://ergouapi.co/v1/responses"}}
"#;
        let o = decode(ResultKind::CodexItemStream, raw, &Value::Null, Some(1));
        assert!(o.platform_error, "404 中转坏应识别为平台错误");
        assert!(o.error_hint.as_deref().unwrap_or("").contains("404"));
    }

    #[test]
    fn codex_command_execution_not_platform_error() {
        let raw = r#"{"type":"item.completed","item":{"id":"i2","type":"command_execution","command":"/bin/zsh -lc pwd","exit_code":0,"status":"completed"}}
"#;
        let o = decode(ResultKind::CodexItemStream, raw, &Value::Null, Some(0));
        assert!(!o.platform_error, "正常命令执行不应是平台错误");
    }
}

