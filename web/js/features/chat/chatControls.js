/**
 * [INPUT]: legacy.state · composer DOM · localStorage
 * [OUTPUT]: 模型选择同步 · 本轮上下文 DisclosureRow
 * [POS]: features/chat 的 composer 控件层；只维护展示偏好与发送参数，不写会话/规划策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { state } from "./legacy.js";

const MODEL_KEY = "cco.chatModel";

function $(id) {
  return document.getElementById(id);
}

function savedModel() {
  try {
    return localStorage.getItem(MODEL_KEY) || "";
  } catch (_) {
    return "";
  }
}

function persistModel(value) {
  try {
    localStorage.setItem(MODEL_KEY, value || "");
  } catch (_) {}
}

function syncModelAvailability() {
  const cli = $("chat-cli")?.value || "claude";
  const model = $("chat-model");
  if (!model) return;
  const supported = cli === "claude";
  model.disabled = !supported;
  model.title = supported
    ? "本会话使用的模型；默认使用当前 CLI 的默认模型"
    : "当前通道不接受 Claude 模型参数";
}

export function syncChatModelFromResponse(model) {
  const value = String(model || "").trim();
  const select = $("chat-model");
  if (select && [...select.options].some((option) => option.value === value)) {
    select.value = value;
  }
  persistModel(value);
}

export function renderChatComposerContext() {
  const summary = $("chat-context-summary");
  const body = $("chat-context-body");
  if (!summary || !body) return;
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
  summary.textContent = attachmentCount
    ? "本轮上下文 · 附件 " + attachmentCount
    : "本轮上下文";
  body.textContent =
    "项目：" +
    project +
    " · 计划：" +
    planName +
    (attachmentCount ? " · 附件：" + attachmentCount : "");
}

export function installChatControls() {
  const model = $("chat-model");
  const cli = $("chat-cli");
  if (model && !model.dataset.ccoBound) {
    model.dataset.ccoBound = "1";
    const initial = savedModel();
    if ([...model.options].some((option) => option.value === initial)) {
      model.value = initial;
    }
    model.addEventListener("change", () => persistModel(model.value));
  }
  if (cli && !cli.dataset.ccoModelBound) {
    cli.dataset.ccoModelBound = "1";
    cli.addEventListener("change", syncModelAvailability);
  }
  syncModelAvailability();
  renderChatComposerContext();
}
