//! Claude CLI spawn for chat / normalize turns (adapter · reuses WorkerPort provider).

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::domain::chat::{extract_assistant_text, stream_result_summary, truncate_chars};
use crate::plan::TaskIR;
use crate::runtime::provider::{claude::ClaudeProvider, StartCtx, WorkerProvider, WorkerStatus};

use super::paths::{chat_work_task_dir, normalize_work_task_dir};
use super::types::ChatSession;

pub(crate) fn system_prompt(project: &Path) -> String {
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

/// System prompt with optional project memory context (P2-2 · pins/summary only).
pub(crate) fn system_prompt_with_memory(config: &Config, project: &Path) -> String {
    let base = system_prompt(project);
    let mem = crate::app::memory::prompt_context(config, project);
    if mem.is_empty() {
        base
    } else {
        format!("{base}\n\n{mem}")
    }
}

pub(crate) fn build_user_prompt(config: &Config, sess: &ChatSession, project: &Path) -> String {
    let mut parts = vec![system_prompt_with_memory(config, project)];
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
    parts.push("\n请根据最新用户消息回复。若应输出计划，请使用 ```plan 代码块。\n".into());
    parts.join("")
}

/// Production soft-fallback assistant body: short human note, **no** ```plan fence.
pub(crate) fn soft_fallback_assistant_reply() -> String {
    "暂时无法联系本机 Claude CLI。请到「环境检查」确认 CLI 与密钥后重试，或设置 CCO_CHAT_FAKE=1 仅作 UI 联调。"
        .to_string()
}

/// Short env note for UI system bar; full diagnostic stays in logs only.
pub(crate) fn soft_fallback_env_note(diagnostic: &str) -> String {
    let short = diagnostic.chars().take(160).collect::<String>();
    let short = if diagnostic.chars().count() > 160 {
        format!("{short}…")
    } else {
        short
    };
    format!("本机 Claude CLI 暂不可用：{short}")
}

/// Forced mock (CCO_CHAT_FAKE / provider=fake) — keeps ```plan for UI 联调就绪条.
pub(crate) fn fake_chat_reply(user_msg: &str, project: &Path) -> String {
    let name = project
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "项目".into());
    let short = if user_msg.chars().count() > 80 {
        format!("{}…", user_msg.chars().take(80).collect::<String>())
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

fn make_claude_provider(config: &Config) -> ClaudeProvider {
    let bin_cfg = config
        .provider("claude")
        .map(|p| p.bin.clone())
        .unwrap_or_else(|| "claude".into());
    let bin = crate::runtime::provider::resolve_provider_bin(&bin_cfg, "CCO_CLAUDE_BIN");
    let extra = config
        .provider("claude")
        .map(|p| p.extra_args.clone())
        .unwrap_or_default();
    ClaudeProvider::new(bin, extra)
}

pub(crate) fn call_claude_chat(
    config: &Config,
    project: &Path,
    sess: &ChatSession,
) -> Result<String> {
    let provider = make_claude_provider(config);

    let task_dir = chat_work_task_dir(project);
    std::fs::create_dir_all(&task_dir)?;
    // Defense-in-depth: provider.start also clears this; chat reuses a fixed dir.
    let _ = std::fs::remove_file(task_dir.join(".done"));
    // Drop prior stream so collect cannot pick up a truncated previous turn.
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");

    let prompt = build_user_prompt(config, sess, project);
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
        tags: vec![],
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

pub(crate) fn call_claude_normalize(
    config: &Config,
    project: &Path,
    prompt: &str,
) -> Result<String> {
    let provider = make_claude_provider(config);

    let task_dir = normalize_work_task_dir(project);
    std::fs::create_dir_all(&task_dir)?;
    let _ = std::fs::remove_file(task_dir.join(".done"));
    let _ = std::fs::write(task_dir.join("stdout.json"), "");
    let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), "");
    std::fs::write(task_dir.join("prompt.md"), prompt)?;

    let chat_task = TaskIR {
        id: "__normalize__".into(),
        title: "plan normalize".into(),
        depends_on: vec![],
        group: None,
        provider: "claude".into(),
        mode: "print".into(),
        prompt: prompt.to_string(),
        acceptance: None,
        timeout_secs: Some(120),
        worktree: Some(false),
        provider_opts: serde_json::json!({
            "max_turns": null,
            "max_budget_usd": null,
            "permission_mode": "dontAsk",
        }),
        optional: false,
        include: true,
        role: None,
        scope: None,
        outputs: vec![],
        tags: vec![],
    };

    let ctx = StartCtx {
        run_id: "chat-normalize".into(),
        project_root: project.to_path_buf(),
        work_dir: project.to_path_buf(),
        task_dir: task_dir.clone(),
        env_extra: vec![],
    };

    // ~120s @ 400ms + slack
    const MAX_POLL_TICKS: u32 = 400;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("tokio runtime for chat normalize")?;
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
                        bail!("normalize Claude CLI timeout");
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
        let _ = std::fs::write(task_dir.join("stdout.raw.ndjson"), &stdout);
        let text = extract_assistant_text(&stdout);
        if !text.trim().is_empty() {
            return Ok::<String, anyhow::Error>(text);
        }
        if !matches!(result.status, crate::runtime::provider::TaskStatus::Done) {
            let err = result
                .error
                .unwrap_or_else(|| "normalize worker failed".into());
            bail!("normalize worker not done: {err}");
        }
        Ok::<String, anyhow::Error>(text)
    })?;
    if raw_out.trim().is_empty() {
        bail!("empty normalize reply");
    }
    Ok(raw_out)
}
