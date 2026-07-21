//! Anthropic Messages **tool loop** backend for [`super::SdkProvider`] (P2-7 S2).
//!
//! [INPUT]: TaskIR prompt · work_dir scope · env/config API key · model
//! [OUTPUT]: NDJSON stdout (stream-json shape) + exit code; optional writes under work_dir
//! [POS]: runtime/provider — inject via `SdkBackend`; scheduler never sees HTTP/tools
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: S2 = multi-turn messages + tools (cwd-scoped); S1 one-shot stays in sdk_http

mod ndjson;
mod tools;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::sdk_http::{
    resolve_api_key, resolve_base_url, resolve_model, MessagesHttpClient, ReqwestMessagesClient,
};
use super::{SdkBackend, StartCtx};
use crate::plan::TaskIR;

use ndjson::{
    push_line, truncate_chars, truncate_for_error, write_error_ndjson, write_full_result,
};
use tools::{
    extract_text_blocks, extract_tool_uses, run_tool, tool_defs, MAX_TOOL_RESULT_CHARS,
};

pub use tools::resolve_under;

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;
const DEFAULT_TIMEOUT_SECS: u64 = 180;
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 8;

/// Multi-turn Messages API with cwd-scoped tools (read / list / write).
pub struct AnthropicToolLoopBackend<C: MessagesHttpClient = ReqwestMessagesClient> {
    client: C,
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: u32,
    max_tool_rounds: u32,
}

impl AnthropicToolLoopBackend<ReqwestMessagesClient> {
    pub fn from_env(extra_args: &[String]) -> Result<Self> {
        let api_key = resolve_api_key().unwrap_or_default();
        let model = resolve_model(extra_args);
        let base_url = resolve_base_url();
        let max_tokens = resolve_max_tokens();
        let max_tool_rounds = resolve_max_tool_rounds();
        let timeout = std::time::Duration::from_secs(
            std::env::var("CCO_SDK_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECS),
        );
        let client = ReqwestMessagesClient::new(timeout)?;
        Ok(Self {
            client,
            api_key,
            model,
            base_url,
            max_tokens,
            max_tool_rounds,
        })
    }
}

impl<C: MessagesHttpClient> AnthropicToolLoopBackend<C> {
    pub fn new(
        client: C,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        max_tokens: u32,
        max_tool_rounds: u32,
    ) -> Self {
        Self {
            client,
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            max_tokens,
            max_tool_rounds: max_tool_rounds.max(1),
        }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn headers(&self) -> [(&'static str, String); 3] {
        [
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", ANTHROPIC_VERSION.to_string()),
            ("content-type", "application/json".to_string()),
        ]
    }

    fn build_request(&self, messages: &[Value]) -> Value {
        json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "tools": tool_defs(),
            "messages": messages,
        })
    }

    async fn post_messages(&self, messages: &[Value]) -> Result<(u16, String)> {
        let url = self.messages_url();
        let body = self.build_request(messages);
        let headers = self.headers();
        self.client
            .post_json(&url, &headers, body)
            .await
            .map_err(|e| anyhow!("sdk tool-loop HTTP: {e}"))
    }
}

#[async_trait]
impl<C: MessagesHttpClient> SdkBackend for AnthropicToolLoopBackend<C> {
    fn kind(&self) -> &str {
        "tools"
    }

    async fn preflight(&self) -> Result<()> {
        if self.api_key.trim().is_empty() {
            bail!(
                "sdk tools backend: missing API key (set CCO_SDK_API_KEY or ANTHROPIC_API_KEY)"
            );
        }
        if self.model.trim().is_empty() {
            bail!("sdk tools backend: empty model");
        }
        Ok(())
    }

    async fn execute(
        &self,
        task: &TaskIR,
        ctx: &StartCtx,
        stdout_path: &PathBuf,
    ) -> Result<i32> {
        if self.api_key.trim().is_empty() {
            write_error_ndjson(
                stdout_path,
                "missing API key (CCO_SDK_API_KEY or ANTHROPIC_API_KEY)",
            )?;
            return Ok(1);
        }

        let mut ndjson = String::new();
        push_line(
            &mut ndjson,
            &json!({
                "type": "system",
                "subtype": "init",
                "provider": "sdk",
                "backend": "tools",
                "model": self.model,
                "max_tool_rounds": self.max_tool_rounds,
                "work_dir": ctx.work_dir.display().to_string(),
            }),
        )?;

        let mut messages = vec![json!({
            "role": "user",
            "content": task.prompt,
        })];

        let mut final_text = String::new();
        let mut session_id = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut rounds: u32 = 0;

        loop {
            if rounds >= self.max_tool_rounds {
                write_full_result(
                    stdout_path,
                    &ndjson,
                    false,
                    &format!("tool loop hit max rounds ({})", self.max_tool_rounds),
                    &session_id,
                    input_tokens,
                    output_tokens,
                )?;
                return Ok(1);
            }
            rounds += 1;

            let (status, text) = match self.post_messages(&messages).await {
                Ok(v) => v,
                Err(e) => {
                    write_error_ndjson(stdout_path, &format!("http error: {e}"))?;
                    return Ok(1);
                }
            };

            if !(200..300).contains(&status) {
                write_error_ndjson(
                    stdout_path,
                    &format!("HTTP {status}: {}", truncate_for_error(&text, 500)),
                )?;
                return Ok(1);
            }

            let parsed: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    write_error_ndjson(stdout_path, &format!("invalid JSON response: {e}"))?;
                    return Ok(1);
                }
            };

            if let Some(id) = parsed.get("id").and_then(|v| v.as_str()) {
                if session_id.is_empty() {
                    session_id = id.to_string();
                }
            }
            if let Some(n) = parsed.pointer("/usage/input_tokens").and_then(|v| v.as_u64()) {
                input_tokens = input_tokens.saturating_add(n);
            }
            if let Some(n) = parsed
                .pointer("/usage/output_tokens")
                .and_then(|v| v.as_u64())
            {
                output_tokens = output_tokens.saturating_add(n);
            }

            let content = parsed
                .get("content")
                .cloned()
                .unwrap_or_else(|| json!([]));
            let stop = parsed
                .get("stop_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("end_turn");

            if let Some(t) = extract_text_blocks(&content) {
                if !t.is_empty() {
                    final_text = t.clone();
                    push_line(
                        &mut ndjson,
                        &json!({
                            "type": "assistant",
                            "message": { "content": [{ "type": "text", "text": t }] }
                        }),
                    )?;
                }
            }

            let tool_uses = extract_tool_uses(&content);
            if tool_uses.is_empty() || stop != "tool_use" {
                write_full_result(
                    stdout_path,
                    &ndjson,
                    true,
                    if final_text.is_empty() {
                        "ok"
                    } else {
                        &final_text
                    },
                    &session_id,
                    input_tokens,
                    output_tokens,
                )?;
                return Ok(0);
            }

            messages.push(json!({
                "role": "assistant",
                "content": content,
            }));

            let mut tool_results = Vec::new();
            for tu in &tool_uses {
                let name = tu.name.as_str();
                let result = run_tool(name, &tu.input, &ctx.work_dir);
                let (ok, body) = match result {
                    Ok(s) => (true, s),
                    Err(e) => (false, format!("error: {e}")),
                };
                push_line(
                    &mut ndjson,
                    &json!({
                        "type": "tool",
                        "subtype": "result",
                        "name": name,
                        "tool_use_id": tu.id,
                        "ok": ok,
                        "preview": truncate_for_error(&body, 200),
                    }),
                )?;
                tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": tu.id,
                    "content": truncate_chars(&body, MAX_TOOL_RESULT_CHARS),
                    "is_error": !ok,
                }));
            }
            messages.push(json!({
                "role": "user",
                "content": tool_results,
            }));
        }
    }
}

/// Build production tool-loop backend from provider extra_args.
pub fn tool_loop_backend_from_config(extra_args: &[String]) -> Result<Arc<dyn SdkBackend>> {
    let backend = AnthropicToolLoopBackend::from_env(extra_args)?;
    Ok(Arc::new(backend))
}

/// Whether config `bin` selects the S2 tool-loop backend.
pub fn is_tools_bin(bin: &str) -> bool {
    matches!(
        bin.trim().to_ascii_lowercase().as_str(),
        "tools" | "tool_loop" | "tool-loop" | "agent"
    )
}

fn resolve_max_tokens() -> u32 {
    std::env::var("CCO_SDK_MAX_TOKENS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_TOKENS)
}

fn resolve_max_tool_rounds() -> u32 {
    std::env::var("CCO_SDK_MAX_TOOL_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_TOOL_ROUNDS)
}
