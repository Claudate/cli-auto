/**
 * [INPUT]: all chat modules + planRail compatibility data API
 * [OUTPUT]: register host · public desk API for window.ccoChat
 * [POS]: A5-2a features/chat/installChat.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { register, host } from "./host.js";
import * as chatState from "./chatState.js";
import * as planDir from "./planDir.js";
import * as planRail from "./planRail.js";
import * as plansMgmt from "./plansMgmt.js";
import * as chatAttachments from "./chatAttachments.js";
import * as chatFormat from "./chatFormat.js";
import * as chatSessions from "./chatSessions.js";
import * as chatSessionRename from "./chatSessionRename.js";
import * as chatActions from "./chatActions.js";
import * as planFull from "./planFull.js";
import * as chatApi from "./chatApi.js";
import * as chatClarify from "./chatClarify.js";
import * as chatMode from "./chatMode.js";
import * as chatMsgEnhance from "./chatMsgEnhance.js";
import { installChatControls } from "./chatControls.js";
import { createChatViewModel } from "./ChatViewModel.js";

function renderPlanRailCompat() {
  const s = typeof window !== "undefined" ? window.state : null;
  if (s?.page === "plans" && typeof host.renderPlansMgmtPage === "function") {
    host.renderPlansMgmtPage();
  }
}

function loadPlanRailCompat() {
  return planRail.loadPlanItems();
}

function paintModeAfter(fn) {
  if (typeof fn !== "function") return fn;
  return (...args) => {
    const out = fn(...args);
    try {
      chatMode.paintChatMode();
    } catch (_) {}
    return out;
  };
}

/** Wire host bag once (idempotent). */
export function installChatHost() {
  register({
    ...chatState,
    ...planDir,
    ...planRail,
    loadPlanRail: loadPlanRailCompat,
    renderPlanRail: renderPlanRailCompat,
    ...plansMgmt,
    ...chatAttachments,
    ...chatFormat,
    ...chatSessions,
    ...chatSessionRename,
    ...chatActions,
    ...planFull,
    ...chatClarify,
    ...chatMsgEnhance,
    ...chatMode,
  });
  // t3: bind clarify click once at host install
  try {
    chatClarify.installClarifyUi();
  } catch (_) {}
  // F1: mode chips above composer
  try {
    chatMode.installChatModeUi();
  } catch (_) {}
  // Direct paint hook — skip/pick must not rely only on host-bag order
  // (missing renderChatMessages made skip look like toast-only).
  try {
    if (typeof chatClarify.setClarifyPaint === "function") {
      chatClarify.setClarifyPaint(() => {
        if (typeof chatActions.renderChatMessages === "function") {
          chatActions.renderChatMessages({ force: true });
        } else if (typeof host.renderChatMessages === "function") {
          host.renderChatMessages({ force: true });
        }
        try {
          chatMode.paintChatMode();
        } catch (_) {}
      });
    }
  } catch (_) {}
  // Keep mode bar in sync when page/messages paint via host
  try {
    if (!host.__modePaintWrapped) {
      if (host.renderChatPage) {
        host.renderChatPage = paintModeAfter(host.renderChatPage);
      }
      if (host.renderChatMessages) {
        host.renderChatMessages = paintModeAfter(host.renderChatMessages);
      }
      host.__modePaintWrapped = true;
    }
  } catch (_) {}
  return host;
}

/**
 * Full desk surface for window.ccoChat (classic stubs + main.js).
 * No confirm_start / start_run.
 */
export function createChatDesk(opts = {}) {
  installChatHost();
  installChatControls();
  const vm = createChatViewModel({
    projectPath: opts.projectPath || null,
  });

  const desk = {
    vm,
    api: chatApi,
    host,
    // sessions / send / save (A2 + A5)
    listSessions: (project) => chatSessions.loadChatSessionList(),
    send: (args) => chatActions.sendChatMessage(),
    // Note: sendChatMessage reads from DOM; bridge below accepts args
    async sendMessage(args) {
      const s = typeof window !== "undefined" ? window.state : null;
      if (args?.project && s) s.selectedPath = args.project;
      if (args?.sessionId && s?.chatSession) s.chatSession.session_id = args.sessionId;
      if (args?.message != null) {
        const input = document.getElementById("chat-input");
        if (input) input.value = args.message;
      }
      // attachments: leave pending if already set
      return chatActions.sendChatMessage();
    },
    savePlan: (args) => chatActions.saveChatPlan(args || {}),
    // classic globals surface
    ensureChatState: chatState.ensureChatState,
    stashChatSession: chatState.stashChatSession,
    restoreChatSession: chatState.restoreChatSession,
    openChatPage: chatActions.openChatPage,
    renderChatPage: chatActions.renderChatPage,
    renderChatMessages: chatActions.renderChatMessages,
    sendChatMessage: chatActions.sendChatMessage,
    cancelChatMessage: chatActions.cancelChatMessage,
    saveChatPlan: chatActions.saveChatPlan,
    assignFromChat: chatActions.assignFromChat,
    assignAndSplitFromChat: chatActions.assignAndSplitFromChat,
    assignAndDirectFromChat: chatActions.assignAndDirectFromChat,
    previewChatPlan: chatActions.previewChatPlan,
    normalizeChatDraft: chatActions.normalizeChatDraft,
    toggleChatPlanExpand: chatFormat.toggleChatPlanExpand,
    adoptChatPlanFromCard: chatFormat.adoptChatPlanFromCard,
    dismissChatPlanFromCard: chatFormat.dismissChatPlanFromCard,
    copyChatMessageFromBtn: chatFormat.copyChatMessageFromBtn,
    loadChatSession: chatSessions.loadChatSession,
    loadChatSessionList: chatSessions.loadChatSessionList,
    switchChatSession: chatSessions.switchChatSession,
    newChatSession: chatSessions.newChatSession,
    renameChatSession: chatSessionRename.renameChatSession,
    beginChatSessionRename: chatSessionRename.beginChatSessionRename,
    deleteChatSession: chatSessions.deleteChatSession,
    addChatAttachments: chatAttachments.addChatAttachments,
    removeChatAttachment: chatAttachments.removeChatAttachment,
    handleChatPaste: chatAttachments.handleChatPaste,
    pickChatAttachments: chatAttachments.pickChatAttachments,
    openImageLightbox: chatAttachments.openImageLightbox,
    closeImageLightbox: chatAttachments.closeImageLightbox,
    fillChatExample: chatActions.fillChatExample,
    setPathMode: chatActions.setPathModeAndPaint,
    setPersona: chatActions.setPersonaAndPaint,
    setPersonaAndPaint: chatActions.setPersonaAndPaint,
    reviseChatDraft: chatActions.reviseChatDraft,
    claimWaveBundle: chatActions.claimWaveBundle,
    handleLastSummaryAction: chatActions.handleLastSummaryAction,
    loadChatLastSummary: chatActions.loadChatLastSummary,
    // t3+t4 clarify phase (入口/卡片/Brief/认领/黄条)
    selectClarifyEntry: chatClarify.selectClarifyEntry,
    pickClarifyOption: chatClarify.pickClarifyOption,
    skipClarify: chatClarify.skipClarify,
    setClarifyUiStatus: chatClarify.setClarifyUiStatus,
    simulateClarifyStatus: chatClarify.simulateClarifyStatus,
    getClarifyCopySnapshot: chatClarify.getClarifyCopySnapshot,
    ensureClarifyState: chatClarify.ensureClarifyState,
    resetClarifyState: chatClarify.resetClarifyState,
    hydrateClarifyFromSession: chatClarify.hydrateClarifyFromSession,
    claimBriefToPlan: chatClarify.claimBriefToPlan,
    rechatFromBrief: chatClarify.rechatFromBrief,
    buildBriefFromClarify: chatClarify.buildBriefFromClarify,
    buildPlanMarkdownFromBrief: chatClarify.buildPlanMarkdownFromBrief,
    detectHollowGaps: chatClarify.detectHollowGaps,
    fillClarifySlotsForTest: chatClarify.fillClarifySlotsForTest,
    forceHollowForTest: chatClarify.forceHollowForTest,
    shouldShowBrief: chatClarify.shouldShowBrief,
    renderHollowBarHtml: chatClarify.renderHollowBarHtml,
    CLARIFY_COPY: chatClarify.CLARIFY_COPY,
    // F1 双模式 chip（setMode 只写 entry；fast 首 send 见 prepareFastSendIfNeeded）
    setChatMode: chatMode.setChatMode,
    getChatMode: chatMode.getChatMode,
    paintChatMode: chatMode.paintChatMode,
    prepareFastSendIfNeeded: chatMode.prepareFastSendIfNeeded,
    renderClarifySecondaryHtml: chatMode.renderClarifySecondaryHtml,
    ensureModeDefault: chatMode.ensureModeDefault,
    CHAT_MODES: chatMode.CHAT_MODES,
    // 可点选 A/B/C + 历史折叠
    pickChatQuizOption: chatMsgEnhance.pickChatQuizOption,
    fillChatQuizDraft: chatMsgEnhance.fillChatQuizDraft,
    sendChatQuizDraft: chatMsgEnhance.sendChatQuizDraft,
    unfoldChatMessage: chatMsgEnhance.unfoldChatMessage,
    foldChatMessage: chatMsgEnhance.foldChatMessage,
    expandChatMsgBody: chatMsgEnhance.expandChatMsgBody,
    collapseChatMsgBody: chatMsgEnhance.collapseChatMsgBody,
    dismissChatEnvBar: chatActions.dismissChatEnvBar,
    openChatEnvDoctor: chatActions.openChatEnvDoctor,
    toggleChatPlanRail: planDir.toggleChatPlanRail,
    setPlanRailOpen: planDir.setPlanRailOpen,
    getPlansDir: planDir.getPlansDir,
    setPlansDir: planDir.setPlansDir,
    promptPlansDir: planDir.promptPlansDir,
    pickPlansFolderForMgmt: planDir.pickPlansFolderForMgmt,
    pickPlanFileForMgmt: planDir.pickPlanFileForMgmt,
    getPlansMgmtScopeDir: planDir.getPlansMgmtScopeDir,
    setPlansMgmtScopeDir: planDir.setPlansMgmtScopeDir,
    isPathInPlansDir: planDir.isPathInPlansDir,
    openPlanManagement: plansMgmt.openPlanManagement,
    renderPlansMgmtPage: plansMgmt.renderPlansMgmtPage,
    selectPlansMgmtItem: plansMgmt.selectPlansMgmtItem,
    openPlansMgmtItem: plansMgmt.openPlansMgmtItem,
    assignFromPlansMgmt: plansMgmt.assignFromPlansMgmt,
    viewSplitFromPlansMgmt: plansMgmt.viewSplitFromPlansMgmt,
    viewSplitFromPlanRail: planRail.viewSplitFromPlanRail,
    loadPlanItems: planRail.loadPlanItems,
    loadPlanRail: loadPlanRailCompat,
    renderPlanRail: renderPlanRailCompat,
    selectPlanRailItem: planRail.selectPlanRailItem,
    openPlanRailItem: planRail.openPlanRailItem,
    openPlanFullView: planFull.openPlanFullView,
    closePlanFullView: planFull.closePlanFullView,
    beginPlanFullEdit: planFull.beginPlanFullEdit,
    cancelPlanFullEdit: planFull.cancelPlanFullEdit,
    onPlanFullEditorInput: planFull.onPlanFullEditorInput,
    savePlanFullView: planFull.savePlanFullView,
    assignFromPlanFullView: planFull.assignFromPlanFullView,
    openPlanFullDiff: planFull.openPlanFullDiff,
    closePlanFullDiff: planFull.closePlanFullDiff,
    adoptPlanDiffSide: planFull.adoptPlanDiffSide,
    renderPlanFullView: planFull.renderPlanFullView,
  };

  // Better listSessions/send for A2 bridge (project arg, no DOM)
  desk.listSessions = async (project) => {
    vm.setProject(project || null);
    const list = await chatApi.listSessions(project);
    const s = window.state;
    if (s) {
      s.chatSessionList = Array.isArray(list) && list.length
        ? list
        : [{ session_id: "default", title: null, message_count: 0 }];
    }
    return list;
  };
  desk.send = async (args) => {
    const path = args?.project;
    vm.setProject(path || null);
    return chatApi.sendMessage({
      project: path,
      message: args?.message || "",
      sessionId: args?.sessionId || "default",
      attachments: args?.attachments ?? null,
      effort: args?.effort || null,
    });
  };
  desk.savePlan = (args) => chatApi.savePlan(args);

  // Classic / bindUiClick g("name") looks up window[name] — mirror so both
  // capture (chatClarify) and bubble (bindUiClick) paths resolve without ccoChat.
  if (typeof window !== "undefined") {
    window.pickClarifyOption = chatClarify.pickClarifyOption;
    window.skipClarify = chatClarify.skipClarify;
    window.selectClarifyEntry = chatClarify.selectClarifyEntry;
    window.setChatMode = chatMode.setChatMode;
    window.getChatMode = chatMode.getChatMode;
    window.paintChatMode = chatMode.paintChatMode;
    window.claimBriefToPlan = chatClarify.claimBriefToPlan;
    window.rechatFromBrief = chatClarify.rechatFromBrief;
    window.ensureClarifyState = chatClarify.ensureClarifyState;
    window.installClarifyUi = chatClarify.installClarifyUi;
    window.fillChatExample = chatActions.fillChatExample;
    window.setPathMode = chatActions.setPathModeAndPaint;
    window.setPersona = chatActions.setPersonaAndPaint;
    window.setPersonaAndPaint = chatActions.setPersonaAndPaint;
    window.reviseChatDraft = chatActions.reviseChatDraft;
    window.claimWaveBundle = chatActions.claimWaveBundle;
    window.handleLastSummaryAction = chatActions.handleLastSummaryAction;
    window.loadChatSession = chatSessions.loadChatSession;
    window.loadChatSessionList = chatSessions.loadChatSessionList;
    window.switchChatSession = chatSessions.switchChatSession;
    window.newChatSession = chatSessions.newChatSession;
    window.deleteChatSession = chatSessions.deleteChatSession;
    window.renderPlanRail = renderPlanRailCompat;
    window.loadPlanRail = loadPlanRailCompat;
    window.pickChatQuizOption = chatMsgEnhance.pickChatQuizOption;
    window.fillChatQuizDraft = chatMsgEnhance.fillChatQuizDraft;
    window.sendChatQuizDraft = chatMsgEnhance.sendChatQuizDraft;
    window.unfoldChatMessage = chatMsgEnhance.unfoldChatMessage;
    window.foldChatMessage = chatMsgEnhance.foldChatMessage;
    window.expandChatMsgBody = chatMsgEnhance.expandChatMsgBody;
    window.collapseChatMsgBody = chatMsgEnhance.collapseChatMsgBody;
    window.toggleChatPlanExpand = chatFormat.toggleChatPlanExpand;
    window.adoptChatPlanFromCard = chatFormat.adoptChatPlanFromCard;
    window.dismissChatPlanFromCard = chatFormat.dismissChatPlanFromCard;
    window.copyChatMessageFromBtn = chatFormat.copyChatMessageFromBtn;
  }

  return desk;
}
