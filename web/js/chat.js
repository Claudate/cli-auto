/**
 * [INPUT]: window.ccoChat（main.js ESM 安装）
 * [OUTPUT]: 经典全局函数名兼容（doctor/plan/templates 调用）
 * [POS]: A5-2a chat.js ≤200 facade — 逻辑在 features/chat/*
 * note: 禁止堆新功能；禁止 invoke/confirm_start/start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — chat classic facade (A5-2a strangler) */

function _ccoChat() {
  return typeof window !== "undefined" ? window.ccoChat : null;
}

function _chatCall(name, ...args) {
  const d = _ccoChat();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[chat facade] ccoChat." + name + " not ready");
  return undefined;
}

function ensureChatState(...a) { return _chatCall("ensureChatState", ...a); }
function stashChatSession(...a) { return _chatCall("stashChatSession", ...a); }
function restoreChatSession(...a) { return _chatCall("restoreChatSession", ...a); }
function openChatPage(...a) { return _chatCall("openChatPage", ...a); }
function renderChatPage(...a) { return _chatCall("renderChatPage", ...a); }
function sendChatMessage(...a) { return _chatCall("sendChatMessage", ...a); }
function saveChatPlan(...a) { return _chatCall("saveChatPlan", ...a); }
function assignFromChat(...a) { return _chatCall("assignFromChat", ...a); }
function previewChatPlan(...a) { return _chatCall("previewChatPlan", ...a); }
function normalizeChatDraft(...a) { return _chatCall("normalizeChatDraft", ...a); }
function loadChatSession(...a) { return _chatCall("loadChatSession", ...a); }
function loadChatSessionList(...a) { return _chatCall("loadChatSessionList", ...a); }
function switchChatSession(...a) { return _chatCall("switchChatSession", ...a); }
function newChatSession(...a) { return _chatCall("newChatSession", ...a); }
function deleteChatSession(...a) { return _chatCall("deleteChatSession", ...a); }
function addChatAttachments(...a) { return _chatCall("addChatAttachments", ...a); }
function removeChatAttachment(...a) { return _chatCall("removeChatAttachment", ...a); }
function handleChatPaste(...a) { return _chatCall("handleChatPaste", ...a); }
function pickChatAttachments(...a) { return _chatCall("pickChatAttachments", ...a); }
function openImageLightbox(...a) { return _chatCall("openImageLightbox", ...a); }
function closeImageLightbox(...a) { return _chatCall("closeImageLightbox", ...a); }
function fillChatExample(...a) { return _chatCall("fillChatExample", ...a); }
function toggleChatPlanExpand(...a) { return _chatCall("toggleChatPlanExpand", ...a); }
function adoptChatPlanFromCard(...a) { return _chatCall("adoptChatPlanFromCard", ...a); }
function dismissChatEnvBar(...a) { return _chatCall("dismissChatEnvBar", ...a); }
function openChatEnvDoctor(...a) { return _chatCall("openChatEnvDoctor", ...a); }
function toggleChatPlanRail(...a) { return _chatCall("toggleChatPlanRail", ...a); }
function setPlanRailOpen(...a) { return _chatCall("setPlanRailOpen", ...a); }
function getPlansDir(...a) { return _chatCall("getPlansDir", ...a); }
function setPlansDir(...a) { return _chatCall("setPlansDir", ...a); }
function promptPlansDir(...a) { return _chatCall("promptPlansDir", ...a); }
function pickPlansFolderForMgmt(...a) { return _chatCall("pickPlansFolderForMgmt", ...a); }
function pickPlanFileForMgmt(...a) { return _chatCall("pickPlanFileForMgmt", ...a); }
function getPlansMgmtScopeDir(...a) { return _chatCall("getPlansMgmtScopeDir", ...a); }
function setPlansMgmtScopeDir(...a) { return _chatCall("setPlansMgmtScopeDir", ...a); }
function isPathInPlansDir(...a) { return _chatCall("isPathInPlansDir", ...a); }
function openPlanManagement(...a) { return _chatCall("openPlanManagement", ...a); }
function renderPlansMgmtPage(...a) { return _chatCall("renderPlansMgmtPage", ...a); }
function selectPlansMgmtItem(...a) { return _chatCall("selectPlansMgmtItem", ...a); }
function openPlansMgmtItem(...a) { return _chatCall("openPlansMgmtItem", ...a); }
function assignFromPlansMgmt(...a) { return _chatCall("assignFromPlansMgmt", ...a); }
function loadPlanRail(...a) { return _chatCall("loadPlanRail", ...a); }
function renderPlanRail(...a) { return _chatCall("renderPlanRail", ...a); }
function selectPlanRailItem(...a) { return _chatCall("selectPlanRailItem", ...a); }
function openPlanRailItem(...a) { return _chatCall("openPlanRailItem", ...a); }
function openPlanFullView(...a) { return _chatCall("openPlanFullView", ...a); }
function closePlanFullView(...a) { return _chatCall("closePlanFullView", ...a); }
function beginPlanFullEdit(...a) { return _chatCall("beginPlanFullEdit", ...a); }
function cancelPlanFullEdit(...a) { return _chatCall("cancelPlanFullEdit", ...a); }
function onPlanFullEditorInput(...a) { return _chatCall("onPlanFullEditorInput", ...a); }
function savePlanFullView(...a) { return _chatCall("savePlanFullView", ...a); }
function assignFromPlanFullView(...a) { return _chatCall("assignFromPlanFullView", ...a); }
function openPlanFullDiff(...a) { return _chatCall("openPlanFullDiff", ...a); }
function closePlanFullDiff(...a) { return _chatCall("closePlanFullDiff", ...a); }
function adoptPlanDiffSide(...a) { return _chatCall("adoptPlanDiffSide", ...a); }
function renderPlanFullView(...a) { return _chatCall("renderPlanFullView", ...a); }
