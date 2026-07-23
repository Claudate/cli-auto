/**
 * [INPUT]: status / path / task DTO 字段（展示用）
 * [OUTPUT]: 人话标签 · badge · elapsed · path 短名（纯函数）
 * [POS]: D9 自 state.js 抽出；features 可 import，classic 经 installStatusUi → window
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * note: 不写 Mode B / confirm / soft-fill；不读调度内部策略
 */

/* ── Status labels (人话 · R2 五态优先；stop ≠ fail) ── */
export const STATUS_LABEL = {
  completed: "已完成",
  done: "已完成",
  ok: "已完成",
  running: "进行中",
  starting: "进行中",
  queued: "排队中",
  validated: "进行中",
  init: "进行中",
  paused: "已暂停",
  resuming: "进行中",
  failed: "失败",
  aborted: "已中止",
  timeout: "失败",
  stopped: "已停止",
  cancelled: "已停止",
  canceled: "已停止",
  pending: "排队中",
  waiting: "排队中",
  ready: "排队中",
  skipped: "已完成",
  idle: "排队中",
  err: "失败",
  stall: "已卡住",
  stalled: "已卡住",
};

/** R2 product states (+ stop: 用户中止，不是业务失败) */
export const FIVE_STATE_LABEL = {
  wait: "排队中",
  run: "进行中",
  done: "已完成",
  stall: "已卡住",
  fail: "失败",
  stop: "已停止",
};

export function statusLabel(status) {
  const s = String(status || "").toLowerCase();
  return STATUS_LABEL[s] || status || "—";
}

export function fiveStateLabel(bucket) {
  return FIVE_STATE_LABEL[bucket] || "排队中";
}

export function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function shortPath(p) {
  if (!p) return "—";
  const parts = String(p).split("/").filter(Boolean);
  return parts.length > 3 ? "…/" + parts.slice(-3).join("/") : p;
}

/**
 * 绝对路径 → 项目相对路径。
 * @param {string|null|undefined} planPath
 * @param {string|null|undefined} [projectRoot] 缺省时读 window.state.selectedPath（strangler）
 */
export function normalizePlanPath(planPath, projectRoot) {
  if (!planPath) return null;
  let p = String(planPath).trim();
  if (!p) return null;
  let root = projectRoot;
  if (root === undefined && typeof window !== "undefined") {
    root = window.state?.selectedPath;
  }
  if (root) {
    const r = String(root).replace(/\/+$/, "");
    if (p === r) return null;
    if (p.startsWith(r + "/")) p = p.slice(r.length + 1);
  }
  p = p.replace(/^file:\/\//, "");
  return p;
}

export function planDisplayName(path) {
  if (!path) return "—";
  const parts = String(path).split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

export function isLiveStatus(s) {
  return ["running", "starting", "queued", "validated", "init", "resuming"].includes(
    String(s || "").toLowerCase()
  );
}

export function isPausedStatus(s) {
  return String(s || "").toLowerCase() === "paused";
}

/** True business failure only — user stop / run abort are NOT failures. */
export function isFailedStatus(s) {
  return ["failed", "timeout", "err", "error"].includes(
    String(s || "").toLowerCase()
  );
}

/** User stop (task) or whole-run abort — distinct from fail. */
export function isStoppedStatus(s) {
  return ["stopped", "aborted", "cancelled", "canceled"].includes(
    String(s || "").toLowerCase()
  );
}

export function isDoneStatus(s) {
  return ["completed", "done", "ok", "skipped"].includes(
    String(s || "").toLowerCase()
  );
}

/** Task may be edited only when not yet executed (pending/queued). */
export function isTaskPendingStatus(s) {
  const v = String(s || "").toLowerCase();
  return !v || v === "pending" || v === "queued" || v === "waiting" || v === "ready";
}

/**
 * Display-only stall flag from DTO fields (stall_* / last_retry_reason).
 * Does not invent thresholds — only reads task fields.
 * @param {object|null|undefined} t
 */
export function isStalledTask(t) {
  if (!t) return false;
  const st = String(t.status || "").toLowerCase();
  if (!isLiveStatus(st)) return false;
  const thr =
    t.stall_threshold_secs != null ? Number(t.stall_threshold_secs) : null;
  const idle = t.stall_idle_secs != null ? Number(t.stall_idle_secs) : null;
  if (idle != null && thr != null && thr > 0 && idle >= Math.max(15, thr * 0.5)) {
    return true;
  }
  return String(t.last_retry_reason || "").toLowerCase() === "stall";
}

export function badge(status) {
  const s = String(status || "").toLowerCase();
  let cls = "muted";
  if (["completed", "done", "ok", "skipped"].includes(s)) cls = "ok";
  else if (
    ["running", "starting", "queued", "validated", "init", "paused", "resuming", "pending"].includes(
      s
    )
  )
    cls = "warn";
  else if (["failed", "timeout", "err", "error"].includes(s)) cls = "err";
  else if (["stopped", "aborted", "cancelled", "canceled"].includes(s)) cls = "muted";
  return `<span class="badge ${cls}">${esc(statusLabel(status))}</span>`;
}

/** @param {string} status @param {object} [task] for stall tint */
export function statusDot(status, task) {
  if (task && isStalledTask(task)) return "warn";
  const s = String(status || "").toLowerCase();
  if (["running", "starting", "queued", "validated", "init"].includes(s)) return "live";
  if (["paused", "resuming", "pending"].includes(s)) return "warn";
  if (["failed", "timeout", "err", "error"].includes(s)) return "err";
  if (["stopped", "aborted", "cancelled", "canceled"].includes(s)) return "muted";
  if (["completed", "done", "ok", "skipped"].includes(s)) return "ok";
  return "";
}

/**
 * @param {string|null|undefined} startedAt
 * @param {string|null|undefined} finishedAt
 * @param {number} [nowMs] 缺省 Date.now()；轮询可用 state.now
 */
export function formatElapsed(startedAt, finishedAt, nowMs) {
  if (!startedAt) return "—";
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return "—";
  const end = finishedAt
    ? Date.parse(finishedAt)
    : nowMs != null
      ? nowMs
      : typeof window !== "undefined" && window.state?.now != null
        ? window.state.now
        : Date.now();
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

export function taskErrorSummary(t) {
  if (!t) return "";
  if (t.error) return String(t.error).split("\n")[0].slice(0, 160);
  if (isFailedStatus(t.status) && t.log_tail) {
    const lines = String(t.log_tail).trim().split("\n").filter(Boolean);
    return (lines[lines.length - 1] || "").slice(0, 160);
  }
  return "";
}

/**
 * Bridge pure helpers onto globalThis/window for classic scripts + g() hosts.
 * @param {typeof globalThis} [g]
 */
export function installStatusUi(g = typeof window !== "undefined" ? window : globalThis) {
  if (!g) return;
  Object.assign(g, {
    STATUS_LABEL,
    FIVE_STATE_LABEL,
    statusLabel,
    fiveStateLabel,
    esc,
    shortPath,
    normalizePlanPath,
    planDisplayName,
    isLiveStatus,
    isPausedStatus,
    isFailedStatus,
    isStoppedStatus,
    isDoneStatus,
    isTaskPendingStatus,
    isStalledTask,
    badge,
    statusDot,
    formatElapsed,
    taskErrorSummary,
  });
}

const statusUi = {
  STATUS_LABEL,
  FIVE_STATE_LABEL,
  statusLabel,
  fiveStateLabel,
  esc,
  shortPath,
  normalizePlanPath,
  planDisplayName,
  isLiveStatus,
  isPausedStatus,
  isFailedStatus,
  isStoppedStatus,
  isDoneStatus,
  isTaskPendingStatus,
  isStalledTask,
  badge,
  statusDot,
  formatElapsed,
  taskErrorSummary,
  installStatusUi,
};

export default statusUi;
