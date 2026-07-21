/**
 * [INPUT]: RunViewModel · 既有 DOM ids（result-card / stall / task-strip）
 * [OUTPUT]: 进度台绑定 + 意图转发；View 不写 stall 策略
 * [POS]: A4-1 RunView；禁止 invoke / start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  taskBucket,
  fiveStateLabel,
  sortTasksByStatus,
  countBuckets,
  runContext,
  isFailedStatus,
} from "./runBuckets.js";
import { paintLogSecondaryVisibility, syncMonitorLogsFold } from "./logPanel.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function $(id) {
  return document.getElementById(id);
}

function esc(s) {
  const fn = g("esc");
  if (typeof fn === "function") return fn(s);
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function formatElapsed(a, b) {
  const fn = g("formatElapsed");
  if (typeof fn === "function") return fn(a, b);
  return "";
}

function statusDot(status, task) {
  const fn = g("statusDot");
  if (typeof fn === "function") return fn(status, task);
  return "";
}

function stallStripText(t) {
  const fn = g("stallStripText");
  if (typeof fn === "function") return fn(t);
  return "";
}

function flowStallUserText(raw) {
  const fn = g("flowStallUserText");
  if (typeof fn === "function") return fn(raw);
  return raw || "较久没有新进展";
}

function taskErrorSummary(t) {
  const fn = g("taskErrorSummary");
  if (typeof fn === "function") return fn(t);
  return "";
}

function isStalledTask(t) {
  const fn = g("isStalledTask");
  if (typeof fn === "function") return fn(t);
  return taskBucket(t) === "stall";
}

/**
 * Bind progress desk. Call once; re-render via renderProgress().
 * @param {ReturnType<import("./RunViewModel.js").createRunViewModel>} vm
 * @param {object} [bridge]
 */
export function bindRunView(vm, bridge = {}) {
  function legacy() {
    return (typeof bridge.getLegacy === "function" && bridge.getLegacy()) || {};
  }

  function pullFromLegacy() {
    const L = legacy();
    if (L.live !== undefined) {
      vm.setLive(L.live, {
        selectedTaskId:
          L.selectedTaskId !== undefined ? L.selectedTaskId : undefined,
        dashCollapsed:
          L.dashCollapsed !== undefined ? L.dashCollapsed : undefined,
      });
    }
  }

  /** R2: top stall human banner while any step is stuck. */
  function renderStallBanner(tasks, active) {
    const banner = $("stall-banner");
    if (!banner) return;
    if (!active) {
      banner.hidden = true;
      banner.textContent = "";
      return;
    }
    const stalled = (tasks || []).filter((t) => isStalledTask(t));
    if (!stalled.length) {
      banner.hidden = true;
      banner.textContent = "";
      return;
    }
    const first = stalled[0];
    const title = first.title || first.task_id;
    const raw = stallStripText(first);
    const human = flowStallUserText(raw) || "较久没有新进展";
    banner.hidden = false;
    banner.textContent =
      stalled.length === 1
        ? `「${title}」好像卡住了 · ${human}`
        : `${stalled.length} 个步骤好像卡住了（含「${title}」）· ${human}`;
  }

  function paintKpis(counts, ctx, live, tasks) {
    const card = $("result-card");
    if (!card) return;
    const { hasRun, active, finished, runStatus, planning } = ctx;
    card.hidden = !(hasRun && !planning);
    if (card.hidden) return;

    const { done, run, wait, fail, stall } = counts;
    card.classList.toggle("ok", finished && fail === 0 && done > 0);
    card.classList.toggle("bad", fail > 0 || stall > 0);
    card.classList.toggle("is-result", !!finished && !active);
    card.classList.toggle("is-running", !!active);

    const setN = (id, n) => {
      const el = $(id);
      if (el) el.textContent = String(n);
    };
    setN("stat-done-n", done);
    setN("stat-run-n", run);
    setN("stat-wait-n", wait);
    setN("stat-fail-n", fail);
    setN("stat-stall-n", stall);
    const kpiFail = $("kpi-fail");
    if (kpiFail) kpiFail.hidden = fail === 0;
    const kpiStall = $("kpi-stall");
    if (kpiStall) kpiStall.hidden = stall === 0;

    const setStat = (id, label, n) => {
      const el = $(id);
      if (el) el.textContent = `${label} ${n}`;
    };
    setStat("stat-done", "已完成", done);
    setStat("stat-run", "进行中", run);
    setStat("stat-wait", "排队中", wait);
    setStat("stat-fail", "失败", fail);

    const heading = $("task-dash-heading");
    if (heading && !finished) heading.textContent = "执行进度";

    const pill = $("run-status-pill");
    if (pill) {
      if (active) {
        pill.hidden = false;
        pill.textContent =
          stall > 0
            ? "有步骤卡住"
            : run > 0
              ? "进行中"
              : wait > 0
                ? "排队中"
                : "执行中";
        pill.className =
          "run-status-pill" +
          (stall > 0 ? " is-stall" : run > 0 ? " is-run" : "");
      } else if (finished) {
        pill.hidden = false;
        pill.textContent = fail > 0 ? "有失败" : "已结束";
        pill.className =
          "run-status-pill" + (fail > 0 ? " is-fail" : " is-done");
      } else {
        pill.hidden = true;
      }
    }

    const runEnd = finished
      ? (tasks || [])
          .map((t) => t.finished_at)
          .filter(Boolean)
          .sort()
          .slice(-1)[0] || null
      : null;
    const meta = $("result-meta-text");
    if (meta) {
      const bits = [];
      if (tasks.length) bits.push(`共 ${tasks.length} 步`);
      if (live?.started_at) bits.push(formatElapsed(live.started_at, runEnd));
      if (live?.current_wave != null && live?.layers?.length) {
        bits.push(`第 ${live.current_wave}/${live.layers.length} 波`);
      }
      const pc = live?.planner_cost_usd;
      const ec = live?.exec_cost_usd;
      if (pc != null || ec != null) {
        const fmt = (n) => (n != null ? `$${Number(n).toFixed(2)}` : "—");
        bits.push(`花费 规划 ${fmt(pc)} · 执行 ${fmt(ec)}`);
      }
      meta.textContent = bits.join(" · ");
    }
    try {
      if (typeof g("updateBudgetChip") === "function") g("updateBudgetChip")();
    } catch (_) {}

    renderStallBanner(tasks, active);

    const errText = $("error-summary-text");
    const collapsed = !!vm.getSnapshot().dashCollapsed;
    if (errText) {
      if (fail > 0 && !collapsed && !finished) {
        const first = tasks.find((t) => isFailedStatus(t.status));
        const sum = first ? taskErrorSummary(first) : "";
        errText.hidden = false;
        errText.textContent = sum
          ? `${first.title || first.task_id}：${sum}`
          : `${fail} 个步骤失败`;
      } else {
        errText.hidden = true;
        errText.textContent = "";
      }
    }

    const stop = $("btn-ws-stop-all");
    if (stop) stop.hidden = !active;
    const monWin = $("btn-open-monitor-window");
    if (monWin) {
      const isMon =
        typeof bridge.isMonitorWindow === "function"
          ? bridge.isMonitorWindow()
          : !!legacy().isMonitorWindow;
      monWin.hidden = !!isMon || !(hasRun || active);
    }
    const resume = $("btn-ws-resume");
    if (resume) {
      resume.hidden = !["paused", "failed", "aborted"].includes(
        String(runStatus || "").toLowerCase()
      );
    }

    const backChat = $("btn-ws-back-chat");
    if (backChat) {
      const showBack = !!finished && !active;
      backChat.hidden = !showBack;
      if (showBack) {
        backChat.textContent = fail > 0 ? "回聊天改计划" : "回聊天";
        backChat.title =
          fail > 0
            ? "回聊天调整计划后再拆成步骤"
            : "回聊天写下一份计划";
        backChat.className = fail > 0 ? "btn primary sm" : "btn ghost sm";
      }
    }

    const toggle = $("btn-task-dash-toggle");
    if (toggle) {
      toggle.hidden = !hasRun;
      toggle.textContent = collapsed ? "▸" : "▾";
      toggle.title = collapsed ? "展开进度看板" : "折叠进度看板";
      toggle.setAttribute("aria-label", toggle.title);
      toggle.setAttribute("aria-expanded", collapsed ? "false" : "true");
    }
    card.classList.toggle("collapsed", collapsed);

    const body = $("task-strip-body");
    if (body) {
      body.hidden = collapsed;
      body.classList.toggle("is-secondary", !!finished && !active);
    }
  }

  function paintTiles(tasks, selectedTaskId) {
    const list = $("task-strip-list");
    if (!list) return;
    if (!tasks.length) {
      list.innerHTML = `<div class="task-dash-empty muted">暂无拆分任务</div>`;
      return;
    }
    list.innerHTML = sortTasksByStatus(tasks)
      .map((t) => {
        const b = taskBucket(t);
        const label = fiveStateLabel(b);
        const title = t.title || t.task_id;
        const sel = t.task_id === selectedTaskId ? " selected" : "";
        const elapsed = formatElapsed(t.started_at, t.finished_at);
        const cost =
          t.cost_usd != null ? `$${Number(t.cost_usd).toFixed(2)}` : "";
        return `<button type="button" class="task-tile ${b}${sel}" data-task="${esc(
          t.task_id
        )}">
        <div class="task-tile-top">
          <span class="dot ${statusDot(t.status, t)}"></span>
          <span class="task-tile-st">${esc(label)}</span>
        </div>
        <div class="task-tile-name" title="${esc(title)}">${esc(title)}</div>
        <div class="task-tile-foot muted">
          <span class="task-tile-wave">${esc(elapsed)}</span>
          <span>${cost ? esc(cost) : ""}</span>
        </div>
      </button>`;
      })
      .join("");
  }

  function renderProgress() {
    pullFromLegacy();
    const s = vm.getSnapshot();
    const live = s.live;
    const L = legacy();
    const tasks = live?.tasks || [];
    const ctx = runContext(live, { phase: L.phase });
    const counts = countBuckets(tasks);

    // Phase panels / doctor / picker still classic — only progress + logs fold here.
    paintKpis(counts, ctx, live, tasks);

    // Inspect strip + result desk: prefer feature/result when available
    if (typeof bridge.renderInspectAndResult === "function") {
      try {
        bridge.renderInspectAndResult(live, tasks, ctx);
      } catch (e) {
        console.error("[RunView] renderInspectAndResult", e);
      }
    } else {
      // fallback classic hooks
      try {
        if (typeof g("renderInspectLoopStrip") === "function") {
          g("renderInspectLoopStrip")(live, ctx.finished);
        }
      } catch (_) {}
      try {
        if (typeof g("renderResultDesk") === "function") {
          g("renderResultDesk")(live, tasks, ctx);
        }
      } catch (_) {}
    }

    paintTiles(tasks, s.selectedTaskId || L.selectedTaskId);

    // Hide legacy banners
    const runBanner = $("run-banner");
    if (runBanner) runBanner.hidden = true;
    const errBar = $("error-summary");
    if (errBar) errBar.hidden = true;
    const comp = $("completion-panel");
    if (comp) comp.hidden = true;

    const rerun = $("btn-rerun");
    if (rerun) rerun.hidden = true;
    const change = $("btn-change-plan");
    if (change) change.hidden = true;
    const dismiss = $("btn-ws-dismiss-run");
    if (dismiss) dismiss.hidden = true;

    paintLogSecondaryVisibility({
      planning: ctx.planning,
      hasTasks: tasks.length > 0,
      hasRun: ctx.hasRun,
    });

    if (!ctx.planning && tasks.length) {
      try {
        if (typeof g("refreshFlowStrips") === "function") {
          g("refreshFlowStrips")(
            ctx.active ? "running" : ctx.finished ? "done" : L.phase
          );
        }
      } catch (_) {}
      try {
        if (typeof g("renderCliBoard") === "function") {
          g("renderCliBoard")(tasks);
        }
      } catch (_) {}
    }

    // Terminal → shell result phase
    if (ctx.finished && !ctx.active && typeof bridge.onFinished === "function") {
      try {
        bridge.onFinished(live, ctx);
      } catch (e) {
        console.error("[RunView] onFinished", e);
      }
    }

    return { live, tasks, ctx, counts };
  }

  return {
    render: renderProgress,
    renderProgress,
    syncLogsFold: syncMonitorLogsFold,
    stopAll: () => vm.stopAll(),
    resume: () => vm.resume(),
    stopTask: (id) => vm.stopTask(id),
    toggleDash: () => {
      vm.toggleDashCollapsed();
      if (typeof bridge.syncLegacy === "function") {
        bridge.syncLegacy({ dashCollapsed: vm.getSnapshot().dashCollapsed });
      }
      renderProgress();
    },
  };
}

export default bindRunView;
