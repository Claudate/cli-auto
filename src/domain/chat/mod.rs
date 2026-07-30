//! Chat pure rules (A1-6 · P2-17).
//!
//! ## Pure parse/normalize vs session IO
//! | Pure (this module) | IO (`services/chat` · future adapters) |
//! |--------------------|----------------------------------------|
//! | extract_plan_fence · extract_session_digest_fence · strip · nest fence depth | `.cco/chat/*.json` load/save/list |
//! | sanitize_plan_title · extract_title_from_md | attachments write · plan.md write |
//! | normalize/structure_plan_markdown · acceptance_quality (P1-4) · parse_acceptance_checklist / build_verification (P2-1) | Claude CLI spawn / stream poll |
//! | truncate_chars · sanitize_session_id | chat_stream_partial disk read |
//! | extract_assistant_text · stream_result_summary | TTL cleanup · path resolve |
//! | **clarify** 槽位/三入口/缺槽检测（纯） | session.clarify meta 读写 |
//! | **session_digest** 合格浅检 · prompt 块（内置压缩） | session.session_digest 持久化 |
//!
//! [INPUT]: strings only (assistant text · md · session id tokens) · clarify fill state
//! [OUTPUT]: pure transforms; **no** path join / fs / provider
//! [POS]: Domain Chat 上下文；只服务「生成计划」步，**禁止** confirm/start_run
//! [PROTOCOL]: 变更时更新此头部与 domain/CLAUDE.md

mod clarify;
mod fence;
mod id;
mod normalize;
mod plan_writing_guidance;
mod session_digest;
mod stream_parse;
mod text;
mod title;

pub use clarify::{
    apply_skip_with_assumptions, detect_missing_slots, has_assumed_fills, set_slot_fill,
    ClarifyAssumption, ClarifyEntry, ClarifyOptionalFill, ClarifyPhase, ClarifySlotFill,
    ClarifySlotId, ClarifyState, MissingSlotsReport, SlotFillKind, CLARIFY_SCHEMA_VERSION,
    REQUIRED_SLOTS,
};
pub use fence::{
    extract_all_plan_fences, extract_all_tagged_fences, extract_plan_fence,
    extract_session_digest_fence, extract_tagged_fence, extract_wave_index_fence,
    strip_session_digest_fences,
};
pub use plan_writing_guidance::{
    backend_architecture_guidance, chat_plan_writing_guidance, chat_visual_review_guidance,
    planner_greenfield_stack_blurb, split_agent_delivery_guidance, ui_color_systems_guidance,
    ui_copy_systems_guidance, ui_delivery_recipes_guidance, ui_layout_systems_guidance,
    ui_motion_effects_guidance, ui_typography_systems_guidance,
};
pub use id::{sanitize_session_id, DEFAULT_SESSION};
pub use normalize::{
    acceptance_hint, acceptance_is_stub, acceptance_quality, build_verification,
    collect_task_acceptance_items, normalize_plan_markdown, parse_acceptance_checklist,
    structure_plan_markdown, AcceptanceQuality, PlanChecklistItem, TaskAcceptanceItem,
    VerificationInputs, VerificationItem, VerificationItemStatus, VerificationSource,
    VerificationView,
};
pub use session_digest::{
    format_session_digest_prompt_block, session_digest_looks_valid, session_digest_reject_reason,
    truncate_session_digest, SESSION_DIGEST_SOFT_MAX_CHARS,
};
pub use stream_parse::{extract_assistant_text, stream_result_summary};
pub use text::truncate_chars;
pub use title::{extract_title_from_md, sanitize_plan_title, PLAN_TITLE_MAX_CHARS};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_plan_fence_last_wins() {
        let t = "intro\n```plan\n# A\n```\nmore\n```plan\n# B\nbody\n```\n";
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# B\nbody"));
    }

    #[test]
    fn extract_all_plan_fences_keeps_order() {
        let t = "```plan\n# A\na\n```\nmid\n```plan\n# B\nb\n```\n";
        let all = extract_all_plan_fences(t);
        assert_eq!(all.len(), 2);
        assert!(all[0].starts_with("# A"));
        assert!(all[1].starts_with("# B"));
        // last-wins still holds for single extract
        assert_eq!(extract_plan_fence(t).as_deref(), Some("# B\nb"));
    }

    #[test]
    fn extract_wave_index_fence_ok() {
        let t = "说明\n```wave-index\n# 本波索引\n## 计划列表\n- a\n```\n```plan\n# 执行A\n```\n";
        let idx = extract_wave_index_fence(t).expect("index");
        assert!(idx.contains("本波索引"));
        assert!(idx.contains("计划列表"));
    }

    #[test]
    fn extract_and_strip_session_digest_fence() {
        let t = "人话回复\n\n```session-digest\nschema: session-digest/v1\ngoal: 测\nconstraints:\n  - id: C1\n    text: x\n    source: s\n```\n\n```plan\n# P\n```\n";
        let dig = extract_session_digest_fence(t).expect("digest");
        assert!(dig.contains("session-digest/v1"));
        assert!(dig.contains("goal: 测"));
        let stripped = strip_session_digest_fences(t);
        assert!(
            !stripped.contains("session-digest"),
            "strip left digest: {stripped}"
        );
        assert!(stripped.contains("```plan"), "plan fence must remain");
        assert!(stripped.contains("人话回复"));
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
            "```\u{00e9}若",
            "聊天界面设计进度不合理",
            "a```b若c",
        ];
        for c in cases {
            let r = std::panic::catch_unwind(|| extract_plan_fence(c));
            assert!(r.is_ok(), "panicked on {c:?}: {r:?}");
            assert!(r.unwrap().is_none(), "unexpected plan from {c:?}");
        }
    }

    #[test]
    fn extract_plan_fence_plan_after_cjk_plain_fence() {
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
    fn extract_plan_fence_nested_text_block_keeps_full_body() {
        let t = r#"先说明

```plan
# cco 全量落地总计划

## 目标
把多轮诉求收成一条产品：

```text
选项目 → 聊天共建 → 落盘 .md
     → 计划管理 → 分配 → 跑
```

## 任务大纲
### T1 · 修 fence
- 说明：嵌套 ```text 不得截断
- 验收：全文落盘

## 验收
- [ ] 计划管理预览可见全文
```
"#;
        let got = extract_plan_fence(t).expect("plan fence");
        assert!(
            got.contains("## 任务大纲"),
            "must not stop at nested ```text; got:\n{got}"
        );
        assert!(
            got.contains("选项目 → 聊天共建"),
            "nested diagram body must be kept; got:\n{got}"
        );
        assert!(
            got.contains("- [ ] 计划管理预览可见全文"),
            "tail after nested fence must remain; got:\n{got}"
        );
        assert!(!got.trim_end().ends_with("```"));
    }

    #[test]
    fn extract_plan_fence_nested_multiple_diagrams() {
        let t = "```plan\n# A\n\n```text\none\n```\n\nmid\n\n```text\ntwo\n```\n\ntail\n```\n";
        assert_eq!(
            extract_plan_fence(t).as_deref(),
            Some("# A\n\n```text\none\n```\n\nmid\n\n```text\ntwo\n```\n\ntail")
        );
    }

    #[test]
    fn extract_plan_fence_ignores_midline_triple_backticks() {
        let t = "```plan\n# T\nuse ```inline``` sparingly\n## 尾\n```\n";
        let got = extract_plan_fence(t).expect("plan");
        assert!(got.contains("## 尾"), "got:\n{got}");
        assert!(got.contains("```inline```"), "got:\n{got}");
    }

    #[test]
    fn truncate_chars_cjk_safe() {
        let s = "若和计划".repeat(10);
        let out = truncate_chars(&s, 5);
        assert_eq!(out.chars().count(), 6);
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
    fn extract_title_cuts_jammed_single_line_wall() {
        let wall = "# cco 全局体验优化：简单主路径 · 计划管理## 目标把桌面端收成一条## 任务大纲1. 做";
        let t = extract_title_from_md(wall).expect("title");
        assert!(t.contains("全局体验优化") || t.contains("简单主路径"), "got: {t}");
        assert!(!t.contains("##"), "title must not include ##: {t}");
        assert!(!t.contains("目标把"), "title must stop before body: {t}");
        assert!(
            t.chars().count() <= PLAN_TITLE_MAX_CHARS + 1,
            "got len {}",
            t.chars().count()
        );
    }

    #[test]
    fn extract_title_clamps_long_h1() {
        let long = format!("# {}\n\nbody\n", "字".repeat(120));
        let t = extract_title_from_md(&long).expect("title");
        assert!(t.ends_with('…'), "got: {t}");
        assert!(t.chars().count() <= PLAN_TITLE_MAX_CHARS + 1);
    }

    #[test]
    fn normalize_plan_markdown_breaks_single_line_headings() {
        let wall = "# 标题短## 目标说明一下### T1 · 任务";
        let out = normalize_plan_markdown(wall);
        assert!(out.contains('\n'), "expected newlines: {out}");
        assert!(out.contains("## 目标"), "got: {out}");
        assert!(out.contains("### T1"), "got: {out}");
        let title = extract_title_from_md(&out).unwrap();
        assert_eq!(title, "标题短");
    }

    #[test]
    fn structure_plan_markdown_fills_missing_sections() {
        let thin = "# 登录优化\n\n做快点\n";
        let out = structure_plan_markdown(thin);
        assert!(out.contains("## 目标"), "got: {out}");
        assert!(out.contains("## 范围"), "got: {out}");
        assert!(out.contains("## 任务"), "got: {out}");
        assert!(out.contains("## 验收"), "got: {out}");
        assert_eq!(extract_title_from_md(&out).as_deref(), Some("登录优化"));
    }

    #[test]
    fn sanitize_session_id_strips_unsafe() {
        assert_eq!(sanitize_session_id("default"), "default");
        assert_eq!(sanitize_session_id("s-20260720-120000"), "s-20260720-120000");
        assert_eq!(sanitize_session_id("../evil"), "___evil");
        assert_eq!(sanitize_session_id(""), "default");
    }

    #[test]
    fn clarify_contract_reachable() {
        let empty = ClarifyState::default();
        let report = detect_missing_slots(&empty);
        assert!(report.missing_required.len() >= 1);
        assert_eq!(ClarifyEntry::default(), ClarifyEntry::IdeaToPlan);
        assert_eq!(REQUIRED_SLOTS.len(), 5);
        assert_eq!(ClarifyEntry::parse("从想法到计划"), Some(ClarifyEntry::IdeaToPlan));
    }
}
