/**
 * [INPUT]: 任意可序列化快照字段
 * [OUTPUT]: 可订阅薄 store（无 React；VM 用）
 * [POS]: A2-1 shared；单向数据流最小实现
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/**
 * @template T
 * @param {T} initial
 */
export function createStore(initial) {
  let state = initial;
  /** @type {Set<(s: T, prev: T) => void>} */
  const listeners = new Set();

  return {
    get() {
      return state;
    },
    /**
     * Replace or shallow-merge. Pass a function for (prev) => next.
     * @param {T | ((prev: T) => T) | Partial<T>} patch
     * @param {{ merge?: boolean }} [opts] merge=true shallow-assigns Partial
     */
    set(patch, opts) {
      const prev = state;
      let next;
      if (typeof patch === "function") {
        next = patch(prev);
      } else if (opts?.merge && patch && typeof patch === "object") {
        next = { ...prev, ...patch };
      } else {
        next = patch;
      }
      if (Object.is(next, prev)) return state;
      state = next;
      listeners.forEach((fn) => {
        try {
          fn(state, prev);
        } catch (e) {
          console.error("[store] listener", e);
        }
      });
      return state;
    },
    /** @param {(s: T, prev: T) => void} fn */
    subscribe(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
  };
}

/**
 * Shell-level snapshot used by AppViewModel.
 * phase: author | split | run | result（与 DOM page/mode 映射见 routes.js）
 */
export const PHASES = Object.freeze(["author", "split", "run", "result"]);

export function createAppSnapshot() {
  return {
    /** @type {"author"|"split"|"run"|"result"} */
    phase: "author",
    /** @type {string|null} */
    projectPath: null,
    /** @type {string|null} */
    projectName: null,
  };
}
