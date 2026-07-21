/**
 * [INPUT]: AppViewModel phase · legacy state.page / state.phase
 * [OUTPUT]: page 名与 body dataset 映射
 * [POS]: A2-2 五步 ↔ 页面；不写业务策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * Target phase (PRODUCT / arch §5):
 *   author | split | run | result
 *
 * Legacy DOM pages (index.html):
 *   welcome | workspace | chat | plans | doctor | help | settings
 *
 * Mapping (wire only — no UI redraw):
 *   author  → page-chat（有项目）/ welcome（冷启动无项目）
 *   split   → workspace + Mode B confirm/planning
 *   run     → workspace + running
 *   result  → workspace + done（result.js 结果台）
 */

/** @typedef {"author"|"split"|"run"|"result"} AppPhase */

/**
 * Map app phase → legacy showPage name when a project is selected.
 * @param {AppPhase} phase
 * @param {{ hasProject?: boolean }} [ctx]
 * @returns {string}
 */
export function phaseToPage(phase, ctx = {}) {
  const hasProject = !!ctx.hasProject;
  switch (phase) {
    case "author":
      return hasProject ? "chat" : "welcome";
    case "split":
    case "run":
    case "result":
      return hasProject ? "workspace" : "welcome";
    default:
      return hasProject ? "chat" : "welcome";
  }
}

/**
 * Infer app phase from legacy state (for bridging old UI → VM).
 * @param {{ page?: string, phase?: string, live?: { run_id?: string, status?: string }|null, planJobId?: string|null }} legacy
 * @returns {AppPhase}
 */
export function legacyToPhase(legacy) {
  const page = legacy?.page || "welcome";
  const modeB = legacy?.phase || "pick";
  const live = legacy?.live;
  const hasRun = !!(live?.run_id);
  const st = String(live?.status || "").toLowerCase();

  if (page === "chat" || page === "plans" || page === "welcome") {
    // author surface; if confirm is active keep split when on workspace only
    if (page === "workspace") {
      /* fall through */
    } else {
      return "author";
    }
  }
  if (page === "doctor" || page === "help" || page === "settings") {
    // advanced overlays — keep last business phase if known
    if (modeB === "confirm" || modeB === "planning") return "split";
    if (hasRun && (st === "completed" || st === "failed" || modeB === "done"))
      return "result";
    if (hasRun || modeB === "running") return "run";
    return "author";
  }
  // workspace
  if (modeB === "planning" || modeB === "confirm") return "split";
  if (modeB === "done" || st === "completed" || st === "failed") return "result";
  if (modeB === "running" || hasRun) return "run";
  if (legacy?.planJobId) return "split";
  return "author";
}

/**
 * Map app phase → legacy Mode B phase hint (does not own confirm rules).
 * @param {AppPhase} phase
 * @returns {string}
 */
export function phaseToLegacyModeB(phase) {
  switch (phase) {
    case "split":
      return "confirm";
    case "run":
      return "running";
    case "result":
      return "done";
    case "author":
    default:
      return "pick";
  }
}

/** Human titles for shell (PM 文案；无引擎名). */
export const PHASE_TITLE = Object.freeze({
  author: "写计划",
  split: "拆成步骤",
  run: "执行中",
  result: "结果",
});
