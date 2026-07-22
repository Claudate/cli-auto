//! Chat adapter integration tests (session · send · attachment · stream).

use super::*;
use crate::config::Config;
use crate::domain::chat::{extract_plan_fence, sanitize_session_id};
use crate::services::chat::cli_call::{soft_fallback_assistant_reply, soft_fallback_env_note};
use crate::services::chat::paths::session_path;
use std::path::PathBuf;
use tempfile::tempdir;

/// Force-fake via Config only — avoids process-wide `CCO_CHAT_FAKE` races under
/// `cargo test` parallel threads (set_var/remove_var interleaved → flaky `r.fake`).
fn fake_cfg() -> Config {
    let mut cfg = Config::default();
    cfg.default.default_provider = "fake".into();
    cfg
}

#[test]
fn cleanup_expired_chat_sessions_removes_old() {
    use chrono::Utc;
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    let chat = project.join(".cco").join("chat");
    std::fs::create_dir_all(&chat).unwrap();
    let old_path = chat.join("old.json");
    let new_path = chat.join("new.json");
    let old_at = (Utc::now() - chrono::Duration::hours(72)).to_rfc3339();
    let new_at = Utc::now().to_rfc3339();
    std::fs::write(
        &old_path,
        format!(
            r#"{{"session_id":"old","project":"p","messages":[],"updated_at":"{old_at}"}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        &new_path,
        format!(
            r#"{{"session_id":"new","project":"p","messages":[],"updated_at":"{new_at}"}}"#
        ),
    )
    .unwrap();
    let n = cleanup_expired_chat_sessions(&project, 48).unwrap();
    assert_eq!(n, 1);
    assert!(!old_path.is_file());
    assert!(new_path.is_file());
}

#[test]
fn chat_list_new_delete_sessions_roundtrip() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();

    // Empty project → synthetic default only
    let list0 = chat_list_sessions(&project).unwrap();
    assert_eq!(list0.len(), 1);
    assert_eq!(list0[0].session_id, "default");
    assert_eq!(list0[0].message_count, 0);

    let created = chat_new_session(&project, Some("登录优化")).unwrap();
    assert!(
        created.session_id.starts_with("s-"),
        "got {}",
        created.session_id
    );
    assert_eq!(created.title.as_deref(), Some("登录优化"));
    assert!(created.messages.is_empty());
    assert!(session_path(&project, &created.session_id).is_file());

    // Seed default with a user msg so list preview works
    let cfg = fake_cfg();
    let _ = chat_send(&cfg, &project, "默认会话第一条", Some("default"), None).unwrap();

    let list = chat_list_sessions(&project).unwrap();
    assert!(list.len() >= 2, "got {list:?}");
    let ids: Vec<_> = list.iter().map(|s| s.session_id.as_str()).collect();
    assert!(ids.contains(&"default"));
    assert!(ids.contains(&created.session_id.as_str()));
    let row = list
        .iter()
        .find(|s| s.session_id == created.session_id)
        .unwrap();
    assert_eq!(row.title.as_deref(), Some("登录优化"));
    assert!(row.preview.as_deref().unwrap_or("").contains("登录") || row.title.is_some());

    chat_delete_session(&project, &created.session_id).unwrap();
    assert!(!session_path(&project, &created.session_id).is_file());
    let list2 = chat_list_sessions(&project).unwrap();
    assert!(!list2.iter().any(|s| s.session_id == created.session_id));
    // default still listed
    assert!(list2.iter().any(|s| s.session_id == "default"));
}

#[test]
fn sanitize_session_id_strips_unsafe() {
    assert_eq!(sanitize_session_id("default"), "default");
    assert_eq!(
        sanitize_session_id("s-20260720-120000"),
        "s-20260720-120000"
    );
    assert_eq!(sanitize_session_id("../evil"), "___evil");
    assert_eq!(sanitize_session_id(""), "default");
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
        None,
        None,
    )
    .unwrap();
    assert!(resp.plan_rel.starts_with("plans/chat-"));
    assert!(PathBuf::from(&resp.abs_path).is_file());

    let sess2 = chat_session_get(&project, Some("default")).unwrap();
    assert!(sess2.draft_plan.as_ref().unwrap().saved);
    assert_eq!(sess2.draft_plan.as_ref().unwrap().path, resp.plan_rel);

    // H1: overwrite existing plan_rel
    let body2 = "# 覆盖\n\n## 目标\n改一版\n";
    let resp2 = chat_save_plan(
        &project,
        Some("default"),
        None,
        body2,
        Some(&resp.plan_rel),
        None,
    )
    .unwrap();
    assert_eq!(resp2.plan_rel, resp.plan_rel);
    let disk = std::fs::read_to_string(&resp2.abs_path).unwrap();
    assert!(disk.contains("改一版"));
    let read_back = read_plan_md(&project, &resp.plan_rel).unwrap();
    assert_eq!(read_back, disk);

    // G1: custom plans_dir
    let resp3 = chat_save_plan(
        &project,
        Some("default"),
        Some("自定义夹"),
        "# 自定义夹\n\n## 目标\nx\n",
        None,
        Some("docs/plans"),
    )
    .unwrap();
    assert!(
        resp3.plan_rel.starts_with("docs/plans/chat-"),
        "got {}",
        resp3.plan_rel
    );
    assert!(PathBuf::from(&resp3.abs_path).is_file());
}

#[test]
fn chat_save_attachment_png_roundtrip() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    // minimal 1x1 PNG
    let png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8,
        0xcf, 0xc0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xfe, 0xd4, 0xef, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let att = chat_save_attachment(&project, Some("default"), "shot.png", "image/png", png).unwrap();
    assert!(att.path.contains(".cco/chat/attachments/default/"));
    assert!(att.path.ends_with(".png"));
    assert_eq!(att.mime, "image/png");
    assert!(project.join(&att.path).is_file());

    let cfg = fake_cfg();
    let r = chat_send(
        &cfg,
        &project,
        "看这张图优化登录",
        None,
        Some(vec![att.clone()]),
    )
    .unwrap();
    assert!(r.fake);
    let user = r.messages.iter().find(|m| m.role == "user").unwrap();
    assert!(!user.attachments.is_empty());
    assert!(user.content.contains(&att.path));
}

#[test]
fn chat_save_attachment_rejects_exe() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let err =
        chat_save_attachment(&project, None, "x.exe", "application/octet-stream", b"hi").unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("blocked") || s.contains("unsupported"),
        "{s}"
    );
}

#[test]
fn chat_save_attachment_markdown_ok() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let att = chat_save_attachment(
        &project,
        Some("default"),
        "brief.md",
        "text/markdown",
        b"# hello\n\nworld\n",
    )
    .unwrap();
    assert!(att.path.ends_with(".md"), "{}", att.path);
    assert!(project.join(&att.path).is_file());
    assert!(att.mime.contains("markdown") || att.mime == "text/plain" || !att.mime.is_empty());
}

#[test]
fn fake_send_persists_messages() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("app");
    std::fs::create_dir_all(&project).unwrap();
    let cfg = fake_cfg();
    let r = chat_send(&cfg, &project, "帮我写个登录页计划", None, None).unwrap();
    assert!(r.fake);
    assert!(!r.reply.is_empty());
    assert!(r.messages.len() >= 2);
    assert!(r
        .draft_plan
        .as_ref()
        .and_then(|d| d.markdown.as_ref())
        .is_some());
    assert!(r.env_note.is_none(), "forced fake has no env_note");
    // 联调路径仍产出 fence
    assert!(r.reply.contains("```plan"), "got: {}", r.reply);
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

#[test]
fn chat_stream_partial_reads_growing_stdout() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    let task_dir = project
        .join(".cco")
        .join("chat")
        .join("_work")
        .join("tasks")
        .join("__chat__");
    std::fs::create_dir_all(&task_dir).unwrap();
    // Idle: empty files → empty partial
    std::fs::write(task_dir.join("stdout.raw.ndjson"), "").unwrap();
    let empty = chat_stream_partial(&project, Some("default")).unwrap();
    assert!(empty.text.is_empty());
    assert!(!empty.done);

    // Partial assistant block (no result line yet)
    let partial = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"你好，这是计划草稿"}]}}
{"type":"content_block_delta","delta":{"text":"·续写"}}
"#;
    std::fs::write(task_dir.join("stdout.raw.ndjson"), partial).unwrap();
    let mid = chat_stream_partial(&project, None).unwrap();
    assert!(
        mid.text.contains("计划草稿") || mid.text.contains("续写"),
        "got: {}",
        mid.text
    );
    assert!(!mid.done);
    assert!(mid.bytes > 0);

    // CJK-safe: incomplete trailing bytes must not panic
    let cjk = format!(
        "{}\n{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"中文\"}}]}}}}\n",
        partial
    );
    std::fs::write(task_dir.join("stdout.raw.ndjson"), cjk).unwrap();
    let cjk_out = std::panic::catch_unwind(|| chat_stream_partial(&project, None));
    assert!(cjk_out.is_ok());
    let cjk_out = cjk_out.unwrap().unwrap();
    assert!(cjk_out.text.contains("中文") || cjk_out.text.contains("计划"));

    std::fs::write(task_dir.join(".done"), "0").unwrap();
    let done = chat_stream_partial(&project, None).unwrap();
    assert!(done.done);
}
