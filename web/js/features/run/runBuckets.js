/**
 * [INPUT]: live task DTO 字段（status · stall_* · last_retry_reason）
 * [OUTPUT]: 五态桶标签（展示用）；不发明重试/failover 策略
 * [POS]: A4-1 features/run 纯函数；策略真源在 domain/run + app
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

const LIVE = new Set([
  "running",
  "starting",
  "queued",
  "validated",
  "init",
  "resuming",
]);
/** Business failure only — stop/abort are a separate bucket. */
const FAIL = new Set(["failed", "error", "err", "timeout"]);
const STOP = new Set(["stopped", "aborted", "cancelled", "canceled"]);
const DONE = new Set(["completed", "done", "success", "passed", "ok"]);

export function isLiveStatus(s) {
  const fn = g("isLiveStatus");
  if (typeof fn === "function") return fn(s);
  return LIVE.has(String(s || "").toLowerCase());
}

export function isFailedStatus(s) {
  const fn = g("isFailedStatus");
  if (typeof fn === "function") return fn(s);
  return FAIL.has(String(s || "").toLowerCase());
}

export function isStoppedStatus(s) {
  const fn = g("isStoppedStatus");
  if (typeof fn === "function") return fn(s);
  return STOP.has(String(s || "").toLowerCase());
}

export function isDoneStatus(s) {
  const fn = g("isDoneStatus");
  if (typeof fn === "function") return fn(s);
  return DONE.has(String(s || "").toLowerCase());
}

/**
 * Display-only stall flag from DTO fields already computed by app.
 * Does not invent thresholds or failover — only reads stall_* on the task.
 * @param {object|null} t
 */
export function isStalledTask(t) {
  const fn = g("isStalledTask");
  if (typeof fn === "function") return fn(t);
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

/**
 * R2 buckets for tiles / KPIs (+ stop: user abort ≠ fail).
 * @param {string|object} st
 * @param {object} [task]
 * @returns {"fail"|"stop"|"done"|"stall"|"run"|"wait"}
 */
export function taskBucket(st, task) {
  let t = task;
  let s = st;
  if (st && typeof st === "object") {
    t = st;
    s = st.status;
  }
  s = String(s || "").toLowerCase();
  if (isFailedStatus(s)) return "fail";
  if (isStoppedStatus(s)) return "stop";
  if (isDoneStatus(s)) return "done";
  if (t && isStalledTask(t)) return "stall";
  if (isLiveStatus(s) || ["starting", "queued", "running"].includes(s)) {
    return "run";
  }
  return "wait";
}

const FIVE = {
  fail: "失败",
  stop: "已停止",
  done: "已完成",
  stall: "已卡住",
  run: "进行中",
  wait: "排队中",
};

export function fiveStateLabel(bucket) {
  const fn = g("fiveStateLabel");
  if (typeof fn === "function") return fn(bucket);
  return FIVE[bucket] || "排队中";
}

/** CLI / 看板排序：卡住 → 进行中 → 排队 → 已完成 → 已停止 → 失败 */
export function cliStatusRank(st, task) {
  const b = taskBucket(st, task);
  if (b === "stall") return 0;
  if (b === "run") return 1;
  if (b === "wait") return 2;
  if (b === "done") return 3;
  if (b === "stop") return 4;
  return 5;
}

export function sortTasksByStatus(tasks) {
  return (tasks || [])
    .map((t, i) => ({ t, i }))
    .sort(
      (a, b) =>
        cliStatusRank(a.t.status, a.t) - cliStatusRank(b.t.status, b.t) ||
        a.i - b.i
    )
    .map((x) => x.t);
}

/**
 * Aggregate KPI counts from task list.
 * @param {object[]} tasks
 */
export function countBuckets(tasks) {
  let done = 0;
  let run = 0;
  let wait = 0;
  let fail = 0;
  let stall = 0;
  let stop = 0;
  (tasks || []).forEach((t) => {
    const b = taskBucket(t);
    if (b === "done") done++;
    else if (b === "stall") stall++;
    else if (b === "run") run++;
    else if (b === "fail") fail++;
    else if (b === "stop") stop++;
    else wait++;
  });
  return { done, run, wait, fail, stall, stop };
}

/**
 * Derive run context from live DTO + optional legacy phase.
 * @param {object|null} live
 * @param {{ phase?: string }} [legacy]
 */
export function runContext(live, legacy = {}) {
  const phase = legacy.phase || "";
  // plan_failed: still on split path — must hide historical run/result desk
  const planning =
    phase === "planning" ||
    phase === "confirm" ||
    phase === "plan_failed";
  // 打开拆分会话时，项目「最近一次」历史 run 不算本轮
  let belongs = true;
  try {
    const w = typeof window !== "undefined" ? window : globalThis;
    if (typeof w.liveBelongsToOpenPlan === "function") {
      belongs = !!w.liveBelongsToOpenPlan();
    } else if (planning) {
      belongs = false;
    }
  } catch (_) {
    if (planning) belongs = false;
  }
  // 双保险：live 仍在跑/暂停时绝不因 planJob 门禁把执行台刷空
  if (live?.run_id && isLiveStatus(live.run_status)) {
    belongs = true;
  } else if (
    live?.run_id &&
    String(live.run_status || "").toLowerCase() === "paused"
  ) {
    belongs = true;
  }
  const runStatus = belongs ? live?.run_status : null;
  const hasRun = belongs && !!live?.run_id;
  const active = hasRun && isLiveStatus(runStatus);
  const finished =
    hasRun &&
    !active &&
    ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
      String(runStatus || "").toLowerCase()
    );
  return { hasRun, active, finished, runStatus, planning, belongs };
}
