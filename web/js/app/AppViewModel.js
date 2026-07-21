/**
 * [INPUT]: store · routes · gateway（项目列表可选）
 * [OUTPUT]: 壳导航意图 → phase/project；View 只绑 DOM
 * [POS]: A2-2 AppViewModel；禁止写 Mode B / confirm / soft-fill 策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createStore, createAppSnapshot, PHASES } from "../shared/store.js";
import {
  phaseToPage,
  phaseToLegacyModeB,
  legacyToPhase,
  PHASE_TITLE,
} from "./routes.js";

/**
 * @typedef {"author"|"split"|"run"|"result"} AppPhase
 * @typedef {{
 *   showPage?: (name: string) => void,
 *   getLegacyState?: () => { page?: string, phase?: string, selectedPath?: string|null, live?: unknown, planJobId?: string|null },
 *   setLegacySelectedPath?: (path: string|null) => void,
 *   setLegacyPhase?: (p: string) => void,
 *   onPhaseChange?: (phase: AppPhase, snap: object) => void,
 * }} AppVmDeps
 */

/**
 * Shell ViewModel: holds project + phase; navigation is intent-only.
 * @param {AppVmDeps} [deps]
 */
export function createAppViewModel(deps = {}) {
  const store = createStore(createAppSnapshot());

  function snap() {
    return store.get();
  }

  function emit() {
    const s = snap();
    try {
      document.body.dataset.ccoAppPhase = s.phase;
      if (s.projectPath) {
        document.body.dataset.ccoProject = s.projectPath;
      } else {
        delete document.body.dataset.ccoProject;
      }
    } catch (_) {}
    if (typeof deps.onPhaseChange === "function") {
      try {
        deps.onPhaseChange(s.phase, s);
      } catch (e) {
        console.error("[AppViewModel] onPhaseChange", e);
      }
    }
  }

  /**
   * Apply phase → legacy page bridge (strangler). Keeps project selection.
   * @param {AppPhase} phase
   */
  function applyPhaseToDom(phase) {
    const s = snap();
    const page = phaseToPage(phase, { hasProject: !!s.projectPath });
    if (typeof deps.setLegacyPhase === "function") {
      // Only set Mode B hint when entering workspace phases; leave confirm
      // machine alone if already planning/confirm under split.
      const legacy = deps.getLegacyState?.() || {};
      const modeB = legacy.phase;
      if (phase === "split") {
        if (modeB !== "planning" && modeB !== "confirm") {
          deps.setLegacyPhase(phaseToLegacyModeB(phase));
        }
      } else if (phase === "run" || phase === "result" || phase === "author") {
        // author: do not force pick if user mid-flow elsewhere
        if (phase !== "author") {
          deps.setLegacyPhase(phaseToLegacyModeB(phase));
        }
      }
    }
    if (typeof deps.showPage === "function") {
      deps.showPage(page);
    }
    emit();
  }

  /** @param {AppPhase} phase */
  function goPhase(phase) {
    if (!PHASES.includes(phase)) {
      console.warn("[AppViewModel] unknown phase", phase);
      return snap();
    }
    store.set({ ...snap(), phase });
    applyPhaseToDom(phase);
    return snap();
  }

  return {
    store,
    getSnapshot: snap,
    subscribe: (fn) => store.subscribe(fn),
    phases: PHASES,
    phaseTitle: (p) => PHASE_TITLE[p] || p,

    /** Cold start / 回欢迎：清选中项目，phase→author */
    goHome() {
      store.set({
        ...snap(),
        projectPath: null,
        projectName: null,
        phase: "author",
      });
      if (typeof deps.setLegacySelectedPath === "function") {
        deps.setLegacySelectedPath(null);
      }
      applyPhaseToDom("author");
      return snap();
    },

    /**
     * Select project without losing it across phase switches.
     * @param {string|null} path
     * @param {string|null} [name]
     */
    selectProject(path, name = null) {
      const s = snap();
      store.set({
        ...s,
        projectPath: path,
        projectName: name || s.projectName,
      });
      if (typeof deps.setLegacySelectedPath === "function") {
        deps.setLegacySelectedPath(path);
      }
      emit();
      return snap();
    },

    goAuthor() {
      return goPhase("author");
    },
    goSplit() {
      return goPhase("split");
    },
    goRun() {
      return goPhase("run");
    },
    goResult() {
      return goPhase("result");
    },
    goPhase,

    /** Sync VM from legacy globals after old UI mutates page/phase. */
    syncFromLegacy() {
      const legacy = deps.getLegacyState?.() || {};
      const path = legacy.selectedPath ?? snap().projectPath;
      const phase = legacyToPhase(legacy);
      store.set({
        ...snap(),
        projectPath: path || null,
        phase,
      });
      emit();
      return snap();
    },
  };
}

export default createAppViewModel;
