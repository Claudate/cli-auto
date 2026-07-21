/**
 * One-shot extractor: web/js/log.js → features/run/log*
 * A5-2c. Run: node scripts/extract-log-a5-2c.mjs
 */
import fs from "fs";
import path from "path";

const root = path.resolve(import.meta.dirname, "..");
const src = fs.readFileSync(path.join(root, "web/js/log.js"), "utf8");
const lines = src.split("\n");
const slice = (a, b) => lines.slice(a - 1, b).join("\n");
const outDir = path.join(root, "web/js/features/run");

const BRIDGE_ESC = `function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}
function S() {
  return g("state") || {};
}
function $(id) {
  const fn = g("$");
  return typeof fn === "function" ? fn(id) : document.getElementById(id);
}
function $$(sel, root) {
  const fn = g("$$");
  if (typeof fn === "function") return fn(sel, root);
  return Array.from((root || document).querySelectorAll(sel));
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
function toast(msg) {
  const fn = g("toast");
  if (typeof fn === "function") return fn(msg);
  console.log("[toast]", msg);
}
function isLiveStatus(s) {
  const fn = g("isLiveStatus");
  if (typeof fn === "function") return fn(s);
  return /run|active|working|pending|starting|queued/i.test(String(s || ""));
}
function isFailedStatus(s) {
  const fn = g("isFailedStatus");
  if (typeof fn === "function") return fn(s);
  return /fail|error|abort/i.test(String(s || ""));
}
function callG(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}
`;

function rewriteState(code) {
  return code
    .replace(/\bstate\./g, "S().")
    .replace(/\bstate\?/g, "S()?");
}

function rewriteCallGlobals(code, names) {
  let out = code;
  for (const n of names) {
    const re = new RegExp(`\\b${n}\\s*\\(`, "g");
    out = out.replace(re, `callG("${n}")(`);
  }
  return out;
}

// ── logFilter.js ──
{
  const body = `/**
 * [INPUT]: log event objects · filter state
 * [OUTPUT]: AI 事件过滤 · 噪音 · ANSI → HTML · event filter
 * [POS]: A5-2c features/run；自 log.js 抽出；无 IPC
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}
function S() {
  return g("state") || {};
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

export ${slice(546, 560)}

export ${slice(16, 30)}

/** P2-3: event-type filter (all | tool | error). */
export function eventPassesFilter(e, filter) {
  const f = filter || S().logEventFilter || "all";
  if (f === "all") return true;
  const k = String(e?.kind || "").toLowerCase();
  if (f === "tool") return k === "tool_use" || k === "tool_result";
  if (f === "error") {
    if (k === "error") return true;
    const lvl = String(e?.level || "").toLowerCase();
    if (lvl === "error" || lvl === "warn") return true;
    const blob = \`\${e?.title || ""} \${e?.summary || ""}\`.toLowerCase();
    return /\\berror\\b|failed|panic|traceback|exception/.test(blob);
  }
  return true;
}

/**
 * P2-3: minimal ANSI → HTML (raw mode only).
 * Supports SGR bold/dim/colors 30–37 / 90–97 and reset. Strips other CSI.
 */
export ${slice(52, 114)}
`;
  fs.writeFileSync(path.join(outDir, "logFilter.js"), body);
  console.log("logFilter.js", body.split("\n").length);
}

// ── logRender.js ──
{
  let tr = slice(1251, 1334);
  tr = tr
    .replace(/^function transcriptRole/, "export function transcriptRole")
    .replace(/^function renderTranscriptLine/m, "export function renderTranscriptLine")
    .replace(/^function renderLogEvent/m, "export function renderLogEvent");
  const body = `/**
 * [INPUT]: log events
 * [OUTPUT]: transcript / pretty log row HTML
 * [POS]: A5-2c features/run；自 log.js 抽出
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { isAiInteractionEvent, isNoiseText } from "./logFilter.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
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

${tr}
`;
  fs.writeFileSync(path.join(outDir, "logRender.js"), body);
  console.log("logRender.js", body.split("\n").length);
}

// ── logVirtual.js ──
{
  const body = `/**
 * [INPUT]: container DOM · event items · mode/stick
 * [OUTPUT]: P2-3 虚拟列表 mount/paint（阈值窗渲染）
 * [POS]: A5-2c features/run；算法自 log.js 原样迁入
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { renderTranscriptLine, renderLogEvent } from "./logRender.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}
function S() {
  return g("state") || {};
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
    return \`<div class="tx-line role-\${role}"><div class="tx-role">\${
      role === "out" ? "log" : role
    }</div><div class="tx-body">\${bodyHtml}</div></div>\`;
  }
  return renderTranscriptLine(e);
}

/**
 * P2-3 virtual list: mount/update a scrollable window over \`items\`.
 * Filter switch or mode change → new key → rebuild (scroll resets to bottom when stick).
 * Returns true if virtual path used.
 */
export function mountVirtualLog(container, items, { mode, stick } = {}) {
  if (!container) return false;
  const list = Array.isArray(items) ? items : [];
  const m = mode || S().logViewMode || "term";
  if (list.length < LOG_VIRTUAL_THRESHOLD) return false;

  const key = \`\${m}|\${list.length}|\${
    list[list.length - 1]?.id || ""
  }|\${S().logEventFilter || "all"}\`;
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
  windowEl.style.transform = \`translateY(\${start * LOG_ROW_EST_PX}px)\`;
  windowEl.innerHTML =
    html || \`<div class="cli-empty-ai muted">当前窗口无可见行</div>\`;
}
`;
  fs.writeFileSync(path.join(outDir, "logVirtual.js"), body);
  console.log("logVirtual.js", body.split("\n").length);
}

// ── logContent.js ──
{
  let content = [
    slice(251, 272), // logPanelSignature
    slice(386, 426), // renderLogConsoleHtml
    slice(428, 457), // humanizePlannerLogLine
    slice(459, 528), // fillPlannerLog
    slice(562, 580), // aiLogPlainText
    slice(582, 672), // panelLogContent
    slice(674, 691), // panelLogHtml
    slice(693, 706), // fillPanelLogBody
  ].join("\n\n");

  content = rewriteState(content);
  content = content
    .replace(/^function logPanelSignature/m, "export function logPanelSignature")
    .replace(/^function renderLogConsoleHtml/m, "export function renderLogConsoleHtml")
    .replace(/^function humanizePlannerLogLine/m, "export function humanizePlannerLogLine")
    .replace(/^function fillPlannerLog/m, "export function fillPlannerLog")
    .replace(/^function aiLogPlainText/m, "export function aiLogPlainText")
    .replace(/^function panelLogContent/m, "export function panelLogContent")
    .replace(/^function panelLogHtml/m, "export function panelLogHtml")
    .replace(/^function fillPanelLogBody/m, "export function fillPanelLogBody");

  const body = `/**
 * [INPUT]: task/planner log DTO · logVirtual / logFilter / logRender
 * [OUTPUT]: panel content · planner log · fill body · plain text
 * [POS]: A5-2c features/run；自 log.js 抽出
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  isAiInteractionEvent,
  eventPassesFilter,
  ansiToHtml,
  isNoiseText,
} from "./logFilter.js";
import { renderTranscriptLine, renderLogEvent } from "./logRender.js";
import {
  LOG_VIRTUAL_THRESHOLD,
  isNearBottom,
  mountVirtualLog,
  renderLogRowHtml,
} from "./logVirtual.js";

${BRIDGE_ESC}

${content}
`;
  fs.writeFileSync(path.join(outDir, "logContent.js"), body);
  console.log("logContent.js", body.split("\n").length);
}

// ── logActions.js ──
{
  const body = `/**
 * [INPUT]: ccoRun / ccoResult / runApi · gateway
 * [OUTPUT]: stop/resume/rework/accept/openTerminal/export/handoff/doctor
 * [POS]: A5-2c features/run；删 classic invoke fallback（cco* 已就绪）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：start_run 旁路；rework 只走 start_rework / ccoResult。
 */

import * as gateway from "../../shared/gateway.js";
import { isAiInteractionEvent, eventPassesFilter } from "./logFilter.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}
function S() {
  return g("state") || {};
}
function $(id) {
  const fn = g("$");
  return typeof fn === "function" ? fn(id) : document.getElementById(id);
}
function toast(msg) {
  const fn = g("toast");
  if (typeof fn === "function") return fn(msg);
  console.log("[toast]", msg);
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
function callG(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

/** multi-cli P2-6: render handoff Board strip from live view. */
export function renderHandoffBoardStrip() {
  const strip = $("handoff-board-strip");
  const rowsEl = $("handoff-board-rows");
  if (!strip || !rowsEl) return;
  const live = S().live || {};
  const board = live.handoff_board || live.handoffBoard || [];
  const mdPath = live.handoff_md_path || live.handoffMdPath || null;
  if (!board.length && !mdPath) {
    strip.hidden = true;
    rowsEl.innerHTML = "";
    return;
  }
  strip.hidden = false;
  const openBtn = $("btn-open-handoff");
  if (openBtn) {
    openBtn.disabled = !mdPath;
    openBtn.title = mdPath ? \`打开 \${mdPath}\` : "暂无 handoff.md";
  }
  if (!board.length) {
    rowsEl.innerHTML =
      '<span class="muted" style="font-size:0.75rem">账本已生成，Board 尚空</span>';
    return;
  }
  rowsEl.innerHTML = board
    .map((r) => {
      const st = String(r.status || "").toLowerCase();
      let cls = "handoff-board-chip";
      if (st.includes("fail") || st.includes("timeout") || st.includes("error")) {
        cls += " is-fail";
      } else if (st === "running" || st === "starting" || st === "queued") {
        cls += " is-run";
      } else if (st === "done" || st === "skipped") {
        cls += " is-done";
      }
      const role = r.role ? \` · \${r.role}\` : "";
      const prov = r.provider ? \` · \${r.provider}\` : "";
      const cost =
        r.cost != null && Number.isFinite(Number(r.cost))
          ? \` · $\${Number(r.cost).toFixed(3)}\`
          : "";
      return (
        \`<span class="\${cls}" title="\${esc(r.scope || "")}">\` +
        \`<span class="hb-id">\${esc(r.id)}</span>\` +
        \`<span class="hb-meta">\${esc(st)}\${esc(role)}\${esc(prov)}\${esc(
          cost
        )}</span>\` +
        \`</span>\`
      );
    })
    .join("");
}

export async function openHandoffLedger() {
  const path =
    S().live?.handoff_md_path || S().live?.handoffMdPath || null;
  if (!path) {
    toast("当前运行尚无 handoff.md");
    return;
  }
  try {
    await gateway.openPath(path);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** P2-3: export visible task logs as Markdown download. */
export function exportBoardLogsMd() {
  const tasks = Array.isArray(S().live?.tasks) ? S().live.tasks : [];
  if (!tasks.length) {
    toast("没有可导出的任务日志");
    return;
  }
  const filter = S().cliStatusFilter || "all";
  const shown =
    filter && filter !== "all"
      ? tasks.filter((t) => callG("taskBucket", t) === filter)
      : tasks;
  const runId = S().live?.run_id || S().live?.runId || "run";
  const lines = [];
  lines.push(\`# cco 执行日志导出\`);
  lines.push("");
  lines.push(\`- run: \\\`\${runId}\\\`\`);
  lines.push(
    \`- project: \\\`\${S().live?.project_path || S().selectedPath || ""}\\\`\`
  );
  lines.push(\`- exported: \${new Date().toISOString()}\`);
  lines.push(\`- filter: \${filter}\`);
  lines.push("");
  for (const t of shown) {
    lines.push(\`## \${t.title || t.task_id} (\\\`\${t.task_id}\\\`)\`);
    lines.push("");
    lines.push(
      \`- status: **\${t.status}** · provider: \\\`\${t.provider || "?"}\\\`\`
    );
    if (t.error_summary || t.error) {
      lines.push(\`- error: \${t.error_summary || t.error}\`);
    }
    lines.push("");
    const events = (Array.isArray(t.log_events) ? t.log_events : [])
      .filter(isAiInteractionEvent)
      .filter((e) => eventPassesFilter(e, S().logEventFilter));
    if (events.length) {
      lines.push("\`\`\`");
      for (const e of events.slice(-80)) {
        lines.push([e.kind, e.title, e.summary].filter(Boolean).join(" · "));
      }
      lines.push("\`\`\`");
    } else if (t.log_tail) {
      lines.push("\`\`\`");
      lines.push(String(t.log_tail).slice(-4000));
      lines.push("\`\`\`");
    } else {
      lines.push("_无日志_");
    }
    lines.push("");
  }
  const blob = new Blob([lines.join("\\n")], {
    type: "text/markdown;charset=utf-8",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = \`cco-log-\${String(runId).replace(/[^\\w.-]+/g, "_")}.md\`;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 0);
  toast(\`已导出 \${shown.length} 个任务日志\`);
}

export async function openExternalTerminal(taskId) {
  const runId = S().live?.run_id;
  if (!runId || !taskId) return toast("无运行中的任务日志可跟随");
  const cco = g("ccoRun");
  try {
    let session;
    if (cco && typeof cco.vm?.openTerminal === "function") {
      session = await cco.vm.openTerminal({
        runId,
        taskId,
        kind: "external",
      });
    } else {
      session = await gateway.openTaskTerminal({
        runId,
        taskId,
        kind: "external",
      });
    }
    const launcher = session?.launcher || "terminal";
    toast(\`已打开外置终端（\${launcher}）跟随 \${taskId}\`);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** Prefer ccoRun (A4+); no classic invoke fallback. */
export async function cancelTask() {
  const cco = g("ccoRun");
  if (cco && typeof cco.stopTask === "function") {
    return cco.stopTask(S().selectedTaskId);
  }
  toast("执行台未就绪，请稍后重试");
}

export async function stopAll() {
  const cco = g("ccoRun");
  if (cco && typeof cco.stopAll === "function") {
    return cco.stopAll();
  }
  toast("执行台未就绪，请稍后重试");
}

export async function resumeRun() {
  const cco = g("ccoRun");
  if (cco && typeof cco.resume === "function") {
    return cco.resume();
  }
  toast("执行台未就绪，请稍后重试");
}

/** P-loop L2: rework via app start_rework (ccoResult). */
export async function startReworkWave() {
  const cco = g("ccoResult");
  if (cco && typeof cco.startRework === "function") {
    return cco.startRework();
  }
  toast("结果台未就绪，请稍后重试");
}

/** P-loop L2: accept residual (ccoResult). */
export async function acceptRunResidual() {
  const cco = g("ccoResult");
  if (cco && typeof cco.acceptResidual === "function") {
    return cco.acceptResidual();
  }
  toast("结果台未就绪，请稍后重试");
}

export async function loadDoctor() {
  try {
    const d = await gateway.doctor(S().selectedPath || null);
    S().doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    const list = $("doctor-list");
    if (list) {
      list.innerHTML = \`<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th></tr></thead>
      <tbody>
        \${lines
          .map(
            (l) => \`<tr>
          <td>\${esc(l.name)}</td>
          <td>\${callG("badge", l.ok ? "ok" : "failed") || (l.ok ? "ok" : "fail")}</td>
          <td class="muted">\${esc(l.detail)}</td>
        </tr>\`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">\${
      d.ok ? "关键检查通过" : "存在失败项，请按详情处理"
    }</p>\`;
    }
    callG("renderDoctorWarn");
  } catch (e) {
    toast(String(e));
  }
}
`;
  fs.writeFileSync(path.join(outDir, "logActions.js"), body);
  console.log("logActions.js", body.split("\n").length);
}

// ── logBoard.js ──
{
  let board = slice(708, 1237); // renderCliBoard + stallStrip + renderTaskList
  // also include renderDetailLog
  board += "\n\n" + slice(1239, 1249);

  board = rewriteState(board);
  const gNames = [
    "taskBucket",
    "sortTasksByStatus",
    "formatElapsed",
    "taskErrorSummary",
    "statusDot",
    "statusLabel",
    "fiveStateLabel",
    "fitCliBodyHeight",
    "savePanelPos",
    "flowEmptyBoard",
    "flowStallUserText",
  ];
  board = rewriteCallGlobals(board, gNames);

  board = board
    .replace(/^function renderCliBoard/m, "export function renderCliBoard")
    .replace(/^function stallStripText/m, "export function stallStripText")
    .replace(/^function renderTaskList/m, "export function renderTaskList")
    .replace(/^function renderDetailLog/m, "export function renderDetailLog");

  // fix typeof fiveStateLabel checks - rewriteCallGlobals already wrapped calls
  // typeof g("fiveStateLabel") won't work for typeof checks - restore carefully
  board = board
    .replace(
      /typeof callG\("fiveStateLabel"\) === "function"/g,
      'typeof g("fiveStateLabel") === "function"'
    )
    .replace(
      /typeof callG\("flowEmptyBoard"\) === "function"/g,
      'typeof g("flowEmptyBoard") === "function"'
    )
    .replace(
      /typeof callG\("flowStallUserText"\) === "function"/g,
      'typeof g("flowStallUserText") === "function"'
    )
    // callG("fiveStateLabel")(bucket) after typeof g check - the ternary used
    // typeof fiveStateLabel === "function" ? fiveStateLabel(bucket) : ...
    // After rewrite: typeof g("fiveStateLabel") === "function" ? callG("fiveStateLabel")(bucket)
    // which is fine.
    // cancelTask / openExternalTerminal / renderHandoffBoardStrip / aiLogPlainText / paintVirtualLogWindow / fillPanelLogBody / logPanelSignature / isNearBottom / isLiveStatus need import or local
    ;

  const body = `/**
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

${BRIDGE_ESC}

${board}
`;
  fs.writeFileSync(path.join(outDir, "logBoard.js"), body);
  console.log("logBoard.js", body.split("\n").length);
}

// ── logDesk.js (install surface for window.ccoLog) ──
{
  const body = `/**
 * [INPUT]: log* modules
 * [OUTPUT]: public desk API → window.ccoLog / classic facade
 * [POS]: A5-2c features/run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as logFilter from "./logFilter.js";
import * as logRender from "./logRender.js";
import * as logVirtual from "./logVirtual.js";
import * as logContent from "./logContent.js";
import * as logActions from "./logActions.js";
import * as logBoard from "./logBoard.js";

/**
 * Full log surface for window.ccoLog (classic log.js facade).
 */
export function createLogDesk() {
  return {
    // board / primary paint
    renderCliBoard: logBoard.renderCliBoard,
    renderTaskList: logBoard.renderTaskList,
    renderDetailLog: logBoard.renderDetailLog,
    stallStripText: logBoard.stallStripText,
    // content
    fillPlannerLog: logContent.fillPlannerLog,
    fillPanelLogBody: logContent.fillPanelLogBody,
    panelLogContent: logContent.panelLogContent,
    panelLogHtml: logContent.panelLogHtml,
    aiLogPlainText: logContent.aiLogPlainText,
    renderLogConsoleHtml: logContent.renderLogConsoleHtml,
    logPanelSignature: logContent.logPanelSignature,
    // virtual
    mountVirtualLog: logVirtual.mountVirtualLog,
    paintVirtualLogWindow: logVirtual.paintVirtualLogWindow,
    isNearBottom: logVirtual.isNearBottom,
    LOG_VIRTUAL_THRESHOLD: logVirtual.LOG_VIRTUAL_THRESHOLD,
    // filter / render
    isAiInteractionEvent: logFilter.isAiInteractionEvent,
    eventPassesFilter: logFilter.eventPassesFilter,
    ansiToHtml: logFilter.ansiToHtml,
    isNoiseText: logFilter.isNoiseText,
    renderLogEvent: logRender.renderLogEvent,
    renderTranscriptLine: logRender.renderTranscriptLine,
    // actions (ccoRun/ccoResult; no invoke fallback)
    cancelTask: logActions.cancelTask,
    stopAll: logActions.stopAll,
    resumeRun: logActions.resumeRun,
    startReworkWave: logActions.startReworkWave,
    acceptRunResidual: logActions.acceptRunResidual,
    openExternalTerminal: logActions.openExternalTerminal,
    exportBoardLogsMd: logActions.exportBoardLogsMd,
    openHandoffLedger: logActions.openHandoffLedger,
    renderHandoffBoardStrip: logActions.renderHandoffBoardStrip,
    loadDoctor: logActions.loadDoctor,
  };
}

export default createLogDesk;
`;
  fs.writeFileSync(path.join(outDir, "logDesk.js"), body);
  console.log("logDesk.js", body.split("\n").length);
}

console.log("done");
