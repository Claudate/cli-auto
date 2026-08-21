/**
 * [INPUT]: window 全局 · A4+ ccoRun
 * [OUTPUT]: workspace 壳 + classic helper（taskBucket/fitCli…）
 * [POS]: A5-2f D2 ≤200；进度只 ccoRun；无 KPI/stall/tile 副本
 * note: body mode-plan 含 planning/confirm/plan_failed（失败不落 idle）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — monitor workspace shell (A5-2f D2) */

function renderWorkspace() {
  updateWorkspaceTitle();
  const live = state.live;
  // 拆分会话打开时，项目历史 completed live 不算本轮
  // SoT dismiss 已在 project_live_view 过滤；此处只信 liveBelongsToOpenPlan
  const belongs =
    typeof liveBelongsToOpenPlan === "function" ? liveBelongsToOpenPlan() : true;
  const runStatus = belongs ? live?.run_status : null;
  const hasRun = belongs && !!live?.run_id;
  const active = hasRun && isLiveStatus(runStatus);
  const finished =
    hasRun &&
    !active &&
    ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
      String(runStatus || "").toLowerCase()
    );
  // phase 只由 applyEntryRoute / confirm / dismiss / loadLive 写；禁止 render 改 phase
  const body = $("#workspace-body");
  if (body) {
    body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
    if (
      state.phase === "planning" ||
      state.phase === "confirm" ||
      state.phase === "plan_failed"
    ) {
      body.classList.add("mode-plan");
    }
    else if (active) body.classList.add("mode-running");
    else if (finished) body.classList.add("mode-done");
    else body.classList.add("mode-idle");
  }
  try {
    document.body.classList.toggle(
      "cco-progress-first",
      active || finished || !!state.isMonitorWindow
    );
  } catch (_) {}
  renderDoctorWarn();
  renderPhasePanels();
  if (state.phase === "pick" || state.phase === "done" || state.phase === "running") {
    if (!state.selectedPlan && belongs && state.live?.plan_path) {
      state.selectedPlan =
        normalizePlanPath(state.live.plan_path) || state.live.plan_path;
    }
    renderPlanPicker();
  }
  updateTopPlanInfo();
  if (window.ccoRun?.renderProgress) {
    try {
      return window.ccoRun.renderProgress();
    } catch (e) {
      console.error("[renderWorkspace] ccoRun", e);
    }
  }
  ["#run-banner", "#error-summary", "#completion-panel"].forEach((sel) => {
    const el = $(sel);
    if (el) el.hidden = true;
  });
}

/** Legacy: 整板 details 已移除；运行端始终可见。保留名供 facade 调用。 */
function syncMonitorLogsFold() {
  if (window.ccoRun?.view?.syncLogsFold) {
    try {
      return window.ccoRun.view.syncLogsFold();
    } catch (e) {
      console.error("[syncMonitorLogsFold] ccoRun", e);
    }
  }
}

/** Classic name for bindUi; paint is ccoRun only. */
function renderTaskStrip() {
  if (window.ccoRun?.renderProgress) {
    try {
      return window.ccoRun.renderProgress();
    } catch (e) {
      console.error("[renderTaskStrip] ccoRun", e);
    }
  }
}

function savePanelPos() {
  try {
    localStorage.setItem("cco.panelPos", JSON.stringify(state.panelPos || {}));
  } catch (_) {}
}

function taskBucket(st, task) {
  let t = task;
  let s = st;
  if (st && typeof st === "object") {
    t = st;
    s = st.status;
  }
  s = String(s || "").toLowerCase();
  if (isFailedStatus(s)) return "fail";
  // stop ≠ fail: user abort / freeze pending
  if (
    (typeof isStoppedStatus === "function" && isStoppedStatus(s)) ||
    ["stopped", "aborted", "cancelled", "canceled"].includes(s)
  ) {
    return "stop";
  }
  if (isDoneStatus(s)) return "done";
  if (t && typeof isStalledTask === "function" && isStalledTask(t)) return "stall";
  if (isLiveStatus(s) || ["starting", "queued", "running"].includes(s)) return "run";
  return "wait";
}

function cliStatusRank(st, task) {
  const b = taskBucket(st, task);
  return b === "stall"
    ? 0
    : b === "run"
      ? 1
      : b === "wait"
        ? 2
        : b === "done"
          ? 3
          : b === "stop"
            ? 4
            : 5;
}

function sortTasksByStatus(tasks) {
  return (tasks || [])
    .map((t, i) => ({ t, i }))
    .sort(
      (a, b) =>
        cliStatusRank(a.t.status, a.t) - cliStatusRank(b.t.status, b.t) || a.i - b.i
    )
    .map((x) => x.t);
}

function applyCliBodyHeight(h) {
  if (h === "auto" || h === "0" || h === 0) {
    state.cliBodyHeight = "auto";
    localStorage.setItem("cco.cliBodyHeight", "auto");
    document.documentElement.setAttribute("data-cli-h", "auto");
    fitCliBodyHeight();
    return;
  }
  const n = Math.max(160, Math.min(900, Number(h) || 300));
  state.cliBodyHeight = n;
  localStorage.setItem("cco.cliBodyHeight", String(n));
  document.documentElement.removeAttribute("data-cli-h");
  document.documentElement.style.setProperty("--cli-body-h", n + "px");
}

function currentCliBodyPx() {
  if (state.cliBodyHeight !== "auto" && Number(state.cliBodyHeight) > 0) {
    return Number(state.cliBodyHeight);
  }
  const n = parseInt(
    getComputedStyle(document.documentElement).getPropertyValue("--cli-body-h").trim(),
    10
  );
  return Number.isFinite(n) && n > 0 ? n : 300;
}

function bindCliHeightGrip() {
  const grip = $("#cli-h-grip");
  if (!grip || grip.dataset.bound === "1") return;
  grip.dataset.bound = "1";
  grip.addEventListener("pointerdown", (e) => {
    if (e.button != null && e.button !== 0) return;
    e.preventDefault();
    const startY = e.clientY;
    const startH = currentCliBodyPx();
    grip.classList.add("dragging");
    try {
      grip.setPointerCapture(e.pointerId);
    } catch (_) {}
    const onMove = (ev) =>
      applyCliBodyHeight(Math.max(160, Math.min(900, startH + (ev.clientY - startY))));
    const onUp = (ev) => {
      grip.classList.remove("dragging");
      grip.removeEventListener("pointermove", onMove);
      grip.removeEventListener("pointerup", onUp);
      grip.removeEventListener("pointercancel", onUp);
      try {
        grip.releasePointerCapture(ev.pointerId);
      } catch (_) {}
    };
    grip.addEventListener("pointermove", onMove);
    grip.addEventListener("pointerup", onUp);
    grip.addEventListener("pointercancel", onUp);
  });
}

function fitCliBodyHeight() {
  if (state.cliBodyHeight !== "auto") return;
  const shell = $("#cli-shell");
  if (!shell) return;
  let shellH = shell.getBoundingClientRect().height;
  if (!shellH || shellH < 80) shellH = Math.max(240, window.innerHeight - 260);
  const board = $("#cli-board");
  const n = Math.max(1, board ? board.querySelectorAll(".cli-window:not(.free)").length || 1 : 1);
  const rows = Math.max(1, Math.ceil(n / (window.innerWidth < 820 ? 1 : 2)));
  const rowH = (shellH - 12 * Math.max(0, rows - 1) - 8) / rows;
  document.documentElement.style.setProperty("--cli-body-h", Math.max(180, Math.floor(rowH - 92)) + "px");
}
