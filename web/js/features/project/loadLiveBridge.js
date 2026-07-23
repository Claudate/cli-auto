/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: loadLive / ensureSelectedTask → ccoLoadLive only
 * [POS]: A5-2b-fin features/project/loadLiveBridge.js
 * note: 业务过滤只在 project_live_view（SQLite dismiss）；本文件不双写
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

/* ── Workspace live：唯一路径 ccoLoadLive（server SoT） ── */
export async function loadLive() {
  if (typeof window.ccoLoadLive === "function") {
    return window.ccoLoadLive({
      getState: () => state,
      hasActiveRun: () => hasActiveRun(),
      refreshPlanJob: () => host.refreshPlanJob(),
      loadProjects: typeof loadProjects === "function" ? () => loadProjects() : undefined,
      logMaxBytes: 96000,
    });
  }
  // main.js 未就绪：直读 gateway（仍信服务端过滤）
  if (!state.selectedPath) {
    state.live = null;
    return null;
  }
  state.now = Date.now();
  if (state.phase === "planning" && state.planJobId) {
    await host.refreshPlanJob().catch(() => {});
  }
  const prevLive = hasActiveRun();
  let live;
  if (window.ccoGateway?.getProjectLive) {
    live = await window.ccoGateway.getProjectLive(state.selectedPath, {
      logMaxBytes: 96000,
    });
  } else {
    live = await requireGateway().getProjectLive(state.selectedPath, {
      logMaxBytes: 96000,
    });
  }
  const path = state.selectedPath;
  if (path && live?.run_id) {
    if (!state.lastRunIdByProject) state.lastRunIdByProject = {};
    state.lastRunIdByProject[path] = String(live.run_id);
  }
  state.live = live;
  const nowLive = hasActiveRun();
  if (prevLive && !nowLive && state.phase === "running" && state.live) {
    state.phase = "done";
  }
  if (prevLive !== nowLive) {
    try {
      if (typeof loadProjects === "function") await loadProjects();
    } catch (_) {}
  }
  ensureSelectedTask();
  const onChat = state.page === "chat";
  // Chat: do not run workspace shell (avoids selectedPlan steal from live).
  if (!onChat) {
    try {
      renderWorkspace();
    } catch (_) {}
  }
  if (prevLive !== nowLive) {
    try {
      renderProjectList();
    } catch (_) {}
    if (!onChat) {
      try {
        host.renderPlanPicker();
      } catch (_) {}
      try {
        host.updateSplitPlanChip();
      } catch (_) {}
    }
    if (onChat) {
      try {
        (window.renderChatMessages || window.renderChatPage)?.();
      } catch (_) {}
    }
  } else if (!onChat) {
    try {
      host.renderPlanPicker();
    } catch (_) {}
    try {
      host.updateBgPlanBanner();
    } catch (_) {}
  } else {
    try {
      host.updateBgPlanBanner();
    } catch (_) {}
  }
  return state.live;
}

export function ensureSelectedTask() {
  if (typeof window.ccoLoadLive === "function" && window.ccoEnsureSelectedTask) {
    return window.ccoEnsureSelectedTask(state);
  }
  const tasks = state.live?.tasks || [];
  if (!tasks.length) {
    state.selectedTaskId = null;
    return null;
  }
  if (
    state.selectedTaskId &&
    tasks.some((t) => t.task_id === state.selectedTaskId || t.id === state.selectedTaskId)
  ) {
    return state.selectedTaskId;
  }
  const running = tasks.find((t) => isLiveStatus(t.status));
  state.selectedTaskId = (running || tasks[0]).task_id || (running || tasks[0]).id || null;
  return state.selectedTaskId;
}
