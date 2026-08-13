/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke 桥
 * [OUTPUT]: state · $ · toast · invoke/requireGateway 兜底 · prefs
 * [POS]: web/js D9 桥/瘦身；壳导航 → shared/shellUi；展示 → shared/statusUi+markdown
 * note: invoke/getInvoke = 迁移期 pre-main 兜底；业务 classic 用 requireGateway()→ccoGateway；
 *       feature 内禁止直接 invoke/__TAURI__（只经 shared/gateway.js）
 *       pages/projects/run-lock 由 main installShellUi 挂 window（pre-main 无 UI 调用）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — state (D9: bridge + session prefs, not shell chrome) */


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
/**
 * A2：聊天/计划卡「拆成步骤」是否跳过选项层直开 analyze。
 * 默认开（无键或 ≠"0"）；仅显式 "0" = 先确认选项。
 * 与 autoStartAfterPlan 无关：跳过选项层 ≠ 自动 confirm_start。
 */
const CHAT_ASSIGN_DIRECT_KEY = "cco.chatAssignDirect";

function chatAssignDirectEnabled() {
  return localStorage.getItem(CHAT_ASSIGN_DIRECT_KEY) !== "0";
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
  /**
   * Plan-list reopen index: plan_path → { job_id, status, task_count, plan_name, updated_at }
   * Source: SQLite plan_jobs dual-write via list_plan_split_index_cmd.
   */
  planSplitByPath: {},
  /** H2: 默认隐藏已成功执行；开关「显示已执行」可展开（chooser/右轨共用） */
  showExecutedPlans: localStorage.getItem("cco.showExecutedPlans") === "1",
  planPollFails: 0,
  planPollGen: 0, // 轮询代际：stop 时 +1，让在飞 tick 不再自我续期
  planPollInFlight: false, // refreshPlanJob 在飞守卫（多入口合并为一次）
  planAdvancedJobId: null, // advancePlannedJob 幂等：同一 job 只推进一次
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
  // lastRunId: only for dismiss when live already null (not SoT)
  if (!state.lastRunIdByProject) state.lastRunIdByProject = {};
  // Purge legacy dual-write key (SoT = SQLite project_ui_prefs)
  try {
    localStorage.removeItem("cco.dismissedRuns.v1");
  } catch (_) {}
  window.state = state;
  window.PAUSE_CONFIRM_KEY = PAUSE_CONFIRM_KEY;
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
  // Longer copy (run-locked / replan hints) needs more than ~3s to read.
  const ms = String(msg || "").length > 28 ? 5200 : 3200;
  toast._t = setTimeout(() => {
    t.hidden = true;
  }, ms);
}

/**
 * pre-main 兜底：main 挂上 ccoGateway 后业务应走 requireGateway()。
 * 与 shared/gateway.js getInvoke 同形，仅 classic 冷启动 / 桥接用。
 */
function getInvoke() {
  const w = window;
  const candidates = [
    w.__TAURI__?.core?.invoke && w.__TAURI__.core.invoke.bind(w.__TAURI__.core),
    w.__TAURI__?.tauri?.invoke && w.__TAURI__.tauri.invoke.bind(w.__TAURI__.tauri),
    w.__TAURI_INTERNALS__?.invoke &&
      w.__TAURI_INTERNALS__.invoke.bind(w.__TAURI_INTERNALS__),
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
    window.__TAURI__?.dialog || window.__TAURI__?.plugins?.dialog || null;
  if (d?.open) return d.open(opts);
  try {
    if (getInvoke()) {
      return await invoke("plugin:dialog|open", { options: opts });
    }
  } catch (_) {}
  throw new Error("对话框不可用");
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

// Expose bridge + session prefs for ESM hosts.
// pages / projects / run-lock → shared/shellUi (main installShellUi).
// status labels / badge / elapsed → shared/statusUi (main installStatusUi).
if (typeof window !== "undefined") {
  window.toast = toast;
  window.chatAssignDirectEnabled = chatAssignDirectEnabled;
  window.setChatAssignDirectEnabled = setChatAssignDirectEnabled;
  window.applyLogFontSize = applyLogFontSize;
  window.requireGateway = requireGateway;
  window.getInvoke = getInvoke;
  window.invoke = invoke;
  window.isTauriReady = isTauriReady;
  window.openNativeDialog = openNativeDialog;
  window.PAUSE_CONFIRM_KEY = PAUSE_CONFIRM_KEY;
}
