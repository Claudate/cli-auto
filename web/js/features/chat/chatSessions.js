/**
 * [INPUT]: legacy · chatApi · chatState · host.renderChatPage
 * [OUTPUT]: sessions · stream ticker
 * [POS]: A5-2a features/chat/chatSessions.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast } from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { host } from "./host.js";
import {
  ensureChatState,
  chatCacheKey,
  stashChatSession,
  restoreChatSession,
  applyChatDraftFromSession,
} from "./chatState.js";
import { chatEsc, chatFormatStreamBody } from "./chatFormat.js";

export function chatWaitLabel() {
  const started = state.chatWaitStartedAt || 0;
  if (!started) return "AI 正在思考…";
  const sec = Math.max(0, Math.floor((Date.now() - started) / 1000));
  if (sec < 5) return "AI 正在思考…";
  if (sec < 60) return `AI 正在思考…（已等 ${sec}s）`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `AI 正在思考…（已等 ${m}分${s}s，可稍候）`;
}

/** Paint the pending assistant bubble (wait label or streaming partial as rendered md). */
export function paintChatPendingBubble() {
  const pending = document.querySelector(".chat-msg-pending .chat-msg-body");
  if (!pending) return;
  const stream = String(state.chatStreamText || "").trim();
  if (stream) {
    pending.classList.add("chat-msg-streaming", "md-body");
    pending.classList.remove("chat-msg-body-wait-only");
    // Stream as rendered markdown (not raw source / "template" text).
    pending.innerHTML =
      chatFormatStreamBody(stream) +
      '<span class="chat-stream-cursor" aria-hidden="true">▍</span>';
  } else {
    pending.classList.remove("chat-msg-streaming", "md-body");
    pending.classList.add("chat-msg-body-wait-only");
    pending.innerHTML =
      `<span class="chat-pending-dots" aria-hidden="true"></span>` +
      chatEsc(chatWaitLabel());
  }
}

/**
 * C3: poll stdout partial while chat_send runs.
 * Must NOT paint previous-turn leftover (stdout still on disk until BE clears it).
 * Gates: generation id · ignore done-without-live · allow reset when bytes shrink.
 */
export async function pollChatStreamPartial() {
  if (!state.chatBusy || !state.selectedPath) return;
  const gen = state.chatStreamGen || 0;
  try {
    const resp = await chatApi.streamPartial({
      project: state.selectedPath,
      sessionId: state.chatSession?.session_id || "default",
    });
    // Drop late responses from a prior send (or after finally cleared gen).
    if (!state.chatBusy || (state.chatStreamGen || 0) !== gen) return;

    const text = String(resp?.text || "").trim();
    const bytes = Number(resp?.bytes) || 0;
    const done = !!resp?.done;
    const prevBytes = Number(state.chatStreamBytes) || 0;
    const prevText = String(state.chatStreamText || "");

    // File wiped for new turn (or still empty): clear any painted stale content.
    if (bytes === 0 || !text) {
      if (prevText && bytes < prevBytes) {
        state.chatStreamText = "";
        state.chatStreamSeenLive = false;
      }
      state.chatStreamBytes = bytes;
      return;
    }

    // Leftover from last completed turn: .done still true and we never saw
    // this turn grow live. Wait until BE clears / new NDJSON arrives.
    if (done && !state.chatStreamSeenLive && !prevText) {
      state.chatStreamBytes = bytes;
      return;
    }

    // Bytes shrank → new turn overwrote the file; reset stream buffer.
    if (prevBytes > 0 && bytes + 32 < prevBytes) {
      state.chatStreamText = text;
      state.chatStreamSeenLive = true;
      state.chatStreamBytes = bytes;
      return;
    }

    // Accept growth (or first live chunk). Never shrink text mid-turn unless
    // bytes already proved a file reset above.
    if (!prevText || text.length >= prevText.length) {
      state.chatStreamText = text;
      state.chatStreamSeenLive = true;
      state.chatStreamBytes = Math.max(prevBytes, bytes);
    } else if (bytes > prevBytes) {
      // Extract may flicker shorter while raw grows — keep longer text, track bytes.
      state.chatStreamSeenLive = true;
      state.chatStreamBytes = bytes;
    }
  } catch (_) {
    // Soft degrade: leave wait label; final reply still comes from chat_send.
  }
}

let _chatWaitTick = null;
let _chatStreamTick = null;
export function startChatWaitTicker() {
  stopChatWaitTicker();
  paintChatPendingBubble();
  _chatWaitTick = setInterval(() => {
    if (!state.chatBusy) {
      stopChatWaitTicker();
      return;
    }
    // Refresh only the pending bubble + send label without full re-render of history.
    paintChatPendingBubble();
    const sendBtn = $("#btn-chat-send");
    if (sendBtn && state.chatBusy) {
      sendBtn.textContent = state.chatStreamText ? "生成中…" : "思考中…";
    }
  }, 1000);
  // Stream poll slightly faster than wait label (best-effort; spawn_blocking free).
  _chatStreamTick = setInterval(() => {
    if (!state.chatBusy) return;
    pollChatStreamPartial().then(() => paintChatPendingBubble()).catch(() => {});
  }, 700);
  // First poll immediately so early deltas show up.
  pollChatStreamPartial().then(() => paintChatPendingBubble()).catch(() => {});
}
export function stopChatWaitTicker() {
  if (_chatWaitTick) {
    clearInterval(_chatWaitTick);
    _chatWaitTick = null;
  }
  if (_chatStreamTick) {
    clearInterval(_chatStreamTick);
    _chatStreamTick = null;
  }
}
export async function loadChatSessionList() {
  ensureChatState();
  if (!state.selectedPath) {
    state.chatSessionList = [{ session_id: "default", title: null, message_count: 0 }];
    renderChatSessionSelect();
    return;
  }
  state.chatSessionListLoading = true;
  try {
    const list = await chatApi.listSessions(state.selectedPath);
    state.chatSessionList = Array.isArray(list) ? list : [];
    if (!state.chatSessionList.length) {
      state.chatSessionList = [
        { session_id: "default", title: null, message_count: 0 },
      ];
    }
  } catch (e) {
    console.warn("chat_list_sessions failed", e);
    if (!state.chatSessionList?.length) {
      state.chatSessionList = [
        {
          session_id: state.chatSession?.session_id || "default",
          title: state.chatSession?.title || null,
          message_count: (state.chatSession?.messages || []).length,
        },
      ];
    }
  } finally {
    state.chatSessionListLoading = false;
    renderChatSessionSelect();
  }
}

export function chatSessionLabel(row) {
  if (!row) return "默认";
  const id = row.session_id || "default";
  if (id === "default") {
    const t = row.title || row.preview || row.draft_plan_title;
    return t ? `默认 · ${t}` : "默认";
  }
  const t = row.title || row.preview || row.draft_plan_title;
  if (t) return t;
  // Compact id: s-20260720-153045 → 07-20 15:30
  const m = /^s-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(id);
  if (m) return `${m[2]}-${m[3]} ${m[4]}:${m[5]}`;
  return id;
}

/** 收起「会话…」面板（选中/新建/删除/点空白后） */
export function collapseChatSessionMore() {
  const el = document.getElementById("chat-session-more");
  if (el && el.tagName === "DETAILS") el.open = false;
}

export function renderChatSessionSelect() {
  ensureChatState();
  const sel = $("#chat-session-select");
  const delBtn = $("#btn-chat-session-del");
  const newBtn = $("#btn-chat-session-new");
  if (!sel) return;
  const cur = state.chatSession?.session_id || "default";
  const list = Array.isArray(state.chatSessionList) ? state.chatSessionList : [];
  // Ensure current is in options even if list lagging
  const ids = new Set(list.map((r) => r.session_id));
  const rows = list.slice();
  if (!ids.has(cur)) {
    rows.unshift({
      session_id: cur,
      title: state.chatSession?.title || null,
      message_count: (state.chatSession?.messages || []).length,
    });
  }
  const prev = sel.value;
  sel.innerHTML = rows
    .map((r) => {
      const id = chatEsc(r.session_id || "default");
      const label = chatEsc(chatSessionLabel(r));
      const n = r.message_count != null ? r.message_count : 0;
      const suffix = n > 0 ? ` (${n})` : "";
      return `<option value="${id}">${label}${suffix}</option>`;
    })
    .join("");
  sel.value = ids.has(cur) || rows.some((r) => r.session_id === cur) ? cur : rows[0]?.session_id || "default";
  // If value set failed (missing option), force
  if (sel.value !== cur && rows.some((r) => r.session_id === cur)) {
    sel.value = cur;
  }
  void prev;
  sel.disabled = !state.selectedPath || !!state.chatBusy || !!state.chatSessionListLoading;
  if (newBtn) newBtn.disabled = !state.selectedPath || !!state.chatBusy;
  if (delBtn) {
    // Allow delete always when a session exists; default clears to empty
    delBtn.disabled =
      !state.selectedPath ||
      !!state.chatBusy ||
      (!list.length && cur === "default" && !(state.chatSession?.messages || []).length);
    delBtn.title =
      cur === "default"
        ? "清空默认会话（删除磁盘记录）"
        : "删除当前会话";
  }
}

/** C3: switch to another session id (stash current, load target). */
export async function switchChatSession(sessionId) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const sid = String(sessionId || "default").trim() || "default";
  const cur = state.chatSession?.session_id || "default";
  if (sid === cur && state.chatProjectPath === state.selectedPath) {
    renderChatSessionSelect();
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再切换会话");
    renderChatSessionSelect();
    return;
  }
  stashChatSession(state.selectedPath, cur);
  state.chatSession = { session_id: sid, messages: [], draft_plan: null, title: null };
  state.chatDraftPlan = null;
  state.chatFake = false;
  state.chatEnvNote = null;
  // Prefer cache for instant paint
  restoreChatSession(state.selectedPath, sid);
  host.renderChatPage();
  await loadChatSession({ force: true });
  await loadChatSessionList();
  collapseChatSessionMore();
}

/** C3: create empty session and switch to it. */
export async function newChatSession() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再新建");
    return;
  }
  const cur = state.chatSession?.session_id || "default";
  stashChatSession(state.selectedPath, cur);
  try {
    const sess = await chatApi.newSession(state.selectedPath, null);
    const sid = sess?.session_id || "default";
    state.chatSession = {
      session_id: sid,
      messages: Array.isArray(sess?.messages) ? sess.messages : [],
      draft_plan: sess?.draft_plan || null,
      title: sess?.title || null,
    };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    state.chatProjectPath = state.selectedPath;
    stashChatSession(state.selectedPath, sid);
    toast(`已新建会话`);
    host.renderChatPage();
    await loadChatSessionList();
    collapseChatSessionMore();
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** C3: delete current session (confirm), then switch to default or first remaining. */
export async function deleteChatSession() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再删");
    return;
  }
  const sid = state.chatSession?.session_id || "default";
  const label = chatSessionLabel(
    (state.chatSessionList || []).find((r) => r.session_id === sid) || {
      session_id: sid,
      title: state.chatSession?.title,
    }
  );
  const ok = window.confirm(
    sid === "default"
      ? "清空默认会话的聊天记录与草稿绑定？计划文件本身不会删除。"
      : `删除会话「${label}」？计划文件本身不会删除。`
  );
  if (!ok) return;
  try {
    await chatApi.deleteSession(state.selectedPath, sid);
    // Drop cache for this session
    const key = chatCacheKey(state.selectedPath, sid);
    delete state.chatSessions[key];
    if (state.chatSessions[state.selectedPath]?.session_id === sid) {
      delete state.chatSessions[state.selectedPath];
    }
    // Switch to default (or list head after refresh)
    state.chatSession = {
      session_id: "default",
      messages: [],
      draft_plan: null,
      title: null,
    };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    toast(sid === "default" ? "已清空默认会话" : "已删除会话");
    await loadChatSessionList();
    const next =
      (state.chatSessionList || []).find((r) => r.session_id === "default")
        ?.session_id ||
      state.chatSessionList?.[0]?.session_id ||
      "default";
    await switchChatSession(next);
    collapseChatSessionMore();
  } catch (e) {
    toast(String(e?.message || e));
  }
}

export async function loadChatSession(opts) {
  ensureChatState();
  const force = !!(opts && opts.force);
  if (!state.selectedPath) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null, title: null };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    state.chatProjectPath = null;
    state.chatSessionList = [];
    host.renderChatPage();
    return;
  }
  const path = state.selectedPath;

  // Leaving a different project's chat: stash previous, then restore/load this one.
  if (state.chatProjectPath && state.chatProjectPath !== path) {
    stashChatSession(state.chatProjectPath);
  }

  // Same project + in-flight send: never re-fetch (keeps optimistic user + pending bubble).
  if (!force && state.chatBusy && state.chatProjectPath === path) {
    host.renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    return;
  }

  // Page hop back to same project: restore cache first so UI is never empty,
  // then optionally refresh from disk if we have no local messages yet.
  if (state.chatProjectPath === path && (state.chatSession?.messages || []).length) {
    stashChatSession(path);
    host.renderChatPage();
    // Background refresh only when idle and not forced skip.
    if (!force && !state.chatBusy) {
      /* keep showing cache; soft refresh below still runs for disk truth */
    } else {
      return;
    }
  } else if (restoreChatSession(path) && (state.chatSession?.messages || []).length) {
    host.renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    // Fall through to soft disk refresh when idle so multi-device/disk edits land.
    if (state.chatBusy) return;
  }

  const seq = ++state._chatLoadSeq;
  const sid = state.chatSession?.session_id || "default";
  try {
    const sess = await chatApi.getSession(path, sid);
    // Stale or project switched mid-flight → drop.
    if (seq !== state._chatLoadSeq || state.selectedPath !== path) return;
    // In-flight send still owns the UI.
    if (state.chatBusy && state.chatProjectPath === path) {
      host.renderChatPage();
      return;
    }

    const diskMsgs = Array.isArray(sess?.messages) ? sess.messages : [];
    const memMsgs = state.chatSession?.messages || [];
    // Prefer longer history (disk after successful send, or mem if send just finished
    // and disk lag / concurrent get). Never replace a non-empty mem with empty disk
    // unless force.
    const takeDisk =
      force ||
      diskMsgs.length > memMsgs.length ||
      (diskMsgs.length === memMsgs.length && diskMsgs.length > 0) ||
      memMsgs.length === 0;

    if (takeDisk) {
      applyChatDraftFromSession(sess);
      if (sess?.draft_plan?.saved && sess.draft_plan.path) {
        state.chatDraftPlan = sess.draft_plan.path;
      } else if (!sess?.draft_plan?.saved) {
        // Keep mem draftPath if we only had unsaved markdown; clear only when empty mem.
        if (!state.chatDraftPlan || memMsgs.length === 0) {
          state.chatDraftPlan = null;
        }
      }
    }
    state.chatProjectPath = path;
    stashChatSession(path);
  } catch (e) {
    console.warn("chat_session_get failed", e);
    if (!state.chatSession?.messages?.length) {
      // Last resort: cache for this path if any.
      if (!restoreChatSession(path)) {
        state.chatSession = { session_id: "default", messages: [], draft_plan: null };
        state.chatFake = false;
      }
    }
    state.chatProjectPath = path;
  }
  host.renderChatPage();
  if (state.chatBusy) startChatWaitTicker();
}
