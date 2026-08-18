/**
 * [INPUT]: 经典 script 已加载的全局（state/showPage/…）+ DOM
 * [OUTPUT]: window.ccoApp / ccoGateway / ccoChat / ccoSplit / ccoRun / ccoResult / ccoSettings / ccoProject / ccoTemplates / ccoSelectUi · phase 壳接线 · P4-2 `#view-ring` 段控委托（wireShellNav · dataset.ccoA2Wired 守卫）
 * [POS]: A2–A5 ESM 入口（type=module）；旧全局仍可用（strangler）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * Load order (index.html):
 *   classic: state → flow → templates(facade) → plan → monitor → result → log → chat → doctor
 *   (A5-2f D3: split.js removed; ccoSplit from this module is sole three-col path)
 *   module:  main.js（defer 于经典脚本之后）
 *
 * Target module graph (arch §2.5):
 *   main.js
 *     app/AppViewModel.js + routes.js + wireRunResult.js
 *     shared/{gateway,store,statusUi,markdown,shellUi,selectUi}.js  ← D9 + selectUi
 *     features/chat/{ChatViewModel,chatApi,installChat,...}.js  ← A5-2a
 *     features/project/{…, installProject}.js  ← A5-2b-fin D5
 *     features/split/{…, splitFillMeta}.js   ← A3 + A5-2b
 *     features/run/{…, loadLive, log*}.js    ← A4 + A5-2b + A5-2c log
 *     features/result/{…}.js                 ← A4
 *     features/settings/{…}.js               ← A5-2d doctor/settings/meta/monitor
 *     features/templates/{…}.js              ← P-ship-D D7 冷启动模板 · 拆分摘要写回
 *   window.ccoChat / ccoProject / ccoSplit / ccoLoadLive / ccoRun / ccoResult / ccoSettings / ccoLog / ccoTemplates / ccoSelectUi
 *   (legacy: chat.js · plan.js · log.js · doctor.js · templates.js facades)
 */

import gateway from "./shared/gateway.js";
import { installStatusUi } from "./shared/statusUi.js";
import { installMarkdown } from "./shared/markdown.js";
import { installShellUi } from "./shared/shellUi.js";
import { installConfirmDialog } from "./shared/confirmDialog.js";
import { installSelectUi } from "./shared/selectUi.js";
import { installThemePreference } from "./shared/themePreference.js";
import { installIconsGlobal, hydrateIcons } from "./shared/icons.js";
import { installClickOutsideGlobal } from "./shared/clickOutside.js";
import {
  paintWorkStyleChooser,
  applyTemplateOrder,
} from "./shared/workStyle.js";
import { createAppViewModel } from "./app/AppViewModel.js";
import { wireRunResult } from "./app/wireRunResult.js";
import { createChatViewModel } from "./features/chat/ChatViewModel.js";
import * as chatApi from "./features/chat/chatApi.js";
import { createChatDesk } from "./features/chat/installChat.js";
import { installProjectHostGlobals } from "./features/project/installProject.js";
import { createSplitViewModel } from "./features/split/SplitViewModel.js";
import { bindSplitView } from "./features/split/SplitView.js";
import * as splitApi from "./features/split/splitApi.js";
import { fillSplitMeta } from "./features/split/splitFillMeta.js";
import {
  loadLive as loadLiveFeature,
  ensureSelectedTask as ensureSelectedTaskFeature,
} from "./features/run/loadLive.js";
import { createLogDesk } from "./features/run/logDesk.js";
import { installSettingsHost, setSoftSyncHook } from "./features/settings/installSettings.js";
import { installTemplatesHost } from "./features/templates/installTemplates.js";

/** D9: display + shell chrome on window before boot renders lists/pages. */
installStatusUi(window);
installMarkdown(window);
installShellUi(window);
installConfirmDialog(window);
installIconsGlobal();
try {
  hydrateIcons(document);
} catch (_) {}
/** shell-chrome B2：展开的 details/菜单点空白收起 */
try {
installClickOutsideGlobal(window);
} catch (_) {}
installThemePreference();
/** Shared form control: macOS-style selects (keep native .value / change). */
installSelectUi();

/** IPC hub first — classic requireGateway() + settings boot wait on it. */
window.ccoGateway = gateway;

/** P-ship-D D7: plan templates + split summary write-back (classic templates.js is facade). */
const templatesDesk = installTemplatesHost();
window.ccoTemplates = templatesDesk;

/** A5-2d: settings/doctor/meta/open_monitor + UI event table + cold boot via gateway. */
const settingsDesk = installSettingsHost({ autoBoot: true });
window.ccoSettings = settingsDesk;

function legacyState() {
  return typeof window.state === "object" && window.state ? window.state : {};
}

function bridgeShowPage(name) {
  if (typeof window.showPage === "function") {
    window.showPage(name);
  }
}

const appVm = createAppViewModel({
  showPage: bridgeShowPage,
  getLegacyState: () => {
    const s = legacyState();
    return {
      page: s.page,
      phase: s.phase,
      selectedPath: s.selectedPath,
      live: s.live,
      planJobId: s.planJobId,
    };
  },
  setLegacySelectedPath: (path) => {
    const s = legacyState();
    if (s) s.selectedPath = path;
  },
  setLegacyPhase: (p) => {
    const s = legacyState();
    if (s) s.phase = p;
  },
  onPhaseChange: (phase, snap) => {
    try {
      // A2 调试角标已撤：主路径不展示 phase 胶囊（顶栏/页内文案已够）
      const el = document.getElementById("cco-app-phase-label");
      if (el) {
        el.textContent = "";
        el.hidden = true;
      }
      document.body.dataset.ccoAppPhase = phase;
      if (snap?.projectPath) {
        document.body.dataset.ccoProject = snap.projectPath;
      }
    } catch (_) {}
  },
});

const chatVm = createChatViewModel({
  projectPath: legacyState().selectedPath || null,
});

/** A5-2a: full chat desk (sessions/stream/save/rail) via gateway; classic chat.js is facade. */
const chatDesk = createChatDesk({
  projectPath: legacyState().selectedPath || null,
});

/** A4 run/result bridges (ccoRun / ccoResult). */
const runResult = wireRunResult({
  appVm,
  legacyState,
});
const { runVm, resultVm, runApi, resultApi } = runResult;

/** A5-2c: log virtual list + CLI board via features/run/log*; classic log.js is facade. */
const logDesk = createLogDesk();
window.ccoLog = logDesk;

/**
 * After confirm_start success: legacy Mode B → running + AppViewModel.goRun.
 * Keeps chip / loadLive behavior from classic confirmAndStart.
 */
function applyConfirmedRun(out) {
  const s = legacyState();
  if (!s) return;
  if (out?.job) {
    s.planJob = out.job;
  } else if (s.planJob) {
    s.planJob = {
      ...s.planJob,
      status: "confirmed",
      run_id: out?.runId || s.planJob.run_id || null,
    };
  }
  s.phase = "running";
  s.confirmEditing = false;
  s.returnPhaseAfterConfirm = null;
  s.selectedTaskId = null;
  s.planCollapsed = true;
  s.closedPanels = {};
  // 新开跑：清 SQLite dismiss（服务端 confirm 也会清；前端再调一次防竞态）
  try {
    if (s.selectedPath && window.ccoGateway?.projectClearDismissedRun) {
      window.ccoGateway.projectClearDismissedRun(s.selectedPath).catch(() => {});
    }
  } catch (_) {}
  try {
    if (s.selectedPath && s.planSessions) delete s.planSessions[s.selectedPath];
  } catch (_) {}
  if (typeof window.setAssignBusy === "function") window.setAssignBusy(false);
  if (typeof window.renderPhasePanels === "function") window.renderPhasePanels();
  if (typeof window.renderPlanPicker === "function") window.renderPlanPicker();
  if (typeof window.updateSplitPlanChip === "function") window.updateSplitPlanChip();
  if (typeof window.updateBgPlanBanner === "function") window.updateBgPlanBanner();
  // A3-4 / A4: shell phase → run (clear result latch for new run)
  try {
    runResult.clearResultLatch();
    if (s.selectedPath) appVm.selectProject(s.selectedPath);
    appVm.goRun();
  } catch (e) {
    console.error("[ccoSplit] goRun", e);
  }
  setTimeout(() => {
    if (typeof window.loadLive === "function") window.loadLive().catch(() => {});
    if (typeof window.loadProjects === "function") {
      window.loadProjects().catch(() => {});
    }
    if (typeof window.renderProjectList === "function") {
      try {
        window.renderProjectList();
      } catch (_) {}
    }
  }, 0);
}

const splitVm = createSplitViewModel({
  onJobUpdated: (job) => {
    const s = legacyState();
    if (!s) return;
    s.planJob = job;
    s.planJobId = job?.job_id || job?.jobId || s.planJobId;
    if (typeof window.stashPlanSession === "function") {
      try {
        window.stashPlanSession(s.selectedPath);
      } catch (_) {}
    }
  },
  onConfirmed: (out) => {
    applyConfirmedRun(out);
  },
  onPhaseRun: () => {
    // resume path also lands here
    const s = legacyState();
    if (s) {
      s.phase = "running";
      s.confirmEditing = false;
      s.returnPhaseAfterConfirm = null;
    }
    try {
      runResult.clearResultLatch();
      appVm.goRun();
    } catch (_) {}
    if (typeof window.renderPhasePanels === "function") window.renderPhasePanels();
    if (typeof window.renderPlanPicker === "function") window.renderPlanPicker();
    if (typeof window.updateSplitPlanChip === "function") window.updateSplitPlanChip();
    setTimeout(() => {
      if (typeof window.loadLive === "function") window.loadLive().catch(() => {});
      if (typeof window.loadProjects === "function") {
        window.loadProjects().catch(() => {});
      }
    }, 600);
  },
});

const splitView = bindSplitView(splitVm, {
  getLegacy: () => {
    const s = legacyState();
    return {
      planJob: s.planJob,
      planJobId: s.planJobId,
      confirmTaskId: s.confirmTaskId,
      confirmEditing: s.confirmEditing,
      phase: s.phase,
    };
  },
  syncLegacy: (patch) => {
    const s = legacyState();
    if (!s) return;
    if (patch.planJob !== undefined) s.planJob = patch.planJob;
    if (patch.planJobId !== undefined) s.planJobId = patch.planJobId;
    if (patch.confirmTaskId !== undefined) s.confirmTaskId = patch.confirmTaskId;
    if (patch.confirmEditing !== undefined) s.confirmEditing = patch.confirmEditing;
  },
  afterMutate: () => {
    if (typeof window.renderPlanPicker === "function") {
      try {
        window.renderPlanPicker();
      } catch (_) {}
    }
    if (typeof window.stashPlanSession === "function") {
      try {
        window.stashPlanSession(legacyState().selectedPath);
      } catch (_) {}
    }
    if (typeof window.updateSplitPlanChip === "function") {
      try {
        window.updateSplitPlanChip();
      } catch (_) {}
    }
  },
  fillMeta: (job) => {
    // Prefer feature module; plan.js may still set window.ccoSplitFillMeta
    if (typeof window.ccoSplitFillMeta === "function") {
      try {
        window.ccoSplitFillMeta(job);
        return;
      } catch (e) {
        console.error("[ccoSplit] fillMeta bridge", e);
      }
    }
    fillSplitMeta(job, {
      runLocked:
        typeof window.hasActiveRun === "function"
          ? window.hasActiveRun()
          : false,
      paused:
        typeof window.isRunPaused === "function" ? window.isRunPaused() : false,
      editing: !!legacyState().confirmEditing,
      planJobId: legacyState().planJobId,
    });
  },
});

/** Expose for classic scripts + DevTools / tests. */
window.ccoGateway = gateway;
window.ccoApp = appVm;
window.ccoChat = {
  ...chatDesk,
  vm: chatVm,
  api: chatApi,
  /** A2/A5: list + send keep ViewModel busy flags for softSync. */
  async listSessions(project) {
    const path = project || legacyState().selectedPath;
    chatVm.setProject(path);
    try {
      return await chatDesk.listSessions(path);
    } catch (e) {
      return chatVm.loadSessions();
    }
  },
  async send(args) {
    const path = args?.project || legacyState().selectedPath;
    chatVm.setProject(path);
    // Prefer desk→chatApi (same DTO); VM tracks busy for shell
    return chatVm.send({
      message: args?.message || "",
      sessionId: args?.sessionId,
      attachments: args?.attachments,
    });
  },
  async savePlan(args) {
    return chatApi.savePlan(args);
  },
};

/**
 * A3/A5-2b split desk bridge — plan.js delegates confirm path here.
 * IPC only via features/split → gateway (no start_run).
 * fillMeta lives in features/split/splitFillMeta (not plan.js).
 */
window.ccoSplitFillMeta = function ccoSplitFillMeta(job) {
  fillSplitMeta(job, {
    runLocked:
      typeof window.hasActiveRun === "function" ? window.hasActiveRun() : false,
    paused:
      typeof window.isRunPaused === "function" ? window.isRunPaused() : false,
    editing: !!legacyState().confirmEditing,
    planJobId: legacyState().planJobId,
  });
};

window.ccoSplit = {
  vm: splitVm,
  api: splitApi,
  view: splitView,
  fillMeta: (job) => window.ccoSplitFillMeta(job),
  render() {
    const job = legacyState().planJob;
    if (job && typeof window.ccoSplitFillMeta === "function") {
      try {
        window.ccoSplitFillMeta(job);
      } catch (e) {
        console.error("[ccoSplit] fillMeta", e);
      }
    }
    splitView.render();
    if (typeof window.refreshSplitQualityOpen === "function") {
      try {
        window.refreshSplitQualityOpen(legacyState().planJob);
      } catch (_) {}
    }
    paintSplitExtraChrome();
  },
  beginEdit: () => splitView.beginEdit(),
  cancelEdit: () => splitView.cancelEdit(),
  saveEdit: () => splitView.saveEdit(),
  deleteTask: () => splitView.deleteTask(),
  confirmAndStart: (opts) =>
    splitView.confirmAndStart({
      ensureDoctor:
        opts?.ensureDoctor ||
        (typeof window.ensureDoctor === "function"
          ? () => window.ensureDoctor(true)
          : undefined),
    }),
  goSplitPhase() {
    const s = legacyState();
    if (s.selectedPath) appVm.selectProject(s.selectedPath);
    appVm.goSplit();
  },
};

/**
 * A5-2b: loadLive shell via features/run (IPC → gateway).
 * Classic plan.js keeps the global name; prefers this when present.
 */
window.ccoLoadLive = loadLiveFeature;
window.ccoEnsureSelectedTask = ensureSelectedTaskFeature;

/**
 * A5-2b-fin: project/plan desk after ccoSplit + ccoLoadLive (confirm/open-run only via ccoSplit).
 * Overwrites classic plan.js facade globals.
 */
const projectDesk = installProjectHostGlobals({
  projectPath: legacyState().selectedPath || null,
});
window.ccoProject = projectDesk;

function paintSplitExtraChrome() {
  const s = legacyState();
  const job = s.planJob;
  if (!job) return;
  const runLocked =
    typeof window.hasActiveRun === "function" ? window.hasActiveRun() : false;
  const paused =
    typeof window.isRunPaused === "function" ? window.isRunPaused() : false;
  const editing = !!s.confirmEditing;
  const st = String(job.status || "").toLowerCase();

  const sanitizeBtn = document.getElementById("btn-sanitize-deps");
  if (sanitizeBtn) {
    sanitizeBtn.disabled = !!runLocked || editing || !s.planJobId;
    sanitizeBtn.hidden = !!runLocked;
    if (!runLocked) sanitizeBtn.textContent = "让可并行的真正并行";
  }
  if (typeof window.refreshSplitWritebackBtn === "function") {
    try {
      window.refreshSplitWritebackBtn(runLocked, editing);
    } catch (_) {}
  }
  const backBtn = document.getElementById("btn-confirm-back");
  if (backBtn) {
    const showBack =
      !!runLocked ||
      !!paused ||
      s.returnPhaseAfterConfirm != null ||
      s.phase === "running" ||
      (st === "confirmed" && s.live?.run_id);
    backBtn.hidden = !showBack;
  }
  if (typeof window.updateSplitPlanChip === "function") {
    try {
      window.updateSplitPlanChip();
    } catch (_) {}
  }
}

/**
 * Wire shell intents that don't need full DOM rewrite.
 */
function wireShellNav() {
  const brand = document.getElementById("brand-home");
  if (brand && !brand.dataset.ccoA2Wired) {
    brand.dataset.ccoA2Wired = "1";
    brand.addEventListener(
      "click",
      () => {
        setTimeout(() => appVm.syncFromLegacy(), 0);
      },
      true
    );
  }

  const list = document.getElementById("project-list");
  if (list && !list.dataset.ccoA2Wired) {
    list.dataset.ccoA2Wired = "1";
    list.addEventListener(
      "click",
      (ev) => {
        // P4-2：hover 复制卡内点击只复制，不切项目（复制处理在 shellUi）
        if (ev.target?.closest?.("[data-copy-path], .project-hover-card")) {
          return;
        }
        const btn = ev.target?.closest?.("[data-path]");
        if (!btn) return;
        const path = btn.getAttribute("data-path");
        if (!path) return;
        setTimeout(() => {
          const s = legacyState();
          const proj = (s.projects || []).find((p) => p.path === path);
          appVm.selectProject(path, proj?.name || null);
          appVm.syncFromLegacy();
        }, 0);
      },
      true
    );
  }

  // P4-2 view-ring 段控：拆分|执行|结果|聊天 → AppViewModel 意图（routes 语义不变）
  const ring = document.getElementById("view-ring");
  if (ring && !ring.dataset.ccoA2Wired) {
    ring.dataset.ccoA2Wired = "1";
    ring.addEventListener(
      "click",
      (ev) => {
        const btn = ev.target?.closest?.(".view-ring-item");
        if (!btn?.dataset?.ring) return;
        ev.preventDefault();
        setTimeout(() => {
          const s = legacyState();
          if (s.selectedPath) appVm.selectProject(s.selectedPath);
          const target = btn.dataset.ring;
          if (target === "chat") appVm.goAuthor();
          else if (target === "split") appVm.goSplit();
          else if (target === "run") appVm.goRun();
          else if (target === "result") appVm.goResult();
        }, 0);
      },
      true
    );
  }

  const btnChat = document.getElementById("btn-open-chat");
  if (btnChat && !btnChat.dataset.ccoA2Wired) {
    btnChat.dataset.ccoA2Wired = "1";
    btnChat.addEventListener(
      "click",
      () => {
        setTimeout(() => {
          const s = legacyState();
          if (s.selectedPath) {
            appVm.selectProject(s.selectedPath);
          }
          appVm.goAuthor();
          // Also sync chat sessions when navigating to chat page
          softSyncFromLegacy();
        }, 0);
      },
      true
    );
  }

  const btnAnalyze = document.getElementById("btn-pp-analyze");
  if (btnAnalyze && !btnAnalyze.dataset.ccoA2Wired) {
    btnAnalyze.dataset.ccoA2Wired = "1";
    btnAnalyze.addEventListener(
      "click",
      () => {
        setTimeout(() => {
          const s = legacyState();
          if (s.selectedPath) appVm.selectProject(s.selectedPath);
          if (s.phase === "planning" || s.phase === "confirm" || s.planJobId) {
            appVm.goSplit();
          } else {
            appVm.syncFromLegacy();
          }
        }, 50);
      },
      true
    );
  }

  const btnConfirm = document.getElementById("btn-confirm-start");
  if (btnConfirm && !btnConfirm.dataset.ccoA3Wired) {
    btnConfirm.dataset.ccoA3Wired = "1";
    btnConfirm.addEventListener(
      "click",
      () => {
        setTimeout(() => {
          try {
            appVm.syncFromLegacy();
          } catch (_) {}
        }, 100);
      },
      false
    );
  }
}

function softSyncFromLegacy() {
  const s = legacyState();
  if (s.selectedPath) {
    const proj = (s.projects || []).find((p) => p.path === s.selectedPath);
    appVm.selectProject(s.selectedPath, proj?.name || null);
  }
  appVm.syncFromLegacy();
  const path = s.selectedPath || null;
  if (path !== chatVm.getSnapshot().projectPath) {
    chatVm.setProject(path);
    // P2: chat session list/messages are now event-driven (open chat page /
    // switch project / send message). Only mirror projectPath here so the VM
    // stays in sync; do NOT pull session list + session every 2s poll tick.
    // Previous softSync called loadChatSessionList + loadChatSession on every
    // tick — 2 extra IPC roundtrips per 2s, plus a full chat re-render.
    if (s.page === "chat") {
      // Only when the user is already looking at chat do we mirror the project
      // into the chat VM so the next page paint is correct. Session loading is
      // triggered by openChatPage / sendChatMessage paths, not by polling.
      if (typeof chatDesk.loadChatSession === "function") {
        chatDesk.loadChatSession().catch(() => {});
      }
    }
  }
  if (s.planJob && (s.phase === "confirm" || s.phase === "planning")) {
    try {
      splitVm.setJob(s.planJob, {
        jobId: s.planJobId,
        selectedTaskId: s.confirmTaskId,
        editing: s.confirmEditing,
      });
    } catch (_) {}
  }
}

/**
 * softSync is invoked by shellBoot.startPolling on the same 2s tick (passed via
 * deps.softSync). No second setInterval here — it used to double IPC + double
 * render every 2s and was the main cause of click latency under load.
 */
function boot() {
  wireShellNav();
  // Register softSync into the single 2s poll tick (replaces duplicate setInterval)
  try {
    setSoftSyncHook(() => {
      if (document.hidden) return;
      softSyncFromLegacy();
    });
  } catch (_) {}
  try {
    hydrateIcons(document);
  } catch (_) {}
  try {
    paintWorkStyleChooser();
    applyTemplateOrder();
  } catch (e) {
    console.error("[cco main] workStyle", e);
  }
  try {
    softSyncFromLegacy();
  } catch (e) {
    console.error("[cco main] softSync", e);
  }
  const delays = [300, 800, 2000];
  delays.forEach((ms) => {
    setTimeout(() => {
      try {
        softSyncFromLegacy();
      } catch (_) {}
    }, ms);
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}

export {
  appVm,
  chatVm,
  splitVm,
  runVm,
  resultVm,
  gateway,
  chatApi,
  splitApi,
  runApi,
  resultApi,
};
