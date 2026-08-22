/**
 * [INPUT]: settingsApi · classic globals (loadProjects/selectProject/…)
 * [OUTPUT]: startPolling（含 chat 页 loadLive）· openMonitorWindow · boot · waitTauri
 * [POS]: A5-2d features/settings — 冷启动壳；IPC 只经 settingsApi
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as settingsApi from "./settingsApi.js";

function $(sel) {
  return typeof window.$ === "function"
    ? window.$(sel)
    : document.querySelector(sel);
}

function state() {
  return typeof window !== "undefined" ? window.state : null;
}

function toast(msg) {
  if (typeof window.toast === "function") window.toast(msg);
}

function isLiveStatus(st) {
  return typeof window.isLiveStatus === "function"
    ? window.isLiveStatus(st)
    : false;
}

/**
 * Settings-derived poll interval — same clamp as saveSettings
 * (`poll_interval_secs * 1000`, capped at 5000ms). Falls back on IPC failure.
 */
async function settingsPollMs(fallbackMs = 2000) {
  try {
    const s = await settingsApi.getSettings();
    const st = state();
    // Seed global settings cache (channels catalog) as early as boot poll.
    if (st && s) st.settings = s;
    const secs = Number(s?.poll_interval_secs);
    if (Number.isFinite(secs) && secs > 0) {
      return Math.min(secs * 1000, 5000);
    }
  } catch (_) {}
  return fallbackMs;
}

/**
 * softSync hook: main.js registers AppViewModel/chat session mirror here so
 * the single 2s poll tick also runs VM sync (replaces main.js's duplicate
 * setInterval that doubled IPC + render every tick).
 * @type {null | (() => void)}
 */
let softSyncHook = null;
export function setSoftSyncHook(fn) {
  softSyncHook = typeof fn === "function" ? fn : null;
}

/**
 * B1: Global event subscription state (once per session, not per-VM).
 */
let unsubscribeRunEvents = null;
let eventStaleCounter = 0; // B1: consecutive ticks without events
let eventFailureCounter = 0; // B2: consecutive stale ticks **while a run is live**
let isDegraded = false; // B2: degradation state flag
/** Ticks without events during an active run before showing degrade banner (~8s @2s). */
const DEGRADE_AFTER_LIVE_STALE_TICKS = 4;

/**
 * True when live run status means workers may emit run events.
 * Idle chat / finished history must not count as "event bus failure".
 */
function hasActiveRunForEvents(st) {
  if (!st) return false;
  const runSt = String(st.live?.run_status || "").toLowerCase();
  if (!runSt) return false;
  if (typeof window.isLiveStatus === "function") {
    return !!window.isLiveStatus(runSt);
  }
  return ["running", "starting", "queued", "pausing", "stopping"].includes(runSt);
}

/**
 * B2: Show degradation banner (dismissible, non-blocking).
 * Only meaningful on workspace while a run is active.
 */
function showDegradationBanner() {
  if (isDegraded) return; // Already showing
  const st = state();
  if (!st || st.page !== "workspace" || !hasActiveRunForEvents(st)) return;
  isDegraded = true;
  const banner = document.getElementById("event-degradation-banner");
  if (banner) {
    banner.hidden = false;
    // Keep label stable; close button is in HTML markup — don't wipe children.
    const label = banner.querySelector("span");
    if (label) label.textContent = "实时连接已降级";
    else if (!banner.textContent.trim()) banner.textContent = "实时连接已降级";
  }
}

/**
 * B2: Hide degradation banner.
 */
function hideDegradationBanner() {
  if (!isDegraded) {
    const banner = document.getElementById("event-degradation-banner");
    if (banner) banner.hidden = true;
    return;
  }
  isDegraded = false;
  eventFailureCounter = 0;
  const banner = document.getElementById("event-degradation-banner");
  if (banner) {
    banner.hidden = true;
  }
}

/**
 * B1: Handle incoming run event and dispatch to ccoRun.
 * Any received event proves the bus is alive — always reset degrade counters
 * (even when not on workspace / no selected project).
 * @param {object} evt
 */
function handleRunEvent(evt) {
  // Channel alive: clear stale + failure regardless of page.
  eventStaleCounter = 0;
  eventFailureCounter = 0;
  hideDegradationBanner();

  const st = state();
  if (!st) return;
  // Only dispatch incremental patches on workspace with a project.
  if (st.page !== "workspace" || !st.selectedPath) return;
  if (window.ccoRun?.handleRunEvent) {
    window.ccoRun.handleRunEvent(evt);
  }
}

/** Workspace / plan-job poll tick (no business policy). */
export function startPolling(intervalMs = 2000) {
  const st = state();
  if (!st) return;
  clearInterval(st.pollTimer);

  // B1: Subscribe to run events once (global, not per-VM)
  if (!unsubscribeRunEvents && typeof window !== "undefined") {
    import("../../shared/gateway.js")
      .then(({ subscribeRunEvents }) => {
        if (!subscribeRunEvents) return;
        unsubscribeRunEvents = subscribeRunEvents((evt) => {
          handleRunEvent(evt);
        });
      })
      .catch(() => {});
  }

  st.pollTimer = setInterval(() => {
    st.now = Date.now();

    // B1: Increment stale counter each tick
    eventStaleCounter += 1;

    // 窗口在后台时降频：只保留规划轮询（拆分完成要自动接续），
    // 项目/live 刷新等回到前台的下一个 tick 再恢复。
    if (typeof document !== "undefined" && document.hidden) {
      if (
        st.planJobId &&
        st.phase === "planning" &&
        typeof window.refreshPlanJob === "function"
      ) {
        window.refreshPlanJob().catch(() => {});
      }
      return;
    }
    // single softSync per tick (replaces main.js duplicate setInterval)
    if (softSyncHook) {
      try {
        softSyncHook();
      } catch (_) {}
    }
    // 规划轮询不绑死 workspace：切到设置/帮助/环境检查也继续
    if (st.planJobId && st.phase === "planning") {
      if (typeof window.refreshPlanJob === "function") {
        window.refreshPlanJob().catch(() => {});
      }
    }

    // B2: Degradation — only while a run is live and the bus stays silent.
    // Idle chat / finished runs have no events by design; never treat silence
    // as "实时连接已降级".
    const runLive = hasActiveRunForEvents(st);
    if (runLive && unsubscribeRunEvents) {
      if (eventStaleCounter >= DEGRADE_AFTER_LIVE_STALE_TICKS) {
        eventFailureCounter = eventStaleCounter;
        showDegradationBanner();
      }
    } else {
      if (isDegraded || eventFailureCounter > 0) {
        hideDegradationBanner();
      }
      // Non-workspace (e.g. chat) never expects run events — don't let idle
      // ticks accumulate into an instant degrade when a run starts later.
      if (st.page !== "workspace") {
        eventStaleCounter = 0;
      }
    }

    // B1: Reconciliation logic — if 3+ ticks without events, force loadLive
    const shouldReconcile = eventStaleCounter >= 3;
    // B2: When degraded (live-run silence) OR no event subscription, always poll
    const shouldPoll =
      !unsubscribeRunEvents || isDegraded || shouldReconcile;

    if (st.page === "workspace" && st.selectedPath) {
      if (typeof window.loadProjects === "function") {
        window.loadProjects().catch(() => {});
      }
      // B1/B2: Poll when degraded OR stale counter hit threshold
      if (shouldPoll) {
        if (typeof window.loadLive === "function") {
          window.loadLive().catch(() => {});
        }
        // Idle: reset after reconcile. Live run: keep accumulating stale so
        // DEGRADE_AFTER_LIVE_STALE_TICKS can trip (events alone clear it).
        if (!runLive) {
          eventStaleCounter = 0;
        }
      }
    } else if (st.page === "chat" && st.selectedPath) {
      // T4: idle chat must NOT trigger a full run scan every tick. Only refresh
      // live SoT while a run is actually live (so the plan-card canExec unlocks
      // when it ends — loadLive re-paints chat on the lock flip); run events keep
      // it fresh otherwise. Idle chat with no active run = zero polling scan.
      if (runLive && typeof window.loadLive === "function") {
        window.loadLive().catch(() => {});
      }
    } else if (st.page === "welcome") {
      if (typeof window.loadProjects === "function") {
        window.loadProjects().catch(() => {});
      }
    }
    if (typeof window.updateBgPlanBanner === "function") {
      window.updateBgPlanBanner();
    }
  }, intervalMs);
}

/** P2-4: URL query for detached system window (`?cco_window=monitor`). */
export function parseCcoWindowBoot() {
  try {
    const q = new URLSearchParams(window.location.search || "");
    const role = (q.get("cco_window") || "").trim().toLowerCase();
    let project = q.get("project");
    if (project) {
      try {
        project = decodeURIComponent(project);
      } catch (_) {
        /* keep raw */
      }
    }
    return {
      isMonitor: role === "monitor",
      project: project && project.trim() ? project.trim() : null,
    };
  } catch (_) {
    return { isMonitor: false, project: null };
  }
}

/**
 * Open/focus system monitor window via gateway.
 * Prefer window.ccoRun when present (A4 bridge).
 */
export async function openMonitorWindow() {
  if (window.ccoRun?.openMonitorWindow) {
    return window.ccoRun.openMonitorWindow({
      project: state()?.selectedPath || null,
    });
  }
  if (!settingsApi.isTauriReady()) {
    toast("请在 CCO.app 内使用独立监视窗");
    return;
  }
  try {
    const res = await settingsApi.openMonitorWindow({
      project: state()?.selectedPath || null,
    });
    if (res?.created) toast("已打开独立监视窗（可拖到另一显示器）");
    else toast("已聚焦独立监视窗");
    return res;
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/**
 * Cold-start: meta + projects + optional monitor window / H0 routing.
 * @param {{ bindGlobalUI?: () => void }} [opts]
 */
export async function boot(opts = {}) {
  if (typeof opts.bindGlobalUI === "function") opts.bindGlobalUI();
  else if (typeof window.bindGlobalUI === "function") window.bindGlobalUI();

  let ready = settingsApi.isTauriReady();
  for (let i = 0; !ready && i < 100; i++) {
    await new Promise((r) => setTimeout(r, 50));
    ready = settingsApi.isTauriReady();
  }
  if (!ready) {
    const cs = $("#conn-status");
    if (cs) cs.textContent = "需要通过 CCO.app 启动";
    return;
  }

  const st = state();
  try {
    const meta = await settingsApi.meta();
    const cs = $("#conn-status");
    if (cs) cs.textContent = `桌面应用 · v${meta.version}`;
    // Global channel catalog cache before any workspace click (switch / split).
    try {
      const s = await settingsApi.getSettings();
      if (st && s) st.settings = s;
    } catch (_) {}
    if (typeof window.loadProjects === "function") {
      await window.loadProjects();
    }

    const bootWin = parseCcoWindowBoot();
    if (st) st.isMonitorWindow = !!bootWin.isMonitor;
    if (bootWin.isMonitor) {
      document.body.classList.add("cco-window-monitor");
      if (cs) cs.textContent = `监视窗 · v${meta.version}`;
      let path = bootWin.project;
      if (path && !(st?.projects || []).some((p) => p.path === path)) {
        path = bootWin.project;
      }
      if (!path && (st?.projects || []).length === 1) {
        path = st.projects[0].path;
      }
      if (!path) {
        const active = (st?.projects || []).find(
          (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
        );
        if (active) path = active.path;
      }
      if (path && typeof window.selectProject === "function") {
        await window.selectProject(path);
        if (typeof window.showPage === "function") window.showPage("workspace");
        if (st) st.phase = st.phase === "pick" ? "running" : st.phase;
        try {
          if (typeof window.loadLive === "function") await window.loadLive();
        } catch (_) {}
      } else {
        if (typeof window.showPage === "function") window.showPage("welcome");
        toast("监视窗：请先在主窗选择项目");
      }
      startPolling(await settingsPollMs(1500));
      return;
    }

    // H0 冷启动：仅「有活动 run」的项目自动进执行
    const projects = st?.projects || [];
    const active = projects.find(
      (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
    );
    if (active && typeof window.selectProject === "function") {
      await window.selectProject(active.path);
    } else if (projects.length === 1 && typeof window.selectProject === "function") {
      await window.selectProject(projects[0].path);
    } else if (projects.length > 0 && typeof window.goHome === "function") {
      window.goHome();
    } else if (typeof window.showPage === "function") {
      window.showPage("welcome");
    }

    if (
      st?.selectedPath &&
      st.page === "workspace" &&
      typeof window.hasActiveRun === "function" &&
      !window.hasActiveRun()
    ) {
      const jobSt = String(st.planJob?.status || "").toLowerCase();
      if (st.phase !== "planning" && jobSt !== "planning") {
        if (typeof window.openChatPage === "function") await window.openChatPage();
        else if (typeof window.showPage === "function") window.showPage("chat");
      }
    }
    startPolling(await settingsPollMs());
  } catch (e) {
    console.error(e);
    const cs = $("#conn-status");
    if (cs) cs.textContent = "后端连接异常";
    toast(String(e?.message || e));
  }
}

/**
 * Schedule boot after DOM ready.
 * @param {{ bindGlobalUI?: () => void }} [opts]
 */
export function waitTauri(opts = {}) {
  if (typeof opts.bindGlobalUI === "function") opts.bindGlobalUI();
  else if (typeof window.bindGlobalUI === "function") window.bindGlobalUI();

  const run = () => boot(opts).catch(console.error);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", run);
  } else {
    run();
  }
}
