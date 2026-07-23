/**
 * [INPUT]: project modules
 * [OUTPUT]: window.ccoProject + classic global names (strangler)
 * [POS]: A5-2b-fin features/project/installProject.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { register, host } from "./host.js";
import * as projectApi from "./projectApi.js";
import { createProjectViewModel } from "./ProjectViewModel.js";
import * as sessionEntry from "./sessionEntry.js";
import * as shellChrome from "./shellChrome.js";
import * as projectCrud from "./projectCrud.js";
import * as planMeta from "./planMeta.js";
import * as projectPicker from "./projectPicker.js";
import * as planSelect from "./planSelect.js";
import * as jobPoll from "./jobPoll.js";
import * as confirmActions from "./confirmActions.js";
import * as loadLiveBridge from "./loadLiveBridge.js";

/** Wire host bag once. */
export function installProjectHost() {
  register({
    ...sessionEntry,
    ...shellChrome,
    ...projectCrud,
    ...planMeta,
    ...projectPicker,
    ...planSelect,
    ...jobPoll,
    ...confirmActions,
    ...loadLiveBridge,
  });
  return host;
}

/**
 * Public desk for window.ccoProject (classic plan.js is facade).
 * IPC only via projectApi / requireGateway → gateway.
 * confirm/open-run still only via ccoSplit (confirmActions delegates).
 */
export function createProjectDesk(opts = {}) {
  installProjectHost();
  const vm = createProjectViewModel({
    projectPath: opts.projectPath || null,
  });
  return {
    vm,
    api: projectApi,
    host,
    BG_BANNER_DISMISS_KEY: host.BG_BANNER_DISMISS_KEY,
    addProjectFromModal: host.addProjectFromModal,
    advancePlannedJob: host.advancePlannedJob,
    analyzePlanFromPicker: host.analyzePlanFromPicker,
    applyEntryRoute: host.applyEntryRoute,
    applyFlowModeBadge: host.applyFlowModeBadge,
    applyPlanMetaItems: host.applyPlanMetaItems,
    applyRestoredPlanJob: host.applyRestoredPlanJob,
    backFromConfirmToMonitor: host.backFromConfirmToMonitor,
    beginConfirmEdit: host.beginConfirmEdit,
    bgBannerActivitySig: host.bgBannerActivitySig,
    cancelConfirmEdit: host.cancelConfirmEdit,
    cancelPlanning: host.cancelPlanning,
    clearPlanSession: host.clearPlanSession,
    commitSplitMaxParallel: host.commitSplitMaxParallel,
    confirmAndStart: host.confirmAndStart,
    countOptionalIncluded: host.countOptionalIncluded,
    defaultAssignLabel: host.defaultAssignLabel,
    deleteConfirmTask: host.deleteConfirmTask,
    dismissBgPlanBanner: host.dismissBgPlanBanner,
    dismissRun: host.dismissRun,
    enablePlannerCriticAndResplit: host.enablePlannerCriticAndResplit,
    enablePostInspectAndResplit: host.enablePostInspectAndResplit,
    ensureDoctor: host.ensureDoctor,
    ensureSelectedTask: host.ensureSelectedTask,
    goToPlanMonitor: host.goToPlanMonitor,
    hasMonitorableActivity: host.hasMonitorableActivity,
    isBgBannerDismissed: host.isBgBannerDismissed,
    isPlanSessionActive: host.isPlanSessionActive,
    liveBelongsToOpenPlan: host.liveBelongsToOpenPlan,
    hasCurrentRoundLive: host.hasCurrentRoundLive,
    isPlanUnderProject: host.isPlanUnderProject,
    isSystemPostTask: host.isSystemPostTask,
    loadLive: host.loadLive,
    loadPlansForPicker: host.loadPlansForPicker,
    openEditPlan: host.openEditPlan,
    openPlanChooser: host.openPlanChooser,
    partitionPlanItems: host.partitionPlanItems,
    pickFolderToModal: host.pickFolderToModal,
    pickPlanFileForPicker: host.pickPlanFileForPicker,
    planExecBadgeInfo: host.planExecBadgeInfo,
    planHasOptionalTasks: host.planHasOptionalTasks,
    planIsEverCompleted: host.planIsEverCompleted,
    planMetaForPath: host.planMetaForPath,
    planNeedsOptionalConfirm: host.planNeedsOptionalConfirm,
    readSplitMaxParallel: host.readSplitMaxParallel,
    refreshFlowStrips: host.refreshFlowStrips,
    refreshPlanJob: host.refreshPlanJob,
    removeSelectedProject: host.removeSelectedProject,
    renderConfirmPanel: host.renderConfirmPanel,
    renderDoctorWarn: host.renderDoctorWarn,
    renderPhasePanels: host.renderPhasePanels,
    renderPlanChooser: host.renderPlanChooser,
    renderPlanPicker: host.renderPlanPicker,
    renderPlanPreview: host.renderPlanPreview,
    renderWorkspaceShell: host.renderWorkspaceShell,
    replanFromConfirm: host.replanFromConfirm,
    resolveEntryRoute: host.resolveEntryRoute,
    resolveFlowPhaseForStrip: host.resolveFlowPhaseForStrip,
    restorePlanSession: host.restorePlanSession,
    sanitizeDepsFromConfirm: host.sanitizeDepsFromConfirm,
    saveConfirmEdit: host.saveConfirmEdit,
    selectPlan: host.selectPlan,
    selectProject: host.selectProject,
    setAssignBusy: host.setAssignBusy,
    setChooserListExpanded: host.setChooserListExpanded,
    setDefaultPlan: host.setDefaultPlan,
    setPlanCollapsed: host.setPlanCollapsed,
    setShowExecutedPlans: host.setShowExecutedPlans,
    showSplitPlanConfirm: host.showSplitPlanConfirm,
    startExecuteFromSelection: host.startExecuteFromSelection,
    startPlanJobPoll: host.startPlanJobPoll,
    stashPlanSession: host.stashPlanSession,
    stopPlanJobPoll: host.stopPlanJobPoll,
    syncShowExecutedToggles: host.syncShowExecutedToggles,
    syncSplitMaxParallelInputs: host.syncSplitMaxParallelInputs,
    tryRestorePersistedPlanJob: host.tryRestorePersistedPlanJob,
    tryRestorePlanJobForPlan: host.tryRestorePlanJobForPlan,
    loadPlanSplitIndex: host.loadPlanSplitIndex,
    planSplitForPath: host.planSplitForPath,
    planPathLookupKey: host.planPathLookupKey,
    updateBgPlanBanner: host.updateBgPlanBanner,
    updateBudgetChip: host.updateBudgetChip,
    updateChooserAssignState: host.updateChooserAssignState,
    updateSplitPlanChip: host.updateSplitPlanChip,
    updateTopPlanInfo: host.updateTopPlanInfo,
  };
}

/**
 * Install classic global names used by bindUi / chat / templates.
 * @param {{ projectPath?: string|null }} [opts]
 */
export function installProjectHostGlobals(opts = {}) {
  const desk = createProjectDesk(opts);
  window.ccoProject = desk;
  window.BG_BANNER_DISMISS_KEY = host.BG_BANNER_DISMISS_KEY;
  window.addProjectFromModal = host.addProjectFromModal;
  window.advancePlannedJob = host.advancePlannedJob;
  window.analyzePlanFromPicker = host.analyzePlanFromPicker;
  window.applyEntryRoute = host.applyEntryRoute;
  window.applyFlowModeBadge = host.applyFlowModeBadge;
  window.applyPlanMetaItems = host.applyPlanMetaItems;
  window.applyRestoredPlanJob = host.applyRestoredPlanJob;
  window.backFromConfirmToMonitor = host.backFromConfirmToMonitor;
  window.beginConfirmEdit = host.beginConfirmEdit;
  window.bgBannerActivitySig = host.bgBannerActivitySig;
  window.cancelConfirmEdit = host.cancelConfirmEdit;
  window.cancelPlanning = host.cancelPlanning;
  window.clearPlanSession = host.clearPlanSession;
  window.commitSplitMaxParallel = host.commitSplitMaxParallel;
  window.confirmAndStart = host.confirmAndStart;
  window.countOptionalIncluded = host.countOptionalIncluded;
  window.defaultAssignLabel = host.defaultAssignLabel;
  window.deleteConfirmTask = host.deleteConfirmTask;
  window.dismissBgPlanBanner = host.dismissBgPlanBanner;
  window.dismissRun = host.dismissRun;
  window.enablePlannerCriticAndResplit = host.enablePlannerCriticAndResplit;
  window.enablePostInspectAndResplit = host.enablePostInspectAndResplit;
  window.ensureDoctor = host.ensureDoctor;
  window.ensureSelectedTask = host.ensureSelectedTask;
  window.goToPlanMonitor = host.goToPlanMonitor;
  window.hasMonitorableActivity = host.hasMonitorableActivity;
  window.isBgBannerDismissed = host.isBgBannerDismissed;
  window.isPlanSessionActive = host.isPlanSessionActive;
  window.liveBelongsToOpenPlan = host.liveBelongsToOpenPlan;
  window.hasCurrentRoundLive = host.hasCurrentRoundLive;
  window.isPlanUnderProject = host.isPlanUnderProject;
  window.isSystemPostTask = host.isSystemPostTask;
  window.loadLive = host.loadLive;
  window.loadPlansForPicker = host.loadPlansForPicker;
  window.openEditPlan = host.openEditPlan;
  window.openPlanChooser = host.openPlanChooser;
  window.partitionPlanItems = host.partitionPlanItems;
  window.pickFolderToModal = host.pickFolderToModal;
  window.pickPlanFileForPicker = host.pickPlanFileForPicker;
  window.planExecBadgeInfo = host.planExecBadgeInfo;
  window.planHasOptionalTasks = host.planHasOptionalTasks;
  window.planIsEverCompleted = host.planIsEverCompleted;
  window.planMetaForPath = host.planMetaForPath;
  window.planNeedsOptionalConfirm = host.planNeedsOptionalConfirm;
  window.readSplitMaxParallel = host.readSplitMaxParallel;
  window.refreshFlowStrips = host.refreshFlowStrips;
  window.refreshPlanJob = host.refreshPlanJob;
  window.removeSelectedProject = host.removeSelectedProject;
  window.renderConfirmPanel = host.renderConfirmPanel;
  window.renderDoctorWarn = host.renderDoctorWarn;
  window.renderPhasePanels = host.renderPhasePanels;
  window.renderPlanChooser = host.renderPlanChooser;
  window.renderPlanPicker = host.renderPlanPicker;
  window.renderPlanPreview = host.renderPlanPreview;
  window.renderWorkspaceShell = host.renderWorkspaceShell;
  window.replanFromConfirm = host.replanFromConfirm;
  window.resolveEntryRoute = host.resolveEntryRoute;
  window.resolveFlowPhaseForStrip = host.resolveFlowPhaseForStrip;
  window.restorePlanSession = host.restorePlanSession;
  window.sanitizeDepsFromConfirm = host.sanitizeDepsFromConfirm;
  window.saveConfirmEdit = host.saveConfirmEdit;
  window.selectPlan = host.selectPlan;
  window.selectProject = host.selectProject;
  window.setAssignBusy = host.setAssignBusy;
  window.setChooserListExpanded = host.setChooserListExpanded;
  window.setDefaultPlan = host.setDefaultPlan;
  window.setPlanCollapsed = host.setPlanCollapsed;
  window.setShowExecutedPlans = host.setShowExecutedPlans;
  window.showSplitPlanConfirm = host.showSplitPlanConfirm;
  window.startExecuteFromSelection = host.startExecuteFromSelection;
  window.startPlanJobPoll = host.startPlanJobPoll;
  window.stashPlanSession = host.stashPlanSession;
  window.stopPlanJobPoll = host.stopPlanJobPoll;
  window.syncShowExecutedToggles = host.syncShowExecutedToggles;
  window.syncSplitMaxParallelInputs = host.syncSplitMaxParallelInputs;
  window.tryRestorePersistedPlanJob = host.tryRestorePersistedPlanJob;
  window.tryRestorePlanJobForPlan = host.tryRestorePlanJobForPlan;
  window.loadPlanSplitIndex = host.loadPlanSplitIndex;
  window.planSplitForPath = host.planSplitForPath;
  window.planPathLookupKey = host.planPathLookupKey;
  window.updateBgPlanBanner = host.updateBgPlanBanner;
  window.updateBudgetChip = host.updateBudgetChip;
  window.updateChooserAssignState = host.updateChooserAssignState;
  window.updateSplitPlanChip = host.updateSplitPlanChip;
  window.updateTopPlanInfo = host.updateTopPlanInfo;
  return desk;
}
