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
 *   effort?: string|null,
 * }} args
 */
export function sendMessage(args) {
  const payload = {
    project: args.project,
    message: args.message,
    sessionId: args.sessionId || "default",
    attachments: args.attachments ?? null,
  };
  if (args.effort) payload.effort = args.effort;
  if (args.cli) payload.cli = args.cli;
  return gateway.chatSend(payload);
}

/**
 * Persist draft plan markdown via app/chat (分配入口的落盘半步).
 * Does NOT call confirm_start / start_run.
 * @param {Record<string, unknown>} args chat_save_plan_cmd payload
 */
/** L1: chat-capable CLI list for the composer dropdown. */
export function clisList() {
  return gateway.chatClisList();
}

export function savePlan(args) {
  return gateway.chatSavePlan(args);
}

/**
 * W2: persist ```wave-index + all ```plan fences under plans/wave-…/
 * Does NOT call confirm_start / start_run.
 * @param {Record<string, unknown>} args
 */
export function saveWaveBundle(args) {
  return gateway.chatSaveWaveBundle(args);
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

export function renameSession(project, sessionId, title) {
  return gateway.chatRenameSession(project, sessionId, title);
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

/** Project-relative image path → data URL (chat inline / attachment rehydrate). */
export function readImageDataUrl(project, path) {
  return gateway.chatReadImageDataUrl(project, path);
}

export function readPlanMd(project, plan) {
  return gateway.readPlanMd(project, plan);
}

/** Detached local preview — survives chat turn; not confirm/start_run. */
export function previewStart(project) {
  return gateway.previewStart(project);
}

export function previewStop(project) {
  return gateway.previewStop(project);
}

export function previewStatus(project) {
  return gateway.previewStatus(project);
}

/**
 * Per-CLI slash-command catalog for the composer autocomplete.
 * @param {string|null} [cli] picked channel; omit for default (claude)
 */
export function slashCatalog(cli) {
  return gateway.chatSlashCatalog(cli || null);
}

/**
 * True if project-relative plan markdown exists on disk.
 * Used to drop ghost list rows (split index / selection pins after source delete).
 */
export async function planMdExists(project, plan) {
  if (!project || !plan) return false;
  try {
    await gateway.readPlanMd(project, plan);
    return true;
  } catch (_) {
    return false;
  }
}

/** P2-2: project light memory (last_summary + pins). */
export function getProjectMemory(project) {
  return gateway.projectMemoryGet(project);
}

export function getLastSummary(project) {
  return gateway.projectMemoryLastSummary(project);
}

export function cancelMessage(project) {
  return gateway.chatCancel(project);
}
