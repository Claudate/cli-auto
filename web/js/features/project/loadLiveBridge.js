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
  // P0 防串显：记住 fetch 时的项目路径，IPC 往返期间切了项目则丢弃结果。
  const fetchProjectPath = state.selectedPath;
  state.now = Date.now();
  if (state.phase === "planning" && state.planJobId) {
    await host.refreshPlanJob().catch(() => {});
  }
  const prevLive = hasActiveRun();
  // P2: 对齐 features/run/loadLive — idle 态不拉日志（0 字节预算）
  const phaseNeedsLogs =
    state.phase === "running" ||
    (state.phase === "done" && hasActiveRun());
  const logMax = phaseNeedsLogs ? 96000 : 0;
  let live;
  if (window.ccoGateway?.getProjectLive) {
    live = await window.ccoGateway.getProjectLive(fetchProjectPath, {
      logMaxBytes: logMax,
    });
  } else {
    live = await requireGateway().getProjectLive(fetchProjectPath, {
      logMaxBytes: logMax,
    });
  }
  // P0 防串显：fetch 期间切了项目 → 丢弃，不写 state.live
  if (state.selectedPath !== fetchProjectPath) {
    return null;
  }
  const path = state.selectedPath;
  // 与 features/run/loadLive 同：执行/结果台勿被空 live 抹掉
  const prevSnapshot = state.live;
  const emptyIncoming = !live || !live.run_id;
  const keepPrevOnDesk =
    emptyIncoming &&
    prevSnapshot?.run_id &&
    (state.phase === "running" || state.phase === "done") &&
    (!path ||
      !prevSnapshot.project_path ||
      String(prevSnapshot.project_path) === String(path));
  if (keepPrevOnDesk) {
    live = prevSnapshot;
  }
  if (path && live?.run_id) {
    if (!state.lastRunIdByProject) state.lastRunIdByProject = {};
    state.lastRunIdByProject[path] = String(live.run_id);
  }
  state.live = live;
  const nowLive = hasActiveRun();
  if (prevLive && !nowLive && state.phase === "running" && state.live?.run_id) {
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
