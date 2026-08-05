//! Chat slash commands: cco-owned local commands + per-CLI pass-through policy.
//!
//! ## Routing (per-CLI behavior)
//! | Input | claude / codex / … | fake (CCO_CHAT_FAKE / provider=fake) |
//! |-------|--------------------|----------------------------------------|
//! | cco local (`/help` `/clis` `/cli` `/effort` `/rename` `/plan` `/save` `/sessions` `/plans` `/status` `/memory` `/clear` `/new`) | local reply (no AI call) | local reply (same) |
//! | reserved (`/run` `/stop` `/start`) | guidance reply (never executed from chat) | guidance reply (same) |
//! | any other `/cmd` | pass through to the picked CLI, verbatim | local note (no real CLI to route to) |
//!
//! [INPUT]: raw message · Config · project · picked CLI · force_fake · session
//! [OUTPUT]: LocalCommandOutcome (None → keep normal CLI send path)
//! [POS]: services/chat adapter — thin; cco never guesses third-party CLI command
//! semantics, it only routes; heavy queries live in `ops`.
//! [PROTOCOL]: local replies never spawn workers, never touch confirm_start /
//! start_run, never fabricate a ```plan fence; 变更时更新 mod.rs 头部 note。

use std::path::Path;

use anyhow::Result;

use crate::config::Config;

use super::cli_select::available_chat_clis;
use super::ops;
use super::types::{ChatSession, SlashCommandInfo};

/// cco-owned local commands (answered without any AI call, for any CLI).
pub(crate) const LOCAL_COMMANDS: &[&str] = &[
    "help", "clis", "cli", "effort", "model", "models", "clear", "new", "rename", "plan", "save",
    "sessions", "plans", "status", "memory", "resume", "report",
];

/// cco-reserved commands that are **not** executed from chat (would bypass the
/// split-confirm door). They get a guidance reply instead of passing through.
pub(crate) const RESERVED_COMMANDS: &[&str] = &["run", "stop", "start"];

/// Split a `/cmd args` message. Returns `(cmd, args)`; `None` when the message
/// is not a slash command (plain text, bare `/`, `/` followed by whitespace).
pub(crate) fn parse_slash_command(msg: &str) -> Option<(&str, &str)> {
    let t = msg.trim();
    if !t.starts_with('/') {
        return None;
    }
    let rest = &t[1..];
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if !c.is_whitespace() && c.is_alphabetic() => {}
        _ => return None, // bare "/" or "/ …" is plain text
    }
    match rest.find(char::is_whitespace) {
        Some(i) => Some((&rest[..i], rest[i..].trim())),
        None => Some((rest, "")),
    }
}

/// Whether `cmd` is cco-owned (answered locally for every CLI).
pub(crate) fn is_local_command(cmd: &str) -> bool {
    LOCAL_COMMANDS.contains(&cmd)
}

/// Whether `cmd` drops the conversation (history + draft plan) before replying.
pub(crate) fn clears_history(cmd: &str) -> bool {
    matches!(cmd, "clear" | "new")
}

/// Local command catalog — single source for `/help` text AND the autocomplete
/// DTO. `(cmd, args, desc, group)`. Kept in sync with the `local_command` arms.
const LOCAL_CATALOG: &[(&str, &str, &str, &str)] = &[
    // 会话
    ("help", "", "查看可用命令", "会话"),
    ("rename", "<标题>", "给当前会话命名", "会话"),
    ("clear", "", "清空对话", "会话"),
    ("new", "", "另起一轮", "会话"),
    ("sessions", "", "列出本项目的会话", "会话"),
    // 通道与档位
    ("cli", "<名称>", "切换聊天 CLI（不带参数看当前与可用）", "通道与档位"),
    ("clis", "", "列出可用 CLI", "通道与档位"),
    (
        "effort",
        "<档位>",
        "本会话默认思考档位（low/medium/high/xhigh/max/ultracode）",
        "通道与档位",
    ),
    ("model", "<名称>", "本会话模型（如 sonnet / opus；不带参数看可用）", "通道与档位"),
    ("models", "", "列出已知模型", "通道与档位"),
    // 计划
    ("plan", "", "当前计划草稿状态", "计划"),
    ("save", "", "把草稿保存到 plans/", "计划"),
    ("plans", "", "列出项目计划", "计划"),
    // 项目
    ("status", "", "最新运行摘要", "项目"),
    ("report", "", "最近运行报告摘要", "项目"),
    ("resume", "", "列出可恢复的运行（恢复在运行页）", "项目"),
    ("memory", "[add <k> <v> | rm <k>]", "项目记忆（摘要与固定事项）", "项目"),
];

/// Passthrough catalog — per-CLI built-in commands cco does NOT own, sent to the
/// picked CLI verbatim. `(cli_name, cmd, args, desc)`. Static hint only; whether
/// the CLI actually handles it depends on the CLI.
const PASSTHROUGH_CATALOG: &[(&str, &str, &str, &str)] = &[
    ("claude", "compact", "", "压缩上下文"),
    ("claude", "model", "[名称]", "切换 Claude 模型"),
    ("claude", "init", "", "创建项目 CLAUDE.md"),
    ("claude", "memory", "", "读写 Claude 项目记忆"),
    ("claude", "config", "", "查看/编辑 Claude 配置"),
    ("claude", "cost", "", "查看本次会话费用"),
    ("claude", "doctor", "", "Claude 环境自检"),
    ("claude", "permissions", "", "查看权限设置"),
    ("claude", "mcp", "", "管理 MCP 服务器"),
    ("claude", "login", "", "登录 Claude 账号"),
    ("claude", "logout", "", "退出登录"),
    ("claude", "status", "", "当前会话状态"),
    ("claude", "review", "", "代码评审"),
    ("claude", "pr", "", "创建 PR"),
    ("claude", "export", "[路径]", "导出会话"),
    ("claude", "import", "[路径]", "导入会话"),
    ("claude", "add-dir", "<目录>", "添加项目目录"),
    ("codex", "review", "", "代码评审"),
    ("codex", "doctor", "", "Codex 环境诊断"),
    ("codex", "login", "", "登录 Codex"),
    ("codex", "logout", "", "退出登录"),
    ("codex", "mcp", "", "管理 Codex MCP"),
    ("codex", "plugin", "", "管理插件"),
    ("codex", "exec", "", "非交互运行"),
    ("codex", "resume", "", "恢复会话"),
];

/// Slash command catalog for the composer autocomplete. `cli` is the picked
/// channel (None → claude default). Local commands appear for every CLI;
/// passthrough only for the matching CLI; reserved are marked for grey-out.
/// Single source for `/help` (see `help_text`).
pub fn slash_catalog(cli: Option<&str>) -> Vec<SlashCommandInfo> {
    let cli = cli.unwrap_or("claude").to_lowercase();
    let mut out: Vec<SlashCommandInfo> = LOCAL_CATALOG
        .iter()
        .map(|(cmd, args, desc, group)| SlashCommandInfo {
            cmd: (*cmd).into(),
            args: (*args).into(),
            desc: (*desc).into(),
            group: (*group).into(),
            scope: "local".into(),
        })
        .collect();
    out.extend(PASSTHROUGH_CATALOG.iter().filter(|(c, _, _, _)| *c == cli).map(
        |(_, cmd, args, desc)| SlashCommandInfo {
            cmd: (*cmd).into(),
            args: (*args).into(),
            desc: (*desc).into(),
            group: "透传".into(),
            scope: "passthrough".into(),
        },
    ));
    out.extend(RESERVED_COMMANDS.iter().map(|cmd| SlashCommandInfo {
        cmd: (*cmd).into(),
        args: String::new(),
        desc: match *cmd {
            "run" => "执行计划（须经拆分台确认）",
            "stop" => "停止运行（在运行页操作）",
            "start" => "开始运行（在运行页操作）",
            _ => "运行操作（聊天不执行）",
        }
        .into(),
        group: "运行".into(),
        scope: "reserved".into(),
    }));
    out
}

/// Structured outcome of a locally handled slash command.
#[derive(Debug, Clone, Default)]
pub(crate) struct LocalCommandOutcome {
    pub reply: String,
    /// New session CLI when `/cli` switched channels (echoed to the UI).
    pub new_cli: Option<String>,
    /// New session effort when `/effort` changed it (echoed to the UI).
    pub new_effort: Option<String>,
    /// New session model when `/model` changed it (echoed to the UI).
    pub new_model: Option<String>,
}

impl LocalCommandOutcome {
    fn plain(reply: impl Into<String>) -> Self {
        Self {
            reply: reply.into(),
            ..Default::default()
        }
    }
}

/// Run an ops query; errors become human reply text instead of failing the
/// whole send turn (front-end would show "发送失败").
fn reply_or_err(f: impl FnOnce() -> Result<String>) -> LocalCommandOutcome {
    match f() {
        Ok(t) => LocalCommandOutcome::plain(t),
        Err(e) => LocalCommandOutcome::plain(e.to_string()),
    }
}

/// Dispatch a cco-owned slash command for the current session.
/// Returns `None` when the command should pass through to the picked CLI
/// (unknown command on a real CLI). `force_fake` covers CCO_CHAT_FAKE /
/// provider=fake / cli=fake. Mutates `sess` for session mutators
/// (cli/effort/rename). Local replies never spawn workers / never bypass
/// confirm_start.
pub(crate) fn local_command(
    config: &Config,
    project: &Path,
    cmd: &str,
    args: &str,
    cli: Option<&str>,
    force_fake: bool,
    sess: &mut ChatSession,
) -> Result<Option<LocalCommandOutcome>> {
    // Reserved execution commands are never run from chat — guide instead.
    if let Some(reply) = reserved_reply(cmd) {
        return Ok(Some(LocalCommandOutcome::plain(reply)));
    }
    if is_local_command(cmd) {
        let out = match cmd {
            "help" => LocalCommandOutcome::plain(help_text(cli)),
            "clis" => LocalCommandOutcome::plain(clis_text(config)),
            "cli" => {
                let (reply, new_cli) = cli_switch(config, args, cli.or(sess.cli.as_deref()));
                if let Some(nc) = new_cli.clone() {
                    sess.cli = Some(nc);
                }
                LocalCommandOutcome {
                    reply,
                    new_cli,
                    ..Default::default()
                }
            }
            "effort" => match ops::effort_switch(args) {
                Ok(level) => {
                    let reply = format!("本会话默认思考档位已设为 {level}。顶部选择器可临时覆盖。");
                    sess.effort = Some(level.clone());
                    LocalCommandOutcome {
                        reply,
                        new_effort: Some(level),
                        ..Default::default()
                    }
                }
                Err(e) => LocalCommandOutcome::plain(e.to_string()),
            },
            "model" => match ops::model_switch(args) {
                Ok(m) => {
                    let reply = format!("本会话模型已设为 {m}。后续消息经当前 CLI 通道时生效。");
                    sess.model = Some(m.clone());
                    LocalCommandOutcome {
                        reply,
                        new_model: Some(m),
                        ..Default::default()
                    }
                }
                Err(e) => LocalCommandOutcome::plain(e.to_string()),
            },
            "models" => LocalCommandOutcome::plain(ops::models_text()),
            "rename" => {
                let title = ops::rename_title(args);
                if title.is_empty() {
                    LocalCommandOutcome::plain(
                        "用法：/rename <标题>。给当前会话起个名字。".to_string(),
                    )
                } else {
                    sess.title = Some(title.clone());
                    LocalCommandOutcome::plain(format!("当前会话已命名为「{title}」。"))
                }
            }
            "plan" => LocalCommandOutcome::plain(ops::plan_status_text(sess)),
            "save" => reply_or_err(|| ops::save_draft(project, sess)),
            "sessions" => reply_or_err(|| ops::sessions_text(project, &sess.session_id)),
            "plans" => reply_or_err(|| ops::plans_text(project)),
            "status" => reply_or_err(|| ops::status_text(config)),
            "resume" => reply_or_err(|| ops::resume_text(config)),
            "report" => reply_or_err(|| ops::report_text(config)),
            "memory" => reply_or_err(|| ops::memory_text(config, project, args)),
            "clear" | "new" => LocalCommandOutcome::plain(clear_text(cmd)),
            _ => unreachable!("LOCAL_COMMANDS covers all local commands"),
        };
        Ok(Some(out))
    } else if force_fake {
        Ok(Some(LocalCommandOutcome::plain(format!(
            "当前是 fake（联调）通道，没有真实 CLI 可处理「/{cmd}」。输入 /help 查看可用命令。"
        ))))
    } else {
        Ok(None)
    }
}

/// Guidance reply for reserved commands that chat must not execute.
fn reserved_reply(cmd: &str) -> Option<String> {
    if RESERVED_COMMANDS.contains(&cmd) {
        Some(match cmd {
            "run" => "聊天里不开跑：执行计划必须经过「拆分台确认」这一步。\
                 请先在这里把计划写清楚，保存后用「分配计划」进入拆分台；\
                 或用运行页查看/管理已有运行。"
                .to_string(),
            "stop" => "停止运行请到运行页操作（聊天不直接停任务，避免误停）。".to_string(),
            _ => "该操作请在对应页面完成（聊天不旁路确认）。".to_string(),
        })
    } else {
        None
    }
}

/// `/cli <name>`: switch the chat channel for this session.
/// Returns `(reply, new_cli)`. `new_cli` is `Some` only on a successful switch;
/// a missing/unknown name yields an informational reply with `None`.
pub(crate) fn cli_switch(
    config: &Config,
    args: &str,
    current: Option<&str>,
) -> (String, Option<String>) {
    let infos = available_chat_clis(config);
    let name = args.trim().to_lowercase();
    if name.is_empty() {
        let cur = current.unwrap_or("claude");
        let avail: Vec<String> = infos.iter().map(|i| i.name.clone()).collect();
        return (
            format!(
                "当前聊天通道：{cur}。可用：{}。\n输入 /cli <名称> 切换。",
                avail.join(" · ")
            ),
            None,
        );
    }
    if let Some(info) = infos.iter().find(|i| i.name == name) {
        let note = if info.name == "fake" {
            "（联调模板通道，非真实 AI）".to_string()
        } else if !info.installed {
            "（本机未检测到该 CLI 可执行文件）".to_string()
        } else {
            String::new()
        };
        (
            format!("已切换，后续消息走 {} 通道 {note}", info.label),
            Some(info.name.clone()),
        )
    } else {
        let avail: Vec<String> = infos.iter().map(|i| i.name.clone()).collect();
        (
            format!("未知 CLI：{name}。可用：{}。", avail.join(" · ")),
            None,
        )
    }
}

/// `/help`: list cco-owned commands (single source = `slash_catalog`); other
/// `/cmd` goes to the picked CLI.
fn help_text(cli: Option<&str>) -> String {
    let channel = cli.unwrap_or("claude");
    let cat = slash_catalog(cli);
    let mut lines = vec!["cco 聊天命令（不发给 AI，立即生效）：".to_string()];
    // Keep group order: 会话 → 通道与档位 → 计划 → 项目
    for group in ["会话", "通道与档位", "计划", "项目"] {
        lines.push(format!("· {group}"));
        for c in cat.iter().filter(|c| c.group == group && c.scope == "local") {
            let args = if c.args.is_empty() {
                String::new()
            } else {
                format!(" {}", c.args)
            };
            lines.push(format!("· /{}{} — {}", c.cmd, args, c.desc));
        }
    }
    lines.push(format!(
        "/run、/stop 等运行操作不在聊天内执行（须走拆分台确认/运行页）。\n\
         其他以 / 开头的命令会原样交给当前聊天通道（{channel}）处理。"
    ));
    lines.join("\n")
}

/// `/clis`: list chat-capable CLIs (same source as the dropdown).
fn clis_text(config: &Config) -> String {
    let infos = available_chat_clis(config);
    if infos.is_empty() {
        return "当前没有可用的聊天 CLI。".to_string();
    }
    let mut lines: Vec<String> = infos
        .iter()
        .map(|i| {
            let state = if i.name == "fake" {
                "可用（联调模板）"
            } else {
                match (i.installed, i.enabled) {
                    (true, true) => "可用",
                    (true, false) => "已安装但未启用",
                    (false, _) => "未安装",
                }
            };
            format!("· {} — {}", i.label, state)
        })
        .collect();
    lines.insert(0, "当前可用聊天 CLI：".to_string());
    lines.join("\n")
}

/// `/clear` `/new`: drop conversation (messages + draft plan).
fn clear_text(cmd: &str) -> String {
    match cmd {
        "new" => "已清空对话，另起一轮。后续消息会正常进入当前聊天通道。".to_string(),
        _ => "已清空当前对话。后续消息会正常进入当前聊天通道。".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_cfg() -> (tempfile::TempDir, PathBuf, Config) {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("app");
        std::fs::create_dir_all(&project).unwrap();
        let mut cfg = Config::default();
        cfg.default.default_provider = "fake".into();
        // Isolate runs/state so list_runs/status do not read the real ~/.cco.
        cfg.state_root = dir.path().join("state");
        (dir, project, cfg)
    }

    fn sess(sid: &str) -> ChatSession {
        ChatSession {
            session_id: sid.into(),
            project: "app".into(),
            messages: vec![],
            draft_plan: None,
            updated_at: None,
            title: None,
            clarify: None,
            session_digest: None,
            cli: None,
            effort: None,
            model: None,
        }
    }

    #[test]
    fn parse_bare_command_and_args() {
        assert_eq!(parse_slash_command("/help"), Some(("help", "")));
        assert_eq!(
            parse_slash_command("/clear  现在  "),
            Some(("clear", "现在"))
        );
        assert_eq!(parse_slash_command("/clis"), Some(("clis", "")));
    }

    #[test]
    fn parse_non_commands() {
        assert_eq!(parse_slash_command(""), None);
        assert_eq!(parse_slash_command("/"), None);
        assert_eq!(parse_slash_command("/ "), None);
        assert_eq!(parse_slash_command("/  你好"), None);
        assert_eq!(parse_slash_command("你好 /help"), None);
        assert_eq!(parse_slash_command("看下 / 的用法"), None);
    }

    #[test]
    fn parse_cjk_command_passes_through() {
        // Unknown (non-cco) command, even CJK: routed, not local.
        assert_eq!(parse_slash_command("/计划"), Some(("计划", "")));
        assert!(!is_local_command("计划"));
    }

    #[test]
    fn local_command_help_lists_channels_and_commands() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "help", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("/cli"), "got: {}", out.reply);
        assert!(out.reply.contains("/effort"), "got: {}", out.reply);
        assert!(out.reply.contains("/save"), "got: {}", out.reply);
        assert!(out.reply.contains("claude"), "got: {}", out.reply);
    }

    #[test]
    fn local_command_effort_validates_and_persists() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "effort", "max", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert_eq!(out.new_effort.as_deref(), Some("max"));
        assert_eq!(s.effort.as_deref(), Some("max"));
        // invalid level → informational reply, no mutation
        let out = local_command(&cfg, &project, "effort", "turbo", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("未知档位"), "got: {}", out.reply);
        assert!(out.new_effort.is_none());
        assert_eq!(s.effort.as_deref(), Some("max"), "unchanged on reject");
    }

    #[test]
    fn local_command_model_switches_and_persists() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "model", "sonnet", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert_eq!(out.new_model.as_deref(), Some("sonnet"));
        assert_eq!(s.model.as_deref(), Some("sonnet"));
        // empty → hint, no mutation
        let out = local_command(&cfg, &project, "model", "   ", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("用法"), "got: {}", out.reply);
        assert_eq!(s.model.as_deref(), Some("sonnet"));
        // over-long → reject
        let out = local_command(
            &cfg,
            &project,
            "model",
            &"x".repeat(81),
            None,
            true,
            &mut s,
        )
        .unwrap()
        .unwrap();
        assert!(out.reply.contains("过长"), "got: {}", out.reply);
        assert_eq!(s.model.as_deref(), Some("sonnet"));
    }

    #[test]
    fn local_command_models_lists_known() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "models", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("claude"), "got: {}", out.reply);
        assert!(out.reply.contains("/model"), "got: {}", out.reply);
    }

    #[test]
    fn local_command_rename_persists_title() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "rename", " 登录页改造 ", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert_eq!(s.title.as_deref(), Some("登录页改造"));
        assert!(out.reply.contains("登录页改造"), "got: {}", out.reply);
        // empty title → hint, no mutation
        let out = local_command(&cfg, &project, "rename", "   ", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("用法"), "got: {}", out.reply);
        assert_eq!(s.title.as_deref(), Some("登录页改造"));
    }

    #[test]
    fn local_command_reserved_run_guides() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(
            &cfg,
            &project,
            "run",
            "docs/plans/foo.md",
            None,
            false,
            &mut s,
        )
        .unwrap()
        .unwrap();
        assert!(out.reply.contains("拆分台"), "got: {}", out.reply);
        // reserved wins over pass-through even on a real CLI
        assert!(is_local_command("run") == false);
    }

    #[test]
    fn unknown_command_passthrough_policy() {
        let (_d, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        // real CLI → None (pass through to the CLI)
        assert!(
            local_command(&cfg, &project, "compact", "", Some("claude"), false, &mut s)
                .unwrap()
                .is_none()
        );
        // fake channel → local note
        let out = local_command(&cfg, &project, "compact", "", Some("fake"), true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("/compact"), "got: {}", out.reply);
        assert!(out.reply.contains("fake"), "got: {}", out.reply);
    }

    #[test]
    fn cli_switch_without_args_shows_current_and_available() {
        let (_d, _p, cfg) = tmp_cfg();
        let (reply, new_cli) = cli_switch(&cfg, "", Some("claude"));
        assert!(new_cli.is_none(), "no switch without a name");
        assert!(reply.contains("当前聊天通道：claude"), "got: {reply}");
        assert!(reply.contains("codex"), "got: {reply}");
        assert!(reply.contains("/cli"), "got: {reply}");
    }

    #[test]
    fn cli_switch_known_name_succeeds() {
        let (_d, _p, cfg) = tmp_cfg();
        let (reply, new_cli) = cli_switch(&cfg, "codex", Some("claude"));
        assert_eq!(new_cli.as_deref(), Some("codex"));
        assert!(reply.contains("Codex"), "got: {reply}");
        // fake is a legal channel too (marked as template)
        let (reply, new_cli) = cli_switch(&cfg, "fake", Some("claude"));
        assert_eq!(new_cli.as_deref(), Some("fake"));
        assert!(reply.contains("联调"), "got: {reply}");
    }

    #[test]
    fn local_command_memory_add_and_rm_roundtrip() {
        let (dir, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(
            &cfg,
            &project,
            "memory",
            "add 老板 最在意按时交付",
            None,
            true,
            &mut s,
        )
        .unwrap()
        .unwrap();
        assert!(out.reply.contains("已固定事项"), "got: {}", out.reply);
        assert!(out.reply.contains("老板"), "got: {}", out.reply);
        // view shows the pin
        let view = local_command(&cfg, &project, "memory", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(view.reply.contains("老板"), "got: {}", view.reply);
        // rm removes it
        let rm = local_command(&cfg, &project, "memory", "rm 老板", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(rm.reply.contains("已删除"), "got: {}", rm.reply);
        let view2 = local_command(&cfg, &project, "memory", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(!view2.reply.contains("老板"), "got: {}", view2.reply);
        // add with empty value → hint
        let bad = local_command(&cfg, &project, "memory", "add 孤", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(bad.reply.contains("用法"), "got: {}", bad.reply);
        drop(dir);
    }

    #[test]
    fn local_command_report_empty_state() {
        let (dir, project, cfg) = tmp_cfg();
        let mut s = sess("default");
        let out = local_command(&cfg, &project, "report", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("还没有运行记录"), "got: {}", out.reply);
        let out = local_command(&cfg, &project, "resume", "", None, true, &mut s)
            .unwrap()
            .unwrap();
        assert!(out.reply.contains("没有可恢复的运行"), "got: {}", out.reply);
        drop(dir);
    }

    #[test]
    fn slash_catalog_local_covers_all_commands() {
        let cat = slash_catalog(None);
        let locals: Vec<&str> = cat
            .iter()
            .filter(|c| c.scope == "local")
            .map(|c| c.cmd.as_str())
            .collect();
        for cmd in LOCAL_COMMANDS {
            assert!(locals.contains(&cmd), "missing local cmd /{cmd}");
        }
        assert_eq!(locals.len(), LOCAL_COMMANDS.len(), "locals: {locals:?}");
        // every local entry has a description
        for c in cat.iter().filter(|c| c.scope == "local") {
            assert!(!c.desc.is_empty(), "empty desc for /{}", c.cmd);
        }
    }

    #[test]
    fn slash_catalog_passthrough_differs_by_cli() {
        let claude = slash_catalog(Some("claude"));
        let claude_pass: Vec<&str> = claude
            .iter()
            .filter(|c| c.scope == "passthrough")
            .map(|c| c.cmd.as_str())
            .collect();
        assert!(claude_pass.contains(&"compact"), "{claude_pass:?}");
        assert!(claude_pass.contains(&"model"), "{claude_pass:?}");

        let codex = slash_catalog(Some("codex"));
        let codex_pass: Vec<&str> = codex
            .iter()
            .filter(|c| c.scope == "passthrough")
            .map(|c| c.cmd.as_str())
            .collect();
        assert!(codex_pass.contains(&"review"), "{codex_pass:?}");
        assert!(codex_pass.contains(&"doctor"), "{codex_pass:?}");
        assert!(!codex_pass.contains(&"compact"), "codex must not list claude cmds");

        // fake has no passthrough (no real CLI to route to)
        let fake = slash_catalog(Some("fake"));
        let fake_pass: Vec<&str> = fake
            .iter()
            .filter(|c| c.scope == "passthrough")
            .map(|c| c.cmd.as_str())
            .collect();
        assert!(fake_pass.is_empty(), "{fake_pass:?}");
    }

    #[test]
    fn slash_catalog_reserved_marked_for_greyout() {
        let cat = slash_catalog(None);
        for c in cat.iter().filter(|c| c.scope == "reserved") {
            assert!(matches!(c.cmd.as_str(), "run" | "stop" | "start"), "{}", c.cmd);
            assert!(c.group == "运行", "group {}", c.group);
        }
        let reserved: Vec<&str> = cat
            .iter()
            .filter(|c| c.scope == "reserved")
            .map(|c| c.cmd.as_str())
            .collect();
        assert_eq!(reserved.len(), RESERVED_COMMANDS.len(), "{reserved:?}");
    }

    #[test]
    fn help_text_is_generated_from_catalog() {
        let cat = slash_catalog(None);
        let help = help_text(None);
        // every local command appears in help
        for c in cat.iter().filter(|c| c.scope == "local") {
            assert!(help.contains(&format!("/{}", c.cmd)), "help missing /{}", c.cmd);
        }
        // passthrough must NOT leak into local help body (only the tail mentions channel)
        assert!(help.contains("/run"), "reserved guidance present");
        assert!(help.contains("透传") == false, "passthrough not in local help");
    }

    #[test]
    fn cli_switch_unknown_name_rejects() {
        let (_d, _p, cfg) = tmp_cfg();
        let (reply, new_cli) = cli_switch(&cfg, "nope", Some("claude"));
        assert!(new_cli.is_none());
        assert!(reply.contains("未知 CLI：nope"), "got: {reply}");
    }
}
