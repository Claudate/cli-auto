//! NDJSON stdout helpers for S2 tool loop.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub fn push_line(buf: &mut String, v: &Value) -> Result<()> {
    buf.push_str(&serde_json::to_string(v)?);
    buf.push('\n');
    Ok(())
}

pub fn write_error_ndjson(path: &PathBuf, msg: &str) -> Result<()> {
    let line = json!({
        "type": "result",
        "subtype": "error",
        "result": msg,
    });
    let body = format!("{}\n", serde_json::to_string(&line)?);
    std::fs::write(path, body).map_err(|e| anyhow!("write sdk error stdout: {e}"))
}

pub fn write_full_result(
    path: &PathBuf,
    ndjson_prefix: &str,
    success: bool,
    result_text: &str,
    session_id: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<()> {
    let mut body = ndjson_prefix.to_string();
    let line = if success {
        json!({
            "type": "result",
            "subtype": "success",
            "result": result_text,
            "session_id": session_id,
            "total_cost_usd": 0.0,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            },
        })
    } else {
        json!({
            "type": "result",
            "subtype": "error",
            "result": result_text,
            "session_id": session_id,
        })
    };
    body.push_str(&serde_json::to_string(&line)?);
    body.push('\n');
    if success {
        body.push_str("CCO_DONE ok\n");
    }
    std::fs::write(path, body).map_err(|e| anyhow!("write sdk tool-loop stdout: {e}"))
}

pub fn truncate_for_error(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    t.chars().take(max).collect::<String>() + "…"
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}
