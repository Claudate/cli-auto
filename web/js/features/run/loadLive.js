/**
 * [INPUT]: gateway.getProjectLive · legacy window helpers (phase / poll / render)
 * [OUTPUT]: loadLive + ensureSelectedTask（workspace 轮询壳）
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

  state.live = await fetchLive(state.selectedPath, {
    logMaxBytes: deps.logMaxBytes ?? 96000,
  });

  const nowLive = !!hasActiveRun();
  if (prevLive && !nowLive && state.phase === "running") {
    state.phase = "done";
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

  call(deps.renderWorkspace, "renderWorkspace");

  if (prevLive !== nowLive) {
    call(deps.renderProjectList, "renderProjectList");
    call(deps.renderPlanPicker, "renderPlanPicker");
    call(deps.updateSplitPlanChip, "updateSplitPlanChip");
  } else if (state.page !== "workspace") {
    call(deps.renderPlanPicker, "renderPlanPicker");
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
