//! Chat → plan document builder (desktop Mode B front-door) — A1-6 multi-file adapter.
//!
//! ## Pure parse vs session IO vs CLI spawn
//! | Pure (`domain::chat`) | Session / plan IO (this mod) | CLI spawn |
//! |-----------------------|------------------------------|-----------|
//! | extract_plan_fence · title · normalize | session JSON · attachment · plan.md | cli_call (claude print) |
//! | extract_assistant_text · truncate | stream partial disk read | normalize optional CLI |
//! | sanitize_session_id | TTL cleanup 48h | soft-fallback / fake template |
//!
//! [INPUT]: project path · user message · optional session_id · Config (provider bin)
//! [OUTPUT]: chat_session_get · list/new/delete · chat_send · chat_save_plan · read_plan_md · stream_partial
//! [POS]: services 适配器 / thin facade；主产出散文 .md；chat CLI 可本项目内启动验收（bypassPermissions）· **不**走 confirm_start 业务 worker
//! [PROTOCOL]: 变更时更新此头部，然后检查 src/services/CLAUDE.md
//! note: empty CLI reply → soft human note (no plan fence); CCO_CHAT_FAKE keeps template fence
//! note: non-zero CLI exit still yields text when stream has assistant prose (max_turns etc.)
//! note: ChatSendResponse.env_note for UI system bar (diagnostics never in assistant body)
//! note: extract_plan_fence / history truncate 必须 char-boundary 安全（CJK 禁字节硬切）
//! note: extract_plan_fence 嵌套 fence 按行首 depth 计数（```text 图示不得截断 ```plan）
//! note: chat_save_plan 可选 plan_rel 覆盖已有未执行计划；read_plan_md 供右轨全文 modal
//! note: chat_send 新 ```plan fence 必清 draft.path/saved（新正文 = 新身份，禁止挂到旧 plan_rel）
//! note: G0 plan 标题截断（H1 遇 ## / 最长 80 字）+ 写盘换行规范化；G0b 可选 CLI 再整理
//! note: G4 chat_save_attachment · ChatMessage.attachments；chat_save_plan 可选 plans_dir
//! note: C3 多会话：list/new/delete/rename + 可选 title；默认 session_id=default 仍兼容
//! note: C3 流式 partial：chat_stream_partial 读 stdout 增量；失败降级整段 reply（不 panic）
//! note: chat_send 入口 clear_chat_stream_work，防新一轮 poll 把上一条整段当流式刷出

mod attachment;
mod cli_call;
mod normalize;
mod paths;
mod plan_md;
mod send;
mod session;
mod stream;
mod types;

#[cfg(test)]
mod tests;

pub use attachment::chat_save_attachment;
pub use normalize::chat_normalize_plan;
pub use plan_md::{chat_save_plan, read_plan_md};
pub use send::chat_send;
pub use session::{
    chat_delete_session, chat_list_sessions, chat_new_session, chat_rename_session,
    chat_session_get, cleanup_expired_chat_sessions,
};
pub use stream::chat_stream_partial;
pub use types::{
    ChatAttachment, ChatDraftPlan, ChatMessage, ChatNormalizePlanResponse, ChatSavePlanResponse,
    ChatSendResponse, ChatSession, ChatSessionSummary, ChatStreamPartial,
};

// Domain pure surface re-exported for stable `crate::services::chat::*` / services facade call sites.
// `extract_plan_fence` is part of the public pure API (used by tests and callers via re-export).
#[allow(unused_imports)] // re-export surface; used outside this module
pub use crate::domain::chat::{
    extract_plan_fence, extract_title_from_md, normalize_plan_markdown, sanitize_plan_title,
    structure_plan_markdown,
};
