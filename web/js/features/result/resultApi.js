/**
 * [INPUT]: gateway only（禁止 __TAURI__/invoke）
 * [OUTPUT]: Result / Inspect 用例薄封装（rework · accept residual）
 * [POS]: A4-3 features/result；回补走 app::run start_rework，非 confirm 旁路
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as gateway from "../../shared/gateway.js";

/**
 * Start rework wave from inspect ISSUES (existing app surface).
 * @param {string} runId
 */
export function startRework(runId) {
  return gateway.startRework(runId);
}

/**
 * Explicit accept residual → handoff open_risks.
 * @param {string} runId
 * @param {string|null} [note]
 */
export function acceptResidual(runId, note) {
  return gateway.acceptResidual(runId, note || null);
}

/** Stop a still-live run before soft-ending the round (finishRound). */
export function stopRun(runId) {
  return gateway.stopRun(runId);
}

/** P2-2: write project last_summary from run (best-effort). */
export function writebackMemory(runId) {
  return gateway.writebackMemory(runId);
}

/** Live snapshot (read) when result needs refresh. */
export function getProjectLive(project) {
  return gateway.getProjectLive(project);
}
