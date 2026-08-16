/**
 * [INPUT]: live task · logContent · logVirtual · logHost
 * [OUTPUT]: stallStripText · upsertCliWindowCard（单窗 chrome + body + 任务级自动提交状态）
 * [POS]: A5-2c features/run；自 logBoard 纵切（P-ship-D）· P1-3 失败卡执行方式 · P4-4 is-running 追光
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
import { taskBucket, fiveStateLabel } from "./runBuckets.js";

const g = host.g;
const S = host.S;
const esc = host.esc;
const isLiveStatus = host.isLiveStatus;
const callG = host.callG;

/**
 * Parse observation-only CCO_STEP markers from worker log text.
 * Returns HTML checklist (max 7) or empty string. Not a scheduler graph.
 */
export function formatCcoStepProgress(logText) {
  const text = String(logText || "");
  if (!text.includes("CCO_STEP")) return "";
  /** @type {Map<string, "todo"|"start"|"done">} */
  const map = new Map();
  const order = [];
  const re = /CCO_STEP\s+(todo|start|done)\s*:\s*(.+)/gi;
  let m;
  while ((m = re.exec(text)) !== null) {
    const kind = String(m[1] || "").toLowerCase();
    const label = String(m[2] || "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 48);
    if (!label) continue;
    const key = label.toLowerCase();
    if (!map.has(key)) order.push(key);
    const prev = map.get(key);
    if (kind === "done" || prev === "done") map.set(key, "done");
    else if (kind === "start" || prev === "start") map.set(key, "start");
    else map.set(key, "todo");
  }
  if (!order.length) return "";
  const items = order.slice(0, 7).map((key) => {
    const st = map.get(key) || "todo";
    const label = key.length > 40 ? key.slice(0, 38) + "…" : key;
    const mark = st === "done" ? "✓" : st === "start" ? "→" : "·";
    const cls =
      st === "done"
        ? "is-done"
        : st === "start"
          ? "is-start"
          : "is-todo";
    return `<li class="cco-step-item ${cls}"><span class="cco-step-mark">${mark}</span> ${esc(
      label
    )}</li>`;
  });
  return `<ul class="cco-step-list" title="本步内部进度（观察）">${items.join(
    ""
  )}</ul>`;
}

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

function autoCommitSummary(commit) {
  if (!commit) return "";
  if (!commit.ok) return `自动提交失败：${String(commit.message || "未知错误").slice(0, 100)}`;
  if (!commit.commit_hash) return "自动提交：无变更可提交";
  const hash = String(commit.commit_hash).slice(0, 8);
  const files = Array.isArray(commit.files) ? commit.files.length : 0;
  const push = commit.pushed ? " · 已 Push" : "";
  return `自动提交 ${hash}${files ? ` · ${files} 个文件` : ""}${push}`;
}

/**
 * Create or patch one CLI window card on the board.
 * @returns {HTMLElement} card
 */
export function upsertCliWindowCard(board, t, idx, canPatch) {
  const st = String(t.status || "").toLowerCase();
  let bucket = "wait";
  try {
    bucket = taskBucket(t) || "wait";
  } catch (_) {
    bucket = callG("taskBucket", t) || "wait";
  }
  const failed = bucket === "fail";
  const stopped = bucket === "stop";
  const stalled = bucket === "stall";
  const title = t.title || t.task_id;
  const elapsed = callG("formatElapsed", t.started_at, t.finished_at) || "";
  const sum = callG("taskErrorSummary", t) || "";
  const autoCommit = t.auto_commit || t.autoCommit || null;
  const autoCommitText = autoCommitSummary(autoCommit);
  if (!S().panelPos) S().panelPos = {};
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
    stopped ? " stopped" : ""
  }${stalled ? " stalled" : ""}${
    t.task_id === S().selectedTaskId ? " selected" : ""
  }`;
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
    // P1-3: App-composed route_label drives fail/miss copy
    t.route_label || "",
    sum || "",
    failed ? 1 : 0,
    stopped ? 1 : 0,
    stalled ? 1 : 0,
    !isLiveStatus(S().live?.run_status) && S().live?.run_id ? 1 : 0,
    isLiveStatus(st) ? 1 : 0,
    t.attempt || 0,
    t.last_retry_reason || "",
    autoCommitText,
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
      fiveStateLabel(bucket) ||
      callG("fiveStateLabel", bucket) ||
      callG("statusLabel", t.status) ||
      bucket;
    card.classList.toggle("is-log-collapsed", !expanded);
    const dotCls =
      callG("statusDot", st, t) ||
      (failed
        ? "err"
        : stopped
          ? "muted"
          : stalled
            ? "warn"
            : bucket === "done"
              ? "ok"
              : bucket === "run"
                ? "live"
                : "");
    card.innerHTML = `
      <div class="cli-window-head" data-drag="${esc(t.task_id)}">
        <div class="cli-window-title">
          <span class="dot ${esc(dotCls)}"></span>
          <strong title="${esc(title)}">${esc(title)}</strong>
          <span class="badge ${
            bucket === "done"
              ? "ok"
              : bucket === "fail"
                ? "err"
                : bucket === "stop"
                  ? "muted"
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
            // Ensure E4: inspect gate fail → primary CTA is rework, not re-run examiner.
            // Other fails: keep「再跑一次」as primary single-step retry.
            (() => {
              if (isLiveStatus(S().live?.run_status) || !S().live?.run_id) return "";
              if (bucket !== "fail" && bucket !== "stop") return "";
              const loop = S().live?.inspect_loop;
              const role = String(t.role || "").toLowerCase();
              const isInspect =
                role === "inspect" ||
                /inspect|巡检|门禁|verdict|gates/i.test(String(t.title || t.task_id || ""));
              const canRework = !!(loop && loop.can_rework);
              if (isInspect && canRework) {
                const round = Number(loop.rework_round) || 0;
                const max = Number(loop.rework_max) || 2;
                const n = Math.min(round + 1, max);
                const reworkLabel = `回补并再巡检（第 ${n}/${max} 轮）`;
                return `<button type="button" class="btn primary sm cli-rework-btn" data-rework="${esc(
                  S().live.run_id
                )}" title="按巡检遗漏回补并再对照计划">${esc(reworkLabel)}</button>
                <button type="button" class="btn ghost sm cli-rerun-btn" data-rerun="${esc(
                  t.task_id
                )}" title="仅当怀疑巡检本身坏了时再跑考官">再跑一次</button>`;
              }
              return `<button type="button" class="btn primary sm cli-rerun-btn" data-rerun="${esc(
                t.task_id
              )}" title="再跑这一步">再跑一次</button>`;
            })()
          }
          <button type="button" class="btn ghost sm cli-log-toggle" data-log-toggle="${esc(t.task_id)}" title="展开或折叠详细日志">${
            expanded ? "收起日志" : "详细日志"
          }</button>
          <button type="button" class="icon-btn sm" data-focus="${esc(t.task_id)}" title="聚焦" aria-label="聚焦">${typeof g("ccoIcon") === "function" ? g("ccoIcon")("maximize-2", { size: 14 }) : "◎"}</button>
          <button type="button" class="icon-btn sm" data-close="${esc(t.task_id)}" title="关闭窗口" aria-label="关闭">${typeof g("ccoIcon") === "function" ? g("ccoIcon")("x", { size: 14 }) : "×"}</button>
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
      ${
        autoCommitText
          ? `<div class="cli-window-git ${autoCommit?.ok ? "ok" : "err"}" title="${esc(
              String(autoCommit?.message || autoCommitText)
            )}">${esc(autoCommitText)}</div>`
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
                  : bucket === "stop"
                    ? "本步已随全部停止取消"
                    : bucket === "fail"
                      ? "本步未完成"
                      : "等待开始"
        );
        // P1-3: fail card shows App-composed 执行方式（指定/默认/故障切换…）
        // Never surface raw route_source enum on the main path.
        const routeLabel = String(t.route_label || "").trim();
        if (
          routeLabel &&
          (failed || bucket === "fail" || bucket === "stop" || bucket === "stall")
        ) {
          lines.push(`执行方式：${routeLabel}`);
        }
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
        if (autoCommitText) {
          lines.push(autoCommitText);
        }
        const body = lines
          .filter(Boolean)
          .slice(0, 5)
          .map((l) => esc(l))
          .join("<br/>");
        // Observation-only: parse CCO_STEP markers from log_tail (not a second DAG).
        const stepHtml = formatCcoStepProgress(t.log_tail || t.logTail || "");
        const humanBlock = body
          ? `<div class="cli-window-human muted" data-human="${esc(t.task_id)}">${body}</div>`
          : "";
        return humanBlock + stepHtml;
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

  // P4-4 运行中蓝追光：live 态每次轮询都刷新（chrome 重建也会带上）
  card.classList.toggle("is-running", isLiveStatus(st));

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
