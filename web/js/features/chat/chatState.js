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
  // 聊天右栏已撤：始终关；列表走 page-plans
  state.planRailOpen = false;
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
  // Stream gate: ignore leftover stdout from previous turn (done without live growth).
  if (state.chatStreamGen == null) state.chatStreamGen = 0;
  if (state.chatStreamBytes == null) state.chatStreamBytes = 0;
  if (state.chatStreamSeenLive == null) state.chatStreamSeenLive = false;
  // t3 clarify phase (presentation state; wire shape mirrors domain/chat/clarify)
  if (!state.chatClarify || typeof state.chatClarify !== "object") {
    state.chatClarify = {
      schema_version: 1,
      entry: "idea_to_plan",
      phase: "not_started",
      slots: [],
      optional: [],
      assumptions: [],
      skip_requested: false,
      uiStatus: "idle",
      errorText: null,
      questionIndex: 0,
      selectedOption: null,
    };
  }
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
    // t3: clarify presentation snapshot
    clarify: state.chatClarify
      ? {
          ...state.chatClarify,
          slots: Array.isArray(state.chatClarify.slots)
            ? state.chatClarify.slots.map((s) => ({ ...s }))
            : [],
          assumptions: Array.isArray(state.chatClarify.assumptions)
            ? state.chatClarify.assumptions.map((a) => ({ ...a }))
            : [],
        }
      : null,
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
    clarify: c.clarify ? { ...c.clarify } : state.chatSession?.clarify || null,
  };
  // Only restore path when draft is actually saved; unsaved fence must not revive
  // a stale draftPath from an older cache entry.
  const d = state.chatSession.draft_plan;
  state.chatDraftPlan =
    d?.saved && d?.path ? d.path : c.draftPath && d?.saved ? c.draftPath : null;
  state.chatFake = !!c.fake;
  state.chatEnvNote = c.envNote || null;
  // t3: restore clarify UI snapshot
  if (c.clarify && typeof c.clarify === "object") {
    state.chatClarify = {
      ...c.clarify,
      slots: Array.isArray(c.clarify.slots)
        ? c.clarify.slots.map((s) => ({ ...s }))
        : [],
      assumptions: Array.isArray(c.clarify.assumptions)
        ? c.clarify.assumptions.map((a) => ({ ...a }))
        : [],
    };
  }
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
    clarify: sess.clarify || null,
  };
  // Server draft is source of truth for plan file identity.
  // Unsaved draft (new fence after prior save) MUST clear chatDraftPlan so the
  // card does not show「已保存：旧路径」and 拆成步骤 does not reuse that path.
  if (d?.saved && d.path) {
    state.chatDraftPlan = d.path;
  } else {
    state.chatDraftPlan = null;
  }
  // t3: hydrate clarify from session meta when present —
  // never clobber a richer in-memory fill with empty/stale disk meta
  // (local option picks often land before session.clarify is persisted).
  if (sess.clarify && typeof sess.clarify === "object") {
    const diskSlots = Array.isArray(sess.clarify.slots) ? sess.clarify.slots : [];
    const memSlots = Array.isArray(state.chatClarify?.slots)
      ? state.chatClarify.slots
      : [];
    const diskFilled = diskSlots.filter((s) => String(s?.value || "").trim()).length;
    const memFilled = memSlots.filter((s) => String(s?.value || "").trim()).length;
    const diskPhase = String(sess.clarify.phase || "not_started");
    const memPhase = String(state.chatClarify?.phase || "not_started");
    const memAhead =
      memFilled > diskFilled ||
      (memFilled > 0 && diskFilled === 0) ||
      (memPhase === "clarifying" &&
        (diskPhase === "not_started" || diskPhase === "claimed_to_plan") &&
        memFilled >= diskFilled);
    if (memAhead) {
      // Keep local picks; only refresh uiStatus defaults
      if (state.chatClarify) {
        if (state.chatClarify.uiStatus == null) state.chatClarify.uiStatus = "idle";
      }
    } else {
      state.chatClarify = {
        schema_version: sess.clarify.schema_version || 1,
        entry: sess.clarify.entry || "idea_to_plan",
        phase: sess.clarify.phase || "not_started",
        slots: diskSlots.map((s) => ({ ...s })),
        optional: Array.isArray(sess.clarify.optional)
          ? sess.clarify.optional.map((o) => ({ ...o }))
          : [],
        assumptions: Array.isArray(sess.clarify.assumptions)
          ? sess.clarify.assumptions.map((a) => ({ ...a }))
          : [],
        skip_requested: !!sess.clarify.skip_requested,
        uiStatus: state.chatClarify?.uiStatus || "idle",
        errorText: state.chatClarify?.errorText || null,
        questionIndex: state.chatClarify?.questionIndex || 0,
        selectedOption: state.chatClarify?.selectedOption || null,
      };
    }
  }
}

/**
 * Load chat for the selected project.
 * - Prefer in-memory cache (page hop) so history never blanks.
 * - Skip disk reload while a send is in flight (would race and wipe optimistic msgs).
 * - Disk load uses a sequence token so stale responses cannot clobber newer state.
 */
/** C3: load session list for switcher (does not change current session). */
