/**
 * note: **A5-2b-fin 绞杀壳** — 禁止堆新功能；真源 features/project + features/split
 * [INPUT]: window.ccoProject（main.js ESM 安装）· ccoSplit · ccoLoadLive
 * [OUTPUT]: 经典全局函数名兼容（bindUi / chat / templates）
 * [POS]: A5-2b-fin plan.js ≤200 facade — 逻辑在 features/project/*
 * 红线：confirm 唯一开跑（→ccoSplit）；optional 不静默 auto-start；无 start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * ## 迁出表（D5）
 * | 块 | 去向 |
 * |----|------|
 * | session / H0 entry / selectProject / banner | features/project/sessionEntry |
 * | phase panels / flow strips / chips | features/project/shellChrome |
 * | project CRUD / doctor bridge | features/project/projectCrud |
 * | plan meta / executed partition / loadPlans | features/project/planMeta |
 * | picker / chooser / assign busy / execute | features/project/projectPicker |
 * | selectPlan / max parallel / confirm open | features/project/planSelect |
 * | plan job poll · optional gate · advance | features/project/jobPoll |
 * | confirm → ccoSplit · replan/sanitize | features/project/confirmActions |
 * | loadLive 壳 | features/project/loadLiveBridge → ccoLoadLive |
 */
/* cco desktop — plan classic facade (A5-2b-fin strangler) */

function _ccoProject() {
  return typeof window !== "undefined" ? window.ccoProject : null;
}

function _planCall(name, ...args) {
  const d = _ccoProject();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[plan facade] ccoProject." + name + " not ready");
  return undefined;
}

function isPlanSessionActive(...a) { return _planCall("isPlanSessionActive", ...a); }
function stashPlanSession(...a) { return _planCall("stashPlanSession", ...a); }
function restorePlanSession(...a) { return _planCall("restorePlanSession", ...a); }
function clearPlanSession(...a) { return _planCall("clearPlanSession", ...a); }
function applyRestoredPlanJob(...a) { return _planCall("applyRestoredPlanJob", ...a); }
function tryRestorePersistedPlanJob(...a) { return _planCall("tryRestorePersistedPlanJob", ...a); }
function hasMonitorableActivity(...a) { return _planCall("hasMonitorableActivity", ...a); }
function resolveEntryRoute(...a) { return _planCall("resolveEntryRoute", ...a); }
function applyEntryRoute(...a) { return _planCall("applyEntryRoute", ...a); }
function goToPlanMonitor(...a) { return _planCall("goToPlanMonitor", ...a); }
function dismissBgPlanBanner(...a) { return _planCall("dismissBgPlanBanner", ...a); }
function updateBgPlanBanner(...a) { return _planCall("updateBgPlanBanner", ...a); }
function selectProject(...a) { return _planCall("selectProject", ...a); }
function applyFlowModeBadge(...a) { return _planCall("applyFlowModeBadge", ...a); }
function resolveFlowPhaseForStrip(...a) { return _planCall("resolveFlowPhaseForStrip", ...a); }
function refreshFlowStrips(...a) { return _planCall("refreshFlowStrips", ...a); }
function renderPhasePanels(...a) { return _planCall("renderPhasePanels", ...a); }
function addProjectFromModal(...a) { return _planCall("addProjectFromModal", ...a); }
function pickFolderToModal(...a) { return _planCall("pickFolderToModal", ...a); }
function removeSelectedProject(...a) { return _planCall("removeSelectedProject", ...a); }
function dismissRun(...a) { return _planCall("dismissRun", ...a); }
function ensureDoctor(...a) { return _planCall("ensureDoctor", ...a); }
function renderDoctorWarn(...a) { return _planCall("renderDoctorWarn", ...a); }
function isPlanUnderProject(...a) { return _planCall("isPlanUnderProject", ...a); }
function planExecBadgeInfo(...a) { return _planCall("planExecBadgeInfo", ...a); }
function planIsEverCompleted(...a) { return _planCall("planIsEverCompleted", ...a); }
function planMetaForPath(...a) { return _planCall("planMetaForPath", ...a); }
function partitionPlanItems(...a) { return _planCall("partitionPlanItems", ...a); }
function setShowExecutedPlans(...a) { return _planCall("setShowExecutedPlans", ...a); }
function syncShowExecutedToggles(...a) { return _planCall("syncShowExecutedToggles", ...a); }
function applyPlanMetaItems(...a) { return _planCall("applyPlanMetaItems", ...a); }
function loadPlansForPicker(...a) { return _planCall("loadPlansForPicker", ...a); }
function setAssignBusy(...a) { return _planCall("setAssignBusy", ...a); }
function startExecuteFromSelection(...a) { return _planCall("startExecuteFromSelection", ...a); }
function renderWorkspaceShell(...a) { return _planCall("renderWorkspaceShell", ...a); }
function setPlanCollapsed(...a) { return _planCall("setPlanCollapsed", ...a); }
function openPlanChooser(...a) { return _planCall("openPlanChooser", ...a); }
function setChooserListExpanded(...a) { return _planCall("setChooserListExpanded", ...a); }
function updateChooserAssignState(...a) { return _planCall("updateChooserAssignState", ...a); }
function renderPlanChooser(...a) { return _planCall("renderPlanChooser", ...a); }
function renderPlanPicker(...a) { return _planCall("renderPlanPicker", ...a); }
function updateSplitPlanChip(...a) { return _planCall("updateSplitPlanChip", ...a); }
function updateBudgetChip(...a) { return _planCall("updateBudgetChip", ...a); }
function showSplitPlanConfirm(...a) { return _planCall("showSplitPlanConfirm", ...a); }
function openEditPlan(...a) { return _planCall("openEditPlan", ...a); }
function backFromConfirmToMonitor(...a) { return _planCall("backFromConfirmToMonitor", ...a); }
function readSplitMaxParallel(...a) { return _planCall("readSplitMaxParallel", ...a); }
function syncSplitMaxParallelInputs(...a) { return _planCall("syncSplitMaxParallelInputs", ...a); }
function commitSplitMaxParallel(...a) { return _planCall("commitSplitMaxParallel", ...a); }
function updateTopPlanInfo(...a) { return _planCall("updateTopPlanInfo", ...a); }
function renderPlanPreview(...a) { return _planCall("renderPlanPreview", ...a); }
function selectPlan(...a) { return _planCall("selectPlan", ...a); }
function pickPlanFileForPicker(...a) { return _planCall("pickPlanFileForPicker", ...a); }
function setDefaultPlan(...a) { return _planCall("setDefaultPlan", ...a); }
function analyzePlanFromPicker(...a) { return _planCall("analyzePlanFromPicker", ...a); }
function stopPlanJobPoll(...a) { return _planCall("stopPlanJobPoll", ...a); }
function startPlanJobPoll(...a) { return _planCall("startPlanJobPoll", ...a); }
function planHasOptionalTasks(...a) { return _planCall("planHasOptionalTasks", ...a); }
function planNeedsOptionalConfirm(...a) { return _planCall("planNeedsOptionalConfirm", ...a); }
function advancePlannedJob(...a) { return _planCall("advancePlannedJob", ...a); }
function refreshPlanJob(...a) { return _planCall("refreshPlanJob", ...a); }
function renderConfirmPanel(...a) { return _planCall("renderConfirmPanel", ...a); }
function beginConfirmEdit(...a) { return _planCall("beginConfirmEdit", ...a); }
function cancelConfirmEdit(...a) { return _planCall("cancelConfirmEdit", ...a); }
function saveConfirmEdit(...a) { return _planCall("saveConfirmEdit", ...a); }
function deleteConfirmTask(...a) { return _planCall("deleteConfirmTask", ...a); }
function confirmAndStart(...a) { return _planCall("confirmAndStart", ...a); }
function cancelPlanning(...a) { return _planCall("cancelPlanning", ...a); }
function replanFromConfirm(...a) { return _planCall("replanFromConfirm", ...a); }
function enablePostInspectAndResplit(...a) { return _planCall("enablePostInspectAndResplit", ...a); }
function enablePlannerCriticAndResplit(...a) { return _planCall("enablePlannerCriticAndResplit", ...a); }
function sanitizeDepsFromConfirm(...a) { return _planCall("sanitizeDepsFromConfirm", ...a); }
function loadLive(...a) { return _planCall("loadLive", ...a); }
function ensureSelectedTask(...a) { return _planCall("ensureSelectedTask", ...a); }
