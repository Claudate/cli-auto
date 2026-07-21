/**
 * [INPUT]: live task · logContent · logVirtual · logHost
 * [OUTPUT]: stallStripText · upsertCliWindowCard（单窗 chrome + body）
 * [POS]: A5-2c features/run；自 logBoard 纵切（P-ship-D）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  logPanelSignature,
  fillPanelLogBody,
} from "./logContent.js";
import {
  paintVirtualLogWindow,
  isNearBottom,
} from "./logVirtual.js";
import * as host from "./logHost.js";

const g = host.g;
const S = host.S;
const esc = host.esc;
const isLiveStatus = host.isLiveStatus;
const callG = host.callG;

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

/**
 * Create or patch one CLI window card on the board.
 * @returns {HTMLElement} card
 */
export function upsertCliWindowCard(board, t, idx, canPatch) {
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
      ${(() => {
        // C2: human progress blurb (3–5 lines max) before raw log body
        const lines = [];
        lines.push(
          bucket === "run"
            ? "正在推进这一步…"
            : bucket === "wait"
              ? "排队等待前序步骤完成"
              : bucket === "stall"
                ? "较久没有新进展，可点「详细日志」或停止后重试"
                : bucket === "done"
                  ? "本步已完成"
                  : bucket === "fail"
                    ? "本步未完成"
                    : "等待开始"
        );
        if (elapsed && elapsed !== "—" && elapsed !== "-") {
          lines.push(`已用时 ${elapsed}`);
        }
        if (stallTxt) {
          const human =
            typeof g("flowStallUserText") === "function"
              ? g("flowStallUserText")(stallTxt)
              : stallTxt;
          if (human) lines.push(String(human));
        }
        if (sum) lines.push(String(sum).slice(0, 120));
        if (t.attempt && t.attempt > 1) {
          lines.push(`第 ${t.attempt} 次尝试`);
        }
        const body = lines
          .filter(Boolean)
          .slice(0, 5)
          .map((l) => esc(l))
          .join("<br/>");
        return body
          ? `<div class="cli-window-human muted" data-human="${esc(t.task_id)}">${body}</div>`
          : "";
      })()}
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

  return card;
}
