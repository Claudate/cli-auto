/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke 桥
 * [OUTPUT]: state UI 片段 · 全局 invoke/requireGateway（A2/A5-2e）
 * [POS]: web/js D4 自 app.js 纵切；A2 ESM 入口 main.js 后挂 ccoGateway
 * note: invoke/getInvoke = 迁移期桥；业务 classic 用 requireGateway()→ccoGateway；
 *       feature 内禁止直接 invoke/__TAURI__（只经 shared/gateway.js）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — state */


const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];

// A2/A5: classic `const` is not on window; ESM features/* need the same object.
if (typeof window !== "undefined") {
  window.$ = $;
  window.$$ = $$;
}

const LOG_FONT_KEY = "cco.logFontSize";
/**
 * P2-16 / S0：存储键语义仍是「暂停确认」——值为 "1" 表示停在拆分台。
 * 产品默认：停在拆分台（主受众看见拆分）；高级「拆分后自动开始」写 "0"。
 * 未写入过键 → 默认停台（与 D1 旧默认 true 翻转）。
 */
const PAUSE_CONFIRM_KEY = "cco.pauseConfirmAfterPlan";
/** C3 方案 B：聊天/计划卡「拆成步骤」是否直开 analyze（默认关 = 方案 A） */
const CHAT_ASSIGN_DIRECT_KEY = "cco.chatAssignDirect";

function chatAssignDirectEnabled() {
  return localStorage.getItem(CHAT_ASSIGN_DIRECT_KEY) === "1";
}

function setChatAssignDirectEnabled(on) {
  localStorage.setItem(CHAT_ASSIGN_DIRECT_KEY, on ? "1" : "0");
}

const state = {
  page: "welcome", // welcome | workspace | chat | doctor | help | settings
  /** Mode B phase: pick | planning | confirm | running | done */
  phase: "pick",
  /** P2-4: true when this webview is the detached system monitor window */
  isMonitorWindow: false,
  projects: [],
  selectedPath: null,
  live: null,
  pollTimer: null,
  logStick: true,
  now: Date.now(),
  plans: [],
  selectedPlan: null,
  planPreview: null,
  selectedTaskId: null,
  filterFailedOnly: false, // legacy, mapped to cliStatusFilter=fail
  cliStatusFilter: "all", // all | run | wait | stall | done | fail
  planCollapsed: true, // 默认只显示当前计划条
  planChooserOpen: false,
    doctorCache: null, // { ok, at, lines }
  doctorDismissedKey: null, // 用户点忽略后隐藏同类警告
  /** 默认 false：拆完停拆分台；仅当用户显式开「拆分后自动开始」(存 "0") 才 auto confirm_start */
  autoStartAfterPlan: localStorage.getItem(PAUSE_CONFIRM_KEY) === "0",
  closedPanels: {}, // taskId -> true
  panelPos: JSON.parse(localStorage.getItem("cco.panelPos") || "{}"),
  /** P1-1：每任务日志签名，避免全量 innerHTML 重绘闪烁 */
  logPanelSig: {}, // taskId -> signature string
  logFontSize: Number(localStorage.getItem(LOG_FONT_KEY) || 14) || 14,
  logViewMode: (() => {
    if (localStorage.getItem("cco.logViewMigrated") !== "term-v1") {
      localStorage.setItem("cco.logViewMode", "term");
      localStorage.setItem("cco.logViewMigrated", "term-v1");
      return "term";
    }
    return localStorage.getItem("cco.logViewMode") || "term";
  })(),
  /** P2-3: event type filter for log panels — all | tool | error */
  logEventFilter: localStorage.getItem("cco.logEventFilter") || "all",
  planJobId: null,
  planJob: null,
  confirmTaskId: null,
  confirmEditing: false,
  /** When opening confirm during a live run, return here via「返回监视」. */
  returnPhaseAfterConfirm: null,
  planJobPollTimer: null,
  drag: null, // { id, ox, oy }
  dragSession: {}, // taskId -> true 本会话拖过才保留 free
  taskStripExpanded: localStorage.getItem("cco.taskStripExpanded") === "1",
  /** R1: 执行台默认展开进度看板（主视觉）；用户可再折叠 */
  taskDashCollapsed: localStorage.getItem("cco.taskDashCollapsed") === "1",
  /**
   * Per-task log expand map. R1 default OFF (logs secondary).
   * Missing key → collapsed; user toggle writes true/false.
   */
  cliLogExpanded: {},
  /** Monitor details fold open state (session) */
  monitorLogsOpen: localStorage.getItem("cco.monitorLogsOpen") === "1",
  cliBodyHeight: (() => {
    const v = localStorage.getItem("cco.cliBodyHeight");
    if (v === "auto" || v == null || v === "") return "auto";
    const n = Number(v);
    return Number.isFinite(n) && n > 0 ? n : "auto";
  })(),
  assigning: false, // 分配计划进行中（防连点 + 按钮转圈）
  plansLoading: false,
  /** H2: plan meta (ever_completed / last_run_*) for chooser + plan-rail */
  planMetaItems: [],
  planMetaByPath: {},
  /** H2: 默认隐藏已成功执行；开关「显示已执行」可展开（chooser/右轨共用） */
  showExecutedPlans: localStorage.getItem("cco.showExecutedPlans") === "1",
  planPollFails: 0,
  planStartedAt: 0,
  /** 按项目缓存规划会话，切页/切项目不丢 */
  planSessions: {}, // path -> session snapshot
  /** 聊天共建计划（按项目会话在磁盘 .cco/chat/；此处为当前 UI 态） */
  chatSession: { session_id: "default", messages: [], draft_plan: null },
  chatDraftPlan: null, // 已落盘相对路径，启用「分配计划」
  chatBusy: false,
  chatWaitStartedAt: 0,
  chatProjectPath: null, // 当前 chatSession 所属项目，防串台
  /** 按项目缓存聊天 UI 态，切页不丢；与磁盘 .cco/chat 双写 */
  chatSessions: {}, // path -> { session_id, messages, draft_plan, draftPath, busy, waitStartedAt }
};

// A2/A5 ESM bridge — features/* read window.state + classic helpers
if (typeof window !== "undefined") {
  window.state = state;
  window.PAUSE_CONFIRM_KEY = PAUSE_CONFIRM_KEY;
}
// Helpers assigned after definition (toast/esc/badge) — see bottom bridge

/* ── Status labels (人话 · R2 五态优先) ── */
const STATUS_LABEL = {
  completed: "已完成",
  done: "已完成",
  ok: "已完成",
  running: "进行中",
  starting: "进行中",
  queued: "排队中",
  validated: "进行中",
  init: "进行中",
  paused: "排队中",
  resuming: "进行中",
  failed: "失败",
  aborted: "失败",
  timeout: "失败",
  stopped: "失败",
  pending: "排队中",
  waiting: "排队中",
  ready: "排队中",
  skipped: "已完成",
  idle: "排队中",
  err: "失败",
  stall: "已卡住",
  stalled: "已卡住",
};

/** R2 product five states */
const FIVE_STATE_LABEL = {
  wait: "排队中",
  run: "进行中",
  done: "已完成",
  stall: "已卡住",
  fail: "失败",
};

function statusLabel(status) {
  const s = String(status || "").toLowerCase();
  return STATUS_LABEL[s] || status || "—";
}

function fiveStateLabel(bucket) {
  return FIVE_STATE_LABEL[bucket] || "排队中";
}

/** True when live task is past half stall threshold (or last retry was stall). */
function isStalledTask(t) {
  if (!t) return false;
  const st = String(t.status || "").toLowerCase();
  const live =
    typeof isLiveStatus === "function"
      ? isLiveStatus(st)
      : ["running", "starting", "queued", "validated", "init", "resuming"].includes(st);
  if (!live) return false;
  const thr =
    t.stall_threshold_secs != null ? Number(t.stall_threshold_secs) : null;
  const idle = t.stall_idle_secs != null ? Number(t.stall_idle_secs) : null;
  if (idle != null && thr != null && thr > 0 && idle >= Math.max(15, thr * 0.5)) {
    return true;
  }
  return String(t.last_retry_reason || "").toLowerCase() === "stall" && live;
}

function toast(msg) {
  const t = $("#toast");
  if (!t) {
    console.log("[toast]", msg);
    return;
  }
  t.hidden = false;
  t.textContent = msg;
  clearTimeout(toast._t);
  toast._t = setTimeout(() => {
    t.hidden = true;
  }, 3200);
}

function getInvoke() {
  const w = window;
  // Tauri 2 多种全局形态，全部兜底
  const candidates = [
    w.__TAURI__?.core?.invoke && w.__TAURI__.core.invoke.bind(w.__TAURI__.core),
    w.__TAURI__?.tauri?.invoke && w.__TAURI__.tauri.invoke.bind(w.__TAURI__.tauri),
    w.__TAURI_INTERNALS__?.invoke && w.__TAURI_INTERNALS__.invoke.bind(w.__TAURI_INTERNALS__),
    typeof w.__TAURI_INVOKE__ === "function" && w.__TAURI_INVOKE__,
  ];
  for (const c of candidates) {
    if (typeof c === "function") return c;
  }
  return null;
}

function isTauriReady() {
  return !!getInvoke();
}

async function invoke(cmd, args = {}) {
  const inv = getInvoke();
  if (!inv) throw new Error("请通过 CCO.app 启动（invoke 不可用）");
  try {
    return await inv(cmd, args);
  } catch (e) {
    const msg = e?.message || e?.toString?.() || String(e);
    throw new Error(msg);
  }
}

/**
 * A5-2e: classic business IPC → named gateway methods only.
 * Command strings live in shared/gateway.js; features import that module.
 * @returns {import('./shared/gateway.js').gateway}
 */
function requireGateway() {
  const g = typeof window !== "undefined" ? window.ccoGateway : null;
  if (!g) throw new Error("请通过 CCO.app 启动（gateway 未就绪）");
  return g;
}

async function openNativeDialog(opts) {
  // Prefer gateway (A5-2e); bridge falls back for pre-main classic only.
  if (window.ccoGateway?.dialogOpen) {
    return window.ccoGateway.dialogOpen(opts);
  }
  const d =
    window.__TAURI__?.dialog ||
    window.__TAURI__?.plugins?.dialog ||
    null;
  if (d?.open) return d.open(opts);
  try {
    if (getInvoke()) {
      // plugin command 需 { options } 包装（含 defaultPath）
      return await invoke("plugin:dialog|open", { options: opts });
    }
  } catch (_) {}
  throw new Error("对话框不可用");
}

function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** 轻量 Markdown → 安全 HTML（无外部依赖；确认屏/计划说明用） */
function renderMarkdown(src) {
  const raw = String(src ?? "");
  if (!raw.trim()) return '<p class="md-empty">（无任务说明）</p>';

  // 1) 抽出 fenced code，避免内部被二次处理
  const fences = [];
  let text = raw.replace(/```([\w-]+)?\n([\s\S]*?)```/g, (_, lang, code) => {
    const i = fences.length;
    fences.push({ lang: (lang || "").trim(), code: code.replace(/\n$/, "") });
    return `\n\n%%FENCE${i}%%\n\n`;
  });

  // 2) 按行做块级解析
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const blocks = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    // fence placeholder
    const fm = line.trim().match(/^%%FENCE(\d+)%%$/);
    if (fm) {
      blocks.push({ type: "fence", idx: Number(fm[1]) });
      i++;
      continue;
    }
    // hr
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(line.trim())) {
      blocks.push({ type: "hr" });
      i++;
      continue;
    }
    // heading
    const hm = line.match(/^(#{1,6})\s+(.+)$/);
    if (hm) {
      blocks.push({ type: "h", level: hm[1].length, text: hm[2].trim() });
      i++;
      continue;
    }
    // blockquote
    if (/^>\s?/.test(line)) {
      const qs = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) {
        qs.push(lines[i].replace(/^>\s?/, ""));
        i++;
      }
      blocks.push({ type: "quote", text: qs.join("\n") });
      continue;
    }
    // table
    if (
      line.includes("|") &&
      i + 1 < lines.length &&
      /^\s*\|?\s*:?-{3,}/.test(lines[i + 1])
    ) {
      const rows = [];
      while (i < lines.length && lines[i].includes("|")) {
        if (/^\s*\|?\s*:?-{3,}/.test(lines[i])) {
          i++;
          continue; // skip separator
        }
        const cells = lines[i]
          .trim()
          .replace(/^\|/, "")
          .replace(/\|$/, "")
          .split("|")
          .map((c) => c.trim());
        rows.push(cells);
        i++;
      }
      blocks.push({ type: "table", rows });
      continue;
    }
    // ul
    if (/^\s*[-*+]\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      blocks.push({ type: "ul", items });
      continue;
    }
    // ol
    if (/^\s*\d+\.\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+\.\s+/, ""));
        i++;
      }
      blocks.push({ type: "ol", items });
      continue;
    }
    // paragraph
    const paras = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^#{1,6}\s/.test(lines[i]) &&
      !/^>\s?/.test(lines[i]) &&
      !/^\s*[-*+]\s+/.test(lines[i]) &&
      !/^\s*\d+\.\s+/.test(lines[i]) &&
      !/^%%FENCE\d+%%$/.test(lines[i].trim()) &&
      !/^(-{3,}|\*{3,}|_{3,})$/.test(lines[i].trim()) &&
      !(lines[i].includes("|") && i + 1 < lines.length && /^\s*\|?\s*:?-{3,}/.test(lines[i + 1] || ""))
    ) {
      paras.push(lines[i]);
      i++;
    }
    blocks.push({ type: "p", text: paras.join("\n") });
  }

  function inlineMd(s) {
    let x = esc(s);
    // code
    x = x.replace(/`([^`]+)`/g, "<code>$1</code>");
    // bold
    x = x.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    x = x.replace(/__([^_]+)__/g, "<strong>$1</strong>");
    // italic (avoid matching bold leftovers)
    x = x.replace(/(^|[^*])\*([^*]+)\*(?![*])/g, "$1<em>$2</em>");
    x = x.replace(/(^|[^_])_([^_]+)_(?!_)/g, "$1<em>$2</em>");
    // links [t](url)
    x = x.replace(
      /\[([^\]]+)\]\((https?:[^)\s]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
    );
    // auto newline inside paragraph → <br>
    x = x.replace(/\n/g, "<br>");
    return x;
  }

  const html = blocks
    .map((b) => {
      if (b.type === "fence") {
        const f = fences[b.idx] || { lang: "", code: "" };
        const lang = f.lang ? ` data-lang="${esc(f.lang)}"` : "";
        return `<pre class="md-code"${lang}><code>${esc(f.code)}</code></pre>`;
      }
      if (b.type === "hr") return "<hr class=\"md-hr\">";
      if (b.type === "h") {
        const lv = Math.min(6, Math.max(1, b.level));
        return `<h${lv} class="md-h">${inlineMd(b.text)}</h${lv}>`;
      }
      if (b.type === "quote") {
        return `<blockquote class="md-quote">${inlineMd(b.text)}</blockquote>`;
      }
      if (b.type === "ul") {
        return `<ul class="md-ul">${b.items
          .map((it) => `<li>${inlineMd(it)}</li>`)
          .join("")}</ul>`;
      }
      if (b.type === "ol") {
        return `<ol class="md-ol">${b.items
          .map((it) => `<li>${inlineMd(it)}</li>`)
          .join("")}</ol>`;
      }
      if (b.type === "table") {
        if (!b.rows.length) return "";
        const head = b.rows[0];
        const body = b.rows.slice(1);
        return `<table class="md-table"><thead><tr>${head
          .map((c) => `<th>${inlineMd(c)}</th>`)
          .join("")}</tr></thead><tbody>${body
          .map(
            (r) =>
              `<tr>${r.map((c) => `<td>${inlineMd(c)}</td>`).join("")}</tr>`
          )
          .join("")}</tbody></table>`;
      }
      if (b.type === "p") return `<p class="md-p">${inlineMd(b.text)}</p>`;
      return "";
    })
    .join("\n");

  return html || `<p class="md-p">${inlineMd(raw)}</p>`;
}


function shortPath(p) {
  if (!p) return "—";
  const parts = String(p).split("/").filter(Boolean);
  return parts.length > 3 ? "…/" + parts.slice(-3).join("/") : p;
}


/** 把绝对路径收成项目相对路径，便于列表匹配与预览 */
function normalizePlanPath(planPath, projectRoot = state.selectedPath) {
  if (!planPath) return null;
  let p = String(planPath).trim();
  if (!p) return null;
  if (projectRoot) {
    const root = String(projectRoot).replace(/\/+$/, "");
    if (p === root) return null;
    if (p.startsWith(root + "/")) p = p.slice(root.length + 1);
  }
  // 兼容 file:// 与重复项目前缀
  p = p.replace(/^file:\/\//, "");
  return p;
}

function planDisplayName(path) {
  if (!path) return "—";
  const parts = String(path).split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

function badge(status) {
  const s = String(status || "").toLowerCase();
  let cls = "muted";
  if (["completed", "done", "ok", "skipped"].includes(s)) cls = "ok";
  else if (["running", "starting", "queued", "validated", "init", "paused", "resuming", "pending"].includes(s))
    cls = "warn";
  else if (["failed", "aborted", "timeout", "stopped", "err"].includes(s)) cls = "err";
  return `<span class="badge ${cls}">${esc(statusLabel(status))}</span>`;
}

/** @param {string} status @param {object} [task] for stall tint */
function statusDot(status, task) {
  if (task && typeof isStalledTask === "function" && isStalledTask(task)) return "warn";
  const s = String(status || "").toLowerCase();
  if (["running", "starting", "queued", "validated", "init"].includes(s)) return "live";
  if (["paused", "resuming", "pending"].includes(s)) return "warn";
  if (["failed", "aborted", "timeout", "stopped"].includes(s)) return "err";
  if (["completed", "done", "ok", "skipped"].includes(s)) return "ok";
  return "";
}

function isLiveStatus(s) {
  return ["running", "starting", "queued", "validated", "init", "resuming"].includes(
    String(s || "").toLowerCase()
  );
}

function isPausedStatus(s) {
  return String(s || "").toLowerCase() === "paused";
}

/** True when the currently selected project has a live run.
 *  Locks plan/edit actions inside that project only — other projects
 *  may still be switched to and run in parallel (no global single-run lock). */
function hasActiveRun() {
  // Paused is not "active" for lock purposes: user may edit pending tasks.
  return !!(state.live?.run_id && isLiveStatus(state.live?.run_status));
}

function isRunPaused() {
  return !!(state.live?.run_id && isPausedStatus(state.live?.run_status));
}

/** Task may be edited only when not yet executed (pending/queued). */
function isTaskPendingStatus(s) {
  const v = String(s || "").toLowerCase();
  return !v || v === "pending" || v === "queued" || v === "waiting" || v === "ready";
}

function liveTaskById(taskId) {
  if (!taskId) return null;
  return (state.live?.tasks || []).find((t) => t.task_id === taskId || t.id === taskId) || null;
}

/**
 * Edit allowed when:
 * - confirming a fresh split (no run yet), or
 * - run is paused, and the selected task has not started.
 */
function canEditSelectedTask(taskId = state.confirmTaskId) {
  if (!taskId) return false;
  if (hasActiveRun()) return false;
  if (state.phase === "confirm" && !state.live?.run_id) return true;
  if (!isRunPaused()) return false;
  const t = liveTaskById(taskId);
  if (!t) return true; // no live row yet → treat as not started
  return isTaskPendingStatus(t.status);
}

function toastRunLocked(action = "此操作") {
  toast(`计划运行中，请先停止后再${action}`);
}

function isFailedStatus(s) {
  return ["failed", "aborted", "timeout", "stopped"].includes(String(s || "").toLowerCase());
}

function isDoneStatus(s) {
  return ["completed", "done", "ok", "skipped"].includes(String(s || "").toLowerCase());
}

function formatElapsed(startedAt, finishedAt) {
  if (!startedAt) return "—";
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return "—";
  const end = finishedAt ? Date.parse(finishedAt) : state.now;
  if (Number.isNaN(end)) return "—";
  let sec = Math.max(0, Math.floor((end - start) / 1000));
  const h = Math.floor(sec / 3600);
  sec %= 3600;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function taskErrorSummary(t) {
  if (!t) return "";
  if (t.error) return String(t.error).split("\n")[0].slice(0, 160);
  if (isFailedStatus(t.status) && t.log_tail) {
    const lines = String(t.log_tail).trim().split("\n").filter(Boolean);
    return (lines[lines.length - 1] || "").slice(0, 160);
  }
  return "";
}

function applyLogFontSize(px) {
  state.logFontSize = px;
  document.documentElement.style.setProperty("--log-font-size", `${px / 16}rem`);
  localStorage.setItem(LOG_FONT_KEY, String(px));
  $$("#log-font-group button").forEach((b) => {
    b.classList.toggle("active", Number(b.dataset.size) === px);
  });
  const sel = $("#s-log-font");
  if (sel) sel.value = String(px);
}

/* ── Pages ── */
function showPage(name) {
  // 离开聊天前先缓存会话，避免切页把内存历史冲掉
  if (state.page === "chat" && name !== "chat") {
    try {
      if (typeof stashChatSession === "function") {
        stashChatSession(state.chatProjectPath || state.selectedPath);
      }
    } catch (_) {}
  }
  state.page = name;
  try { updateTopPlanInfo(); } catch (_) {}
  try { updateBgPlanBanner(); } catch (_) {}
  // 切走工作区时先缓存当前规划，保证后台可续
  if (name !== "workspace" && state.selectedPath && isPlanSessionActive()) {
    try { stashPlanSession(state.selectedPath); } catch (_) {}
  }
  $$(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${name}`));
  const sub = $("#page-sub");
  // F3：body 上标记主区角色，便于 CSS 互斥噪音
  try {
    document.body.dataset.ccoPage = name;
    document.body.dataset.ccoPhase = state.phase || "pick";
    document.body.classList.toggle(
      "cco-run-active",
      typeof hasActiveRun === "function" && hasActiveRun()
    );
  } catch (_) {}
  try {
    if (typeof refreshFlowStrips === "function") refreshFlowStrips();
  } catch (_) {}
  if (name === "welcome") {
    $("#page-title").textContent = "欢迎";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "添加项目 → 写计划 → 拆成步骤 → 确认并开始";
    }
  } else if (name === "workspace") {
    updateWorkspaceTitle();
  } else if (name === "chat") {
    $("#page-title").textContent = "共建计划";
    if (sub) {
      sub.hidden = false;
      const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
      const label =
        proj?.name ||
        (state.selectedPath
          ? String(state.selectedPath).split(/[/\\]/).filter(Boolean).pop()
          : "");
      // 后台 Mode B 态只走顶栏监控 ghost / 可关 banner，副标题不再夹「待确认/返回确认」
      sub.textContent = label ? `与 AI 写计划 · ${label}` : "与 AI 写计划文档";
    }
    try {
      if (typeof renderPlanPicker === "function") renderPlanPicker();
    } catch (_) {}
  } else if (name === "plans") {
    $("#page-title").textContent = "计划管理";
    if (sub) {
      sub.hidden = false;
      const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
      const label =
        proj?.name ||
        (state.selectedPath
          ? String(state.selectedPath).split(/[/\\]/).filter(Boolean).pop()
          : "");
      sub.textContent = label
        ? `选中 · 预览 · 编辑 · 拆成步骤 · ${label}`
        : "选中计划后预览、编辑或拆成步骤";
    }
    try {
      if (typeof renderPlanPicker === "function") renderPlanPicker();
    } catch (_) {}
  } else if (name === "doctor") {
    $("#page-title").textContent = "环境检查";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "确认本机 CLI 与依赖就绪";
    }
  } else if (name === "help") {
    $("#page-title").textContent = "帮助";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "";
    }
  } else if (name === "settings") {
    $("#page-title").textContent = "设置";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "";
    }
  }
}

function updateWorkspaceTitle() {
  // 工作区标题只展示计划，交给 updateTopPlanInfo
  updateTopPlanInfo();
}

function goHome() {
  // 多项目可并行：离开工作区不停止运行；规划/确认先缓存，回项目可接上
  if (state.selectedPath) {
    state.lastWorkspacePath = state.selectedPath;
    if (isPlanSessionActive()) {
      stashPlanSession(state.selectedPath);
    }
  }
  state.selectedPath = null;
  state.live = null;
  state.selectedTaskId = null;
  // 不清 planJobId/phase：全局 poll 继续；悬浮条可点回
  renderProjectList();
  try { updateBgPlanBanner(); } catch (_) {}
  if (state.projects.length === 0) {
    showPage("welcome");
  } else {
    // 有项目但未选中：欢迎页提示选项目
    showPage("welcome");
    $("#page-title").textContent = "我的项目";
    $("#page-sub").textContent = "从左侧选择一个项目（可多项目同时运行）";
    const we = $("#welcome-empty");
    const tplHtml =
      typeof planTemplateWelcomeHtml === "function"
        ? planTemplateWelcomeHtml()
        : "";
    we.innerHTML = `
      <p class="welcome-kicker muted">本机任务控制台</p>
      <h2>选择左侧项目，或添加新项目</h2>
      <p class="muted">进入项目后写计划、从模板开始或选已有 →「拆成步骤」→ 拆分台确认后开跑。多项目可并行。</p>
      <div class="welcome-actions">
        <button class="btn primary" id="btn-welcome-add2" type="button">添加项目文件夹</button>
      </div>
      ${tplHtml}`;
    // 点击由全局委托处理 btn-welcome-add2 / data-plan-template
  }
}

/* ── Modal（仅添加项目） ── */
function openModal() {
  const m = $("#modal");
  if (m) m.hidden = false;
  const p = $("#m-project-path");
  const n = $("#m-project-name");
  if (p) p.value = "";
  if (n) n.value = "";
}

function closeModal() {
  const m = $("#modal");
  if (m) m.hidden = true;
}

/* ── Projects ── */
async function loadProjects() {
  // A5-2e: named gateway when main mounted; invoke bridge only pre-main
  const g = typeof window !== "undefined" ? window.ccoGateway : null;
  state.projects = g?.getProjects
    ? (await g.getProjects()) || []
    : (await invoke("get_projects")) || [];
  renderProjectList();
  if (state.selectedPath && !state.projects.some((p) => p.path === state.selectedPath)) {
    state.selectedPath = null;
    state.live = null;
  }
}

function renderProjectList() {
  const el = $("#project-list");
  if (!state.projects.length) {
    el.innerHTML = `<p class="muted empty-hint">尚未添加项目<br/>点 ＋ 添加</p>`;
    return;
  }
  // 各项目状态独立展示；允许多项目并行运行，不因当前项目在跑而锁其它项
  el.innerHTML = state.projects
    .map((p) => {
      const st = p.active_status || p.last_status || "";
      const live = p.running_tasks > 0 || isLiveStatus(p.active_status);
      const isCurrent = p.path === state.selectedPath;
      let meta;
      if (live) {
        meta = `${p.running_tasks || 0}/${p.total_tasks || "?"} 任务 · 运行中`;
      } else if (p.last_status) {
        meta = `最近: ${statusLabel(p.last_status)}`;
      } else if (p.exists) {
        meta = "无活动运行";
      } else {
        meta = "路径不存在";
      }
      return `<button type="button" class="project-item ${
        isCurrent ? "active" : ""
      }" data-path="${esc(p.path)}">
        <div class="name"><span class="dot ${statusDot(st) || (live ? "live" : "")}"></span>${esc(
          p.name
        )}</div>
        <div class="path" title="${esc(p.path)}">${esc(shortPath(p.path))}</div>
        <div class="meta">${esc(meta)}</div>
      </button>`;
    })
    .join("");
  $$(".project-item", el).forEach((b) => {
    b.onclick = () => selectProject(b.dataset.path);
  });
}

// A5-2d: expose helpers for features/settings ESM (classic const/function not on window)
if (typeof window !== "undefined") {
  window.toast = toast;
  window.esc = esc;
  window.badge = badge;
  window.chatAssignDirectEnabled = chatAssignDirectEnabled;
  window.setChatAssignDirectEnabled = setChatAssignDirectEnabled;
  window.applyLogFontSize = applyLogFontSize;
  window.isLiveStatus = isLiveStatus;
  if (typeof PAUSE_CONFIRM_KEY !== "undefined") {
    window.PAUSE_CONFIRM_KEY = PAUSE_CONFIRM_KEY;
  }
}
