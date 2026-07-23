/**
 * [INPUT]: resultApi · live / inspect_loop 展示状态
 * [OUTPUT]: 结果台意图（回补 / 接受残留 / 结束本轮）；不写 inspect 门禁
 * [POS]: A4-3 ResultViewModel；IPC 只经 resultApi → gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createStore } from "../../shared/store.js";
import * as resultApi from "./resultApi.js";

/**
 * @typedef {{
 *   live: object|null,
 *   busy: boolean,
 *   lastError: string|null,
 *   lastToast: string|null,
 * }} ResultSnap
 */

/**
 * @param {{
 *   onAfterMutate?: () => void | Promise<void>,
 *   onPhaseRun?: () => void,
 *   onPhaseResult?: () => void,
 *   onFinishRound?: () => void,
 *   toast?: (msg: string) => void,
 *   promptNote?: (defaultNote: string) => string|null,
 * }} [deps]
 */
export function createResultViewModel(deps = {}) {
  const store = createStore({
    live: null,
    busy: false,
    lastError: null,
    lastToast: null,
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
        console.error("[ResultViewModel] onAfterMutate", e);
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

    /** @param {object|null} live */
    setLive(live) {
      return setPatch({ live: live || null });
    },

    /**
     * Rework wave via app start_rework (not confirm, not start_run).
     * On success → phase run (new wave).
     */
    async startRework() {
      const runId = runIdOf(snap().live);
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无运行记录可回补");
        return null;
      }
      if (snap().busy) return null;
      setPatch({ busy: true, lastError: null });
      try {
        const res = await resultApi.startRework(runId);
        toast(res?.message || `回补已启动${res?.run_id ? " · " + res.run_id : ""}`);
        if (typeof deps.onPhaseRun === "function") {
          try {
            deps.onPhaseRun();
          } catch (_) {}
        }
        await after();
        return res;
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
     * Accept residual with optional note (DTO path only).
     */
    async acceptResidual() {
      const runId = runIdOf(snap().live);
      if (!runId || typeof runId !== "string" || !runId.trim()) {
        toast("无运行记录");
        return null;
      }
      if (snap().busy) return null;
      const defaultNote = "用户显式接受巡检残留";
      let note = defaultNote;
      if (typeof deps.promptNote === "function") {
        const n = deps.promptNote(defaultNote);
        if (n === null) return null;
        note = n || null;
      } else if (typeof window !== "undefined" && typeof window.prompt === "function") {
        const n = window.prompt(
          "接受残留说明（将写入 handoff open_risks）",
          defaultNote
        );
        if (n === null) return null;
        note = n || null;
      }
      setPatch({ busy: true, lastError: null });
      try {
        await resultApi.acceptResidual(runId, note);
        toast("已记录「接受残留」");
        if (typeof deps.onPhaseResult === "function") {
          try {
            deps.onPhaseResult();
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
     * Soft end + P2-2 writeback last_summary (best-effort; never blocks finish).
     */
    async finishRound() {
      const runId = runIdOf(snap().live);
      if (runId && typeof runId === "string" && runId.trim()) {
        try {
          await resultApi.writebackMemory(runId);
        } catch (e) {
          // Memory failure must not block ending the round (P2 gate).
          console.warn("[ResultViewModel] writebackMemory", e);
        }
      }
      if (typeof deps.onFinishRound === "function") {
        try {
          await deps.onFinishRound();
        } catch (e) {
          console.error("[ResultViewModel] onFinishRound", e);
        }
      }
      toast("本轮已结束 · 可回聊天写下一份计划");
    },
  };
}

export default createResultViewModel;
