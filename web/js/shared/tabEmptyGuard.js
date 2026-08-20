/**
 * [INPUT]: legacy state 快照 · hasActiveRun/isRunPaused · confirmDialog · openModal · appVm go*
 * [OUTPUT]: guardViewRingClick(target) → Promise<boolean>（true=已导航/放行）
 * [POS]: F3 tab 空态守卫纯逻辑；**仅**由 main wireShellNav 的 #view-ring 用户点击调用
 * note: 程序化 appVm.go* / jobPoll / confirmActions **不得**调用本模块
 * note: 空态弹窗只用 confirmDialog；openModal 仅「去选项目」CTA 之后
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md · docs/chat-dual-mode-empty-guard-2026-08-20.md §5
 */

const SS_PREFIX = "cco-tab-empty:";

function storageKey(reason, selectedPath) {
  return `${SS_PREFIX}${reason}|${String(selectedPath || "")}`;
}

function alreadyShown(reason, selectedPath) {
  try {
    return sessionStorage.getItem(storageKey(reason, selectedPath)) === "1";
  } catch (_) {
    return false;
  }
}

function markShown(reason, selectedPath) {
  try {
    sessionStorage.setItem(storageKey(reason, selectedPath), "1");
  } catch (_) {}
}

function projectLastRunId(s) {
  if (!s?.selectedPath) return null;
  const proj = (s.projects || []).find((p) => p.path === s.selectedPath);
  if (!proj) return null;
  const last = proj.last_run_id ?? proj.lastRunId ?? null;
  return last != null && String(last).trim() ? String(last) : null;
}

function planJobRunId(s) {
  const job = s?.planJob;
  const rid = job?.run_id ?? job?.runId ?? null;
  return rid != null && String(rid).trim() ? String(rid) : null;
}

/** Non-empty draft only — empty object / blank markdown does not count as「有计划」. */
function hasUsableDraftPlan(s) {
  const d = s?.chatSession?.draft_plan;
  if (!d || typeof d !== "object") return false;
  const md = d.markdown ?? d.md ?? d.body ?? "";
  if (String(md).trim()) return true;
  // Saved path with no in-memory markdown still counts (rehydrate may fill later)
  const p = d.path ?? d.plan_path ?? "";
  return !!String(p).trim();
}

/** @returns {{ ok: true } | { ok: false, reason: string }} */
export function checkSplitContent(s) {
  if (!s?.selectedPath) return { ok: false, reason: "split:no-project" };
  if (hasUsableDraftPlan(s)) return { ok: true };
  if (s.planJobId) return { ok: true };
  if (Array.isArray(s.plans) && s.plans.length > 0) return { ok: true };
  return { ok: false, reason: "split:no-plan" };
}

/**
 * @param {object} s
 * @param {{ hasActiveRun: () => boolean, isRunPaused: () => boolean }} run
 */
export function checkRunContent(s, run) {
  if (run.hasActiveRun() || run.isRunPaused()) return { ok: true };
  if (projectLastRunId(s)) return { ok: true };
  if (planJobRunId(s)) return { ok: true };
  if (s?.live?.run_id) return { ok: true };
  return { ok: false, reason: "run:no-run" };
}

/**
 * @param {object} s
 * @param {{ hasActiveRun: () => boolean, isRunPaused: () => boolean }} run
 */
export function checkResultContent(s, run) {
  if (run.hasActiveRun() || run.isRunPaused()) return { ok: true };
  if (s?.phase === "done") return { ok: true };
  if (projectLastRunId(s)) return { ok: true };
  if (planJobRunId(s)) return { ok: true };
  if (s?.live?.run_id) return { ok: true };
  return { ok: false, reason: "result:no-result" };
}

/**
 * @param {string} reason
 * @param {{ goAuthor: Function, goSplit: Function, goRun: Function, openModal: Function }} nav
 */
function dialogSpec(reason, nav) {
  switch (reason) {
    case "split:no-project":
      return {
        title: "还不能打开这里",
        body: "先选一个项目文件夹，再拆计划。",
        okLabel: "去选项目",
        cancelLabel: "留在本页",
        onOk: () => nav.openModal(),
      };
    case "split:no-plan":
      return {
        title: "还不能打开这里",
        body: "这个项目还没有可拆分的计划。先和小叶聊出一份？",
        okLabel: "去聊天写计划",
        cancelLabel: "留在本页",
        onOk: () => nav.goAuthor(),
      };
    case "run:no-run":
      return {
        title: "还不能打开这里",
        body: "还没有开始执行的任务。计划要先在拆分台确认，才会开跑。",
        okLabel: "去拆分台看看",
        cancelLabel: "留在本页",
        onOk: () => nav.goSplit(),
      };
    case "result:no-result":
      return {
        title: "还不能打开这里",
        body: "还没有执行结果。先跑一轮，这里会收口。",
        okLabel: "去执行台",
        cancelLabel: "留在本页",
        onOk: () => nav.goRun(),
      };
    default:
      return null;
  }
}

/**
 * @param {"chat"|"split"|"run"|"result"} target
 * @param {{
 *   getState: () => object,
 *   hasActiveRun: () => boolean,
 *   isRunPaused: () => boolean,
 *   confirmDialog: (opts: object) => Promise<boolean|string>,
 *   openModal: () => void,
 *   goAuthor: () => void,
 *   goSplit: () => void,
 *   goRun: () => void,
 *   goResult: () => void,
 * }} deps
 * @returns {Promise<boolean>} true = 已切页或放行；false = 留在本页
 */
export async function guardViewRingClick(target, deps) {
  const nav = {
    goAuthor: deps.goAuthor,
    goSplit: deps.goSplit,
    goRun: deps.goRun,
    goResult: deps.goResult,
    openModal: deps.openModal,
  };

  const apply = (t) => {
    if (t === "chat") nav.goAuthor();
    else if (t === "split") nav.goSplit();
    else if (t === "run") nav.goRun();
    else if (t === "result") nav.goResult();
  };

  if (target === "chat") {
    apply("chat");
    return true;
  }

  const s = deps.getState() || {};
  const run = {
    hasActiveRun: deps.hasActiveRun,
    isRunPaused: deps.isRunPaused,
  };
  let check = { ok: true };
  if (target === "split") check = checkSplitContent(s);
  else if (target === "run") check = checkRunContent(s, run);
  else if (target === "result") check = checkResultContent(s, run);
  else return false;

  const path = s.selectedPath || "";
  if (check.ok || alreadyShown(check.reason, path)) {
    apply(target);
    return true;
  }

  const spec = dialogSpec(check.reason, nav);
  if (!spec) {
    apply(target);
    return true;
  }

  markShown(check.reason, path);
  const ok = await deps.confirmDialog({
    title: spec.title,
    body: spec.body,
    okLabel: spec.okLabel,
    cancelLabel: spec.cancelLabel,
  });
  if (ok) {
    try {
      spec.onOk();
    } catch (_) {}
    return true;
  }
  return false;
}

export default guardViewRingClick;
