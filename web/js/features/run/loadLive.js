/**
 * [INPUT]: gateway.getProjectLive · legacy window helpers (phase / poll / render)
 * [OUTPUT]: loadLive + ensureSelectedTask（workspace 轮询壳；run-lock 翻转时刷 chat CTA）
 * [POS]: A5-2b 自 plan.js 抽出；IPC 只经 gateway；不写 confirm / start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：旁路 confirm_start、复制 stall 策略、改 run_dir 语义。
 */

import * as gateway from "../../shared/gateway.js";

/**
 * Prefer failed → running → first when no valid selection.
 * @param {object|null|undefined} live
 * @param {string|null|undefined} selectedTaskId
 * @param {{
 *   isFailedStatus?: (s: string) => boolean,
 *   isLiveStatus?: (s: string) => boolean,
 * }} [helpers]
 * @returns {string|null}
 */
export function pickSelectedTaskId(live, selectedTaskId, helpers = {}) {
  const tasks = live?.tasks || [];
  if (!tasks.length) return null;

  const isFailed =
    typeof helpers.isFailedStatus === "function"
      ? helpers.isFailedStatus
      : (s) => {
          const w = typeof window !== "undefined" ? window : globalThis;
          return typeof w.isFailedStatus === "function"
            ? w.isFailedStatus(s)
            : /fail|error|abort/i.test(String(s || ""));
        };
  const isLive =
    typeof helpers.isLiveStatus === "function"
      ? helpers.isLiveStatus
      : (s) => {
          const w = typeof window !== "undefined" ? window : globalThis;
          return typeof w.isLiveStatus === "function"
            ? w.isLiveStatus(s)
            : /run|active|working|pending/i.test(String(s || ""));
        };

  const ids = new Set(tasks.map((t) => t.task_id));
  let selected =
    selectedTaskId && ids.has(selectedTaskId) ? selectedTaskId : null;

  const failed = tasks.find((t) => isFailed(t.status));
  const running = tasks.find((t) => isLive(t.status));
  if (!selected) {
    selected = (failed || running || tasks[0]).task_id;
  } else if (failed && isFailed(failed.status)) {
    const cur = tasks.find((t) => t.task_id === selected);
    if (cur && !isFailed(cur.status) && !isLive(cur.status)) {
      selected = failed.task_id;
    }
  }
  return selected;
}

/**
 * Fetch project live, sync selection, refresh workspace shell.
 * Uses window.state + classic render helpers during strangler period.
 *
 * @param {{
 *   logMaxBytes?: number,
 *   getState?: () => object,
 *   hasActiveRun?: () => boolean,
 *   refreshPlanJob?: () => Promise<void>,
 *   renderWorkspace?: () => void,
 *   renderProjectList?: () => void,
 *   renderPlanPicker?: () => void,
 *   updateSplitPlanChip?: () => void,
 *   updateBgPlanBanner?: () => void,
 *   getProjectLive?: (project: string, opts?: { logMaxBytes?: number }) => Promise<object>,
 * }} [deps]
 */
export async function loadLive(deps = {}) {
  const w = typeof window !== "undefined" ? window : globalThis;
  const state =
    (typeof deps.getState === "function" ? deps.getState() : null) ||
    w.state ||
    null;
  if (!state) return null;

  if (!state.selectedPath) {
    state.live = null;
    return null;
  }

  state.now = Date.now();

  // 规划中时顺带刷新 plan job，防止 setInterval 被卡住时永远转圈
  if (state.phase === "planning" && state.planJobId) {
    const refresh =
      deps.refreshPlanJob ||
      (typeof w.refreshPlanJob === "function"
        ? () => w.refreshPlanJob()
        : null);
    if (refresh) await Promise.resolve(refresh()).catch(() => {});
  }

  const hasActiveRun =
    typeof deps.hasActiveRun === "function"
      ? deps.hasActiveRun
      : typeof w.hasActiveRun === "function"
        ? () => w.hasActiveRun()
        : () => false;

  const prevLive = !!hasActiveRun();
  const fetchLive =
    typeof deps.getProjectLive === "function"
      ? deps.getProjectLive
      : (project, opts) => gateway.getProjectLive(project, opts);

  // SoT = SQLite dismissed_run_id；project_live_view 已在服务端过滤
  let live = await fetchLive(state.selectedPath, {
    logMaxBytes: deps.logMaxBytes ?? 96000,
  });
  const path = state.selectedPath;
  // 防抖：执行/结果台轮询偶发 empty live 时勿抹掉上一次任务板 → #cli-empty
  // （run 刚失败/完成瞬间 list 竞态、或短暂 IO 失败都会返回 run_id=null）
  const prevSnapshot = state.live;
  const emptyIncoming = !live || !live.run_id;
  const keepPrevOnDesk =
    emptyIncoming &&
    prevSnapshot?.run_id &&
    (state.phase === "running" || state.phase === "done") &&
    // 仅保留同项目上一次快照；dismiss 后服务端 empty 且 phase 会变 pick，不走这里
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

  const nowLive = !!hasActiveRun();
  // 仅「本轮正在跑 → 自然结束」才进 done；用户已 dismiss 的不回结果台
  if (prevLive && !nowLive && state.phase === "running" && state.live?.run_id) {
    state.phase = "done";
  }
  // 终态切换后刷新侧栏 last_status
  if (prevLive !== nowLive) {
    try {
      if (typeof deps.loadProjects === "function") {
        await deps.loadProjects();
      } else if (typeof w.loadProjects === "function") {
        await w.loadProjects();
      }
    } catch (_) {}
  }

  state.selectedTaskId = pickSelectedTaskId(state.live, state.selectedTaskId);

  const call = (fn, name) => {
    const f =
      typeof fn === "function"
        ? fn
        : typeof w[name] === "function"
          ? w[name]
          : null;
    if (f) {
      try {
        f();
      } catch (_) {}
    }
  };

  // Chat poll: only refresh live SoT + card CTAs. Skip renderWorkspace so a
  // finished B-run cannot rewrite selectedPlan / phase panels under the author.
  const onChat = state.page === "chat";
  if (!onChat) {
    call(deps.renderWorkspace, "renderWorkspace");
  }

  if (prevLive !== nowLive) {
    call(deps.renderProjectList, "renderProjectList");
    if (!onChat) {
      call(deps.renderPlanPicker, "renderPlanPicker");
      call(deps.updateSplitPlanChip, "updateSplitPlanChip");
    }
    // Run lock toggles canExec on plan-card CTAs — re-paint without openChatPage.
    if (onChat) {
      call(deps.renderChatMessages, "renderChatMessages");
      if (
        !(
          typeof deps.renderChatMessages === "function" ||
          typeof w.renderChatMessages === "function"
        )
      ) {
        call(deps.renderChatPage, "renderChatPage");
      }
    }
  } else if (!onChat && state.page !== "workspace") {
    call(deps.renderPlanPicker, "renderPlanPicker");
    call(deps.updateBgPlanBanner, "updateBgPlanBanner");
  } else if (!onChat) {
    call(deps.updateBgPlanBanner, "updateBgPlanBanner");
  } else {
    call(deps.updateBgPlanBanner, "updateBgPlanBanner");
  }

  return state.live;
}

/**
 * Mutate state.selectedTaskId from current live (classic helper name).
 * @param {object} [stateObj]
 */
export function ensureSelectedTask(stateObj) {
  const w = typeof window !== "undefined" ? window : globalThis;
  const state = stateObj || w.state;
  if (!state) return null;
  state.selectedTaskId = pickSelectedTaskId(state.live, state.selectedTaskId);
  return state.selectedTaskId;
}
