/**
 * [INPUT]: runApi · live DTO 展示状态
 * [OUTPUT]: 执行台意图（停 / 续 / 停步 / 开终端）；不写 stall 策略
 * [POS]: A4-1 RunViewModel；IPC 只经 runApi → gateway；detailCollapsed 为会话内几何瞬态
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
 *   detailCollapsed: boolean,
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
    detailCollapsed: false,
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

  /**
   * B1: Patch a single task in the live object (functional update).
   * @param {object} live
   * @param {string} taskId
   * @param {object} patch
   */
  function patchTask(live, taskId, patch) {
    if (!live || !Array.isArray(live.tasks)) return;
    const updated = {
      ...live,
      tasks: live.tasks.map((t) =>
        t.task_id === taskId ? { ...t, ...patch } : t
      ),
    };
    setPatch({ live: updated });
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

    /**
     * B1: Handle incoming run event (incremental state update).
     * @param {object} evt - { type, payload, run_id }
     */
    handleRunEvent(evt) {
      const current = snap();
      const live = current.live;
      if (!live || !evt || !evt.type) return;

      // Only process events for the current run
      if (evt.run_id && live.run_id && evt.run_id !== live.run_id) return;

      switch (evt.type) {
        case "task_start":
          if (evt.payload?.task_id) {
            patchTask(live, evt.payload.task_id, { status: "running" });
          }
          break;

        case "task_end":
          if (evt.payload?.task_id && evt.payload?.status) {
            patchTask(live, evt.payload.task_id, { status: evt.payload.status });
          }
          break;

        case "checkpoint":
          // A1: Set checkpoint flag to unlock "从这里继续" button
          setPatch({ live: { ...live, has_checkpoint: true } });
          break;

        case "permission_tier":
          // A3bis: Set permission tier for safety label
          if (evt.payload?.tier) {
            setPatch({ live: { ...live, permission_tier: evt.payload.tier } });
          }
          break;

        case "run_end":
          // B2: Delay 300ms before full refresh to allow RunState to persist
          setTimeout(() => {
            if (typeof window.loadLive === "function") {
              window.loadLive().catch(() => {});
            }
          }, 300);
          break;

        case "run_start":
          // Skip: run_start handled by full loadLive after navigation
          break;

        default:
          // Unknown event type: ignore
          break;
      }
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

    /** 右次级列折叠：几何瞬态只在会话内，不入 localStorage。 */
    toggleDetailCollapsed() {
      return setPatch({ detailCollapsed: !snap().detailCollapsed });
    },

    setDetailCollapsed(v) {
      return setPatch({ detailCollapsed: !!v });
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
     * @param {{ provider?: string }} [opts] — optional channel override.
     */
    async retryTask(taskId, opts) {
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
        await runApi.retryTask(runId, id, opts);
        toast(opts?.provider ? `已切换到 ${opts.provider}，正在再跑 ${id}…` : `正在再跑 ${id}…`);
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

    /**
     * B1: Incremental patch one task (event-driven state update).
     * @param {string} taskId
     * @param {Partial<Task>} patch
     */
    patchTask(taskId, patch) {
      const prev = snap();
      if (!prev.live?.tasks) return;
      const nextTasks = prev.live.tasks.map((t) =>
        t.id === taskId ? { ...t, ...patch } : t
      );
      store.set({
        ...prev,
        live: { ...prev.live, tasks: nextTasks },
      });
    },
  };
}

export default createRunViewModel;
