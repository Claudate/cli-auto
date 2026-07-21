/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: loadLive / ensureSelectedTask → ccoLoadLive
 * [POS]: A5-2b-fin features/project/loadLiveBridge.js
 * note: loadLive / ensureSelectedTask → ccoLoadLive
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  isRunPaused,
  isLiveStatus,
  isFailedStatus,
  toastRunLocked,
  normalizePlanPath,
  planDisplayName,
  fillPlannerLog,
  canEditSelectedTask,
  openNativeDialog,
  loadProjects,
  renderProjectList,
  renderWorkspace,
  goHome,
  closeModal,
  openChatPage,
  stashChatSession,
  restoreChatSession,
  stopChatWaitTicker,
  loadPlanRail,
  renderPlanRail,
  selectPlanRailItem,
  renderPlansMgmtPage,
  chatAssignDirectEnabled,
  flowModeLabel,
  flowModeHint,
  flowStageStripHtml,
  flowChooserSub,
  flowJoinSeriousFun,
  flowPickBlurb,
  flowPlanHowLabel,
  flowPlanningSub,
  flowSanitizeDepsLabel,
  flowRunningMonitorTitle,
  esc,
  requireGateway,
} from "./legacy.js";
import { host } from "./host.js";

/* ── Workspace live（A5-2b → features/run/loadLive；IPC 经 gateway） ── */
export async function loadLive() {
  if (typeof window.ccoLoadLive === "function") {
    return window.ccoLoadLive({
      getState: () => state,
      hasActiveRun: () => hasActiveRun(),
      refreshPlanJob: () => host.refreshPlanJob(),
      logMaxBytes: 96000,
    });
  }
  // main.js 未就绪：gateway 优先，禁止散落新策略
  if (!state.selectedPath) {
    state.live = null;
    return null;
  }
  state.now = Date.now();
  if (state.phase === "planning" && state.planJobId) {
    await host.refreshPlanJob().catch(() => {});
  }
  const prevLive = hasActiveRun();
  if (window.ccoGateway?.getProjectLive) {
    state.live = await window.ccoGateway.getProjectLive(state.selectedPath, {
      logMaxBytes: 96000,
    });
  } else {
    state.live = await requireGateway().getProjectLive(state.selectedPath, {
      logMaxBytes: 96000,
    });
  }
  const nowLive = hasActiveRun();
  if (prevLive && !nowLive && state.phase === "running") {
    state.phase = "done";
  }
  ensureSelectedTask();
  try {
    renderWorkspace();
  } catch (_) {}
  if (prevLive !== nowLive) {
    try {
      renderProjectList();
    } catch (_) {}
    try {
      host.renderPlanPicker();
    } catch (_) {}
    try {
      host.updateSplitPlanChip();
    } catch (_) {}
  } else {
    try {
      host.renderPlanPicker();
    } catch (_) {}
    try {
      host.updateBgPlanBanner();
    } catch (_) {}
  }
  return state.live;
}

export function ensureSelectedTask() {
  if (typeof window.ccoEnsureSelectedTask === "function") {
    return window.ccoEnsureSelectedTask(state);
  }
  const tasks = state.live?.tasks || [];
  if (!tasks.length) {
    state.selectedTaskId = null;
    return null;
  }
  const ids = new Set(tasks.map((t) => t.task_id));
  if (!(state.selectedTaskId && ids.has(state.selectedTaskId))) {
    state.selectedTaskId = null;
  }
  const failed = tasks.find((t) => isFailedStatus(t.status));
  const running = tasks.find((t) => isLiveStatus(t.status));
  if (!state.selectedTaskId) {
    state.selectedTaskId = (failed || running || tasks[0]).task_id;
  } else if (failed && isFailedStatus(failed.status)) {
    const cur = tasks.find((t) => t.task_id === state.selectedTaskId);
    if (cur && !isFailedStatus(cur.status) && !isLiveStatus(cur.status)) {
      state.selectedTaskId = failed.task_id;
    }
  }
  return state.selectedTaskId;
}
