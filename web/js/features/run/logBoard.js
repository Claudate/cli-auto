/**
 * [INPUT]: live tasks · logBoardCard · logBoardEvents · logActions · logHost
 * [OUTPUT]: CLI 多窗看板 renderCliBoard · stall strip · detail 兼容
 * [POS]: A5-2c features/run；P-ship-D 纵切 → logBoardCard + logBoardEvents
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  renderHandoffBoardStrip,
} from "./logActions.js";
import * as host from "./logHost.js";
import {
  stallStripText,
  upsertCliWindowCard,
} from "./logBoardCard.js";
import { bindCliBoardEvents } from "./logBoardEvents.js";
import { sortTasksByStatus, taskBucket } from "./runBuckets.js";

const g = host.g;
const S = host.S;
const $ = host.$;
const $$ = host.$$;
const isLiveStatus = host.isLiveStatus;
const callG = host.callG;

export { stallStripText };

function bucketOf(t) {
  // Prefer local pure helper; classic window.taskBucket as fallback.
  try {
    return taskBucket(t);
  } catch (_) {
    const b = callG("taskBucket", t);
    return b || "wait";
  }
}

function sortVisible(list) {
  try {
    return sortTasksByStatus(list);
  } catch (_) {
    const sorted = callG("sortTasksByStatus", list);
    return Array.isArray(sorted) ? sorted : list || [];
  }
}

export function renderCliBoard(tasks) {
  try {
    return renderCliBoardInner(tasks || []);
  } catch (e) {
    console.error("[renderCliBoard]", e);
    const board = $("#cli-board");
    if (board && !board.querySelector(".cli-window")) {
      board.innerHTML =
        `<div class="cli-board-empty muted" style="grid-column:1/-1;padding:1.2rem;text-align:center">运行端渲染失败 · 可点「刷新」重试</div>`;
    }
  }
}

function renderCliBoardInner(tasks) {
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
        try {
          callG("fitCliBodyHeight")();
        } catch (_) {}
        const nextH = document.documentElement.style.getPropertyValue("--cli-body-h");
        // Only second pass when height actually changed (avoids thrash).
        if (prevH !== nextH) {
          requestAnimationFrame(() => {
            try {
              callG("fitCliBodyHeight")();
            } catch (_) {}
          });
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

  // Ensure shell/monitor not accidentally hidden after layout swaps.
  if (shell) shell.hidden = false;
  const mon = $("#monitor");
  if (mon) mon.hidden = false;

  let shown = tasks;
  // 兼容旧 filterFailedOnly
  let filter = S().cliStatusFilter || "all";
  if (S().filterFailedOnly && filter === "all") filter = "fail";
  if (filter && filter !== "all") {
    const filtered = tasks.filter((t) => bucketOf(t) === filter);
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

  if (!S().closedPanels) S().closedPanels = {};
  const closedCount = Object.keys(S().closedPanels || {}).filter((id) =>
    tasks.some((t) => t.task_id === id)
  ).length;
  const restoreBtn = $("#btn-restore-panels");
  if (restoreBtn) {
    restoreBtn.hidden = closedCount === 0;
    restoreBtn.textContent = `恢复已关闭 (${closedCount})`;
  }

  // 可见面板：运行中最上，未运行居中，已完成/失败最底
  const visible = sortVisible(
    shown.filter((t) => t && t.task_id && !S().closedPanels[t.task_id])
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
            : "暂无执行窗口 · 执行规划后这里会按步骤出现"
          : `当前过滤（${
              {
                run: "进行中",
                wait: "排队中",
                stall: "已卡住",
                done: "已完成",
                stop: "已停止",
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
    upsertCliWindowCard(board, t, idx, canPatch);
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
  bindCliBoardEvents(board, tasks, renderCliBoard);
  __fitAfter();
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
