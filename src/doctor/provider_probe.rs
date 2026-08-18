//! Provider key + auth probe for doctor (A: doctor enhancement).
//!
//! [INPUT]: provider id · Config
//! [OUTPUT]: ProbeResult (auth status + probe status + hint)
//! [POS]: src/doctor — reads provider config files + sends lightweight probe request
//! [PROTOCOL]: Key values never leave this function; only tail-4 is returned.
//!
//! Safety: keys are read from env/files, used in-memory for one HTTP request,
//! never logged, never stored in state, never written to disk.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;

const PROBE_TIMEOUT_SECS: u64 = 6;

/// Result of probing one provider's key + endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    /// "present" | "missing" | "unknown"
    pub auth_status: String,
    /// e.g. "env:ANTHROPIC_API_KEY" | "file:~/.codex/auth.json" — no key value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_source: Option<String>,
    /// Last 4 chars of key for display (e.g. "…b930"). None when no key or too short.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_tail: Option<String>,
    /// "ok" | "auth_invalid" | "insufficient_funds" | "rate_limited"
    /// | "endpoint_broken" | "not_supported"
    pub probe_status: String,
    /// HTTP status code from probe request, if one was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    /// Human-readable hint for the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Default for ProbeResult {
    fn default() -> Self {
        Self {
            auth_status: "unknown".into(),
            key_source: None,
            key_tail: None,
            probe_status: "not_supported".into(),
            http_status: None,
            hint: None,
        }
    }
}

impl ProbeResult {
    pub fn is_ok(&self) -> bool {
        self.probe_status == "ok"
    }

    /// One-line detail for the doctor table detail cell.
    pub fn detail_line(&self) -> String {
        let tail = self.key_tail.as_deref().unwrap_or("");
        let src = self.key_source.as_deref().unwrap_or("");
        let key_part = if tail.is_empty() {
            String::new()
        } else {
            format!("（来源 {src}，尾号 …{tail}）")
        };
        match self.probe_status.as_str() {
            "ok" => format!("Key 有效{key_part}"),
            "auth_invalid" => format!("Key 失效{key_part}"),
            "insufficient_funds" => format!("余额不足{key_part}"),
            "rate_limited" => format!("限流中{key_part}"),
            "endpoint_broken" => format!("通道接口异常{key_part}"),
            "not_supported" => "无探活支持（仅查二进制）".to_string(),
            _ => "未知".to_string(),
        }
    }
}

/// Resolve key + source label for a provider.
///
/// Returns `(key_value, source_label)`. Key value is NOT stored anywhere;
/// caller uses it for one HTTP request then drops it.
fn resolve_key(provider: &str) -> Option<(String, String)> {
    match provider {
        "claude" => std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|v| (v, "env:ANTHROPIC_API_KEY".into())),
        "sdk" => std::env::var("CCO_SDK_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|v| (v, "env:CCO_SDK_API_KEY".into()))
            .or_else(|| {
                std::env::var("ANTHROPIC_API_KEY")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .map(|v| (v, "env:ANTHROPIC_API_KEY".into()))
            }),
        "codex" => read_codex_auth().map(|k| (k, "file:~/.codex/auth.json".into())),
        "deepseek" => read_codewhale_secret()
            .map(|k| (k, "file:~/.codewhale/secrets/secrets.json".into())),
        _ => None,
    }
}

/// Read OPENAI_API_KEY from ~/.codex/auth.json.
fn read_codex_auth() -> Option<String> {
    let path = home_path(".codex/auth.json")?;
    let text = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("OPENAI_API_KEY")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Read DeepSeek key from ~/.codewhale/secrets/secrets.json → entries.deepseek.
fn read_codewhale_secret() -> Option<String> {
    let path = home_path(".codewhale/secrets/secrets.json")?;
    let text = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("entries")
        .and_then(|e| e.get("deepseek"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn home_path(rel: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(rel))
}

fn key_tail(key: &str) -> Option<String> {
    let t = key.trim();
    if t.len() < 4 {
        return None;
    }
    Some(t[t.len() - 4..].to_string())
}

/// Codex probe config from ~/.codex/config.toml.
#[derive(Debug)]
struct CodexProbeConfig {
    base_url: String,
    #[allow(dead_code)]
    wire_api: String,
}

fn read_codex_probe_config() -> Option<CodexProbeConfig> {
    let path = home_path(".codex/config.toml")?;
    let text = std::fs::read_to_string(&path).ok()?;
    let v: toml::Value = toml::from_str(&text).ok()?;
    // model_provider = "2"
    let mp = v.get("model_provider").and_then(|s| s.as_str())?;
    // [model_providers.{mp}]
    let provider = v
        .get("model_providers")
        .and_then(|mp_table| mp_table.get(mp))?;
    let base_url = provider
        .get("base_url")
        .and_then(|s| s.as_str())?
        .trim_end_matches('/')
        .to_string();
    let wire_api = provider
        .get("wire_api")
        .and_then(|s| s.as_str())
        .unwrap_or("responses")
        .to_string();
    Some(CodexProbeConfig { base_url, wire_api })
}

/// Probe one provider. Key is read, used for one HTTP request, then dropped.
pub async fn probe_provider(provider: &str, _config: &Config) -> ProbeResult {
    let provider = provider.trim();
    let (key, source) = match resolve_key(provider) {
        Some(kv) => kv,
        None => {
            return ProbeResult {
                auth_status: "missing".into(),
                probe_status: "not_supported".into(),
                hint: Some("未找到 Key（该 provider 可能不持有独立 Key）".into()),
                ..Default::default()
            };
        }
    };
    let tail = key_tail(&key);
    let source = source; // owned; used once below

    let result = match provider {
        "claude" | "sdk" => probe_anthropic(provider, &key).await,
        "codex" => {
            let cfg = read_codex_probe_config();
            match cfg {
                Some(c) => probe_openai_compat(&c.base_url, &key).await,
                None => ProbeResult {
                    auth_status: "present".into(),
                    key_source: Some(source.clone()),
                    key_tail: tail.clone(),
                    probe_status: "not_supported".into(),
                    hint: Some("未读到 ~/.codex/config.toml，无法探活".into()),
                    http_status: None,
                },
            }
        }
        "deepseek" => {
            // CodeWhale uses DeepSeek API; default base_url is api.deepseek.com.
            let base = "https://api.deepseek.com".to_string();
            probe_openai_compat(&base, &key).await
        }
        _ => ProbeResult {
            auth_status: "present".into(),
            key_source: Some(source.clone()),
            key_tail: tail.clone(),
            probe_status: "not_supported".into(),
            ..Default::default()
        },
    };

    // Attach key metadata if not already set.
    let mut result = result;
    if result.key_source.is_none() {
        result.key_source = Some(source);
    }
    if result.key_tail.is_none() {
        result.key_tail = tail;
    }
    if result.auth_status.is_empty() || result.auth_status == "unknown" {
        result.auth_status = "present".into();
    }
    result
}

/// Probe Anthropic-style endpoint (claude / sdk).
/// Sends a minimal 1-token /v1/messages request.
async fn probe_anthropic(provider: &str, key: &str) -> ProbeResult {
    let base = if provider == "sdk" {
        std::env::var("CCO_SDK_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.anthropic.com".into())
    } else {
        std::env::var("ANTHROPIC_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.anthropic.com".into())
    };
    let base = base.trim_end_matches('/').to_string();
    let url = format!("{base}/v1/messages");
    let body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "."}]
    });
    probe_post_json(&url, key, &body, AuthStyle::XApiKey).await
}

/// Probe OpenAI-compatible endpoint (codex / deepseek).
/// Sends a minimal responses request.
async fn probe_openai_compat(base_url: &str, key: &str) -> ProbeResult {
    let url = format!("{}/responses", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": "deepseek-chat",
        "input": ".",
        "max_output_tokens": 1,
    });
    probe_post_json(&url, key, &body, AuthStyle::Bearer).await
}

#[derive(Clone, Copy)]
enum AuthStyle {
    XApiKey,
    Bearer,
}

/// Send a minimal POST to probe auth/balance status.
async fn probe_post_json(url: &str, key: &str, body: &Value, auth: AuthStyle) -> ProbeResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                auth_status: "present".into(),
                probe_status: "endpoint_broken".into(),
                hint: Some(format!("HTTP 客户端构建失败: {e}")),
                ..Default::default()
            };
        }
    };

    let mut req = client.post(url).json(body);
    req = match auth {
        AuthStyle::XApiKey => req
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        AuthStyle::Bearer => req.header("Authorization", format!("Bearer {key}")),
    };

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => {
            return ProbeResult {
                auth_status: "present".into(),
                probe_status: "endpoint_broken".into(),
                hint: Some("探活超时（6s），通道无响应".into()),
                ..Default::default()
            };
        }
        Err(e) => {
            return ProbeResult {
                auth_status: "present".into(),
                probe_status: "endpoint_broken".into(),
                hint: Some(format!("连接失败: {e}")),
                ..Default::default()
            };
        }
    };

    let status = resp.status().as_u16();
    let body_text = resp.text().await.unwrap_or_default();
    let (probe_status, hint) = classify_http_status(status, &body_text);
    ProbeResult {
        auth_status: "present".into(),
        probe_status,
        http_status: Some(status),
        hint: Some(hint),
        ..Default::default()
    }
}

/// Classify HTTP status into probe_status + human hint.
///
/// 200 → ok; 401 → auth_invalid; 402 → insufficient_funds;
/// 429 → rate_limited; 404/5xx → endpoint_broken; other → endpoint_broken.
fn classify_http_status(status: u16, body_text: &str) -> (String, String) {
    match status {
        200..=299 => ("ok".into(), "通道可用".into()),
        401 => (
            "auth_invalid".into(),
            "Key 失效（401 Unauthorized），请更换".into(),
        ),
        402 => (
            "insufficient_funds".into(),
            "余额不足（402 Payment Required），请充值或切换通道".into(),
        ),
        403 => {
            let lower = body_text.to_ascii_lowercase();
            let hint = if lower.contains("quota")
                || lower.contains("余额")
                || lower.contains("insufficient")
            {
                "余额不足或配额耗尽（403）".to_string()
            } else {
                "访问被拒（403 Forbidden），Key 权限不足".into()
            };
            ("insufficient_funds".into(), hint)
        }
        404 => (
            "endpoint_broken".into(),
            "接口路径不存在（404），通道配置可能有误".into(),
        ),
        429 => (
            "rate_limited".into(),
            "限流中（429），稍后重试".into(),
        ),
        500..=599 => (
            "endpoint_broken".into(),
            format!("通道服务异常（{status}）"),
        ),
        _ => (
            "endpoint_broken".into(),
            format!("通道异常（HTTP {status}）"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_tail_basic() {
        assert_eq!(key_tail("sk-abc123b930"), Some("b930".into()));
        assert_eq!(key_tail("ab"), None);
        assert_eq!(key_tail(""), None);
    }

    #[test]
    fn detail_line_variants() {
        let p = ProbeResult {
            auth_status: "present".into(),
            key_source: Some("env:ANTHROPIC_API_KEY".into()),
            key_tail: Some("b930".into()),
            probe_status: "ok".into(),
            ..Default::default()
        };
        assert!(p.detail_line().contains("Key 有效"));
        assert!(p.detail_line().contains("…b930"));

        let p2 = ProbeResult {
            auth_status: "present".into(),
            key_source: Some("file:~/.codex/auth.json".into()),
            key_tail: Some("b930".into()),
            probe_status: "auth_invalid".into(),
            ..Default::default()
        };
        assert!(p2.detail_line().contains("Key 失效"));
    }

    #[test]
    fn probe_result_is_ok() {
        let ok = ProbeResult {
            probe_status: "ok".into(),
            ..Default::default()
        };
        assert!(ok.is_ok());

        let bad = ProbeResult {
            probe_status: "auth_invalid".into(),
            ..Default::default()
        };
        assert!(!bad.is_ok());
    }

    #[test]
    fn missing_key_returns_not_supported() {
        // In test env, ANTHROPIC_API_KEY is likely unset → missing.
        // Just verify the default ProbeResult shape.
        let p = ProbeResult::default();
        assert_eq!(p.auth_status, "unknown");
        assert_eq!(p.probe_status, "not_supported");
    }

    #[test]
    fn read_codex_auth_missing_file() {
        // This test will pass on machines without ~/.codex/auth.json.
        let _ = read_codex_auth(); // Should not panic.
    }

    #[test]
    fn read_codex_probe_config_optional() {
        // Should not panic even if config doesn't exist.
        let _ = read_codex_probe_config();
    }
}
