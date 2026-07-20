/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: monitor UI 片段
 * [POS]: web/js D4 自 app.js 纵切；无构建器，顺序 script 共享全局
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — monitor */

function renderWorkspace() {
  updateWorkspaceTitle();
  const live = state.live;
  const runStatus = live?.run_status;
  const hasRun = !!live?.run_id;
  const active = isLiveStatus(runStatus);
  const tasks = live?.tasks || [];
  const finished =
    hasRun &&
    !active &&
    ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
      String(runStatus || "").toLowerCase()
    );

  // 规划/确认相位不可被历史 run 的 finished 状态冲掉（否则转圈面板闪一下就没）
  if (state.phase === "planning" || state.phase === "confirm") {
    // keep planning UI
  } else if (active) {
    state.phase = "running";
  } else if (finished) {
    state.phase = "done";
  }

  const body = $("#workspace-body");
  if (body) {
    body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
    if (state.phase === "planning" || state.phase === "confirm") body.classList.add("mode-plan");
    else if (active) body.classList.add("mode-running");
    else if (finished) body.classList.add("mode-done");
    else body.classList.add("mode-idle");
  }

  renderDoctorWarn();
  renderPhasePanels();
  if (state.phase === "pick" || state.phase === "done" || state.phase === "running") {
    // 运行态若计划空，从 live 回填
    if (!state.selectedPlan && state.live?.plan_path) {
      state.selectedPlan = normalizePlanPath(state.live.plan_path) || state.live.plan_path;
    }
    renderPlanPicker();
  }
  updateTopPlanInfo();


  // legacy hide
  const runBanner = $("#run-banner");
  if (runBanner) runBanner.hidden = true;
  const errBar = $("#error-summary");
  if (errBar) errBar.hidden = true;
  const comp = $("#completion-panel");
  if (comp) comp.hidden = true;

  renderTaskStrip(live, tasks, {
    hasRun,
    active,
    finished,
    runStatus,
  });

  // Multi-window execution board
  const monitor = $("#monitor");
  const cliEmpty = $("#cli-empty");
  if (state.phase === "planning" || state.phase === "confirm") {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = true;
    return;
  }
  if (!tasks.length) {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = hasRun;
    return;
  }
  if (monitor) monitor.hidden = false;
  if (cliEmpty) cliEmpty.hidden = true;
  try {
    if (typeof refreshFlowStrips === "function") {
      refreshFlowStrips(active ? "running" : finished ? "done" : state.phase);
    }
  } catch (_) {}
  renderCliBoard(tasks);
  // height fit handled inside renderCliBoard

}

function savePanelPos() {
  try {
    localStorage.setItem("cco.panelPos", JSON.stringify(state.panelPos || {}));
  } catch (_) {}
}


function taskBucket(st) {
  const s = String(st || "").toLowerCase();
  if (isFailedStatus(s)) return "fail";
  if (isDoneStatus(s)) return "done";
  if (isLiveStatus(s) || ["starting", "queued", "running"].includes(s)) return "run";
  return "wait"; // pending / unknown
}

/** CLI / 看板排序：运行中 → 未运行 → 已完成 → 失败 */
function cliStatusRank(st) {
  const b = taskBucket(st);
  if (b === "run") return 0;
  if (b === "wait") return 1;
  if (b === "done") return 2;
  return 3; // fail
}

function sortTasksByStatus(tasks) {
  return tasks
    .map((t, i) => ({ t, i }))
    .sort((a, b) => cliStatusRank(a.t.status) - cliStatusRank(b.t.status) || a.i - b.i)
    .map((x) => x.t);
}

/** P-loop: show inspect VERDICT / blocking count and rework actions. */
function renderInspectLoopStrip(live, finished) {
  const strip = $("#inspect-loop-strip");
  const btnRework = $("#btn-ws-rework");
  const btnAccept = $("#btn-ws-accept-residual");
  const loop = live?.inspect_loop;
  if (!strip) return;

  if (!loop || (!loop.verdict && !loop.blocking_count && !loop.require_inspect && !loop.can_rework)) {
    strip.hidden = true;
    strip.textContent = "";
    if (btnRework) btnRework.hidden = true;
    if (btnAccept) btnAccept.hidden = true;
    return;
  }

  const bits = [];
  if (loop.verdict) {
    bits.push(`巡检 ${loop.verdict}`);
  } else if (loop.require_inspect) {
    bits.push("巡检 待产出");
  }
  if (loop.blocking_count > 0) {
    bits.push(`阻塞 ${loop.blocking_count}`);
  }
  if (loop.residual_count > 0) {
    bits.push(`残留 ${loop.residual_count}`);
  }
  if (loop.rework_round > 0) {
    bits.push(`回补轮次 ${loop.rework_round}/${loop.rework_max || 2}`);
  }
  if (loop.accepted_residual) {
    bits.push("已接受残留");
  }
  const preview = (loop.issue_preview || []).slice(0, 2).join(" · ");
  if (preview) bits.push(preview);

  strip.hidden = false;
  strip.textContent = bits.join(" · ");
  strip.classList.toggle("bad", loop.verdict === "FAIL" || loop.blocking_count > 0);
  strip.classList.toggle("ok", loop.verdict === "PASS" && !(loop.blocking_count > 0));

  const showActions = !!finished && !isLiveStatus(live?.run_status);
  if (btnRework) {
    btnRework.hidden = !(showActions && loop.can_rework);
  }
  if (btnAccept) {
    const showAccept =
      showActions &&
      !loop.accepted_residual &&
      (loop.blocking_count > 0 || loop.verdict === "FAIL" || (loop.residual_count > 0 && loop.verdict === "PASS"));
    btnAccept.hidden = !showAccept;
  }
}

function renderTaskStrip(live, tasks, ctx) {
  const card = $("#result-card");
  if (!card) return;
  const { hasRun, active, finished, runStatus } = ctx;
  card.hidden = !(hasRun && state.phase !== "planning" && state.phase !== "confirm");
  if (card.hidden) return;

  let done = 0, run = 0, wait = 0, fail = 0;
  tasks.forEach((t) => {
    const b = taskBucket(t.status);
    if (b === "done") done++;
    else if (b === "run") run++;
    else if (b === "fail") fail++;
    else wait++;
  });

  card.classList.toggle("ok", finished && fail === 0 && done > 0);
  card.classList.toggle("bad", fail > 0);

  const setN = (id, n) => {
    const el = $(id);
    if (el) el.textContent = String(n);
  };
  setN("#stat-done-n", done);
  setN("#stat-run-n", run);
  setN("#stat-wait-n", wait);
  setN("#stat-fail-n", fail);
  const kpiFail = $("#kpi-fail");
  if (kpiFail) kpiFail.hidden = fail === 0;

  const setStat = (id, label, n) => {
    const el = $(id);
    if (el) el.textContent = `${label} ${n}`;
  };
  setStat("#stat-done", "完成", done);
  setStat("#stat-run", "进行中", run);
  setStat("#stat-wait", "未启动", wait);
  setStat("#stat-fail", "失败", fail);

  const runEnd = finished
    ? tasks.map((t) => t.finished_at).filter(Boolean).sort().slice(-1)[0] || null
    : null;
  const meta = $("#result-meta-text");
  if (meta) {
    const bits = [];
    if (tasks.length) bits.push(`共 ${tasks.length} 项`);
    if (live?.started_at) bits.push(formatElapsed(live.started_at, runEnd));
    // P1-5 budget strip in KPI meta
    const pc = live?.planner_cost_usd;
    const ec = live?.exec_cost_usd;
    if (pc != null || ec != null) {
      const fmt = (n) => (n != null ? `$${Number(n).toFixed(2)}` : "—");
      bits.push(`规划 ${fmt(pc)} · 执行 ${fmt(ec)}`);
    }
    meta.textContent = bits.join(" · ");
  }
  try {
    updateBudgetChip();
  } catch (_) {}

  const errText = $("#error-summary-text");
  if (errText) {
    if (fail > 0 && !state.taskDashCollapsed) {
      const first = tasks.find((t) => isFailedStatus(t.status));
      const sum = first ? taskErrorSummary(first) : "";
      errText.hidden = false;
      errText.textContent = sum ? `${first.task_id}：${sum}` : `${fail} 个任务失败`;
    } else {
      errText.hidden = true;
      errText.textContent = "";
    }
  }

  const stop = $("#btn-ws-stop-all");
  if (stop) stop.hidden = !active;
  // P2-4: open system-level monitor window (hidden inside that window itself).
  const monWin = $("#btn-open-monitor-window");
  if (monWin) {
    monWin.hidden = !!state.isMonitorWindow || !(hasRun || active);
  }
  const resume = $("#btn-ws-resume");
  if (resume) {
    resume.hidden = !["paused", "failed", "aborted"].includes(
      String(runStatus || "").toLowerCase()
    );
  }

  // G6: finished run → clear next-step CTA
  const backChat = $("#btn-ws-back-chat");
  if (backChat) {
    const showBack = !!finished && !active;
    backChat.hidden = !showBack;
    if (showBack) {
      backChat.textContent = fail > 0 ? "回聊天改计划" : "回聊天";
      backChat.title =
        fail > 0
          ? "回聊天调整计划后再分配"
          : "回聊天写下一份计划";
      backChat.className = fail > 0 ? "btn primary sm" : "btn ghost sm";
    }
  }

  // P-loop L2: inspect strip + rework / accept residual
  renderInspectLoopStrip(live, finished);
  // 再跑一次改到 CLI 卡片标题栏；换计划删除；收起改为看板伸缩
  const rerun = $("#btn-rerun");
  if (rerun) rerun.hidden = true;
  const change = $("#btn-change-plan");
  if (change) change.hidden = true;
  const dismiss = $("#btn-ws-dismiss-run");
  if (dismiss) dismiss.hidden = true;

  const toggle = $("#btn-task-dash-toggle");
  if (toggle) {
    toggle.hidden = !hasRun;
    toggle.textContent = state.taskDashCollapsed ? "▸" : "▾";
    toggle.title = state.taskDashCollapsed ? "展开任务看板" : "折叠任务看板";
    toggle.setAttribute("aria-label", toggle.title);
    toggle.setAttribute("aria-expanded", state.taskDashCollapsed ? "false" : "true");
  }
  card.classList.toggle("collapsed", !!state.taskDashCollapsed);

  const body = $("#task-strip-body");
  if (body) body.hidden = !!state.taskDashCollapsed;
  const list = $("#task-strip-list");
  if (!list) return;

  if (!tasks.length) {
    list.innerHTML = `<div class="task-dash-empty muted">暂无拆分任务</div>`;
    return;
  }

  // 与 CLI 窗口同序：运行中 → 未启动 → 已完成 → 失败
  list.innerHTML = sortTasksByStatus(tasks)
    .map((t) => {
      const b = taskBucket(t.status);
      const label =
        b === "done" ? "已完成" : b === "run" ? "进行中" : b === "fail" ? "失败" : "未启动";
      const title = t.title || t.task_id;
      const sel = t.task_id === state.selectedTaskId ? " selected" : "";
      const elapsed = formatElapsed(t.started_at, t.finished_at);
      const cost = t.cost_usd != null ? `$${Number(t.cost_usd).toFixed(2)}` : "";
      return `<button type="button" class="task-tile ${b}${sel}" data-task="${esc(t.task_id)}">
        <div class="task-tile-top">
          <span class="dot ${statusDot(t.status)}"></span>
          <span class="task-tile-st">${esc(label)}</span>
        </div>
        <div class="task-tile-name" title="${esc(title)}">${esc(title)}</div>
        <div class="task-tile-foot muted">
          <span>${esc(t.task_id)}</span>
          <span>${esc(elapsed)}${cost ? " · " + cost : ""}</span>
        </div>
      </button>`;
    })
    .join("");
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
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue("--cli-body-h")
    .trim();
  const n = parseInt(raw, 10);
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
    const onMove = (ev) => {
      const next = Math.max(160, Math.min(900, startH + (ev.clientY - startY)));
      applyCliBodyHeight(next);
    };
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

/** 按 CLI shell 可用高度均分窗口 body，消灭下方大片空白 */
function fitCliBodyHeight() {
  if (state.cliBodyHeight !== "auto") return;
  const shell = $("#cli-shell");
  const board = $("#cli-board");
  if (!shell) return;
  const rect = shell.getBoundingClientRect();
  let shellH = rect.height;
  // shell 尚未布局时用视口估算
  if (!shellH || shellH < 80) {
    shellH = Math.max(240, window.innerHeight - 260);
  }
  const wins = board
    ? [...board.querySelectorAll(".cli-window:not(.free)")]
    : [];
  const n = Math.max(1, wins.length || 1);
  const cols = window.innerWidth < 820 ? 1 : 2;
  const rows = Math.max(1, Math.ceil(n / cols));
  const gap = 12;
  const chrome = 92; // head + foot + borders
  const rowH = (shellH - gap * Math.max(0, rows - 1) - 8) / rows;
  const bodyH = Math.max(180, Math.floor(rowH - chrome));
  document.documentElement.style.setProperty("--cli-body-h", bodyH + "px");
}


