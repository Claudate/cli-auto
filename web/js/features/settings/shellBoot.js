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

/** Workspace / plan-job poll tick (no business policy). */
export function startPolling(intervalMs = 2000) {
  const st = state();
  if (!st) return;
  clearInterval(st.pollTimer);
  st.pollTimer = setInterval(() => {
    st.now = Date.now();
    // 规划轮询不绑死 workspace：切到设置/帮助/环境检查也继续
    if (st.planJobId && st.phase === "planning") {
      if (typeof window.refreshPlanJob === "function") {
        window.refreshPlanJob().catch(() => {});
      }
    }
    if (st.page === "workspace" && st.selectedPath) {
      if (typeof window.loadProjects === "function") {
        window.loadProjects().catch(() => {});
      }
      if (typeof window.loadLive === "function") {
        window.loadLive().catch(() => {});
      }
    } else if (st.page === "chat" && st.selectedPath) {
      // Keep live SoT fresh while authoring so plan-card canExec unlocks when
      // a background / other-desk run ends (loadLive re-paints chat on lock flip).
      if (typeof window.loadLive === "function") {
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
      startPolling(1500);
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
    startPolling();
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
