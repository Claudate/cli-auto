/**
 * [INPUT]: gateway only（禁止 __TAURI__/invoke）
 * [OUTPUT]: author/chat 用例薄封装
 * [POS]: A2-4 features/chat；业务规则在 Rust app/chat
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as gateway from "../../shared/gateway.js";

/**
 * List sessions for a project (A2-4 最小路径之一).
 * @param {string} project
 */
export function listSessions(project) {
  return gateway.chatListSessions(project);
}

/**
 * Send a chat message (A2-4 最小路径).
 * Args mirror chat_send_cmd DTO — no Mode B / confirm.
 * @param {{
 *   project: string,
 *   message: string,
 *   sessionId?: string,
 *   attachments?: unknown[]|null,
 * }} args
 */
export function sendMessage(args) {
  return gateway.chatSend({
    project: args.project,
    message: args.message,
    sessionId: args.sessionId || "default",
    attachments: args.attachments ?? null,
  });
}

/**
 * Persist draft plan markdown via app/chat (分配入口的落盘半步).
 * Does NOT call confirm_start / start_run.
 * @param {Record<string, unknown>} args chat_save_plan_cmd payload
 */
export function savePlan(args) {
  return gateway.chatSavePlan(args);
}

export function getSession(project, sessionId) {
  return gateway.chatSessionGet(project, sessionId);
}

export function newSession(project, title) {
  return gateway.chatNewSession(project, title);
}

export function deleteSession(project, sessionId) {
  return gateway.chatDeleteSession(project, sessionId);
}

export function streamPartial(args) {
  return gateway.chatStreamPartial(args);
}

export function normalizePlan(args) {
  return gateway.chatNormalizePlan(args);
}

export function saveAttachment(args) {
  return gateway.chatSaveAttachment(args);
}

export function readPlanMd(project, plan) {
  return gateway.readPlanMd(project, plan);
}
