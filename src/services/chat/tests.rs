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
    let _ = chat_send(&cfg, &project, "默认会话第一条", Some("default"), None, None).unwrap();

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

    // Rename
    let renamed = chat_rename_session(&project, &created.session_id, Some("  新名称  ")).unwrap();
    assert_eq!(renamed.title.as_deref(), Some("新名称"));
    let list_r = chat_list_sessions(&project).unwrap();
    let row_r = list_r
        .iter()
        .find(|s| s.session_id == created.session_id)
        .unwrap();
    assert_eq!(row_r.title.as_deref(), Some("新名称"));
    // Clear title
    let cleared = chat_rename_session(&project, &created.session_id, Some("")).unwrap();
    assert!(cleared.title.is_none());

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
        None,
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
    let r = chat_send(&cfg, &project, "帮我写个登录页计划", None, None, None).unwrap();
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
    // Fresh fence is never "already saved" to a plan path.
    let d = r.draft_plan.as_ref().unwrap();
    assert!(!d.saved, "new fence must be unsaved");
    assert!(d.path.is_empty(), "new fence must not bind a plan path");
}

/// Regression: after saving plan A, a later ```plan fence must drop path/saved
/// so save creates a new file instead of overwriting A (pilotdeck-style bug).
#[test]
fn new_fence_clears_saved_plan_path() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let cfg = fake_cfg();

    let saved = chat_save_plan(
        &project,
        Some("default"),
        Some("旧计划 PilotDeck"),
        "# 旧计划 PilotDeck\n\n## 目标\n历史落地\n",
        Some("docs/pilotdeck-borrow-landing-2026-07-21.md"),
        None,
    )
    .unwrap();
    assert_eq!(
        saved.plan_rel,
        "docs/pilotdeck-borrow-landing-2026-07-21.md"
    );
    let sess = chat_session_get(&project, Some("default")).unwrap();
    let d0 = sess.draft_plan.as_ref().unwrap();
    assert!(d0.saved);
    assert_eq!(d0.path, saved.plan_rel);

    let r = chat_send(&cfg, &project, "另起一份 Markdown 清理计划", None, None, None).unwrap();
    let d = r
        .draft_plan
        .as_ref()
        .expect("fake send yields ```plan draft");
    assert!(
        d.markdown.as_ref().map(|m| !m.is_empty()).unwrap_or(false),
        "draft has markdown"
    );
    assert!(
        !d.saved,
        "new fence must clear saved; got saved={}",
        d.saved
    );
    assert!(
        d.path.is_empty(),
        "new fence must clear path (was {}); got {}",
        saved.plan_rel,
        d.path
    );

    // Disk identity of the old plan must be untouched by chat_send.
    let old_disk = read_plan_md(&project, &saved.plan_rel).unwrap();
    assert!(
        old_disk.contains("旧计划") || old_disk.contains("历史落地"),
        "old plan body must remain; got:\n{old_disk}"
    );

    // Saving without plan_rel creates a *new* chat-*.md, not pilotdeck.
    let md = d.markdown.as_ref().unwrap();
    let resp2 = chat_save_plan(&project, Some("default"), None, md, None, None).unwrap();
    assert!(
        resp2.plan_rel.starts_with("plans/chat-"),
        "expected new chat-*.md, got {}",
        resp2.plan_rel
    );
    assert_ne!(resp2.plan_rel, saved.plan_rel);
    let still = read_plan_md(&project, &saved.plan_rel).unwrap();
    assert!(
        still.contains("旧计划") || still.contains("历史落地"),
        "old plan still intact after new save"
    );
}

/// Polluted session (old build kept path/saved while markdown diverged) must heal on load.
#[test]
fn session_get_heals_stale_draft_path() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(project.join("docs")).unwrap();
    let plan_rel = "docs/pilotdeck-borrow-landing-2026-07-21.md";
    let abs = project.join(plan_rel);
    std::fs::write(&abs, "# PilotDeck 真源\n\n## 目标\n历史落地\n").unwrap();

    // Write a dirty session JSON like the pre-fix desktop bug.
    // Use r## so markdown H1 `"# title` does not terminate the raw string.
    let chat = project.join(".cco").join("chat");
    std::fs::create_dir_all(&chat).unwrap();
    let sess_json = format!(
        r##"{{
  "session_id": "default",
  "project": "{proj}",
  "messages": [],
  "draft_plan": {{
    "path": "{plan}",
    "title": "仓库 Markdown 清理",
    "markdown": "# 仓库 Markdown 清理\n\n## 目标\n删误放\n",
    "saved": true
  }},
  "updated_at": "{updated_at}"
}}"##,
        proj = project.display(),
        plan = plan_rel,
        // Must be within DEFAULT_CHAT_RETENTION_HOURS (48h) or cleanup wipes the fixture.
        updated_at = chrono::Utc::now().to_rfc3339()
    );
    std::fs::write(chat.join("default.json"), sess_json).unwrap();

    let sess = chat_session_get(&project, Some("default")).unwrap();
    let d = sess.draft_plan.as_ref().expect("draft");
    assert!(!d.saved, "stale binding must clear saved");
    assert!(d.path.is_empty(), "stale binding must clear path, got {}", d.path);
    assert!(
        d.markdown.as_ref().unwrap().contains("Markdown 清理"),
        "markdown body kept"
    );
    // Disk plan file untouched.
    let disk = std::fs::read_to_string(&abs).unwrap();
    assert!(disk.contains("PilotDeck"));

    // Healed session persisted.
    let again = chat_session_get(&project, Some("default")).unwrap();
    let d2 = again.draft_plan.as_ref().unwrap();
    assert!(!d2.saved);
    assert!(d2.path.is_empty());
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

/// Clarify meta on session: entry + slots survive save/load; legacy JSON without clarify still loads.
#[test]
fn session_clarify_meta_roundtrip_and_legacy_compat() {
    use crate::domain::chat::{
        apply_skip_with_assumptions, detect_missing_slots, set_slot_fill, ClarifyEntry,
        ClarifyPhase, ClarifySlotId, ClarifyState, SlotFillKind,
    };
    use super::session::save_session;
    use super::types::{ChatMessage, ChatSession};

    let dir = tempdir().unwrap();
    let project = dir.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();

    // Legacy disk JSON (no clarify field) must deserialize cleanly.
    let legacy_path = session_path(&project, "legacy");
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy_path,
        r#"{"session_id":"legacy","project":"p","messages":[],"updated_at":"2026-07-20T00:00:00Z"}"#,
    )
    .unwrap();
    let legacy = chat_session_get(&project, Some("legacy")).unwrap();
    assert!(legacy.clarify.is_none(), "legacy sessions keep clarify=None");

    // Fresh empty session: no clarify until set.
    let empty = chat_session_get(&project, Some("default")).unwrap();
    assert!(empty.clarify.is_none());

    // Persist a clarify state through save_session / get.
    let mut clarify = ClarifyState::new(ClarifyEntry::IdeaToPlan);
    assert!(set_slot_fill(
        &mut clarify,
        ClarifySlotId::TargetAudience,
        "出海运营",
        SlotFillKind::Explicit
    ));
    apply_skip_with_assumptions(&mut clarify, Some("你定"));
    assert_eq!(clarify.phase, ClarifyPhase::SkippedToPlan);
    let report = detect_missing_slots(&clarify);
    assert!(report.may_proceed_with_assumptions);
    assert!(report.missing_required.is_empty());

    let mut sess = ChatSession {
        session_id: "s-clarify".into(),
        project: project.display().to_string(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "想做一个提醒浇水的小工具".into(),
            at: None,
            attachments: vec![],
        }],
        draft_plan: None,
        updated_at: None,
        title: Some("浇水工具".into()),
        clarify: Some(clarify.clone()),
    };
    save_session(&project, &sess).unwrap();

    let loaded = chat_session_get(&project, Some("s-clarify")).unwrap();
    let c = loaded.clarify.as_ref().expect("clarify meta persisted");
    assert_eq!(c.entry, ClarifyEntry::IdeaToPlan);
    assert_eq!(c.phase, ClarifyPhase::SkippedToPlan);
    assert!(c.skip_requested);
    assert_eq!(
        c.slot(ClarifySlotId::TargetAudience)
            .map(|s| s.value.as_str()),
        Some("出海运营")
    );
    assert_eq!(
        c.slot(ClarifySlotId::TargetAudience).map(|s| s.kind),
        Some(SlotFillKind::Explicit)
    );
    // Assumed slots labeled, not forged as explicit facts
    for id in [
        ClarifySlotId::PainMoment,
        ClarifySlotId::ObservableOutcome,
        ClarifySlotId::NonGoals,
        ClarifySlotId::DoneWhen,
    ] {
        let fill = c.slot(id).expect("assumed fill");
        assert_eq!(fill.kind, SlotFillKind::Assumed);
        assert!(fill.value.contains("假设"), "got {}", fill.value);
    }
    // Entry enum wire keys stable
    let entry_json = serde_json::to_string(&c.entry).unwrap();
    assert_eq!(entry_json, "\"idea_to_plan\"");
    let entry_back: ClarifyEntry = serde_json::from_str(&entry_json).unwrap();
    assert_eq!(entry_back, ClarifyEntry::IdeaToPlan);

    // Three-entry labels parse consistently with meta.
    for (label, want) in [
        ("想清楚再说", ClarifyEntry::ThinkFirst),
        ("从想法到计划", ClarifyEntry::IdeaToPlan),
        ("已想清，直接写计划", ClarifyEntry::PlanOnly),
    ] {
        assert_eq!(ClarifyEntry::parse(label), Some(want));
    }

    // Overwrite entry to PlanOnly and round-trip again.
    sess = loaded;
    if let Some(ref mut cl) = sess.clarify {
        cl.entry = ClarifyEntry::PlanOnly;
        cl.phase = ClarifyPhase::ClaimedToPlan;
    }
    save_session(&project, &sess).unwrap();
    let loaded2 = chat_session_get(&project, Some("s-clarify")).unwrap();
    assert_eq!(
        loaded2.clarify.as_ref().map(|c| c.entry),
        Some(ClarifyEntry::PlanOnly)
    );
    assert_eq!(
        loaded2.clarify.as_ref().map(|c| c.phase),
        Some(ClarifyPhase::ClaimedToPlan)
    );
}
