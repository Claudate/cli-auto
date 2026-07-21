//! G0b: reshape free-form plan markdown (CLI or local structure).

use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::domain::chat::{
    extract_plan_fence, extract_title_from_md, normalize_plan_markdown, structure_plan_markdown,
};

use super::cli_call::call_claude_normalize;
use super::types::ChatNormalizePlanResponse;

/// G0b: reshape free-form plan markdown into cco template.
/// Tries Claude CLI with a short independent prompt; on failure / fake → local `structure_plan_markdown`.
pub fn chat_normalize_plan(
    config: &Config,
    project: &Path,
    markdown: &str,
    hint: Option<&str>,
) -> Result<ChatNormalizePlanResponse> {
    if !project.is_dir() {
        bail!("project path is not a directory: {}", project.display());
    }
    let md = markdown.trim();
    if md.is_empty() {
        bail!("empty plan markdown");
    }
    let local = structure_plan_markdown(md);
    let force_fake = std::env::var("CCO_CHAT_FAKE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        || config.default.default_provider.eq_ignore_ascii_case("fake");
    if force_fake {
        return Ok(ChatNormalizePlanResponse {
            title: extract_title_from_md(&local),
            markdown: local,
            used_cli: false,
        });
    }

    let hint_line = hint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n用户补充约束：{s}\n"))
        .unwrap_or_default();
    let prompt = format!(
        r#"你是计划文档整理器。把下面「草稿」改写成结构清晰的 Markdown 计划（不要 JSON 任务图）。

硬规则：
1. 必须多行；禁止整篇挤成一行
2. 首行必须是单一「# 短标题」（≤40 字，标题内禁止 ##）
3. 必须包含：## 目标 · ## 范围 · ## 任务大纲（### T1…）· ## 验收
4. 任务标题要可执行，每任务带验收；不写「已完成」
5. 若输入已合格，只做轻量补全
6. 只输出 Markdown 正文，不要用 ``` 包裹，不要解释
{hint_line}
--- 草稿 ---
{md}
"#
    );

    match call_claude_normalize(config, project, &prompt) {
        Ok(raw) => {
            let body = extract_plan_fence(&raw)
                .or_else(|| {
                    let t = raw.trim();
                    if t.starts_with('#') {
                        Some(t.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or(raw);
            let out = structure_plan_markdown(&normalize_plan_markdown(&body));
            Ok(ChatNormalizePlanResponse {
                title: extract_title_from_md(&out),
                markdown: out,
                used_cli: true,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "chat_normalize_plan: CLI failed, local structure");
            Ok(ChatNormalizePlanResponse {
                title: extract_title_from_md(&local),
                markdown: local,
                used_cli: false,
            })
        }
    }
}
