/**
 * [INPUT]: legacy.state
 * [OUTPUT]: ensure · stash/restore · applyDraft · sanitize · cacheKey
 * [POS]: A5-2a features/chat/chatState.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state } from "./legacy.js";

export function chatProjectName() {
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  if (proj?.name) return proj.name;
  if (!state.selectedPath) return "";
  const parts = String(state.selectedPath).split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || state.selectedPath;
}

export function ensureChatState() {
  if (!state.chatSession) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null };
  }
  if (state.chatBusy == null) state.chatBusy = false;
  if (state.chatWaitStartedAt == null) state.chatWaitStartedAt = 0;
  if (state.chatDraftPlan === undefined) state.chatDraftPlan = null;
  if (state.chatFake == null) state.chatFake = false;
  if (state.chatEnvNote === undefined) state.chatEnvNote = null;
  // P2-2: author empty-state last_summary banner text
  if (state.chatLastSummary === undefined) state.chatLastSummary = null;
  if (!state.chatSessions) state.chatSessions = {};
  if (state.chatProjectPath === undefined) state.chatProjectPath = null;
  if (state._chatLoadSeq == null) state._chatLoadSeq = 0;
  // C3 multi-session list (ChatSessionSummary[])
  if (!Array.isArray(state.chatSessionList)) state.chatSessionList = [];
  if (state.chatSessionListLoading == null) state.chatSessionListLoading = false;
  // H1 plan-rail + full-view modal
  if (!Array.isArray(state.planRailItems)) state.planRailItems = [];
  if (state.planRailLoading == null) state.planRailLoading = false;
  if (!Array.isArray(state.planMetaItems)) state.planMetaItems = [];
  if (!state.planMetaByPath) state.planMetaByPath = {};
  // 聊天右栏：仅 icon 展开，默认关（per-project）
  if (state.planRailOpen == null) {
    const key = state.selectedPath
      ? `cco.planRailOpen:${state.selectedPath}`
      : "cco.planRailOpen";
    state.planRailOpen = localStorage.getItem(key) === "1";
  }
  if (state.planRailSelected == null) state.planRailSelected = null;
  // G1: default plans dir (project-relative), persisted per project — 仅聊天落盘
  if (state.plansDir == null) {
    const k = state.selectedPath
      ? `cco.plansDir:${state.selectedPath}`
      : "cco.plansDir";
    state.plansDir = localStorage.getItem(k) || "plans";
  }
  // 管理页列表作用域（选中的文件夹；null = 项目全量）
  if (state.plansMgmtScopeDir === undefined) state.plansMgmtScopeDir = null;
  // G4: pending attachments before send [{name,mime,dataUrl,size,isImage?}]
  if (!Array.isArray(state.chatPendingAttachments)) state.chatPendingAttachments = [];
  if (state.showExecutedPlans == null) {
    state.showExecutedPlans = localStorage.getItem("cco.showExecutedPlans") === "1";
  }
  if (state.planFull == null) {
    state.planFull = {
      open: false,
      path: null,
      title: null,
      markdown: "",
      original: "",
      editing: false,
      dirty: false,
      everCompleted: false,
      lastRunStatus: null,
      saving: false,
      // C3/P2-9: disk (left) vs current draft (right)
      diffing: false,
      diffLeft: "",
      diffRight: "",
    };
  }
  // C3 streaming partial text while chat_send runs (poll only; falls back to wait label)
  if (state.chatStreamText == null) state.chatStreamText = "";
}

/** G0: short list title from markdown H1 (cut at ## / max 80 chars). */
export function sanitizePlanTitle(raw) {
  if (!raw) return "";
  let s = String(raw).trim();
  const hashIdx = s.indexOf("##");
  if (hashIdx >= 0) s = s.slice(0, hashIdx).trimEnd();
  const nlHash = s.indexOf("\n# ");
  if (nlHash >= 0) s = s.slice(0, nlHash).trimEnd();
  s = s.trim();
  if (!s) return "";
  const chars = Array.from(s);
  if (chars.length <= 80) return s;
  return chars.slice(0, 80).join("") + "…";
}

/** G1: show/hide plan-rail; persist per project. */

export function chatCacheKey(path, sessionId) {
  const p = path || state.selectedPath || state.chatProjectPath || "";
  const sid = sessionId || state.chatSession?.session_id || "default";
  return `${p}::${sid}`;
}

/** Snapshot current chat UI into per-project+session cache (survive page/session switches). */
export function stashChatSession(path, sessionId) {
  ensureChatState();
  const p = path || state.selectedPath || state.chatProjectPath;
  if (!p) return;
  const sid = sessionId || state.chatSession?.session_id || "default";
  const key = chatCacheKey(p, sid);
  state.chatSessions[key] = {
    session_id: sid,
    messages: Array.isArray(state.chatSession?.messages)
      ? state.chatSession.messages.slice()
      : [],
    draft_plan: state.chatSession?.draft_plan
      ? { ...state.chatSession.draft_plan }
      : null,
    draftPath: state.chatDraftPlan || null,
    fake: !!state.chatFake,
    envNote: state.chatEnvNote || null,
    busy: !!state.chatBusy,
    waitStartedAt: state.chatWaitStartedAt || 0,
    title: state.chatSession?.title || null,
  };
  // Legacy single-key (project only) for older page-hop paths still reading it.
  state.chatSessions[p] = state.chatSessions[key];
}

/** Restore cached chat UI for a project (+ optional session). Returns true if cache hit. */
export function restoreChatSession(path, sessionId) {
  ensureChatState();
  const p = path || state.selectedPath;
  if (!p) return false;
  const sid =
    sessionId ||
    state.chatSession?.session_id ||
    "default";
  const key = chatCacheKey(p, sid);
  const c = state.chatSessions[key] || state.chatSessions[p];
  if (!c) return false;
  // If legacy cache has a different session, only accept when sessionId not forced.
  if (sessionId && c.session_id && c.session_id !== sessionId) {
    if (!state.chatSessions[key]) return false;
  }
  state.chatProjectPath = p;
  state.chatSession = {
    session_id: c.session_id || sid || "default",
    messages: Array.isArray(c.messages) ? c.messages.slice() : [],
    draft_plan: c.draft_plan ? { ...c.draft_plan } : null,
    title: c.title || null,
  };
  state.chatDraftPlan = c.draftPath || null;
  state.chatFake = !!c.fake;
  state.chatEnvNote = c.envNote || null;
  // Do not restore busy across project/session switches; only same-session page hops.
  if (
    state.chatBusy &&
    state.chatProjectPath === p &&
    (state.chatSession?.session_id || "default") === (c.session_id || sid)
  ) {
    /* keep in-flight send */
  } else {
    state.chatBusy = !!c.busy;
    state.chatWaitStartedAt = c.waitStartedAt || 0;
  }
  return true;
}

/** Elapsed wait label while Claude CLI runs in the background. */

export function applyChatDraftFromSession(sess) {
  ensureChatState();
  if (!sess) {
    state.chatDraftPlan = null;
    return;
  }
  const d = sess.draft_plan || null;
  state.chatSession = {
    session_id: sess.session_id || "default",
    messages: Array.isArray(sess.messages) ? sess.messages : [],
    draft_plan: d,
    title: sess.title || null,
  };
  if (d && d.saved && d.path) {
    state.chatDraftPlan = d.path;
  } else if (d && d.path && d.saved) {
    state.chatDraftPlan = d.path;
  } else {
    // keep path if previously saved in this UI session
    if (d?.path && d.saved) state.chatDraftPlan = d.path;
    else if (!d?.saved) {
      /* unsaved draft markdown only */
      if (!state.chatDraftPlan) state.chatDraftPlan = null;
    }
  }
  // Prefer server truth for saved path
  if (d?.saved && d.path) state.chatDraftPlan = d.path;
  else if (!d?.saved) {
    // do not clear a previously saved path unless server says different project load
  }
}

/**
 * Load chat for the selected project.
 * - Prefer in-memory cache (page hop) so history never blanks.
 * - Skip disk reload while a send is in flight (would race and wipe optimistic msgs).
 * - Disk load uses a sequence token so stale responses cannot clobber newer state.
 */
/** C3: load session list for switcher (does not change current session). */
