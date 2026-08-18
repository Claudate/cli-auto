/**
 * [INPUT]: gateway only（禁止 __TAURI__/invoke）
 * [OUTPUT]: Run 控制薄封装（stop / resume / stopTask / retryTask / openTerminal）
 * [POS]: A4-1 features/run；策略在 Rust app/run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：start_run 旁路、复制 stall-failover / soft-fill。
 * 回补走 features/result → startRework（非第二开跑入口）。
 * 卡片「再跑一次」→ retryTask（单任务，非 re-split）。
 */

import * as gateway from "../../shared/gateway.js";

/** @param {string} runId */
export function stopRun(runId) {
  return gateway.stopRun(runId);
}

/** @param {string} runId @param {string} taskId */
export function stopTask(runId, taskId) {
  return gateway.stopTask(runId, taskId);
}

/** @param {string} runId */
export function resumeRun(runId) {
  return gateway.resumeRun(runId);
}

/**
 * Re-run one failed/stopped/timeout task in the same run (not re-split).
 * @param {string} runId
 * @param {string} taskId
 * @param {{ provider?: string }} [opts] — optional channel override.
 */
export function retryTask(runId, taskId, opts) {
  return gateway.retryTask(runId, taskId, opts);
}

/** @param {Record<string, unknown>} args open_task_terminal_cmd payload */
export function openTaskTerminal(args) {
  return gateway.openTaskTerminal(args);
}

/**
 * Live snapshot for a project (read-only query).
 * @param {string} project
 * @param {{ logMaxBytes?: number }} [opts]
 */
export function getProjectLive(project, opts = {}) {
  return gateway.getProjectLive(project, opts);
}

/** Optional: system-level monitor window (P2-4). */
export function openMonitorWindow(args) {
  return gateway.openMonitorWindow(args || {});
}
