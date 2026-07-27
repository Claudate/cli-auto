//! Chat wire / session DTO shapes (session JSON · Tauri IPC · app facade).

use serde::{Deserialize, Serialize};

use crate::domain::chat::ClarifyState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachment {
    /// Project-relative path (e.g. .cco/chat/attachments/default/uuid.png)
    pub path: String,
    pub mime: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
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
    /// C3: optional human label for multi-session switcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Clarify-phase meta (entry · slots · assumptions). Coexists with messages/draft;
    /// absent on legacy sessions → None (not a second Planner).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarify: Option<ClarifyState>,
}

/// C3: lightweight row for multi-session list (no full messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionSummary {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub message_count: usize,
    /// Short preview: first user line or draft title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_plan_title: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatStreamPartial {
    /// Accumulated assistant prose so far (may be empty while CLI starts).
    pub text: String,
    /// True when worker left a `.done` marker (turn finished or aborted).
    pub done: bool,
    /// Raw stdout bytes observed (for UI "still growing" hint).
    pub bytes: u64,
}

/// Response of G0b normalize (CLI or local).
#[derive(Debug, Clone, Serialize)]
pub struct ChatNormalizePlanResponse {
    pub markdown: String,
    pub title: Option<String>,
    /// true when CLI was used; false = local structure only
    pub used_cli: bool,
}
