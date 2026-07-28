/**
 * [INPUT]: legacy · chatApi · sessions · host · chatRender · chatPlanOps
 * [OUTPUT]: send · openChat · re-export render* / planOps*
 * [POS]: A5-2a features/chat/chatActions.js；P-ship-D 纵切 → chatRender + chatPlanOps
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  toast,
  showPage,
} from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { host } from "./host.js";
import {
  ensureChatState,
  stashChatSession,
  restoreChatSession,
  applyChatDraftFromSession,
  chatCacheKey,
} from "./chatState.js";
import { applyPlanRailVisibility } from "./planDir.js";
import {
  clearChatAttachments,
  uploadPendingAttachments,
} from "./chatAttachments.js";
import {
  loadChatSessionList,
  loadChatSession,
  startChatWaitTicker,
  stopChatWaitTicker,
} from "./chatSessions.js";
import { renderChatPage, renderChatMessages } from "./chatRender.js";
import { applyPathModeHeadStep, getPathMode } from "./chatPathMode.js";
import { applyPersonaOpener, getPersonaId } from "./chatPersona.js";
import {
  installClarifyUi,
  selectClarifyEntry,
  pickClarifyOption,
  skipClarify,
  setClarifyUiStatus,
  simulateClarifyStatus,
  getClarifyCopySnapshot,
  ensureClarifyState,
  resetClarifyState,
  hydrateClarifyFromSession,
  claimBriefToPlan,
  rechatFromBrief,
  buildBriefFromClarify,
  buildPlanMarkdownFromBrief,
  detectHollowGaps,
  fillClarifySlotsForTest,
  forceHollowForTest,
  shouldShowBrief,
  CLARIFY_COPY,
  DEFAULT_CLARIFY_ENTRY,
} from "./chatClarify.js";

// Re-export surfaces so installChat `...chatActions` stays stable.
export {
  fillChatExample,
  setPathModeAndPaint,
  setPersonaAndPaint,
  reviseChatDraft,
  renderChatMessages,
  renderChatEnvBar,
  dismissChatEnvBar,
  openChatEnvDoctor,
  renderChatReadyBar,
  renderChatPage,
} from "./chatRender.js";

// t3+t4 clarify / Brief / claim surface
export {
  selectClarifyEntry,
  pickClarifyOption,
  skipClarify,
  setClarifyUiStatus,
  simulateClarifyStatus,
  getClarifyCopySnapshot,
  ensureClarifyState,
  resetClarifyState,
  hydrateClarifyFromSession,
  claimBriefToPlan,
  rechatFromBrief,
  buildBriefFromClarify,
  buildPlanMarkdownFromBrief,
  detectHollowGaps,
  fillClarifySlotsForTest,
  forceHollowForTest,
  shouldShowBrief,
  CLARIFY_COPY,
  DEFAULT_CLARIFY_ENTRY,
};

export {
  normalizeChatDraft,
  saveChatPlan,
  assignFromChat,
  assignAndSplitFromChat,
  assignAndDirectFromChat,
  previewChatPlan,
} from "./chatPlanOps.js";

export async function openChatPage() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  // Leaving another page: keep current chat in cache first.
  if (state.chatProjectPath) stashChatSession(state.chatProjectPath);
  // 聊天右栏已撤：强制关轨
  state.planRailOpen = false;
  showPage("chat");
  // Restore immediately so history is never blank while disk loads.
  restoreChatSession(state.selectedPath);
  applyPlanRailVisibility();
  // t3: install clarify click/styles once; ensure default entry
  installClarifyUi();
  ensureClarifyState();
  // W0 / W0-7: head step + persona opener
  try {
    applyPathModeHeadStep(getPathMode());
    applyPersonaOpener(getPersonaId());
  } catch (_) {}
  // P2-2: clear stale last_summary until load
  state.chatLastSummary = null;
  renderChatPage();
  await loadChatSession();
  // Plan-card footer hides 仅保存/拆成步骤 when path is already split —
  // best-effort refresh SQLite index so reopening chat after 拆分 is correct.
  try {
    const loadIdx =
      (typeof window !== "undefined" && window.loadPlanSplitIndex) ||
      host.loadPlanSplitIndex;
    if (typeof loadIdx === "function") await loadIdx(state.selectedPath);
  } catch (_) {}
  // C3: session switcher list (best-effort)
  try {
    await loadChatSessionList();
  } catch (_) {}
  // P2-2: author empty-state last_summary line (best-effort)
  try {
    await loadChatLastSummary();
  } catch (_) {}
  // 计划列表数据在打开「计划管理」时再扫；聊天页不占右栏
  // Re-paint after split index (and session) so card CTAs match disk state.
  renderChatPage();
}

/**
 * P2-2: fetch last_summary for empty author state.
 * Honors per-project ignore flag.
 */
export async function loadChatLastSummary() {
  const project = state.selectedPath;
  if (!project) {
    state.chatLastSummary = null;
    return null;
  }
  const ignoreKey = `cco.ignoreLastSummary:${project}`;
  if (localStorage.getItem(ignoreKey) === "1") {
    state.chatLastSummary = null;
    return null;
  }
  try {
    const row = await chatApi.getLastSummary(project);
    const text =
      row && typeof row === "object"
        ? String(row.text || "").trim()
        : String(row || "").trim();
    state.chatLastSummary = text || null;
  } catch (_) {
    state.chatLastSummary = null;
  }
  // Re-paint empty state banner if still empty.
  try {
    renderChatMessages();
  } catch (_) {}
  return state.chatLastSummary;
}

/**
 * P2-2: reuse / ignore last_summary banner actions.
 * @param {"reuse"|"ignore"} action
 */
export function handleLastSummaryAction(action) {
  const project = state.selectedPath;
  if (action === "ignore") {
    if (project) {
      localStorage.setItem(`cco.ignoreLastSummary:${project}`, "1");
    }
    state.chatLastSummary = null;
    const bar = document.querySelector(".chat-last-summary");
    if (bar) bar.remove();
    toast("好的，这次从新的想法开始");
    return;
  }
  if (action === "reuse") {
    const text = String(state.chatLastSummary || "").trim();
    if (!text) return;
    const input = $("#chat-input");
    if (input) {
      const seed = `接着上次：${text}\n\n接下来我想：`;
      input.value = seed;
      input.focus();
    }
    // Keep banner until user types; optional soft hide
    const bar = document.querySelector(".chat-last-summary");
    if (bar) bar.remove();
  }
}

// Preview short-phrase intercept removed: chat always goes to Claude CLI.

export async function sendChatMessage() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const input = $("#chat-input");
  const text = (input?.value || "").trim();
  const hasAtt = (state.chatPendingAttachments || []).length > 0;
  if (!text && !hasAtt) return;
  if (state.chatBusy) return;

  const projectPath = state.selectedPath;
  state.chatProjectPath = projectPath;
  state.chatBusy = true;
  state.chatWaitStartedAt = Date.now();
  // t3: mark clarify loading while send is in flight (人话载)
  try {
    ensureClarifyState();
    if (
      state.chatClarify &&
      (state.chatClarify.phase === "not_started" ||
        state.chatClarify.phase === "clarifying")
    ) {
      setClarifyUiStatus("loading");
    }
  } catch (_) {}
  // New stream generation: drop any leftover painted text; ignore late polls
  // from a previous turn and refuse previous-turn `.done` stdout until live growth.
  state.chatStreamGen = (state.chatStreamGen || 0) + 1;
  state.chatStreamText = "";
  state.chatStreamBytes = 0;
  state.chatStreamSeenLive = false;
  if (input) {
    input.value = "";
    // Reset Codex-style auto-grow after clear
    input.style.height = "auto";
  }
  const pendingSnap = (state.chatPendingAttachments || []).slice();
  // optimistic user bubble + pending AI bubble (renderChatMessages)
  const optContent =
    text ||
    (pendingSnap.length ? `（附件 ${pendingSnap.length} 个）` : "");
  state.chatSession.messages = [
    ...(state.chatSession.messages || []),
    {
      role: "user",
      content: optContent,
      attachments: pendingSnap.map((p) => ({
        name: p.name,
        mime: p.mime,
        path: "",
        _preview: p.dataUrl,
      })),
    },
  ];
  clearChatAttachments();
  stashChatSession(projectPath);
  renderChatPage();
  startChatWaitTicker();

  try {
    // G4: upload pending attachments first, then send with attachment meta
    let attachments = [];
    if (pendingSnap.length) {
      // restore pending temporarily for upload helper
      state.chatPendingAttachments = pendingSnap;
      try {
        attachments = await uploadPendingAttachments();
      } finally {
        state.chatPendingAttachments = [];
      }
    }
    // Non-blocking for the webview: Tauri command is async + spawn_blocking.
    // User sees "思考中…" bubble; send is disabled only to avoid double-send.
    const effortEl = $("#chat-effort");
    const effortRaw = (effortEl?.value || "").trim().toLowerCase();
    const effortOk = ["low", "medium", "high", "xhigh", "max", "ultracode"].includes(
      effortRaw
    );
    const sendArgs = {
      project: projectPath,
      message: text || (attachments.length ? "（见附件）" : ""),
      sessionId: state.chatSession.session_id || "default",
      attachments: attachments.length ? attachments : null,
      effort: effortOk ? effortRaw : null,
    };
    // Persist last chat pick so reopen keeps depth
    try {
      if (effortOk) localStorage.setItem("cco.chatEffort", effortRaw);
    } catch (_) {}
    const resp = await chatApi.sendMessage(sendArgs);
    // If user switched project mid-send, still write into that project's cache.
    if (state.selectedPath !== projectPath) {
      const sid = resp.session_id || "default";
      const key = chatCacheKey(projectPath, sid);
      const snap = {
        session_id: sid,
        messages: Array.isArray(resp.messages) ? resp.messages : [],
        draft_plan: resp.draft_plan || null,
        // Unsaved new fence → null path (do not keep previous session draftPath).
        draftPath:
          resp.draft_plan?.saved && resp.draft_plan.path
            ? resp.draft_plan.path
            : null,
        fake: !!resp.fake,
        envNote: resp.env_note || null,
        busy: false,
        waitStartedAt: 0,
      };
      state.chatSessions[key] = snap;
      state.chatSessions[projectPath] = snap;
    } else {
      applyChatDraftFromSession({
        session_id: resp.session_id,
        messages: resp.messages,
        draft_plan: resp.draft_plan,
      });
      // applyChatDraftFromSession already mirrors server saved path / clears unsaved.
      // Do not re-promote a stale chatDraftPlan when the new fence is unsaved.
      // 有 markdown 时记 fake；真实 AI 成功则清掉
      state.chatFake = !!resp.fake;
      // 生产 soft-fallback：env_note 进系统条；forced fake 无 env_note 时用简短 mock 提示
      if (resp.env_note) {
        state.chatEnvNote = String(resp.env_note);
      } else if (resp.fake) {
        state.chatEnvNote = "本地模板联调（CCO_CHAT_FAKE / provider=fake）· 非真实 AI";
      } else {
        state.chatEnvNote = null;
      }
      state.chatProjectPath = projectPath;
      stashChatSession(projectPath);
      // C3: refresh switcher counts/preview after a successful turn
      try {
        await loadChatSessionList();
      } catch (_) {}
    }
    if (resp.fake) {
      if (resp.env_note) {
        toast("本机 Claude CLI 暂不可用，请查看上方环境提示");
      } else {
        toast("当前是本地模板联调（非真实 AI）");
      }
    }
  } catch (e) {
    if (state.selectedPath === projectPath) {
      state.chatSession.messages.push({
        role: "system",
        content: `发送失败：${e?.message || e}`,
      });
      // t3: product error copy for clarify phase
      try {
        setClarifyUiStatus("error");
      } catch (_) {}
      stashChatSession(projectPath);
    }
    toast(String(e?.message || e));
  } finally {
    if (state.selectedPath === projectPath) {
      state.chatBusy = false;
      state.chatWaitStartedAt = 0;
      state.chatStreamText = "";
      state.chatStreamBytes = 0;
      state.chatStreamSeenLive = false;
      // Clear loading status unless already error
      try {
        if (state.chatClarify?.uiStatus === "loading") {
          setClarifyUiStatus("idle");
        }
      } catch (_) {}
      // Bump gen so in-flight poll promises cannot re-paint into the next idle frame.
      state.chatStreamGen = (state.chatStreamGen || 0) + 1;
      stopChatWaitTicker();
      stashChatSession(projectPath);
      renderChatPage();
      input?.focus();
    } else if (state.chatSessions[projectPath]) {
      state.chatSessions[projectPath].busy = false;
      state.chatSessions[projectPath].waitStartedAt = 0;
    }
  }
}
