/**
 * [INPUT]: all chat modules
 * [OUTPUT]: register host · public desk API for window.ccoChat
 * [POS]: A5-2a features/chat/installChat.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { register, host } from "./host.js";
import * as chatState from "./chatState.js";
import * as planDir from "./planDir.js";
import * as plansMgmt from "./plansMgmt.js";
import * as chatAttachments from "./chatAttachments.js";
import * as chatFormat from "./chatFormat.js";
import * as chatSessions from "./chatSessions.js";
import * as chatSessionRename from "./chatSessionRename.js";
import * as chatActions from "./chatActions.js";
import * as planRail from "./planRail.js";
import * as planFull from "./planFull.js";
import * as chatApi from "./chatApi.js";
import * as chatClarify from "./chatClarify.js";
import { createChatViewModel } from "./ChatViewModel.js";

/** Wire host bag once (idempotent). */
export function installChatHost() {
  register({
    ...chatState,
    ...planDir,
    ...plansMgmt,
    ...chatAttachments,
    ...chatFormat,
    ...chatSessions,
    ...chatSessionRename,
    ...chatActions,
    ...planRail,
    ...planFull,
    ...chatClarify,
  });
  // t3: bind clarify click once at host install
  try {
    chatClarify.installClarifyUi();
  } catch (_) {}
  return host;
}

/**
 * Full desk surface for window.ccoChat (classic stubs + main.js).
 * No confirm_start / start_run.
 */
export function createChatDesk(opts = {}) {
  installChatHost();
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
    saveChatPlan: chatActions.saveChatPlan,
    assignFromChat: chatActions.assignFromChat,
    assignAndSplitFromChat: chatActions.assignAndSplitFromChat,
    assignAndDirectFromChat: chatActions.assignAndDirectFromChat,
    previewChatPlan: chatActions.previewChatPlan,
    normalizeChatDraft: chatActions.normalizeChatDraft,
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
    toggleChatPlanExpand: chatFormat.toggleChatPlanExpand,
    adoptChatPlanFromCard: chatFormat.adoptChatPlanFromCard,
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
    loadPlanRail: planRail.loadPlanRail,
    renderPlanRail: planRail.renderPlanRail,
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

  // Classic / bindUiClick g("pickClarifyOption") looks up window[name].
  // Mirror clarify actions so capture handlers outside ccoChat still work.
  if (typeof window !== "undefined") {
    window.pickClarifyOption = chatClarify.pickClarifyOption;
    window.skipClarify = chatClarify.skipClarify;
    window.selectClarifyEntry = chatClarify.selectClarifyEntry;
    window.claimBriefToPlan = chatClarify.claimBriefToPlan;
    window.ensureClarifyState = chatClarify.ensureClarifyState;
    window.installClarifyUi = chatClarify.installClarifyUi;
  }

  return desk;
}
