/**
 * [INPUT]: legacy.state · composer DOM
 * [OUTPUT]: 本轮上下文 DisclosureRow（右侧只读模型名徽标 · 仅 /model 斜杠命令切换）
 * [POS]: features/chat 的 composer 控件层；只维护展示，不写会话/规划策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { state } from "./legacy.js";

function $(id) {
  return document.getElementById(id);
}

/**
 * 会话模型只读徽标：不可点击、不可编辑；无覆盖时显示「默认」。
 * 切换唯一入口 = 聊天输入框的 /model <名称> 斜杠命令。
 */
function paintChatModelBadge() {
  const badge = $("chat-context-model");
  if (!badge) return;
  const model = String(state.chatSession?.model || "").trim();
  badge.textContent = model || "默认";
  badge.title = model
    ? `当前模型：${model} · 在输入框输入 /model <名称> 切换`
    : "使用 CLI 默认模型 · 在输入框输入 /model <名称> 切换";
}

/** `/model` 切换后由 resp.model 回流：更新会话快照与徽标（无选择器）。 */
export function syncChatModelFromResponse(model) {
  const value = String(model || "").trim();
  if (value) {
    if (!state.chatSession || typeof state.chatSession !== "object") {
      state.chatSession = {
        session_id: "default",
        messages: [],
        draft_plan: null,
      };
    }
    state.chatSession.model = value;
  }
  paintChatModelBadge();
}

export function renderChatComposerContext() {
  const summaryText = $("chat-context-summary-text");
  const body = $("chat-context-body");
  if (!summaryText || !body) return;
  const project =
    String(state.selectedPath || "").split(/[/\\]/).filter(Boolean).pop() ||
    "未选择项目";
  const plan = state.chatDraftPlan || state.selectedPlan || "";
  const planName = plan
    ? String(plan).split(/[/\\]/).pop()
    : "未绑定计划";
  const attachmentCount = Array.isArray(state.chatPendingAttachments)
    ? state.chatPendingAttachments.length
    : 0;
  summaryText.textContent = attachmentCount
    ? "本轮上下文 · 附件 " + attachmentCount
    : "本轮上下文";
  body.textContent =
    "项目：" +
    project +
    " · 计划：" +
    planName +
    (attachmentCount ? " · 附件：" + attachmentCount : "");
  paintChatModelBadge();
}

export function installChatControls() {
  const badge = $("chat-context-model");
  if (badge && !badge.dataset.ccoModelGuard) {
    badge.dataset.ccoModelGuard = "1";
    // 徽标不可点击：吞掉点击，避免触发 summary 的展开/收起
    badge.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
    });
  }
  renderChatComposerContext();
}
