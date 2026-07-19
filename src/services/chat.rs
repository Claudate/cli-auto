//! Chat → plan document builder (desktop Mode B front-door).
//!
//! [INPUT]: project path · user message · optional session_id · Config (provider bin)
//! [OUTPUT]: chat_session_get · chat_send · chat_save_plan · session JSON under .cco/chat/
//! [POS]: services 子模块；只写散文 .md，不 spawn worker / 不走 confirm_start
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//! note: empty CLI reply → soft human note (no plan fence); CCO_CHAT_FAKE keeps template fence
//! note: non-zero CLI exit still yields text when stream has assistant prose (max_turns etc.)
//! note: ChatSendResponse.env_note for UI system bar (diagnostics never in assistant body)
//! note: extract_plan_fence / history truncate 必须 char-boundary 安全（CJK 禁字节硬切）

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::plan::TaskIR;
use crate::runtime::provider::{
    claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus,
};

const DEFAULT_SESSION: &str = "default";
const MAX_HISTORY_MSGS: usize = 24;
const MAX_MSG_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatDraftPlan {
    /// Relative to project root (e.g. plans/chat-20260718-1530.md)
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Markdown body ready to save (from ```plan fence); not yet on disk unless path set after save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default)]
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub session_id: String,
    pub project: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan: Option<ChatDraftPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSendResponse {
    pub session_id: String,
    pub reply: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_plan: Option<ChatDraftPlan>,
    /// true when reply came from fake template (no CLI / forced)
    pub fake: bool,
    /// Short human-readable env/CLI fault for UI system bar (not assistant body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSavePlanResponse {
    pub plan_rel: String,
    pub abs_path: String,
    pub session_id: String,
}

fn chat_dir(project: &Path) -> PathBuf {
    project.join(".cco").join("chat")
}

fn session_path(project: &Path, session_id: &str) -> PathBuf {
    let safe: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = if safe.is_empty() {
        DEFAULT_SESSION.to_string()
    } else {
        safe
    };
    chat_dir(project).join(format!("{safe}.json"))
}

fn empty_session(project: &Path, session_id: &str) -> ChatSession {
    ChatSession {
        session_id: session_id.to_string(),
        project: project.display().to_string(),
        messages: vec![],
        draft_plan: None,
        updated_at: None,
    }
}

/// Load chat session from disk; missing → empty default session.
pub fn chat_session_get(project: &Path, session_id: Option<&str>) -> Result<ChatSession> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let path = session_path(project, sid);
    if !path.is_file() {
        return Ok(empty_session(project, sid));
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read chat session {}", path.display()))?;
    let mut sess: ChatSession = serde_json::from_str(&text)
        .with_context(|| format!("parse chat session {}", path.display()))?;
    sess.session_id = sid.to_string();
    sess.project = project.display().to_string();
    Ok(sess)
}

fn save_session(project: &Path, sess: &ChatSession) -> Result<()> {
    let dir = chat_dir(project);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create chat dir {}", dir.display()))?;
    let path = session_path(project, &sess.session_id);
    let mut out = sess.clone();
    out.updated_at = Some(Utc::now().to_rfc3339());
    out.project = project.display().to_string();
    let json = serde_json::to_string_pretty(&out)?;
    std::fs::write(&path, json).with_context(|| format!("write chat session {}", path.display()))?;
    Ok(())
}

/// One round-trip: append user message, call Claude print (or fake), append assistant.
pub fn chat_send(
    config: &Config,
    project: &Path,
    message: &str,
    session_id: Option<&str>,
) -> Result<ChatSendResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let msg = message.trim();
    if msg.is_empty() {
        bail!("empty message");
    }
    if msg.chars().count() > MAX_MSG_CHARS {
        bail!("message too long (max {MAX_MSG_CHARS} chars)");
    }

    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let mut sess = chat_session_get(project, Some(sid))?;
    let now = Utc::now().to_rfc3339();
    sess.messages.push(ChatMessage {
        role: "user".into(),
        content: msg.to_string(),
        at: Some(now.clone()),
    });
    // Persist the user turn *before* the long CLI call so leaving the chat page
    // and reloading still shows the question (reply may still be pending).
    if let Err(e) = save_session(project, &sess) {
        tracing::warn!(error = %e, "chat: early save of user message failed");
    }

    let force_fake = std::env::var("CCO_CHAT_FAKE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || config.default.default_provider.eq_ignore_ascii_case("fake");

    // force_fake (CCO_CHAT_FAKE / provider=fake): full template with ```plan for UI联调.
    // production soft-fallback: short human reply + env_note; **no** plan fence → 不点亮就绪分配.
    let (reply, used_fake, env_note) = if force_fake {
        (fake_chat_reply(msg, project), true, None)
    } else {
        match call_claude_chat(config, project, &sess) {
            Ok(r) => (r, false, None),
            Err(e) => {
                let diagnostic = e.to_string();
                tracing::warn!(
                    error = %diagnostic,
                    project = %project.display(),
                    "chat: soft-fallback (CLI unavailable or empty reply)"
                );
                let env = soft_fallback_env_note(&diagnostic);
                let human = soft_fallback_assistant_reply();
                (human, true, Some(env))
            }
        }
    };

    // Only extract plan fence for real AI or forced fake-template联调.
    // Production soft-fallback has no fence; do not fabricate draft_plan.
    let draft_from_reply = if used_fake && env_note.is_some() {
        None
    } else {
        extract_plan_fence(&reply)
    };
    if let Some(md) = draft_from_reply {
        let title = extract_title_from_md(&md);
        let mut draft = sess.draft_plan.take().unwrap_or_default();
        draft.markdown = Some(md);
        draft.title = title.or(draft.title);
        // Not saved until chat_save_plan
        draft.saved = draft.saved && !draft.path.is_empty();
        if draft.path.is_empty() {
            draft.saved = false;
        }
        sess.draft_plan = Some(draft);
    }

    sess.messages.push(ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
        at: Some(Utc::now().to_rfc3339()),
    });
    // Cap history
    if sess.messages.len() > MAX_HISTORY_MSGS {
        let drop_n = sess.messages.len() - MAX_HISTORY_MSGS;
        sess.messages.drain(0..drop_n);
    }
    save_session(project, &sess)?;

    Ok(ChatSendResponse {
        session_id: sess.session_id.clone(),
        reply,
        messages: sess.messages.clone(),
        draft_plan: sess.draft_plan.clone(),
        fake: used_fake,
        env_note,
    })
}

/// Write markdown plan under project plans/ (or root), bind to session draft_plan.
pub fn chat_save_plan(
    project: &Path,
    session_id: Option<&str>,
    title: Option<&str>,
    markdown: &str,
) -> Result<ChatSavePlanResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let md = markdown.trim();
    if md.is_empty() {
        bail!("empty plan markdown");
    }
    let sid = session_id.unwrap_or(DEFAULT_SESSION);
    let mut sess = chat_session_get(project, Some(sid))?;

    let stamp = Utc::now().format("%Y%m%d-%H%M").to_string();
    let plans_dir = project.join("plans");
    let (rel, abs) = if plans_dir.is_dir() || std::fs::create_dir_all(&plans_dir).is_ok() {
        let name = format!("chat-{stamp}.md");
        let abs = plans_dir.join(&name);
        (format!("plans/{name}"), abs)
    } else {
        let name = format!("cco-plan-{stamp}.md");
        let abs = project.join(&name);
        (name, abs)
    };

    let heading = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .or_else(|| extract_title_from_md(md))
        .unwrap_or_else(|| format!("聊天生成计划 {stamp}"));

    let body = if md.starts_with('#') {
        md.to_string()
    } else {
        format!("# {heading}\n\n{md}\n")
    };

    std::fs::write(&abs, &body).with_context(|| format!("write plan {}", abs.display()))?;

    sess.draft_plan = Some(ChatDraftPlan {
        path: rel.clone(),
        title: Some(heading),
        markdown: Some(body),
        saved: true,
    });
    save_session(project, &sess)?;

    Ok(ChatSavePlanResponse {
        plan_rel: rel,
        abs_path: abs.display().to_string(),
        session_id: sess.session_id,
    })
}

fn system_prompt(project: &Path) -> String {
    format!(
        r#"你是 cco 桌面应用里的「计划写作助手」。用户在项目目录中与你对话，目标是共建一份可执行的**计划文档**（Markdown 散文/大纲），不是直接写代码或执行任务。

项目路径：{}

职责：
1. 用简短中文澄清：目标、范围、约束、验收标准、风险。
2. 当信息足够，或用户要求「生成计划/收口/写计划」时，输出完整 Markdown 计划。
3. 计划正文必须用下面 fence 包起来（便于应用解析预填；用户仍需点「保存」才会落盘）：

```plan
# 计划标题
## 目标
…
## 范围
…
## 任务大纲
1. …
2. …
## 验收
- …
```

硬规则：
- **不要**输出 cco-plan/v1 JSON 或任务图 JSON（那是后续「分配计划」阶段 Planner 的事）。
- **不要**假装已经执行了任务；你只写计划文档。
- 日常澄清轮可先不写 fence；收口轮务必带 ```plan。
- 保持简洁，优先可分配、可拆分的任务大纲。"#,
        project.display()
    )
}

fn build_user_prompt(sess: &ChatSession, project: &Path) -> String {
    let mut parts = vec![system_prompt(project)];
    parts.push("\n\n--- 对话历史 ---\n".into());
    for m in &sess.messages {
        let role = match m.role.as_str() {
            "assistant" => "助手",
            "system" => "系统",
            _ => "用户",
        };
        let content = truncate_chars(&m.content, 4000);
        parts.push(format!("\n[{role}]\n{content}\n"));
    }
    parts.push(
        "\n请根据最新用户消息回复。若应输出计划，请使用 ```plan 代码块。\n".into(),
    );
    parts.join("")
}

/// Char-count truncate (never mid-rune). Appends `…` when shortened.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    format!("{}…", s.chars().take(max_chars).collect::<String>())
}

fn call_claude_chat(config: &Config, project: &Path, sess: &ChatSession) -> Result<String> {
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    let provider = ClaudeProvider::new(bin, extra);

    let work = project.join(".cco").join("chat").join("_work");
    let task_dir = work.join("tasks").join("__chat__");
    std::fs::create_dir_all(&task_dir)?;
    // Defense-in-depth: provider.start also clears this; chat reuses a fixed dir.
    let _ = std::fs::remove_file(task_dir.join(".done"));
    // Drop prior stream so collect cannot pick up a truncated previous turn.
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");

    let prompt = build_user_prompt(sess, project);
    std::fs::write(task_dir.join("prompt.md"), &prompt)?;

    let chat_task = TaskIR {
        id: "__chat__".into(),
        title: "plan chat".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt,
        acceptance: None,
        // Wall-clock only (process timeout). Chat must NOT pass --max-turns /
        // --max-budget-usd: null omits those flags so Claude is not turn-capped.
        timeout_secs: Some(600),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            // null = omit CLI limit flags (see ClaudeProvider::opt_limit_*).
            "max_turns": null,
            "max_budget_usd": null,
            "permission_mode": "dontAsk",
            // No allowed_tools key → CLI default tools (Read/Bash/Edit…), scope-locked
            // via --append-system-prompt. Empty [] used to pass --allowedTools "" which
            // Claude 2.1.x still seeds with defaults and then hits error_max_turns at 2.
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
    };

    let ctx = StartCtx {
        run_id: format!("chat-{}", sess.session_id),
        project_root: project.to_path_buf(),
        work_dir: project.to_path_buf(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio for chat")?;

    // Match timeout_secs (~10 min) at 400ms poll interval + small slack.
    const MAX_POLL_TICKS: u32 = 1_600;

    let raw_out = rt.block_on(async {
        provider.preflight().await?;
        provider.validate_task(&chat_task)?;
        let handle = provider.start(&chat_task, &ctx).await?;
        let mut ticks = 0u32;
        loop {
            match provider.poll(&handle).await? {
                WorkerStatus::Running => {
                    ticks += 1;
                    if ticks > MAX_POLL_TICKS {
                        bail!("chat Claude CLI timeout");
                    }
                    tokio::time::sleep(Duration::from_millis(400)).await;
                }
                WorkerStatus::Done
                | WorkerStatus::Failed
                | WorkerStatus::Stopped
                | WorkerStatus::Timeout => break,
            }
        }
        let result = provider.collect(&handle).await?;
        let stdout = result
            .stdout_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        // Always keep a copy for "empty reply" post-mortems.
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), &stdout);
        // Chat is text product, not a task graph: non-zero exit (e.g. error_max_turns)
        // is fine when stream-json already has assistant prose. Soft-template only
        // when we truly have nothing usable.
        let text = extract_assistant_text(&stdout);
        if !text.trim().is_empty() {
            return Ok::<String, anyhow::Error>(stdout);
        }
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result.error.unwrap_or_else(|| "chat worker failed".into());
            let detail = stream_result_summary(&stdout);
            let snip: String = stdout.chars().take(240).collect();
            bail!("chat worker not done: {err}{detail} · stdout_snip={snip}");
        }
        Ok::<String, anyhow::Error>(stdout)
    })?;

    let text = extract_assistant_text(&raw_out);
    if text.trim().is_empty() {
        // Persist full raw so the user/doctor can open .cco/chat/_work/…
        let snip: String = raw_out.chars().take(280).collect();
        let detail = stream_result_summary(&raw_out);
        let _ = std::fs::write(
            project
                .join(".cco")
                .join("chat")
                .join("_work")
                .join("last_empty_reply.txt"),
            &raw_out,
        );
        bail!(
            "empty assistant reply from Claude CLI ({} bytes stdout{detail}; snip: {snip})",
            raw_out.len()
        );
    }
    Ok(text)
}

/// Summarize the terminal stream-json `result` line for diagnostics.
fn stream_result_summary(raw: &str) -> String {
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
fn extract_assistant_text(raw: &str) -> String {
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
        // Fall back to joining all blocks when none is clearly dominant.
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
            // already handled as JSON above
            continue;
        }
        plain_parts.push(t);
    }
    if !plain_parts.is_empty() {
        return plain_parts.join("\n");
    }
    String::new()
}

/// Byte length of a markdown fence language tag at the start of `after`.
/// Tag is ASCII `[A-Za-z0-9_+-]*` only, so the returned index is always a char boundary.
fn fence_lang_tag_len(after: &str) -> usize {
    after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '+' || *c == '-')
        .map(|c| c.len_utf8())
        .sum()
}

/// Pull ```plan … ``` body (last fence wins).
///
/// CJK-safe: never advances with a fixed byte offset into multi-byte runes.
/// Desktop crash (2026-07-19): `after[3..]` landed inside `若`/`和` after a plain
/// ``` fence → `chat_send` join panic.
pub fn extract_plan_fence(text: &str) -> Option<String> {
    let mut search = text;
    let mut best: Option<String> = None;
    while let Some(idx) = search.find("```") {
        // ``` is ASCII; idx and idx+3 are always char boundaries.
        let after = &search[idx + 3..];
        let tag_len = fence_lang_tag_len(after);
        let tag = &after[..tag_len];
        if tag.eq_ignore_ascii_case("plan") {
            let body = after[tag_len..]
                .trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
            if let Some(end) = body.find("```") {
                // end is the byte index of ASCII ``` inside body → char boundary.
                let block = body[..end].trim();
                if !block.is_empty() {
                    best = Some(block.to_string());
                }
                search = &body[end + 3..];
            } else {
                break;
            }
        } else {
            // Not a plan fence (plain / markdown / rust / …). Skip this opener;
            // prefer jumping past a closer so we do not re-scan the body.
            if let Some(end) = after.find("```") {
                search = &after[end + 3..];
            } else {
                search = after;
            }
        }
    }
    best
}

fn extract_title_from_md(md: &str) -> Option<String> {
    for line in md.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// Production soft-fallback assistant body: short human note, **no** ```plan fence.
fn soft_fallback_assistant_reply() -> String {
    "暂时无法联系本机 Claude CLI。请到「环境检查」确认 CLI 与密钥后重试，或设置 CCO_CHAT_FAKE=1 仅作 UI 联调。"
        .to_string()
}

/// Short env note for UI system bar; full diagnostic stays in logs only.
fn soft_fallback_env_note(diagnostic: &str) -> String {
    let short = diagnostic.chars().take(160).collect::<String>();
    let short = if diagnostic.chars().count() > 160 {
        format!("{short}…")
    } else {
        short
    };
    format!("本机 Claude CLI 暂不可用：{short}")
}

/// Forced mock (CCO_CHAT_FAKE / provider=fake) — keeps ```plan for UI 联调就绪条.
fn fake_chat_reply(user_msg: &str, project: &Path) -> String {
    let name = project
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "项目".into());
    let short = if user_msg.chars().count() > 80 {
        format!(
            "{}…",
            user_msg.chars().take(80).collect::<String>()
        )
    } else {
        user_msg.to_string()
    };
    format!(
        r#"好的，我根据你的描述整理了一份计划草稿（模拟回复，便于无 CLI 时联调 UI）。

你提到：{short}

```plan
# {name}：协作计划草稿

## 目标
根据用户描述完成可验证的交付。

## 范围
- 纳入：与「{short}」直接相关的实现与验收
- 不纳入：无关重构、范围外功能

## 任务大纲
1. 澄清需求与验收标准，对齐目录与约束
2. 实现核心改动并保证可编译/可运行
3. 补最小验证（单测或手工检查清单）
4. 整理变更说明与回滚点

## 验收
- [ ] 主路径可走通
- [ ] 无新增编译错误
- [ ] 文档/注释与行为一致

## 约束
- 仅改项目内必要文件
- 不引入第二套执行入口
```

若需调整范围或拆分粒度，直接说；满意后点「保存为计划」，再「分配计划」。"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extract_plan_fence_last_wins() {
        let t = "intro\n```plan\n# A\n```\nmore\n```plan\n# B\nbody\n```\n";
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# B\nbody"));
    }

    /// Desktop 2026-07-19: plain ``` then CJK used to panic with
    /// `start byte index 3 is not a char boundary; it is inside '若'/'和'`.
    #[test]
    fn extract_plan_fence_cjk_after_plain_fence_no_panic() {
        let cases = [
            "```\n若无异议\n```\n",
            "```\n和xx\n```\n",
            "好的\n```\n若需调整\n```\n后面",
            "text ``` 若xxx",
            "x```若y",
            "x```和y",
            "```ab若",
            "```\u{00e9}若", // 2-byte latin + CJK after non-plan tag path
            "聊天界面设计进度不合理",
            "a```b若c",
        ];
        for c in cases {
            let r = std::panic::catch_unwind(|| extract_plan_fence(c));
            assert!(r.is_ok(), "panicked on {c:?}: {r:?}");
            // plain fences must not be treated as plan
            assert!(r.unwrap().is_none(), "unexpected plan from {c:?}");
        }
    }

    #[test]
    fn extract_plan_fence_plan_after_cjk_plain_fence() {
        // Real chat shape: assistant writes Chinese + a non-plan code block, then ```plan.
        let t = "先说明一下\n```\n若无异议，可直接点保存\n```\n\n```plan\n# 真实计划\n## 目标\n做完\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# 真实计划\n## 目标\n做完")
        );
    }

    #[test]
    fn extract_plan_fence_plan_with_cjk_body() {
        let t = "```plan\n# 若和计划\n若无异议\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# 若和计划\n若无异议")
        );
    }

    #[test]
    fn extract_plan_fence_skips_markdown_and_keeps_later_plan() {
        let t = "```markdown\n# not plan\n```\n```plan\n# yes\n```\n";
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# yes"));
    }

    #[test]
    fn truncate_chars_cjk_safe() {
        let s = "若和计划".repeat(10); // each char 3 bytes; 40 chars
        let out = truncate_chars(&s, 5);
        assert_eq!(out.chars().count(), 6); // 5 + …
        assert!(out.ends_with('…'));
        assert!(!out.contains('\u{FFFD}'));
        assert_eq!(truncate_chars("短", 4000), "短");
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("abcd", 3), "abc…");
    }

    #[test]
    fn extract_assistant_text_from_result_line() {
        let raw = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}
{"type":"result","result":"final answer"}
"#;
        assert_eq!(extract_assistant_text(raw), "final answer");
    }

    #[test]
    fn extract_assistant_text_falls_back_to_longest_assistant() {
        // Build lines without raw-string traps around `"##` in JSON text fields.
        let long = "## 较长的计划草稿\n\n- a\n- b";
        let long_line = serde_json::json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": long }] }
        })
        .to_string();
        let raw = format!(
            "{}\n{}\n{}\n{}\n",
            r#"{"type":"system","subtype":"init"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"short"}]}}"#,
            long_line,
            r#"{"type":"result","subtype":"success","result":""}"#,
        );
        let t = extract_assistant_text(&raw);
        assert!(t.contains("较长的计划草稿"), "got: {t}");
    }

    #[test]
    fn extract_assistant_text_from_error_max_turns_keeps_prose() {
        // Real failure mode: CLI exits 1 with subtype=error_max_turns and null result,
        // but earlier assistant text is still useful (must not soft-fallback to template).
        let prose = "先摸清项目结构和现有计划/进度文档，再据此梳理框架与进度。";
        let raw = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"system","subtype":"init"}"#,
            serde_json::json!({
                "type": "assistant",
                "message": { "content": [{ "type": "text", "text": prose }] }
            }),
            serde_json::json!({
                "type": "result",
                "subtype": "error_max_turns",
                "is_error": true,
                "result": null,
                "errors": ["Reached maximum number of turns (2)"],
                "num_turns": 3,
                "stop_reason": "tool_use",
            }),
        );
        let t = extract_assistant_text(&raw);
        assert!(t.contains("先摸清项目结构"), "got: {t}");
        let summary = stream_result_summary(&raw);
        assert!(summary.contains("error_max_turns"), "got: {summary}");
        assert!(summary.contains("turns=3"), "got: {summary}");
    }

    #[test]
    fn extract_assistant_text_plain_non_json() {
        assert_eq!(
            extract_assistant_text("hello plan\nsecond line\n"),
            "hello plan\nsecond line"
        );
    }

    #[test]
    fn session_roundtrip_and_save_plan() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let sess = chat_session_get(&project, None).unwrap();
        assert!(sess.messages.is_empty());
        assert_eq!(sess.session_id, "default");

        // manual save without send
        let resp = chat_save_plan(
            &project,
            Some("default"),
            Some("测试计划"),
            "## 目标\n做一件事\n",
        )
        .unwrap();
        assert!(resp.plan_rel.starts_with("plans/chat-"));
        assert!(PathBuf::from(&resp.abs_path).is_file());

        let sess2 = chat_session_get(&project, Some("default")).unwrap();
        assert!(sess2.draft_plan.as_ref().unwrap().saved);
        assert_eq!(
            sess2.draft_plan.as_ref().unwrap().path,
            resp.plan_rel
        );
    }

    #[test]
    fn fake_send_persists_messages() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::env::set_var("CCO_CHAT_FAKE", "1");
        let cfg = Config::default();
        let r = chat_send(&cfg, &project, "帮我写个登录页计划", None).unwrap();
        assert!(r.fake);
        assert!(!r.reply.is_empty());
        assert!(r.messages.len() >= 2);
        assert!(r.draft_plan.as_ref().and_then(|d| d.markdown.as_ref()).is_some());
        assert!(r.env_note.is_none(), "forced fake has no env_note");
        // 联调路径仍产出 fence
        assert!(r.reply.contains("```plan"), "got: {}", r.reply);
        std::env::remove_var("CCO_CHAT_FAKE");
    }

    #[test]
    fn soft_fallback_reply_has_no_plan_fence() {
        let reply = soft_fallback_assistant_reply();
        assert!(extract_plan_fence(&reply).is_none());
        assert!(!reply.contains("```plan"));
        assert!(reply.contains("Claude CLI") || reply.contains("环境检查"));
    }

    #[test]
    fn soft_fallback_env_note_truncates() {
        let long = "x".repeat(300);
        let n = soft_fallback_env_note(&long);
        assert!(n.chars().count() < 220, "got len {}", n.chars().count());
        assert!(n.contains("暂不可用"));
    }

    #[test]
    fn call_prep_clears_stale_chat_work_dir() {
        // Mirrors the desktop bug: second chat_send reused __chat__ with leftover .done
        // and empty stdout → extract_assistant_text("") → soft local template.
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        let task_dir = project
            .join(".cco")
            .join("chat")
            .join("_work")
            .join("tasks")
            .join("__chat__");
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(task_dir.join(".done"), "0").unwrap();
        std::fs::write(task_dir.join("stdout.json"), "stale").unwrap();
        std::fs::write(task_dir.join("stdout.raw.ndjson"), "stale").unwrap();

        // Same cleanup call_claude_chat performs before provider.start.
        let _ = std::fs::remove_file(task_dir.join(".done"));
        let _ = std::fs::write(task_dir.join("stdout.json"), "");
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");

        assert!(!task_dir.join(".done").exists());
        assert_eq!(
            std::fs::read_to_string(task_dir.join("stdout.json")).unwrap(),
            ""
        );
        assert_eq!(
            std::fs::read_to_string(task_dir.join("stdout.raw.ndjson")).unwrap(),
            ""
        );
    }
}
