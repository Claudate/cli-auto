/**
 * [INPUT]: board DOM · tasks · logActions · logHost · renderCliBoard cb
 * [OUTPUT]: bindCliBoardEvents — 窗内 click/drag 重绑
 * [POS]: A5-2c features/run；自 logBoard 纵切（P-ship-D）· P4-4 聚焦分发同步右次级列
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  aiLogPlainText,
} from "./logContent.js";
import {
  openExternalTerminal,
  cancelTask,
} from "./logActions.js";
import * as host from "./logHost.js";

const S = host.S;
const $$ = host.$$;
const toast = host.toast;
const callG = host.callG;

/**
 * Rebind per-card controls after board paint.
 * Capture-phase document handler in settings also covers some clicks;
 * these local handlers stopPropagation for board-local actions.
 *
 * @param {HTMLElement} board
 * @param {Array} tasks
 * @param {(tasks: Array) => void} renderCliBoard
 */
export function bindCliBoardEvents(board, tasks, renderCliBoard) {
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
      // P4-4：聚焦同步右次级列
      if (typeof window.ccoRunDetail?.render === "function") {
        window.ccoRunDetail.render(tasks);
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
}
