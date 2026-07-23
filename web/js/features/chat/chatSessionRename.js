/**
 * [INPUT]: legacy · chatApi · chatState · chatSessions (label/render)
 * [OUTPUT]: renameChatSession · beginChatSessionRename
 * [POS]: A5-2a features/chat；自 chatSessions 纵切（会话重命名）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast } from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { ensureChatState, stashChatSession } from "./chatState.js";
import {
  chatSessionLabel,
  renderChatSessionSelect,
} from "./chatSessions.js";

/**
 * C3: rename session title (empty string clears custom title).
 * @param {string} sessionId
 * @param {string|null|undefined} title
 */
export async function renameChatSession(sessionId, title) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return null;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再改名");
    return null;
  }
  const sid = String(sessionId || "").trim() || "default";
  const raw = title == null ? "" : String(title);
  const cleaned = raw.trim().slice(0, 80);
  try {
    const sess = await chatApi.renameSession(
      state.selectedPath,
      sid,
      cleaned || null
    );
    const nextTitle = sess?.title ?? (cleaned || null);
    if ((state.chatSession?.session_id || "default") === sid) {
      state.chatSession = {
        ...(state.chatSession || {}),
        session_id: sid,
        title: nextTitle,
      };
      stashChatSession(state.selectedPath, sid);
    }
    const list = Array.isArray(state.chatSessionList)
      ? state.chatSessionList.slice()
      : [];
    const idx = list.findIndex((r) => r.session_id === sid);
    if (idx >= 0) {
      list[idx] = {
        ...list[idx],
        title: nextTitle,
        preview: nextTitle || list[idx].preview,
      };
    } else {
      list.unshift({
        session_id: sid,
        title: nextTitle,
        message_count: (state.chatSession?.messages || []).length,
      });
    }
    state.chatSessionList = list;
    renderChatSessionSelect();
    toast(cleaned ? `已命名为「${cleaned}」` : "已清除自定义名称");
    return sess;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  }
}

/**
 * Inline rename editor inside history panel row.
 * @param {string} sessionId
 * @param {string} [seed]
 */
export function beginChatSessionRename(sessionId, seed) {
  ensureChatState();
  if (!state.selectedPath || state.chatBusy) return;
  const sid = String(sessionId || "").trim() || "default";
  const panelList = $("#chat-session-panel-list");
  if (!panelList) return;
  panelList
    .querySelectorAll(".chat-session-item.is-renaming")
    .forEach((el) => el.classList.remove("is-renaming"));
  const esc =
    typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(sid)
      : String(sid).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const row = panelList.querySelector(
    `.chat-session-item[data-session-id="${esc}"]`
  );
  if (!row) return;
  const main = row.querySelector(".chat-session-item-main");
  if (!main) return;
  row.classList.add("is-renaming");
  const initial =
    seed != null
      ? String(seed)
      : chatSessionLabel(
          (state.chatSessionList || []).find((r) => r.session_id === sid) || {
            session_id: sid,
            title: state.chatSession?.title,
          }
        );
  const titleEl = main.querySelector(".chat-session-item-title");
  const metaEl = main.querySelector(".chat-session-item-meta");
  if (titleEl) titleEl.hidden = true;
  if (metaEl) metaEl.hidden = true;
  let committed = false;
  const input = document.createElement("input");
  input.type = "text";
  input.className = "chat-session-rename-input";
  input.value = initial === "默认" && sid === "default" ? "" : initial;
  input.maxLength = 80;
  input.setAttribute("aria-label", "会话名称");
  input.placeholder = sid === "default" ? "默认" : "会话名称";
  main.insertBefore(input, main.firstChild);

  const finish = async (save) => {
    if (committed) return;
    committed = true;
    const val = input.value;
    input.removeEventListener("keydown", onKey);
    input.removeEventListener("blur", onBlur);
    try {
      input.remove();
    } catch (_) {}
    row.classList.remove("is-renaming");
    if (titleEl) titleEl.hidden = false;
    if (metaEl) metaEl.hidden = false;
    if (!save) {
      renderChatSessionSelect();
      return;
    }
    const prev = String(initial || "").trim();
    const next = String(val || "").trim();
    if (next === prev || (next === "" && (prev === "默认" || !prev))) {
      renderChatSessionSelect();
      return;
    }
    await renameChatSession(sid, next);
  };
  const onKey = (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      finish(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      finish(false);
    }
  };
  const onBlur = () => {
    setTimeout(() => finish(true), 0);
  };
  input.addEventListener("keydown", onKey);
  input.addEventListener("blur", onBlur);
  input.focus();
  input.select();
}
