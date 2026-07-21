/**
 * [INPUT]: legacy · chatState · format · sessions · attachments · host
 * [OUTPUT]: renderChat* · fillChatExample · env-bar helpers
 * [POS]: A5-2a features/chat；自 chatActions 纵切（P-ship-D）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  toast,
  showPage,
  planTemplateChatEmptyHtml,
  openDoctorPage,
  runDoctor,
  loadDoctor,
} from "./legacy.js";
import { host } from "./host.js";
import {
  ensureChatState,
  chatProjectName,
  stashChatSession,
} from "./chatState.js";
import { chatEsc, chatFormatBody } from "./chatFormat.js";
import { renderChatAttachPreview } from "./chatAttachments.js";
import {
  renderChatSessionSelect,
  chatWaitLabel,
} from "./chatSessions.js";

export function fillChatExample(text) {
  const input = $("#chat-input");
  if (!input || !state.selectedPath) return;
  input.value = text;
  input.focus();
}

export function renderChatMessages() {
  const list = $("#chat-messages");
  if (!list) return;
  ensureChatState();
  const msgs = state.chatSession.messages || [];
  if (!msgs.length && !state.chatBusy) {
    // T2: empty state + 模板入口委托 ccoTemplates / planTemplateChatEmptyHtml（不在此堆功能）
    list.innerHTML =
      typeof planTemplateChatEmptyHtml === "function"
        ? planTemplateChatEmptyHtml()
        : `<div class="chat-empty muted"><p>用自然语言说明你要做什么，保存后再点「拆成步骤」。</p></div>`;
    return;
  }
  // Only the last assistant message's plan card gets save/execute CTAs
  let lastAssistantIdx = -1;
  for (let i = msgs.length - 1; i >= 0; i--) {
    if (msgs[i]?.role === "assistant") {
      lastAssistantIdx = i;
      break;
    }
  }
  let html = msgs
    .map((m, mi) => {
      const role = m.role === "assistant" ? "assistant" : m.role === "system" ? "system" : "user";
      const label = role === "assistant" ? "AI" : role === "system" ? "系统" : "我";
      const atts = Array.isArray(m.attachments) ? m.attachments : [];
      const attHtml = atts.length
        ? `<div class="chat-msg-atts">${atts
            .map((a) => {
              const src = a._preview || "";
              const name = chatEsc(a.name || a.path || "图");
              if (src) {
                return (
                  `<div class="chat-msg-att">` +
                  `<img class="chat-img-zoomable" src="${src}" alt="${name}" data-img-src="${chatEsc(src)}" data-img-name="${name}" title="点击放大" />` +
                  `<span>${name}</span></div>`
                );
              }
              return `<div class="chat-msg-att chat-msg-att-path" title="${chatEsc(a.path || "")}">📎 ${name}</div>`;
            })
            .join("")}</div>`
        : "";
      const activePlan = role === "assistant" && mi === lastAssistantIdx;
      return `<div class="chat-msg chat-msg-${role}">
        <div class="chat-msg-role">${label}</div>
        <div class="chat-msg-body">${chatFormatBody(m.content || "", { activePlan })}${attHtml}</div>
      </div>`;
    })
    .join("");
  // Waiting bubble: user already sent; UI must stay responsive while CLI runs.
  // C3: if stream partial arrived, show it in place of the wait label.
  if (state.chatBusy) {
    const stream = String(state.chatStreamText || "").trim();
    if (stream) {
      const shown =
        stream.length > 6000 ? "…\n" + stream.slice(-6000) : stream;
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending chat-msg-streaming">${chatEsc(
        shown
      )}<span class="chat-stream-cursor" aria-hidden="true">▍</span></div>
    </div>`;
    } else {
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending">
        <span class="chat-pending-dots" aria-hidden="true"></span>
        ${chatEsc(chatWaitLabel())}
      </div>
    </div>`;
    }
  }
  list.innerHTML = html;
  list.scrollTop = list.scrollHeight;
}

export function renderChatEnvBar() {
  const bar = $("#chat-env-bar");
  if (!bar) return;
  ensureChatState();
  const note = state.chatEnvNote;
  // forced fake 联调也可显示简短 mock 条；有 env_note 优先
  const show = !!(note && String(note).trim());
  bar.hidden = !show;
  const noteEl = $("#chat-env-note");
  if (noteEl && show) noteEl.textContent = String(note).trim();
}

export function dismissChatEnvBar() {
  state.chatEnvNote = null;
  stashChatSession(state.selectedPath || state.chatProjectPath);
  renderChatEnvBar();
}

export function openChatEnvDoctor() {
  try {
    if (typeof showPage === "function") showPage("doctor");
    else if (typeof openDoctorPage === "function") openDoctorPage();
  } catch (_) {
    toast("请从侧栏打开「环境检查」");
  }
  try {
    if (typeof runDoctor === "function") runDoctor();
    else if (typeof loadDoctor === "function") loadDoctor();
  } catch (_) {}
}

/**
 * Sticky ready-bar retired: save / re-save / execute live only on the plan card
 * footer inside the assistant reply (bottom of that message). Keep this function
 * so old call sites stay safe; always hide the bar and its fixed buttons.
 */
export function renderChatReadyBar() {
  const bar = $("#chat-ready-bar");
  if (bar) {
    bar.hidden = true;
    bar.classList.remove("is-fake");
  }
  const saveBtn = $("#btn-chat-save");
  const assignBtn = $("#btn-chat-assign");
  const previewBtn = $("#btn-chat-preview");
  const normalizeBtn = $("#btn-chat-normalize");
  if (saveBtn) saveBtn.hidden = true;
  if (assignBtn) assignBtn.hidden = true;
  if (previewBtn) previewBtn.hidden = true;
  if (normalizeBtn) normalizeBtn.hidden = true;
}

export function renderChatPage() {
  const projLabel = $("#chat-project-label");
  if (projLabel) {
    projLabel.textContent = state.selectedPath
      ? chatProjectName()
      : "未选择项目";
  }
  const input = $("#chat-input");
  const sendBtn = $("#btn-chat-send");
  const attachBtn = $("#btn-chat-attach");
  if (input) {
    // Keep the composer editable while waiting so the app never feels frozen;
    // only the send button is gated (double-send guard).
    input.disabled = !state.selectedPath;
    input.placeholder = !state.selectedPath
      ? "请先在左侧选择项目"
      : state.chatBusy
        ? "AI 正在回复，可先写下一条…"
        : "说清目标与约束；可附图；满意后让 AI 生成计划…";
  }
  if (sendBtn) {
    // Disabled while waiting = prevent double-send, NOT app freeze.
    // Backend chat_send runs on a worker thread so the rest of the UI stays live.
    sendBtn.disabled = !state.selectedPath || !!state.chatBusy;
    sendBtn.textContent = state.chatBusy ? "思考中…" : "发送";
    sendBtn.title = state.chatBusy
      ? "正在等待本机 Claude CLI 回复，请稍候"
      : "发送消息";
  }
  if (attachBtn) {
    attachBtn.disabled = !state.selectedPath || !!state.chatBusy;
  }
  renderChatSessionSelect();
  renderChatAttachPreview();
  renderChatMessages();
  renderChatEnvBar();
  renderChatReadyBar();
  host.renderPlanRail();
  host.renderPlanFullView();
}
