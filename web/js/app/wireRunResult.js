/**
 * [INPUT]: AppViewModel · legacy state · features/run + features/result
 * [OUTPUT]: window.ccoRun / ccoResult 桥；终态 goResult；回补 goRun
 * [POS]: A4 main 接线抽出（控 main.js 体量）；禁止 start_run 旁路
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createRunViewModel } from "../features/run/RunViewModel.js";
import { bindRunView } from "../features/run/RunView.js";
import * as runApi from "../features/run/runApi.js";
import { createResultViewModel } from "../features/result/ResultViewModel.js";
import { bindResultView } from "../features/result/ResultView.js";
import * as resultApi from "../features/result/resultApi.js";

/**
 * @param {{
 *   appVm: { selectProject: Function, goRun: Function, goResult: Function, goAuthor: Function },
 *   legacyState: () => object,
 *   clearResultLatch?: () => void,
 *   setResultLatch?: (runId: string|null) => void,
 *   getResultLatch?: () => string|null,
 * }} deps
 */
export function wireRunResult(deps) {
  const { appVm, legacyState } = deps;
  let lastGoResultRunId = null;

  function afterRunMutate() {
    return Promise.all([
      typeof window.loadLive === "function"
        ? window.loadLive().catch(() => {})
        : Promise.resolve(),
      typeof window.loadProjects === "function"
        ? window.loadProjects().catch(() => {})
        : Promise.resolve(),
    ]).then(() => {
      try {
        if (typeof window.renderProjectList === "function") {
          window.renderProjectList();
        }
      } catch (_) {}
      try {
        if (typeof window.renderPlanPicker === "function") {
          window.renderPlanPicker();
        }
      } catch (_) {}
      try {
        if (typeof window.updateSplitPlanChip === "function") {
          window.updateSplitPlanChip();
        }
      } catch (_) {}
    });
  }

  function goShellResult() {
    const s = legacyState();
    // 仅当人还在执行/结果工作区时才切结果台；chat/设置中不拽回
    if (!s || s.page !== "workspace") return;
    if (s.phase === "pick" || s.phase === "planning" || s.phase === "confirm") {
      return;
    }
    if (s.phase === "running") s.phase = "done";
    try {
      appVm.goResult();
    } catch (e) {
      console.error("[ccoRun] goResult", e);
    }
  }

  function goShellRun() {
    lastGoResultRunId = null;
    const s = legacyState();
    if (s) {
      s.phase = "running";
      s.confirmEditing = false;
      s.returnPhaseAfterConfirm = null;
    }
    try {
      if (s?.selectedPath) appVm.selectProject(s.selectedPath);
      appVm.goRun();
    } catch (e) {
      console.error("[ccoRun] goRun", e);
    }
    try {
      if (typeof window.renderPhasePanels === "function") {
        window.renderPhasePanels();
      }
    } catch (_) {}
  }

  const resultVm = createResultViewModel({
    onAfterMutate: afterRunMutate,
    onPhaseRun: () => goShellRun(),
    onPhaseResult: () => goShellResult(),
    onFinishRound: async () => {
      // await：必须等 SQLite dismiss 写完再切页，否则 loadLive 竞态会回绑 paused
      try {
        if (typeof window.dismissRun === "function") {
          await window.dismissRun();
        }
      } catch (e) {
        console.warn("[ccoResult] dismissRun", e);
      }
      // 确保不再停在结果 workspace（dismissRun 内也会清；双保险）
      try {
        if (typeof window.state === "object" && window.state) {
          window.state.phase = "pick";
          window.state.live = null;
        }
      } catch (_) {}
      try {
        if (typeof window.openChatPage === "function") {
          await window.openChatPage();
        } else if (typeof window.showPage === "function") {
          window.showPage("chat");
        }
      } catch (_) {}
      try {
        appVm.goAuthor();
      } catch (_) {}
      try {
        if (typeof window.renderProjectList === "function") {
          window.renderProjectList();
        }
      } catch (_) {}
    },
    toast: (msg) => {
      if (typeof window.toast === "function") window.toast(msg);
    },
  });

  const resultView = bindResultView(resultVm, {
    getLegacy: () => {
      const s = legacyState();
      return { live: s.live };
    },
  });

  const runVm = createRunViewModel({
    onAfterMutate: afterRunMutate,
    onPhaseResult: () => goShellResult(),
    onPhaseRun: () => goShellRun(),
    toast: (msg) => {
      if (typeof window.toast === "function") window.toast(msg);
    },
  });

  const runView = bindRunView(runVm, {
    getLegacy: () => {
      const s = legacyState();
      return {
        live: s.live,
        phase: s.phase,
        selectedTaskId: s.selectedTaskId,
        dashCollapsed: s.taskDashCollapsed,
        isMonitorWindow: s.isMonitorWindow,
      };
    },
    syncLegacy: (patch) => {
      const s = legacyState();
      if (!s) return;
      if (patch.dashCollapsed !== undefined) {
        s.taskDashCollapsed = !!patch.dashCollapsed;
        try {
          localStorage.setItem(
            "cco.taskDashCollapsed",
            s.taskDashCollapsed ? "1" : "0"
          );
        } catch (_) {}
      }
      if (patch.selectedTaskId !== undefined) {
        s.selectedTaskId = patch.selectedTaskId;
      }
    },
    isMonitorWindow: () => !!legacyState().isMonitorWindow,
    renderInspectAndResult: (live, tasks, ctx) => {
      resultView.renderInspectAndResult(live, tasks, ctx);
    },
    onFinished: (live) => {
      const id = live?.run_id || null;
      if (!id || id === lastGoResultRunId) return;
      const s = legacyState();
      // 仅 workspace 执行/结果台进结果态；chat / 拆分台不抢屏
      // loadLive 可能已把 phase 从 running→done，仍须 goShellResult 一次
      if (!s || s.page !== "workspace") return;
      if (s.phase !== "running" && s.phase !== "done") return;
      if (!live?.run_id) return;
      lastGoResultRunId = id;
      goShellResult();
    },
  });

  window.ccoRun = {
    vm: runVm,
    api: runApi,
    view: runView,
    renderProgress() {
      return runView.renderProgress();
    },
    stopAll: () => runView.stopAll(),
    resume: () => runView.resume(),
    stopTask: (id) => runView.stopTask(id),
    /** 失败步骤「再跑一次」：只重跑该任务，不重拆 */
    retryTask: (id) => runView.retryTask(id),
    toggleDash: () => runView.toggleDash(),
    openMonitorWindow: (args) => runVm.openMonitorWindow(args),
  };

  window.ccoResult = {
    vm: resultVm,
    api: resultApi,
    view: resultView,
    renderResultDesk(live, tasks, ctx) {
      resultView.renderResultDesk(live, tasks, ctx);
    },
    renderInspectLoopStrip(live, finished, active) {
      resultView.renderInspectLoopStrip(live, finished, active);
    },
    startRework: () => resultView.startRework(),
    acceptResidual: () => resultView.acceptResidual(),
    finishRound: () => resultView.finishRound(),
  };

  return {
    runVm,
    resultVm,
    runApi,
    resultApi,
    /** Call when confirm starts a new business run. */
    clearResultLatch() {
      lastGoResultRunId = null;
    },
    goShellRun,
    goShellResult,
  };
}

export default wireRunResult;
