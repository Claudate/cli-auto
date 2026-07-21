/**
 * [INPUT]: gateway only（禁止 __TAURI__/invoke）
 * [OUTPUT]: settings / doctor / meta / open_monitor 薄封装
 * [POS]: A5-2d features/settings；策略在 Rust services/settings · doctor
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：业务策略、confirm_start / start_run 旁路。
 */

import * as gateway from "../../shared/gateway.js";

/** @returns {Promise<object>} AppSettings DTO */
export function getSettings() {
  return gateway.getSettings();
}

/**
 * Partial update; backend merges + returns full settings.
 * @param {Record<string, unknown>} update
 */
export function setSettings(update) {
  return gateway.setSettings(update || {});
}

/**
 * Environment doctor report.
 * @param {string|null|undefined} project
 */
export function runDoctor(project) {
  return gateway.doctor(project || null);
}

/** Desktop meta (version, …). */
export function meta() {
  return gateway.meta();
}

/**
 * System-level monitor window (P2-4).
 * @param {{ project?: string|null }} [args]
 */
export function openMonitorWindow(args) {
  return gateway.openMonitorWindow(args || {});
}

export function isTauriReady() {
  return gateway.isTauriReady();
}
