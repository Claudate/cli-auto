/**
 * [INPUT]: container DOM · event items · mode/stick
 * [OUTPUT]: P2-3 虚拟列表 mount/paint（阈值窗渲染）
 * [POS]: A5-2c features/run；算法自 log.js 原样迁入
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { renderTranscriptLine, renderLogEvent } from "./logRender.js";
import { S, esc } from "./logHost.js";

/** Virtual list: only render a window when event count exceeds this. */
export const LOG_VIRTUAL_THRESHOLD = 80;
/** Estimated row height (px) for spacer math; overscan absorbs variance. */
export const LOG_ROW_EST_PX = 30;
export const LOG_VIRT_OVERSCAN = 10;

export function isNearBottom(el, px = 48) {
  if (!el) return true;
  return el.scrollHeight - el.scrollTop - el.clientHeight < px;
}

/** Render one event row HTML for a given mode (term / pretty / planner-term). */
export function renderLogRowHtml(e, mode) {
  const m = mode || S().logViewMode || "term";
  if (m === "pretty") return renderLogEvent(e);
  const k = String(e.kind || "").toLowerCase();
  if (k === "raw_line" || k === "meta" || k === "stderr") {
    const bodyHtml = esc(e.summary || e.title || "…");
    const role = k === "stderr" ? "stderr" : k === "meta" ? "meta" : "out";
    return `<div class="tx-line role-${role}"><div class="tx-role">${
      role === "out" ? "log" : role
    }</div><div class="tx-body">${bodyHtml}</div></div>`;
  }
  return renderTranscriptLine(e);
}

/**
 * P2-3 virtual list: mount/update a scrollable window over `items`.
 * Filter switch or mode change → new key → rebuild (scroll resets to bottom when stick).
 * Returns true if virtual path used.
 */
export function mountVirtualLog(container, items, { mode, stick } = {}) {
  if (!container) return false;
  const list = Array.isArray(items) ? items : [];
  const m = mode || S().logViewMode || "term";
  if (list.length < LOG_VIRTUAL_THRESHOLD) return false;

  const key = `${m}|${list.length}|${
    list[list.length - 1]?.id || ""
  }|${S().logEventFilter || "all"}`;
  let root = container.querySelector(":scope > .log-virt");
  const reuse = root && container.dataset.virtKey === key;

  if (!reuse) {
    container.innerHTML = "";
    root = document.createElement("div");
    root.className = "log-virt";
    root.innerHTML =
      '<div class="log-virt-spacer"><div class="log-virt-window"></div></div>';
    container.appendChild(root);
    container.dataset.virtKey = key;
    container._virtItems = list;
    container._virtMode = m;
    if (!container._virtScrollBound) {
      container._virtScrollBound = true;
      let raf = 0;
      container.addEventListener(
        "scroll",
        () => {
          if (raf) return;
          raf = requestAnimationFrame(() => {
            raf = 0;
            paintVirtualLogWindow(container, false);
          });
        },
        { passive: true }
      );
    }
  } else {
    container._virtItems = list;
    container._virtMode = m;
  }

  const spacer = root.querySelector(".log-virt-spacer");
  if (spacer) {
    spacer.style.height = Math.max(list.length * LOG_ROW_EST_PX, 1) + "px";
  }

  if (stick) {
    container.scrollTop = container.scrollHeight;
  }
  paintVirtualLogWindow(container, !!stick);
  return true;
}

export function paintVirtualLogWindow(container, forceBottom) {
  const items = container._virtItems;
  if (!Array.isArray(items) || !items.length) return;
  const root = container.querySelector(":scope > .log-virt");
  if (!root) return;
  const windowEl = root.querySelector(".log-virt-window");
  const spacer = root.querySelector(".log-virt-spacer");
  if (!windowEl || !spacer) return;

  if (forceBottom) {
    container.scrollTop = container.scrollHeight;
  }

  const viewH = container.clientHeight || 240;
  const scrollTop = container.scrollTop || 0;
  const total = items.length;
  let start = Math.floor(scrollTop / LOG_ROW_EST_PX) - LOG_VIRT_OVERSCAN;
  if (start < 0) start = 0;
  let end = Math.ceil((scrollTop + viewH) / LOG_ROW_EST_PX) + LOG_VIRT_OVERSCAN;
  if (end > total) end = total;
  if (end < start) end = start;

  const mode = container._virtMode || S().logViewMode || "term";
  const sliceItems = items.slice(start, end);
  const html = sliceItems
    .map((e) => renderLogRowHtml(e, mode))
    .filter(Boolean)
    .join("");
  windowEl.style.transform = `translateY(${start * LOG_ROW_EST_PX}px)`;
  windowEl.innerHTML =
    html || `<div class="cli-empty-ai muted">当前窗口无可见行</div>`;
}
