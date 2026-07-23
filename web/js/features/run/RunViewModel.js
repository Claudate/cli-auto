/**
 * [INPUT]: runApi · live DTO 展示状态
 * [OUTPUT]: 执行台意图（停 / 续 / 停步 / 开终端）；不写 stall 策略
 * [POS]: A4-1 RunViewModel；IPC 只经 runApi → gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createStore } from "../../shared/store.js";
import * as runApi from "./runApi.js";
import { countBuckets, runContext } from "./runBuckets.js";

/**
 * @typedef {{
 *   live: object|null,
 *   selectedTaskId: string|null,
 *   busy: boolean,
 *   lastError: string|null,
 *   lastToast: string|null,
 *   dashCollapsed: boolean,
 * }} RunSnap
 */

/**
 * @param {{
 *   onAfterMutate?: () => void | Promise<void>,
 *   onPhaseResult?: () => void,
 *   onPhaseRun?: () => void,
 *   toast?: (msg: string) => void,
 * }} [deps]
 */
export function createRunViewModel(deps = {}) {
  const store = createStore({
    live: null,
    selectedTaskId: null,
    busy: false,
    lastError: null,
    lastToast: null,
    dashCollapsed: false,
  });

  function snap() {
    return store.get();
  }

  function setPatch(partial) {
    store.set({ ...snap(), ...partial });
    return snap();
  }

  function toast(msg) {
    setPatch({ lastToast: msg });
    if (typeof deps.toast === "function") deps.toast(msg);
    else {
      const w = typeof window !== "undefined" ? window : globalThis;
      if (typeof w.toast === "function") w.toast(msg);
    }
  }

  async function after() {
    if (typeof deps.onAfterMutate === "function") {
      try {
        await deps.onAfterMutate();
      } catch (e) {
        console.error("[RunViewModel] onAfterMutate", e);
      }
    }
  }

  function runIdOf(live) {
    return live?.run_id || live?.runId || null;
  }

  return {
    store,
    getSnapshot: snap,
    subscribe: (fn) => store.subscribe(fn),

    /**
     * Mirror live into VM (strangler). Does not fetch.
     * @param {object|null} live
     * @param {{ selectedTaskId?: string|null, dashCollapsed?: boolean }} [opts]
     */
    setLive(live, opts = {}) {
      const patch = { live: live || null };
      if (opts.selectedTaskId !== undefined) {
        patch.selectedTaskId = opts.selectedTaskId;
      }
      if (opts.dashCollapsed !== undefined) {
        patch.dashCollapsed = !!opts.dashCollapsed;
      }
      return setPatch(patch);
    },

    selectTask(taskId) {
      return setPatch({ selectedTaskId: taskId || null });
    },

    toggleDashCollapsed() {
      return setPatch({ dashCollapsed: !snap().dashCollapsed });
    },

    setDashCollapsed(v) {
      return setPatch({ dashCollapsed: !!v });
    },

    /** Display helpers from current live. */
    getContext(legacyPhase) {
      return runContext(snap().live, { phase: legacyPhase });
    },

    getCounts() {
      return countBuckets(snap().live?.tasks || []);
    },

    /**
     * Stop entire run (freezes Pending per app). Not a new open-run.
     */
    async stopAll() {
      const live = snap().live;
      const runId = runIdOf(live);
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无活动运行");
        return null;
      }
      if (snap().busy) return null;
      setPatch({ busy: true, lastError: null });
      try {
        await runApi.stopRun(runId);
        toast("已请求全部停止");
        await after();
        if (typeof deps.onPhaseResult === "function") {
          try {
            deps.onPhaseResult();
          } catch (_) {}
        }
        return { ok: true, runId };
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ lastError: msg });
        toast(msg);
        return null;
      } finally {
        setPatch({ busy: false });
      }
    },

    /**
     * Stop one task.
     * @param {string} [taskId]
     */
    async stopTask(taskId) {
      const live = snap().live;
      const runId = runIdOf(live);
      const id = taskId || snap().selectedTaskId;
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无活动任务");
        return null;
      }
      if (!id) {
        toast("请先选择任务");
        return null;
      }
      if (snap().busy) return null;
      setPatch({ busy: true, lastError: null });
      try {
        await runApi.stopTask(runId, id);
        toast(`已停止 ${id}`);
        await after();
        return { ok: true, runId, taskId: id };
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ lastError: msg });
        toast(msg);
        return null;
      } finally {
        setPatch({ busy: false });
      }
    },

    /**
     * Resume paused/failed/aborted run (not confirm open-run).
     */
    async resume() {
      const runId = runIdOf(snap().live);
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无运行记录可继续");
        return null;
      }
      if (snap().busy) return null;
      setPatch({ busy: true, lastError: null });
      try {
        await runApi.resumeRun(runId);
        toast("正在继续…");
        if (typeof deps.onPhaseRun === "function") {
          try {
            deps.onPhaseRun();
          } catch (_) {}
        }
        await after();
        return { ok: true, runId };
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ lastError: msg });
        toast(msg);
        return null;
      } finally {
        setPatch({ busy: false });
      }
    },

    /**
     * Re-run one failed/stopped/timeout task (same run; not re-split).
     * @param {string} [taskId]
     */
    async retryTask(taskId) {
      const live = snap().live;
      const runId = runIdOf(live);
      const id = taskId || snap().selectedTaskId;
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无运行记录可再跑");
        return null;
      }
      if (!id) {
        toast("请先选择失败的任务");
        return null;
      }
      if (snap().busy) return null;
      setPatch({ busy: true, lastError: null, selectedTaskId: id });
      try {
        await runApi.retryTask(runId, id);
        toast(`正在再跑 ${id}…`);
        if (typeof deps.onPhaseRun === "function") {
          try {
            deps.onPhaseRun();
          } catch (_) {}
        }
        await after();
        return { ok: true, runId, taskId: id };
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ lastError: msg });
        toast(msg);
        return null;
      } finally {
        setPatch({ busy: false });
      }
    },

    /**
     * Open external terminal for a task (advanced).
     * @param {Record<string, unknown>} args
     */
    async openTerminal(args) {
      try {
        return await runApi.openTaskTerminal(args);
      } catch (e) {
        const msg = e?.message || String(e);
        toast(msg);
        return null;
      }
    },

    async openMonitorWindow(args) {
      try {
        return await runApi.openMonitorWindow(args || {});
      } catch (e) {
        toast(e?.message || String(e));
        return null;
      }
    },
  };
}

export default createRunViewModel;
