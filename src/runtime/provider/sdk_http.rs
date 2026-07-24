//! Anthropic Messages HTTP backend for [`super::SdkProvider`] (P2-7 S1).
//!
//! [INPUT]: TaskIR prompt · env/config API key · model · base_url
//! [OUTPUT]: NDJSON stdout (same shape as InlineSdkBackend) + exit code
//! [POS]: runtime/provider — inject via `SdkBackend`; scheduler never sees HTTP
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/provider/CLAUDE.md
//! note: S1 = one-shot messages.create only；S2 tool loop 另立，不进本文件

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{SdkBackend, StartCtx};
use crate::plan::TaskIR;

/// Default Anthropic Messages API root (no trailing slash).
pub const DEFAULT_MESSAGES_BASE_URL: &str = "https://api.anthropic.com";
/// Design-doc default model for S1 (override with `CCO_SDK_MODEL` or extra_args[0]).
pub const DEFAULT_SDK_MODEL: &str = "claude-sonnet-4-5";
const DEFAULT_MAX_TOKENS: u32 = 4096;
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Minimal HTTP client surface so unit tests inject a mock without a live socket.
#[async_trait]
pub trait MessagesHttpClient: Send + Sync {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: Value,
    ) -> Result<(u16, String)>;
}

/// Production client (reqwest).
pub struct ReqwestMessagesClient {
    client: reqwest::Client,
}

impl ReqwestMessagesClient {
    pub fn new(timeout: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .context("build reqwest client for sdk messages")?;
        Ok(Self { client })
    }
}

#[async_trait]
impl MessagesHttpClient for ReqwestMessagesClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: Value,
    ) -> Result<(u16, String)> {
        let mut req = self.client.post(url).json(&body);
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let resp = req.send().await.context("sdk messages HTTP request")?;
        let status = resp.status().as_u16();
        let text = resp
            .text()
            .await
            .context("sdk messages HTTP read body")?;
        Ok((status, text))
    }
}

/// One-shot Anthropic `/v1/messages` backend (no tools, no agent loop).
pub struct AnthropicMessagesBackend<C: MessagesHttpClient = ReqwestMessagesClient> {
    client: C,
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: u32,
}

impl AnthropicMessagesBackend<ReqwestMessagesClient> {
    /// Resolve key/model/url from env + optional config hints (`extra_args[0]` = model).
    pub fn from_env(extra_args: &[String]) -> Result<Self> {
        let api_key = resolve_api_key().unwrap_or_default();
        let model = resolve_model(extra_args);
        let base_url = resolve_base_url();
        let max_tokens = resolve_max_tokens();
        let timeout = Duration::from_secs(
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
        })
    }
}

impl<C: MessagesHttpClient> AnthropicMessagesBackend<C> {
    pub fn new(
        client: C,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        max_tokens: u32,
    ) -> Self {
        Self {
            client,
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            max_tokens,
        }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }

    fn build_request_body(&self, task: &TaskIR) -> Value {
        json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": [{
                "role": "user",
                "content": task.prompt,
            }],
        })
    }
}

#[async_trait]
impl<C: MessagesHttpClient> SdkBackend for AnthropicMessagesBackend<C> {
    fn kind(&self) -> &str {
        "messages"
    }

    async fn preflight(&self) -> Result<()> {
        if self.api_key.trim().is_empty() {
            bail!(
                "sdk messages backend: missing API key (set CCO_SDK_API_KEY or ANTHROPIC_API_KEY)"
            );
        }
        if self.model.trim().is_empty() {
            bail!("sdk messages backend: empty model");
        }
        Ok(())
    }

    async fn execute(
        &self,
        task: &TaskIR,
        _ctx: &StartCtx,
        stdout_path: &PathBuf,
    ) -> Result<i32> {
        if self.api_key.trim().is_empty() {
            write_error_ndjson(
                stdout_path,
                "missing API key (CCO_SDK_API_KEY or ANTHROPIC_API_KEY)",
            )?;
            return Ok(1);
        }

        let url = self.messages_url();
        let body = self.build_request_body(task);
        let headers = [
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", ANTHROPIC_VERSION.to_string()),
            ("content-type", "application/json".to_string()),
        ];

        let (status, text) = match self.client.post_json(&url, &headers, body).await {
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

        let text_out = extract_text_content(&parsed).unwrap_or_else(|| text.clone());
        let session_id = parsed
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let input_tokens = parsed
            .pointer("/usage/input_tokens")
            .and_then(|v| v.as_u64());
        let output_tokens = parsed
            .pointer("/usage/output_tokens")
            .and_then(|v| v.as_u64());

        let lines = vec![
            json!({
                "type": "system",
                "subtype": "init",
                "provider": "sdk",
                "backend": "messages",
                "model": self.model,
            }),
            json!({
                "type": "assistant",
                "message": {
                    "content": [{ "type": "text", "text": text_out }]
                }
            }),
            json!({
                "type": "result",
                "subtype": "success",
                "result": text_out,
                "session_id": session_id,
                "total_cost_usd": 0.0,
                "usage": {
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                },
            }),
        ];
        let mut body_out = String::new();
        for v in lines {
            body_out.push_str(&serde_json::to_string(&v)?);
            body_out.push('\n');
        }
        body_out.push_str("CCO_DONE ok\n");
        std::fs::write(stdout_path, body_out)?;
        Ok(0)
    }
}

/// Build the production messages backend (reqwest) from provider extra_args.
pub fn messages_backend_from_config(extra_args: &[String]) -> Result<Arc<dyn SdkBackend>> {
    let backend = AnthropicMessagesBackend::from_env(extra_args)?;
    Ok(Arc::new(backend))
}

pub fn resolve_api_key() -> Option<String> {
    for key in ["CCO_SDK_API_KEY", "ANTHROPIC_API_KEY"] {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

pub fn resolve_model(extra_args: &[String]) -> String {
    if let Ok(m) = std::env::var("CCO_SDK_MODEL") {
        let t = m.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Some(m) = extra_args.first() {
        let t = m.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    DEFAULT_SDK_MODEL.to_string()
}

pub fn resolve_base_url() -> String {
    std::env::var("CCO_SDK_BASE_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_MESSAGES_BASE_URL.to_string())
}

fn resolve_max_tokens() -> u32 {
    std::env::var("CCO_SDK_MAX_TOKENS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_MAX_TOKENS)
}

/// Whether config `bin` selects the messages HTTP backend (vs S0 inline).
pub fn is_messages_bin(bin: &str) -> bool {
    matches!(
        bin.trim().to_ascii_lowercase().as_str(),
        "messages" | "http" | "anthropic" | "api"
    )
}

fn extract_text_content(parsed: &Value) -> Option<String> {
    let content = parsed.get("content")?.as_array()?;
    let mut parts = Vec::new();
    for block in content {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn write_error_ndjson(path: &PathBuf, msg: &str) -> Result<()> {
    let line = json!({
        "type": "result",
        "subtype": "error",
        "result": msg,
    });
    let body = format!("{}\n", serde_json::to_string(&line)?);
    std::fs::write(path, body).map_err(|e| anyhow!("write sdk error stdout: {e}"))
}

fn truncate_for_error(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    t.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::TaskIR;
    use crate::runtime::provider::{SdkProvider, TaskStatus, WorkerPort, WorkerStatus};
    use std::sync::Mutex;
    use tempfile::tempdir;

    struct MockClient {
        /// (status, body) to return; or error message if status==0 and body starts with "ERR:"
        response: Mutex<(u16, String)>,
        last_url: Mutex<Option<String>>,
        last_body: Mutex<Option<Value>>,
    }

    impl MockClient {
        fn ok_text(text: &str, id: &str) -> Self {
            let body = json!({
                "id": id,
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "text", "text": text }],
                "model": "claude-sonnet-4-5",
                "usage": { "input_tokens": 10, "output_tokens": 20 },
            });
            Self {
                response: Mutex::new((200, body.to_string())),
                last_url: Mutex::new(None),
                last_body: Mutex::new(None),
            }
        }

        fn http_error(status: u16, body: &str) -> Self {
            Self {
                response: Mutex::new((status, body.to_string())),
                last_url: Mutex::new(None),
                last_body: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl MessagesHttpClient for MockClient {
        async fn post_json(
            &self,
            url: &str,
            _headers: &[(&str, String)],
            body: Value,
        ) -> Result<(u16, String)> {
            *self.last_url.lock().unwrap() = Some(url.to_string());
            *self.last_body.lock().unwrap() = Some(body);
            let (status, text) = self.response.lock().unwrap().clone();
            Ok((status, text))
        }
    }

    fn sample_task(id: &str, prompt: &str) -> TaskIR {
        TaskIR {
            id: id.into(),
            title: id.into(),
            depends_on: vec![],
            group: None,
            provider: "sdk".into(),
            mode: "print".into(),
            prompt: prompt.into(),
            verify_cmd: None,
            acceptance: None,
            timeout_secs: None,
            worktree: None,
            provider_opts: serde_json::json!({}),
            optional: false,
            include: true,
            role: None,
            scope: None,
            outputs: vec![],
            tags: vec![],
        }
    }

    fn ctx(dir: &std::path::Path) -> StartCtx {
        let task_dir = dir.join("tasks").join("t1");
        std::fs::create_dir_all(&task_dir).unwrap();
        StartCtx {
            run_id: "run-sdk-s1".into(),
            project_root: dir.to_path_buf(),
            work_dir: dir.to_path_buf(),
            task_dir,
            env_extra: vec![],
        }
    }

    /// Shared Arc so the test can assert on request URL/body after the call.
    struct SharedMock(Arc<MockClient>);

    #[async_trait]
    impl MessagesHttpClient for SharedMock {
        async fn post_json(
            &self,
            url: &str,
            headers: &[(&str, String)],
            body: Value,
        ) -> Result<(u16, String)> {
            self.0.post_json(url, headers, body).await
        }
    }

    #[tokio::test]
    async fn messages_backend_start_poll_collect_via_mock_http() {
        let shared = Arc::new(MockClient::ok_text("hello from messages", "msg_s1_test"));
        let backend = AnthropicMessagesBackend::new(
            SharedMock(Arc::clone(&shared)),
            "test-key",
            "claude-test-model",
            "https://api.example.test",
            1024,
        );
        backend.preflight().await.unwrap();

        let dir = tempdir().unwrap();
        let provider = SdkProvider::with_backend(Arc::new(backend));
        let task = sample_task("t1", "say hi");
        let start_ctx = ctx(dir.path());

        let handle = provider.start(&task, &start_ctx).await.unwrap();
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Done
        ));
        let result = provider.collect(&handle).await.unwrap();
        assert_eq!(result.status, TaskStatus::Done);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.session_id.as_deref(), Some("msg_s1_test"));
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
        assert!(stdout.contains("CCO_DONE"), "stdout: {stdout}");
        assert!(stdout.contains("hello from messages"));
        assert!(stdout.contains("\"backend\":\"messages\""));

        let url = shared.last_url.lock().unwrap().clone().unwrap();
        assert_eq!(url, "https://api.example.test/v1/messages");
        let req = shared.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(req["model"], "claude-test-model");
        assert_eq!(req["messages"][0]["content"], "say hi");
        assert_eq!(req["max_tokens"], 1024);

        let meta = std::fs::read_to_string(&handle.meta_path).unwrap();
        assert!(
            meta.contains("messages"),
            "meta should name messages backend: {meta}"
        );
        assert!(
            meta.contains("\"inline_sdk\": false") || meta.contains("\"inline_sdk\":false"),
            "meta: {meta}"
        );
    }

    #[tokio::test]
    async fn messages_preflight_requires_api_key() {
        let backend = AnthropicMessagesBackend::new(
            MockClient::ok_text("x", "id"),
            "",
            "m",
            "https://api.example.test",
            64,
        );
        let err = backend.preflight().await.unwrap_err().to_string();
        assert!(err.contains("API key"), "err: {err}");
    }

    #[tokio::test]
    async fn messages_http_error_becomes_failed() {
        let backend = AnthropicMessagesBackend::new(
            MockClient::http_error(401, r#"{"error":{"message":"bad key"}}"#),
            "bad-key",
            "m",
            "https://api.example.test",
            64,
        );
        let dir = tempdir().unwrap();
        let provider = SdkProvider::with_backend(Arc::new(backend));
        let handle = provider
            .start(&sample_task("e1", "x"), &ctx(dir.path()))
            .await
            .unwrap();
        assert!(matches!(
            provider.poll(&handle).await.unwrap(),
            WorkerStatus::Failed
        ));
        let result = provider.collect(&handle).await.unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        let stdout = std::fs::read_to_string(&handle.stdout_path).unwrap();
        assert!(stdout.contains("HTTP 401"), "stdout: {stdout}");
    }

    #[test]
    fn is_messages_bin_recognizes_aliases() {
        assert!(is_messages_bin("messages"));
        assert!(is_messages_bin("HTTP"));
        assert!(is_messages_bin("anthropic"));
        assert!(is_messages_bin("api"));
        assert!(!is_messages_bin("inline"));
        assert!(!is_messages_bin("claude"));
    }

    #[test]
    fn extract_text_joins_blocks() {
        let v = json!({
            "content": [
                { "type": "text", "text": "a" },
                { "type": "tool_use", "id": "1" },
                { "type": "text", "text": "b" },
            ]
        });
        assert_eq!(extract_text_content(&v).as_deref(), Some("a\nb"));
    }
}
