//! Chat slash-command operations (session · plan · run · memory queries).
//!
//! Local, human-readable replies — never spawn workers, never touch
//! confirm_start / start_run. Read-mostly; `/save` writes a plan file the same
//! way the "保存计划" button does (still no run).
//!
//! [INPUT]: Config · project · session · command args
//! [OUTPUT]: human reply text (OpsReply) for send.rs to persist as a turn
//! [POS]: services/chat adapter — thin queries over existing services/state
//! [PROTOCOL]: 变更时更新 mod.rs 头部 note；禁止在聊天内开跑/停任务。

use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::state::project_memory::{delete_pin, get_memory, set_project_pin, ProjectPin};

use super::plan_md::chat_save_plan;
use super::session::chat_list_sessions;
use super::types::ChatSession;

/// Supported effort levels (same set as the UI dropdown + CLI).
pub(crate) const EFFORT_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultracode"];

/// `/effort <level>`: validate and return the normalized level.
pub(crate) fn effort_switch(args: &str) -> Result<String> {
    let level = args.trim().to_lowercase();
    if !EFFORT_LEVELS.contains(&level.as_str()) {
        bail!("未知档位：{level}。可选：{}。", EFFORT_LEVELS.join(" · "));
    }
    Ok(level)
}

/// Known Claude model families for `/models` query + `/model` hint.
/// Not a hard allow-list: `/model <name>` accepts any non-empty token so future
/// / aliased models keep working; this list only feeds the query reply.
pub(crate) const KNOWN_MODELS: &[&str] = &[
    "claude-opus-5",
    "claude-sonnet-5",
    "claude-fable-5",
    "claude-haiku-4-5",
];

/// `/model <name>`: switch the chat model for this session. Accepts any
/// non-empty token (Claude `--model` accepts aliases like `sonnet`, `opus`,
/// `haiku`). Returns the normalized model name.
pub(crate) fn model_switch(args: &str) -> Result<String> {
    let m = args.trim().to_lowercase();
    if m.is_empty() {
        bail!("用法：/model <名称>。例如 /model sonnet 或 /model claude-opus-5。");
    }
    if m.chars().count() > 80 {
        bail!("模型名过长（上限 80 字符）。");
    }
    Ok(m)
}

/// `/models`: list known models. cco cannot enumerate the provider's live
/// catalogue, so this is a curated list + a pointer to `/model` free-form.
pub(crate) fn models_text() -> String {
    format!(
        "可用的 Claude 模型（cco 已知）：\n{}\n\n\
         /model <名称> 可切换到任意模型（含别名，如 sonnet / opus / haiku）。\n\
         未填时走 CLI 默认模型；当前通道若是 claude，模型随 `--model` 生效。",
        KNOWN_MODELS
            .iter()
            .map(|m| format!("· {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// `/rename <title>`: clean the title arg (trim · cap 80 chars).
pub(crate) fn rename_title(args: &str) -> String {
    let t = args.trim();
    t.chars().take(80).collect()
}

/// `/plan`: current draft status in human terms.
pub(crate) fn plan_status_text(sess: &ChatSession) -> String {
    match sess.draft_plan.as_ref() {
        None => {
            "当前没有计划草稿。跟我说要做什么，或者直接说「生成计划」，我会写出带目标/验收的草稿。"
                .to_string()
        }
        Some(d) => {
            let title = d
                .title
                .as_deref()
                .filter(|t| !t.is_empty())
                .unwrap_or("（未命名）");
            if d.saved {
                format!(
                    "当前草稿「{title}」已保存：`{}`。可去拆分台确认分配，或继续修改。",
                    d.path
                )
            } else if d.markdown.as_ref().is_some() {
                format!("当前草稿「{title}」还没保存。输入 /save 存到 plans/ 目录。")
            } else {
                "当前有一个空的草稿位。".to_string()
            }
        }
    }
}

/// `/save`: persist the current ```plan draft via the same path as the save button.
pub(crate) fn save_draft(project: &Path, sess: &mut ChatSession) -> Result<String> {
    let Some(draft) = sess.draft_plan.as_ref() else {
        bail!("当前没有可保存的计划草稿。先让 AI 输出 ```plan 草稿，或直接说「生成计划」。");
    };
    let Some(md) = draft.markdown.as_deref().filter(|m| !m.trim().is_empty()) else {
        bail!("草稿为空，无法保存。");
    };
    let title = draft.title.clone();
    let resp = chat_save_plan(
        project,
        Some(&sess.session_id),
        title.as_deref(),
        md,
        None,
        None,
    )?;
    // chat_save_plan updates its own loaded session copy; mirror the saved
    // state back so the caller's later save_session cannot clobber it.
    if let Some(d) = sess.draft_plan.as_mut() {
        d.saved = true;
        d.path = resp.plan_rel.clone();
    }
    Ok(format!(
        "已保存计划草稿：`{}`。下一步可在拆分台确认后分配执行。",
        resp.plan_rel
    ))
}

/// `/sessions`: list chat sessions (newest first, current marked).
pub(crate) fn sessions_text(project: &Path, current_id: &str) -> Result<String> {
    let list = chat_list_sessions(project)?;
    if list.is_empty() {
        return Ok("还没有会话。".to_string());
    }
    let mut lines = vec!["本项目会话：".to_string()];
    for s in list.iter().take(10) {
        let marker = if s.session_id == current_id {
            " ← 当前"
        } else {
            ""
        };
        let name = s
            .title
            .as_deref()
            .filter(|t| !t.is_empty())
            .unwrap_or(&s.session_id);
        let n = s.message_count;
        let preview = s
            .preview
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(24)
            .collect::<String>();
        let preview = if preview.is_empty() {
            String::new()
        } else {
            format!("（{preview}…）")
        };
        lines.push(format!("· {name} — {n} 条{preview}{marker}"));
    }
    lines.push("提示：会话切换在聊天页顶部；/clear 清空当前，/rename <标题> 改名。".to_string());
    Ok(lines.join("\n"))
}

/// `/plans`: list plan files under the project (plans/ dir + root .md).
pub(crate) fn plans_text(project: &Path) -> Result<String> {
    let plans = crate::services::runs::list_plans(project)?;
    if plans.is_empty() {
        return Ok("项目下还没有计划文件。写好草稿后 /save 会存到 plans/ 目录。".to_string());
    }
    let mut lines = vec![format!("项目计划（{} 个）：", plans.len())];
    for p in plans.iter().take(15) {
        lines.push(format!("· `{p}`"));
    }
    lines.push("在拆分台可对计划做拆分与确认；确认后才会进入执行。".to_string());
    Ok(lines.join("\n"))
}

/// `/status`: latest run summary (most recent run dir first).
pub(crate) fn status_text(config: &Config) -> Result<String> {
    let runs = crate::services::runs::list_runs(config)?;
    let Some(r) = runs.first() else {
        return Ok("还没有运行记录。计划确认后才会开跑（拆分台 → 确认）。".to_string());
    };
    let status = if r.status.contains("running") || r.status.contains("paused") {
        format!("{}（{} 个任务）", r.status, r.task_count)
    } else {
        r.status.clone()
    };
    Ok(format!(
        "最近一次运行：`{}` · 状态 {status} · 计划 `{}` · 启动于 {}。\
         详细进度与报告请到运行页查看。",
        r.run_id, r.plan_path, r.started_at
    ))
}

/// `/resume`: list runs that can be resumed (paused/failed/aborted) and point
/// to the run page. Chat does not start workers (L1 rule 10: unique run entry
/// is split confirm); resume happens on the run page / `cco resume`.
pub(crate) fn resume_text(config: &Config) -> Result<String> {
    let runs = crate::services::runs::list_runs(config)?;
    let resumable: Vec<_> = runs
        .iter()
        .filter(|r| {
            let s = r.status.to_lowercase();
            s.contains("paused") || s.contains("fail") || s.contains("abort")
        })
        .collect();
    if resumable.is_empty() {
        return Ok("没有可恢复的运行（paused / failed / aborted 为空）。恢复操作在运行页完成。".to_string());
    }
    let mut lines = vec!["可恢复的运行：".to_string()];
    for r in resumable.iter().take(8) {
        lines.push(format!(
            "· `{}` — {}（{}）· 计划 `{}`",
            r.run_id, r.status, r.started_at, r.plan_path
        ));
    }
    lines.push("聊天不直接恢复运行（避免旁路确认）；请到运行页选择对应运行恢复。".to_string());
    Ok(lines.join("\n"))
}

/// `/report`: latest run report summary (most recent run dir first).
pub(crate) fn report_text(config: &Config) -> Result<String> {
    let runs = crate::services::runs::list_runs(config)?;
    let Some(r) = runs.first() else {
        return Ok("还没有运行记录。计划确认后才会开跑。".to_string());
    };
    Ok(format!(
        "最近一次运行报告：`{}` · 状态 {} · {} 个任务 · 计划 `{}`。\
         完整报告与巡检对照请到运行页查看。",
        r.run_id, r.status, r.task_count, r.plan_path
    ))
}

/// `/memory [add <key> <value> | rm <key>]`: project memory (last summary +
/// pins). Bare `/memory` reads; `add` writes a pin; `rm` deletes one.
pub(crate) fn memory_text(config: &Config, project: &Path, args: &str) -> Result<String> {
    let trimmed = args.trim();
    if let Some(sub) = trimmed.strip_prefix("add ") {
        return memory_add_pin(config, project, sub);
    }
    if let Some(sub) = trimmed.strip_prefix("rm ") {
        return memory_rm_pin(config, project, sub);
    }
    if !trimmed.is_empty() {
        bail!(
            "用法：/memory（查看）· /memory add <key> <value>（固定事项）· /memory rm <key>（删除）。"
        );
    }
    memory_view(config, project)
}

/// `/memory add <key> <value>`: write/upsert a project pin.
fn memory_add_pin(config: &Config, project: &Path, arg: &str) -> Result<String> {
    let mut it = arg.splitn(2, char::is_whitespace);
    let key = it.next().unwrap_or("").trim();
    let value = it.next().unwrap_or("").trim();
    if key.is_empty() {
        bail!("用法：/memory add <key> <value>。key 不能为空。");
    }
    if value.is_empty() {
        bail!("用法：/memory add <key> <value>。value 不能为空。");
    }
    if key.chars().count() > 60 {
        bail!("记忆 key 过长（上限 60 字符）。");
    }
    let value_capped: String = value.chars().take(200).collect();
    let pid = project.to_string_lossy().trim_end_matches('/').to_string();
    set_project_pin(config, &pid, key, &value_capped)?;
    Ok(format!("已固定事项：{key} —— {value_capped}。"))
}

/// `/memory rm <key>`: delete a project pin.
fn memory_rm_pin(config: &Config, project: &Path, arg: &str) -> Result<String> {
    let key = arg.trim();
    if key.is_empty() {
        bail!("用法：/memory rm <key>。");
    }
    let pid = project.to_string_lossy().trim_end_matches('/').to_string();
    let removed = delete_pin(config, &pid, key)?;
    if removed {
        Ok(format!("已删除固定事项：{key}。"))
    } else {
        Ok(format!("没有名为「{key}」的固定事项。"))
    }
}

/// `/memory` (bare): project memory (last summary + pins).
fn memory_view(config: &Config, project: &Path) -> Result<String> {
    let pid = project.to_string_lossy().trim_end_matches('/').to_string();
    let view = get_memory(config, &pid)?;
    let mut lines = vec!["项目记忆：".to_string()];
    match view.last_summary {
        Some(s) if !s.text.trim().is_empty() => {
            let head: String = s.text.trim().chars().take(200).collect();
            lines.push(format!("· 最近总结（{}）：{head}", s.updated_at));
        }
        _ => lines.push("· 还没有运行总结。".to_string()),
    }
    if view.pins.is_empty() {
        lines.push("· 没有固定事项（pins）。".to_string());
    } else {
        for pin in &view.pins {
            lines.push(format!("· {}：{}", pin.key, pin_value_head(pin)));
        }
    }
    lines.push("提示：/memory add <key> <value> 固定事项，/memory rm <key> 删除。".to_string());
    Ok(lines.join("\n"))
}

fn pin_value_head(pin: &ProjectPin) -> String {
    pin.value.trim().chars().take(120).collect()
}
