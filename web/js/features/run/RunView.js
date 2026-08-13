/**
 * [INPUT]: RunViewModel · 既有 DOM ids（result-card / stall / task-strip）
 * [OUTPUT]: 进度台绑定 + 计划级自动提交展示 + 意图转发；View 不写 stall 策略
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

    const { done, run, wait, fail, stall, stop = 0 } = counts;
    const runAborted = ["aborted", "stopped"].includes(
      String(runStatus || "").toLowerCase()
    );
    // bad only for real fail/stall — pure user-stop is neutral
    card.classList.toggle("ok", finished && fail === 0 && done > 0 && !runAborted);
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

    // H1-3: bind app-composed status_one_liner when present (no JS strategy).
    const oneLiner = $("status-one-liner") || $("result-status-line");
    if (oneLiner) {
      const line =
        (live && (live.status_one_liner || live.statusOneLiner)) || "";
      if (line) {
        oneLiner.hidden = false;
        oneLiner.textContent = String(line).replace(/\*\*/g, "");
      } else {
        oneLiner.hidden = true;
        oneLiner.textContent = "";
      }
    }

    const gitStatus = $("git-auto-commit-status");
    if (gitStatus) {
      const commits = Array.isArray(live?.auto_commits)
        ? live.auto_commits
        : Array.isArray(live?.autoCommits)
          ? live.autoCommits
          : [];
      const latest = commits[commits.length - 1];
      if (!latest) {
        gitStatus.hidden = true;
        gitStatus.textContent = "";
      } else if (!latest.ok) {
        gitStatus.hidden = false;
        gitStatus.textContent = `自动提交失败：${String(latest.message || "未知错误").slice(0, 140)}`;
      } else if (latest.commit_hash || latest.commitHash) {
        const hash = String(latest.commit_hash || latest.commitHash).slice(0, 8);
        const files = Array.isArray(latest.files) ? latest.files.length : 0;
        gitStatus.hidden = false;
        gitStatus.textContent = `自动提交：${hash}${files ? ` · ${files} 个文件` : ""}${latest.pushed ? " · 已 Push" : ""}`;
      } else {
        gitStatus.hidden = false;
        gitStatus.textContent = "自动提交：无变更可提交";
      }
    }

    const pill = $("run-status-pill");
    if (pill) {
      const line =
        live && (live.status_one_liner || live.statusOneLiner);
      if (line) {
        pill.hidden = false;
        // Short badge still uses five-state; full sentence lives in status-one-liner when present.
        if (active) {
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
          if (fail > 0) {
            pill.textContent = "有失败";
            pill.className = "run-status-pill is-fail";
          } else if (runAborted || stop > 0) {
            pill.textContent = "已中止";
            pill.className = "run-status-pill is-stop";
          } else {
            pill.textContent = "已结束";
            pill.className = "run-status-pill is-done";
          }
        } else {
          pill.hidden = true;
        }
      } else if (active) {
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
        if (fail > 0) {
          pill.textContent = "有失败";
          pill.className = "run-status-pill is-fail";
        } else if (runAborted || stop > 0) {
          pill.textContent = "已中止";
          pill.className = "run-status-pill is-stop";
        } else {
          pill.textContent = "已结束";
          pill.className = "run-status-pill is-done";
        }
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
      // 费用改到标题右侧 #result-cost-chip；meta 只留步数/耗时/波次
      const bits = [];
      if (tasks.length) bits.push(`共 ${tasks.length} 步`);
      if (live?.started_at) bits.push(formatElapsed(live.started_at, runEnd));
      if (live?.current_wave != null && live?.layers?.length) {
        bits.push(`第 ${live.current_wave}/${live.layers.length} 波`);
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
        // P1-3: first fail strip may include App-composed 执行方式 (route_label)
        const route = first && String(first.route_label || "").trim();
        const name = first ? first.title || first.task_id : "";
        errText.hidden = false;
        if (sum && name) {
          errText.textContent = route
            ? `${name}：${sum} · 执行方式：${route}`
            : `${name}：${sum}`;
        } else if (name && route) {
          errText.textContent = `${name} · 执行方式：${route}`;
        } else {
          errText.textContent = `${fail} 个步骤失败`;
        }
      } else {
        errText.hidden = true;
        errText.textContent = "";
      }
    }

    // stop count already bound above from counts; button uses distinct name
    const stopAllBtn = $("btn-ws-stop-all");
    if (stopAllBtn) stopAllBtn.hidden = !active;
    // 结果台动作收口：独立监视/继续/再写 不在 task-dash-actions 露出
    const monWin = $("btn-open-monitor-window");
    if (monWin) monWin.hidden = true;
    const resume = $("btn-ws-resume");
    if (resume) resume.hidden = true;
    const backChat = $("btn-ws-back-chat");
    if (backChat) backChat.hidden = true;

    // 日志栏：继续（暂停/失败/中止）+ 结束计划（有 run 即显）
    const canResume = ["paused", "failed", "aborted"].includes(
      String(runStatus || "").toLowerCase()
    );
    const logResume = $("btn-log-resume");
    if (logResume) logResume.hidden = !canResume;
    const logEnd = $("btn-log-end-plan");
    if (logEnd) {
      logEnd.hidden = !(hasRun || active || finished);
      // Live run: ending the round stops workers first — say so up front.
      logEnd.textContent = active ? "停止并结束" : "结束计划";
      logEnd.title = active
        ? "先停止所有执行中的任务，再结束本轮"
        : "结束本轮计划";
    }

    const toggle = $("btn-task-dash-toggle");
    if (toggle) {
      toggle.hidden = !hasRun;
      if (typeof g("ccoIcon") === "function") {
        toggle.innerHTML = g("ccoIcon")(collapsed ? "chevron-right" : "chevron-down", {
          size: 14,
        });
      } else {
        toggle.textContent = collapsed ? "▸" : "▾";
      }
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
    const rawLive = s.live;
    const L = legacy();
    const ctx = runContext(rawLive, { phase: L.phase });
    // 拆分台/规划中：勿用历史 completed 填 KPI / 结果台（顶栏计划与本轮脱节）
    const live = ctx.belongs === false ? null : rawLive;
    const tasks = live?.tasks || [];
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
        } else if (g("ccoLog") && typeof g("ccoLog").renderCliBoard === "function") {
          g("ccoLog").renderCliBoard(tasks);
        }
      } catch (e) {
        console.error("[RunView] renderCliBoard", e);
      }
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
    retryTask: (id) => vm.retryTask(id),
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
