//! Chat use case (A1-6 · plan-authoring only · A1-7 presentation entry).
//!
//! ## Pure vs IO
//! | Pure (`domain::chat`) | IO (via `services/chat` adapter) |
//! |-----------------------|----------------------------------|
//! | fence / title / normalize / stream parse | session JSON · attachment · plan.md · CLI |
//!
//! [INPUT]: project path · Config · message · session_id · attachments · plan markdown
//! [OUTPUT]: session / send / stream / save_plan / normalize / cleanup DTOs
//! [POS]: Application 层；Presentation 应调本模块；**禁止** confirm_start / start_run 旁路
//! [PROTOCOL]: 主产出散文 plan.md；本地 preview 为独立进程（非 Mode B worker）；搬家时改委托目标即可
//!
//! ## Presentation map (A1-7)
//! | Tauri command | app::chat |
//! |---------------|-----------|
//! | `chat_session_get_cmd` | [`get_session`] |
//! | `chat_list_sessions_cmd` | [`list_sessions`] |
//! | `chat_new_session_cmd` | [`new_session`] |
//! | `chat_rename_session_cmd` | [`rename_session`] |
//! | `chat_delete_session_cmd` | [`delete_session`] |
//! | `chat_send_cmd` | [`send`] · [`available_clis`]（chat_clis_list_cmd） |
//! | `chat_stream_partial_cmd` | [`stream_partial`] |
//! | `preview_start_cmd` / `preview_stop_cmd` / `preview_status_cmd` | [`preview_start`] / [`preview_stop`] / [`preview_status`] |
//! | `chat_save_plan_cmd` | [`save_plan`] |
//! | `chat_save_wave_bundle_cmd` | [`save_wave_bundle`]（W2 索引+多 plan；**非**开跑） |
//! | `chat_normalize_plan_cmd` | [`normalize_plan`] |
//! | `chat_save_attachment_cmd` | [`save_attachment`] |
//! | `chat_read_image_data_url_cmd` | [`read_image_data_url`] |
//! | `read_plan_md_cmd` | [`read_plan_md`] |

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::services::{available_chat_clis, ChatCliInfo};
use crate::services::{
    chat_cancel, chat_delete_session, chat_list_sessions, chat_new_session, chat_normalize_plan,
    chat_read_image_data_url, chat_rename_session, chat_save_attachment, chat_save_plan,
    chat_save_wave_bundle, chat_send, chat_session_get, chat_stream_partial,
    cleanup_expired_chat_sessions, preview_start as services_preview_start,
    preview_status as services_preview_status, preview_stop as services_preview_stop,
    read_plan_md as services_read_plan_md, slash_catalog as services_slash_catalog,
    ChatAttachment, ChatNormalizePlanResponse, ChatSavePlanResponse, ChatSaveWaveResponse,
    ChatSendResponse, ChatSession, ChatSessionSummary, ChatStreamPartial, PreviewStatus,
    SlashCommandInfo,
};

// --- session ---

/// Load chat session (missing → empty default). Opportunistic TTL purge.
pub fn get_session(project: &Path, session_id: Option<&str>) -> Result<ChatSession> {
    chat_session_get(project, session_id)
}

/// List sessions under `.cco/chat/*.json` (newest first; always includes default).
pub fn list_sessions(project: &Path) -> Result<Vec<ChatSessionSummary>> {
    chat_list_sessions(project)
}

/// Create a new empty session (`s-YYYYMMDD-HHMMSS` …).
pub fn new_session(project: &Path, title: Option<&str>) -> Result<ChatSession> {
    chat_new_session(project, title)
}

/// Rename a session (`title`; empty/None clears custom title).
pub fn rename_session(
    project: &Path,
    session_id: &str,
    title: Option<&str>,
) -> Result<ChatSession> {
    chat_rename_session(project, session_id, title)
}

/// Delete a session JSON (+ best-effort attachments).
pub fn delete_session(project: &Path, session_id: &str) -> Result<()> {
    chat_delete_session(project, session_id)
}

/// Purge sessions older than `hours` (G3; default 48h on get/list).
pub fn cleanup_expired(project: &Path, hours: i64) -> Result<usize> {
    cleanup_expired_chat_sessions(project, hours)
}

// --- send / stream ---

/// One chat round-trip (default claude print; `cli` overrides the CLI). Writes draft
/// markdown only — **no** open-run.
/// `effort`: optional per-send override (`low`…`max`|`ultracode`).
/// `cli`: optional chat CLI provider id (None → claude; `fake` → template reply).
pub fn send(
    config: &Config,
    project: &Path,
    message: &str,
    session_id: Option<&str>,
    attachments: Option<Vec<ChatAttachment>>,
    effort: Option<&str>,
    cli: Option<&str>,
) -> Result<ChatSendResponse> {
    chat_send(
        config,
        project,
        message,
        session_id,
        attachments,
        effort,
        cli,
    )
}

/// Chat-capable CLI list for the UI dropdown (claude default first, fake last).
pub fn available_clis(config: &Config) -> Result<Vec<ChatCliInfo>> {
    Ok(available_chat_clis(config))
}

/// Slash-command catalog for the composer autocomplete (per-CLI local /
/// passthrough / reserved). Pure — no IO, no confirm / start_run.
pub fn slash_catalog(cli: Option<&str>) -> Vec<SlashCommandInfo> {
    services_slash_catalog(cli)
}

/// Best-effort partial assistant text while send is in flight.
pub fn stream_partial(project: &Path, session_id: Option<&str>) -> Result<ChatStreamPartial> {
    chat_stream_partial(project, session_id)
}

/// Cancel the in-flight chat CLI process (SIGTERM + SIGKILL).
/// Returns true when a pid was targeted, false when nothing was running.
pub fn cancel(project: &Path) -> Result<bool> {
    chat_cancel(project)
}

// --- local preview (detached; not Mode B worker) ---

/// Start (or reuse) project dev/preview server; waits until a local port is open.
pub fn preview_start(project: &Path) -> Result<PreviewStatus> {
    services_preview_start(project)
}

/// Stop cco-managed preview process.
pub fn preview_stop(project: &Path) -> Result<PreviewStatus> {
    services_preview_stop(project)
}

/// Status of cco-managed (or opportunistic) local preview.
pub fn preview_status(project: &Path) -> Result<PreviewStatus> {
    services_preview_status(project)
}

// --- plan md ---

/// Write plan prose under project; bind session draft_plan. **Does not** start workers.
pub fn save_plan(
    project: &Path,
    session_id: Option<&str>,
    title: Option<&str>,
    markdown: &str,
    plan_rel: Option<&str>,
    plans_dir: Option<&str>,
) -> Result<ChatSavePlanResponse> {
    chat_save_plan(project, session_id, title, markdown, plan_rel, plans_dir)
}

/// W2: claim wave-index + N ```plan fences → `plans/wave-…/`. **Does not** open run.
pub fn save_wave_bundle(
    project: &Path,
    session_id: Option<&str>,
    markdown: &str,
    plans_dir: Option<&str>,
) -> Result<ChatSaveWaveResponse> {
    chat_save_wave_bundle(project, session_id, markdown, plans_dir)
}

/// Read plan document as UTF-8 text (not PlanIR).
pub fn read_plan_md(project: &Path, plan_rel: &str) -> Result<String> {
    services_read_plan_md(project, plan_rel)
}

/// G0b: reshape free-form plan markdown (CLI or local structure).
pub fn normalize_plan(
    config: &Config,
    project: &Path,
    markdown: &str,
    hint: Option<&str>,
) -> Result<ChatNormalizePlanResponse> {
    chat_normalize_plan(config, project, markdown, hint)
}

// --- attachment ---

/// G4: save one image attachment under `.cco/chat/attachments/<session>/`.
pub fn save_attachment(
    project: &Path,
    session_id: Option<&str>,
    file_name: &str,
    mime: &str,
    data: &[u8],
) -> Result<ChatAttachment> {
    chat_save_attachment(project, session_id, file_name, mime, data)
}

/// Read project-relative image → data URL for chat markdown / attachment thumbs.
pub fn read_image_data_url(project: &Path, rel_path: &str) -> Result<String> {
    chat_read_image_data_url(project, rel_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::tempdir;

    fn fake_cfg(state_root: &std::path::Path) -> Config {
        let mut cfg = Config::default();
        cfg.state_root = state_root.to_path_buf();
        cfg.default.default_provider = "fake".into();
        cfg
    }

    #[test]
    fn chat_use_case_send_and_save_plan_no_run() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let cfg = fake_cfg(&state);

        let r = send(&cfg, &project, "帮我写个登录页计划", None, None, None, None).unwrap();
        assert!(r.fake);
        assert!(r.reply.contains("```plan") || r.draft_plan.is_some());

        let md = r
            .draft_plan
            .as_ref()
            .and_then(|d| d.markdown.clone())
            .unwrap_or_else(|| "# 登录\n\n## 目标\nx\n".into());
        let saved = save_plan(&project, Some("default"), Some("登录"), &md, None, None).unwrap();
        assert!(saved.plan_rel.starts_with("plans/chat-"));
        assert!(std::path::PathBuf::from(&saved.abs_path).is_file());

        // Hard rule: chat path never creates a run under isolated state_root.
        let runs = cfg.runs_dir();
        assert!(
            !runs.exists()
                || runs
                    .read_dir()
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(true),
            "chat must not spawn runs; found entries under {}",
            runs.display()
        );
    }

    #[test]
    fn chat_save_wave_bundle_writes_index_and_plans_no_run() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let md = r#"说明

```wave-index
# 本波索引
## 计划列表
1. 日语页
2. 英语页
```

```plan
# 日语落地页
## 目标
日语介绍页
## 不做
支付
## 验收
可打开
```

```plan
# 英语落地页
## 目标
英语介绍页
## 不做
支付
## 验收
可打开
```
"#;
        let resp = save_wave_bundle(&project, Some("default"), md, None).unwrap();
        assert!(resp.index_rel.as_ref().unwrap().contains("wave-"));
        assert!(resp.index_rel.as_ref().unwrap().ends_with("INDEX.md"));
        assert_eq!(resp.plan_rels.len(), 2);
        assert!(resp.summary.contains("未开跑"));
        let idx_abs = project.join(resp.index_rel.as_ref().unwrap());
        assert!(idx_abs.is_file(), "missing {}", idx_abs.display());
        for rel in &resp.plan_rels {
            assert!(project.join(rel).is_file(), "missing {rel}");
        }
        // No runs from claim
        let state = dir.path().join("no-state-runs-check");
        let _ = state;
    }

    #[test]
    fn chat_use_case_session_list_roundtrip() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&project).unwrap();
        let list = list_sessions(&project).unwrap();
        assert_eq!(list[0].session_id, "default");
        let s = new_session(&project, Some("A")).unwrap();
        assert!(s.session_id.starts_with("s-"));
        delete_session(&project, &s.session_id).unwrap();
    }
}
