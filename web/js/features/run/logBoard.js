/**
 * [INPUT]: live tasks · logContent · logActions · logVirtual
 * [OUTPUT]: CLI 多窗看板 renderCliBoard · stall strip · detail 兼容
 * [POS]: A5-2c features/run；自 log.js 抽出；日志仍次级
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  logPanelSignature,
  fillPanelLogBody,
  aiLogPlainText,
} from "./logContent.js";
import {
  paintVirtualLogWindow,
  isNearBottom,
} from "./logVirtual.js";
import {
  renderHandoffBoardStrip,
  openExternalTerminal,
  cancelTask,
} from "./logActions.js";
import * as host from "./logHost.js";

const g = host.g;
const S = host.S;
const $ = host.$;
const $$ = host.$$;
const esc = host.esc;
const toast = host.toast;
const isLiveStatus = host.isLiveStatus;
const callG = host.callG;

export function renderCliBoard(tasks) {
  try {
    if (typeof renderHandoffBoardStrip === "function") renderHandoffBoardStrip();
  } catch (_) {}
  // Sync event-filter chips
  try {
    const f = S().logEventFilter || "all";
    $$("#log-event-filter [data-ev-filter]").forEach((btn) => {
      btn.classList.toggle("active", (btn.dataset.evFilter || "all") === f);
    });
  } catch (_) {}
  const shell = $("#cli-shell");
  // Preserve outer shell scroll: re-layout / height fit must not yank the user back to top.
  const shellScrollTop = shell ? shell.scrollTop : 0;
  const __fitAfter = () => {
    if (S().cliBodyHeight === "auto") {
      requestAnimationFrame(() => {
        const before = shell ? shell.scrollTop : 0;
        const prevH = document.documentElement.style.getPropertyValue("--cli-body-h");
        callG("fitCliBodyHeight")();
        const nextH = document.documentElement.style.getPropertyValue("--cli-body-h");
        // Only second pass when height actually changed (avoids thrash).
        if (prevH !== nextH) {
          requestAnimationFrame(() => callG("fitCliBodyHeight")());
        }
        if (shell && shell.scrollTop !== before) shell.scrollTop = before;
        if (shell && shellScrollTop > 0 && shell.scrollTop === 0) {
          shell.scrollTop = shellScrollTop;
        }
      });
    } else if (shell && shellScrollTop > 0) {
      // Restore even without auto-fit.
      requestAnimationFrame(() => {
        if (shell.scrollTop === 0) shell.scrollTop = shellScrollTop;
      });
    }
  };

  const board = $("#cli-board");
  if (!board) return;

  let shown = tasks;
  // 兼容旧 filterFailedOnly
  let filter = S().cliStatusFilter || "all";
  if (S().filterFailedOnly && filter === "all") filter = "fail";
  if (filter && filter !== "all") {
    const filtered = tasks.filter((t) => callG("taskBucket")(t) === filter);
    // 无匹配时不回退，展示空板 + 过滤态更清晰
    shown = filtered;
  }
  // 同步过滤 chip 高亮
  $$("#cli-status-filters [data-cli-filter]").forEach((btn) => {
    const f = btn.getAttribute("data-cli-filter") || "all";
    btn.classList.toggle("active", f === filter);
  });
  // 单任务时工具条更安静（字号/视图保留）
  const toolbar = document.querySelector(".board-toolbar");
  if (toolbar) toolbar.classList.toggle("quiet", tasks.length <= 1);

  const closedCount = Object.keys(S().closedPanels || {}).filter((id) =>
    tasks.some((t) => t.task_id === id)
  ).length;
  const restoreBtn = $("#btn-restore-panels");
  if (restoreBtn) {
    restoreBtn.hidden = closedCount === 0;
    restoreBtn.textContent = `恢复已关闭 (${closedCount})`;
  }

  // 可见面板：运行中最上，未运行居中，已完成/失败最底
  const visible = callG("sortTasksByStatus")(
    shown.filter((t) => !S().closedPanels[t.task_id])
  );
  // 自动布局：网格，若用户拖过则用绝对坐标
  const cols = Math.max(1, Math.min(2, visible.length));
  board.classList.toggle("single", visible.length === 1);
  board.classList.add("cols-2");
  board.dataset.cols = "2";
  // 强制布局属性，防止旧 inline / 缓存样式
  board.style.display = "grid";
  board.style.gridTemplateColumns = "calc((100% - 0.75rem) / 2) calc((100% - 0.75rem) / 2)";
  board.style.gap = "0.75rem";
  board.style.overflowX = "hidden";
  if (S().cliBodyHeight === "auto") {
    document.documentElement.setAttribute("data-cli-h", "auto");
  } else {
    document.documentElement.removeAttribute("data-cli-h");
    document.documentElement.style.setProperty(
      "--cli-body-h",
      (Number(S().cliBodyHeight) || 300) + "px"
    );
  }
  // P1-1：任务集合未变时增量更新 body，避免 2s 全量 innerHTML 闪烁
  const visKey = visible.map((t) => t.task_id).join("|") + "#" + (S().logViewMode || "term");
  const canPatch =
    board.dataset.visKey === visKey &&
    board.querySelectorAll(".cli-window").length === visible.length &&
    !board.querySelector(".cli-board-empty");

  if (!visible.length) {
    if (board.dataset.visKey !== "empty:" + filter) {
      board.innerHTML = "";
      const empty = document.createElement("div");
      empty.className = "cli-board-empty muted";
      empty.style.gridColumn = "1 / -1";
      empty.style.padding = "1.2rem";
      empty.style.textAlign = "center";
      const f = S().cliStatusFilter || "all";
      empty.textContent =
        f === "all"
          ? typeof g("flowEmptyBoard") === "function"
            ? callG("flowEmptyBoard")()
            : "暂无执行窗口 · 确认并开始后这里会按步骤出现"
          : `当前过滤（${
              {
                run: "进行中",
                wait: "排队中",
                stall: "已卡住",
                done: "已完成",
                fail: "失败",
              }[f] || f
            }）无匹配步骤`;
      board.appendChild(empty);
      board.dataset.visKey = "empty:" + filter;
      S().logPanelSig = {};
    }
    __fitAfter();
    return;
  }

  if (!canPatch) {
    board.innerHTML = "";
    S().logPanelSig = {};
    board.dataset.visKey = visKey;
  }

  visible.forEach((t, idx) => {
    const st = String(t.status || "").toLowerCase();
    const bucket = callG("taskBucket")(t);
    const failed = bucket === "fail";
    const stalled = bucket === "stall";
    const title = t.title || t.task_id;
    const elapsed = callG("formatElapsed")(t.started_at, t.finished_at);
    const sum = callG("taskErrorSummary")(t);
    const pos = S().panelPos[t.task_id];
    let card = canPatch
      ? board.querySelector(`.cli-window[data-task="${CSS.escape(t.task_id)}"]`)
      : null;
    const half = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
    const usableFree =
      pos &&
      typeof pos.x === "number" &&
      typeof pos.y === "number" &&
      S().dragSession &&
      S().dragSession[t.task_id];

    if (!card) {
      card = document.createElement("div");
      card.dataset.task = t.task_id;
      board.appendChild(card);
      // force full chrome build
      card.dataset.chrome = "";
    }

    card.className = `cli-window${failed ? " failed" : ""}${
      stalled ? " stalled" : ""
    }${t.task_id === S().selectedTaskId ? " selected" : ""}`;
    if (usableFree) {
      card.classList.add("free");
      card.style.left = pos.x + "px";
      card.style.top = pos.y + "px";
      card.style.width = half + "px";
      card.style.maxWidth = half + "px";
    } else {
      card.classList.remove("free");
      card.dataset.slot = String(idx);
      card.style.left = "";
      card.style.top = "";
      card.style.width = "";
      card.style.maxWidth = "";
      card.style.gridColumn = "";
    }

    // Do NOT include `elapsed` / stall idle in chromeSig — they tick every poll and
    // used to wipe the whole card (resetting log scroll to top). Light-updated below.
    const chromeSig = [
      t.status,
      title,
      t.cost_usd != null ? Number(t.cost_usd).toFixed(4) : "",
      t.provider || "",
      sum || "",
      failed ? 1 : 0,
      stalled ? 1 : 0,
      !isLiveStatus(S().live?.run_status) && S().live?.run_id ? 1 : 0,
      isLiveStatus(st) ? 1 : 0,
      t.attempt || 0,
      t.last_retry_reason || "",
      // presence of stall strip (not the ticking idle seconds)
      stallStripText(t) ? 1 : 0,
      S().cliLogExpanded?.[t.task_id] === true ? 1 : 0,
    ].join("|");

    if (card.dataset.chrome !== chromeSig) {
      // Preserve log scroll across chrome rebuild (status/badge/cost changes).
      const prevBody = card.querySelector(".cli-window-body");
      const prevScroll = prevBody ? prevBody.scrollTop : 0;
      const wasNearBottom = prevBody ? isNearBottom(prevBody) : true;
      const stallTxt = stallStripText(t);
      // R1: logs default collapsed (progress-first); only true expands.
      if (!S().cliLogExpanded) S().cliLogExpanded = {};
      const expanded = S().cliLogExpanded[t.task_id] === true;
      const five =
        typeof g("fiveStateLabel") === "function"
          ? callG("fiveStateLabel")(bucket)
          : callG("statusLabel")(t.status);
      card.classList.toggle("is-log-collapsed", !expanded);
      card.innerHTML = `
      <div class="cli-window-head" data-drag="${esc(t.task_id)}">
        <div class="cli-window-title">
          <span class="dot ${callG("statusDot")(st, t)}"></span>
          <strong title="${esc(title)}">${esc(title)}</strong>
          <span class="badge ${
            bucket === "done"
              ? "ok"
              : bucket === "fail"
                ? "err"
                : bucket === "stall"
                  ? "warn"
                  : bucket === "run"
                    ? "warn"
                    : ""
          }">${esc(five)}</span>
          <span class="cli-elapsed muted" data-elapsed="${esc(t.task_id)}">· ${esc(elapsed)}</span>
        </div>
        <div class="cli-window-actions">
          ${
            !isLiveStatus(S().live?.run_status) && S().live?.run_id
              ? `<button type="button" class="btn primary sm cli-rerun-btn" data-rerun="${esc(t.task_id)}" title="再跑一次">再跑一次</button>`
              : ""
          }
          <button type="button" class="btn ghost sm cli-log-toggle" data-log-toggle="${esc(t.task_id)}" title="展开或折叠详细日志">${
            expanded ? "收起日志" : "详细日志"
          }</button>
          <button type="button" class="icon-btn sm" data-focus="${esc(t.task_id)}" title="聚焦">◉</button>
          <button type="button" class="icon-btn sm" data-close="${esc(t.task_id)}" title="关闭窗口">×</button>
        </div>
      </div>
      <div class="cli-window-meta muted">
        ${
          t.attempt && t.attempt > 1
            ? `第 ${t.attempt} 次尝试${t.last_retry_reason ? " · " + esc(String(t.last_retry_reason)) : ""}`
            : "步骤日志"
        }${t.cost_usd != null ? ` · $${Number(t.cost_usd).toFixed(4)}` : ""}
      </div>
      ${
        stallTxt
          ? `<div class="cli-window-stall" data-stall="${esc(t.task_id)}" title="${esc(
              typeof g("flowStallUserText") === "function"
              ? g("flowStallUserText")(stallTxt) : stallTxt
            )}">${esc(
              typeof g("flowStallUserText") === "function"
              ? g("flowStallUserText")(stallTxt) : stallTxt
            )}</div>`
          : ""
      }
      ${
        sum && failed
          ? `<div class="cli-window-err" title="${esc(sum)}">${esc(sum)}</div>`
          : ""
      }
      <div class="cli-window-body log-console term-mode" data-log="${esc(t.task_id)}" ${
        expanded ? "" : "hidden"
      }></div>
      <div class="cli-window-foot">
        <button type="button" class="btn ghost sm" data-copy="${esc(t.task_id)}">复制</button>
        <button type="button" class="btn ghost sm" data-extterm="${esc(t.task_id)}" title="在系统终端查看日志">外置终端</button>
        <button type="button" class="btn danger sm" data-stop="${esc(t.task_id)}" ${
          isLiveStatus(st) ? "" : "hidden"
        }>停止</button>
      </div>`;
      card.dataset.chrome = chromeSig;
      // chrome rebuild invalidates log body sig
      delete S().logPanelSig[t.task_id];
      // stash so the body fill below can restore scroll
      card.dataset.prevScroll = String(prevScroll);
      card.dataset.wasNearBottom = wasNearBottom ? "1" : "0";
    } else {
      // light elapsed / meta / stall refresh without wiping log body
      const elEl = card.querySelector(`[data-elapsed="${CSS.escape(t.task_id)}"]`);
      if (elEl) elEl.textContent = `· ${elapsed}`;
      const stallEl = card.querySelector(`[data-stall="${CSS.escape(t.task_id)}"]`);
      const stallTxt = stallStripText(t);
      if (stallEl && stallTxt) {
        const human =
          typeof g("flowStallUserText") === "function"
              ? g("flowStallUserText")(stallTxt)
            : stallTxt;
        stallEl.textContent = human;
        stallEl.title = human;
      }
      const stopBtn = card.querySelector("[data-stop]");
      if (stopBtn) stopBtn.hidden = !isLiveStatus(st);
    }

    const body = card.querySelector(".cli-window-body");
    if (body) {
      body.style.height = "";
      body.style.maxHeight = "";
      body.style.minHeight = "";
      const sig = logPanelSignature(t);
      if (S().logPanelSig[t.task_id] !== sig) {
        const stick =
          card.dataset.wasNearBottom === "1" ||
          (card.dataset.wasNearBottom == null && isNearBottom(body));
        const keepScroll = parseInt(card.dataset.prevScroll || "0", 10) || 0;
        // P2-3: virtual list when event count is large; else plain HTML
        fillPanelLogBody(body, t, { stick });
        S().logPanelSig[t.task_id] = sig;
        if (stick) {
          body.scrollTop = body.scrollHeight;
        } else if (keepScroll > 0) {
          body.scrollTop = keepScroll;
          // Re-paint virtual window at restored scroll
          if (body.querySelector(":scope > .log-virt")) {
            paintVirtualLogWindow(body, false);
          }
        }
        delete card.dataset.prevScroll;
        delete card.dataset.wasNearBottom;
      } else if (card.dataset.prevScroll != null) {
        // chrome rebuilt but log content unchanged — still restore scroll
        const stick = card.dataset.wasNearBottom === "1";
        const keepScroll = parseInt(card.dataset.prevScroll || "0", 10) || 0;
        if (stick) body.scrollTop = body.scrollHeight;
        else if (keepScroll > 0) body.scrollTop = keepScroll;
        if (body.querySelector(":scope > .log-virt")) {
          paintVirtualLogWindow(body, !!stick);
        }
        delete card.dataset.prevScroll;
        delete card.dataset.wasNearBottom;
      }
    }
  });

  // remove stale cards + re-order to match visible sort when patching
  if (canPatch) {
    const keep = new Set(visible.map((t) => t.task_id));
    $$(".cli-window", board).forEach((el) => {
      if (!keep.has(el.dataset.task)) el.remove();
    });
    // Only reorder when order actually changed — appendChild on every poll
    // moves DOM nodes and can jump the outer .cli-shell scroll.
    const kids = $$(".cli-window", board);
    let needsReorder =
      kids.length !== visible.length ||
      kids.some((el, i) => el.dataset.task !== visible[i]?.task_id);
    if (needsReorder) {
      visible.forEach((t) => {
        const el = board.querySelector(
          `.cli-window[data-task="${CSS.escape(t.task_id)}"]`
        );
        if (el) board.appendChild(el);
      });
    }
  }
  // Pin outer shell scroll after any DOM churn.
  if (shell && shellScrollTop > 0) {
    shell.scrollTop = shellScrollTop;
  }

  // events (rebind only on full structure rebuild — capture-phase document handler covers clicks)
  $$("[data-close]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      S().closedPanels[b.dataset.close] = true;
      renderCliBoard(tasks);
    };
  });
  $$("[data-focus]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      S().selectedTaskId = b.dataset.focus;
      const card = board.querySelector(`.cli-window[data-task="${CSS.escape(b.dataset.focus)}"]`);
      if (card) {
        card.style.zIndex = String(Date.now() % 100000);
        card.classList.add("selected");
      }
    };
  });
  $$("[data-copy]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      const t = tasks.find((x) => x.task_id === b.dataset.copy);
      const text = aiLogPlainText(t);
      try {
        await navigator.clipboard.writeText(text || "");
        toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
      } catch (_) {
        toast("复制失败");
      }
    };
  });
  $$("[data-extterm]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      await openExternalTerminal(b.dataset.extterm);
    };
  });
  $$("[data-stop]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      S().selectedTaskId = b.dataset.stop;
      await cancelTask();
    };
  });
  $$("[data-log-toggle]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      const id = b.dataset.logToggle;
      if (!id) return;
      if (!S().cliLogExpanded) S().cliLogExpanded = {};
      const next = !S().cliLogExpanded[id];
      S().cliLogExpanded[id] = next;
      const card = board.querySelector(`.cli-window[data-task="${CSS.escape(id)}"]`);
      if (card) {
        card.classList.toggle("is-log-collapsed", !next);
        const body = card.querySelector(".cli-window-body");
        if (body) body.hidden = !next;
        b.textContent = next ? "收起日志" : "详细日志";
        // Force log body refill when expanding
        if (next) delete S().logPanelSig[id];
      }
      if (next) renderCliBoard(tasks);
    };
  });

  // drag move
  $$("[data-drag]", board).forEach((head) => {
    head.onpointerdown = (ev) => {
      if (ev.button !== 0) return;
      if (ev.target.closest("button")) return;
      const id = head.dataset.drag;
      const card = head.closest(".cli-window");
      if (!card) return;
      const rect = card.getBoundingClientRect();
      const boardRect = board.getBoundingClientRect();
      card.classList.add("free");
      const x = rect.left - boardRect.left + board.scrollLeft;
      const y = rect.top - boardRect.top + board.scrollTop;
      const half = Math.max(260, Math.floor((board.clientWidth - 12) / 2));
      card.style.left = x + "px";
      card.style.top = y + "px";
      card.style.width = Math.min(rect.width || half, half * 1.15) + "px";
      card.style.zIndex = String(Date.now() % 100000);
      S().drag = {
        id,
        ox: ev.clientX - rect.left,
        oy: ev.clientY - rect.top,
      };
      head.setPointerCapture(ev.pointerId);
    };
    head.onpointermove = (ev) => {
      if (!S().drag || S().drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const boardRect = board.getBoundingClientRect();
      let x = ev.clientX - boardRect.left - S().drag.ox + board.scrollLeft;
      let y = ev.clientY - boardRect.top - S().drag.oy + board.scrollTop;
      x = Math.max(0, x);
      y = Math.max(0, y);
      card.style.left = x + "px";
      card.style.top = y + "px";
    };
    head.onpointerup = (ev) => {
      if (!S().drag || S().drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const id = S().drag.id;
      S().drag = null;
      const halfW = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
      S().panelPos[id] = {
        x: parseFloat(card.style.left) || 0,
        y: parseFloat(card.style.top) || 0,
        w: halfW,
      };
      S().dragSession = S().dragSession || {};
      S().dragSession[id] = true;
      card.style.width = halfW + "px";
      card.style.maxWidth = halfW + "px";
      callG("savePanelPos")();
      try {
        head.releasePointerCapture(ev.pointerId);
      } catch (_) {}
    };
  });
  __fitAfter();
}

/**
 * H3 stall strip copy. Prefer live stall_idle_secs + threshold; fall back to
 * last_retry_reason=stall after a retry was scheduled. Idle ticking is light-
 * updated (not in chromeSig) so the card does not rebuild every poll.
 */
/**
 * H3 / R2 stall strip — human first; threshold as secondary detail.
 */
export function stallStripText(t) {
  if (!t) return "";
  const thr =
    t.stall_threshold_secs != null
      ? Number(t.stall_threshold_secs)
      : null;
  const idle = t.stall_idle_secs != null ? Number(t.stall_idle_secs) : null;
  const reason = String(t.last_retry_reason || "").toLowerCase();
  const live = isLiveStatus(t.status);
  // Approaching / over threshold while still running → warn strip.
  if (live && idle != null && thr != null && thr > 0 && idle >= Math.max(15, thr * 0.5)) {
    if (idle >= thr) {
      return `已约 ${Math.round(idle)}s 没有新进展，系统将自动再推一把（阈值 ${Math.round(thr)}s）`;
    }
    return `已约 ${Math.round(idle)}s 没有新进展（超过一半等待阈值 ${Math.round(thr)}s）`;
  }
  // After a stall-triggered retry, surface reason on the next attempt chrome.
  if (reason === "stall") {
    const attemptBit =
      t.attempt && t.attempt > 1 ? `，正在第 ${t.attempt} 次尝试` : "";
    return `因较久无进展已自动重试${attemptBit}`;
  }
  return "";
}

export function renderTaskList(tasks) {
  // 兼容旧调用：转交看板
  renderCliBoard(tasks);
}

export function renderDetailLog(tasks) {
  // 紧凑多窗口模式下，日志已在各窗口内；保留隐藏 detail 同步以便复制按钮
  const t = tasks.find((x) => x.task_id === S().selectedTaskId) || tasks[0];
  if (!t) return;
  const logEl = $("#cli-detail-log");
  if (logEl) {
    logEl.textContent = t.log_tail || "";
  }
  const stop = $("#btn-stop-task");
  if (stop) stop.hidden = !isLiveStatus(t.status);
}
