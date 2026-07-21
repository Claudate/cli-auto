/**
 * [INPUT]: gateway / ccoChat（禁止 features 内 invoke）
 * [OUTPUT]: 模板落盘与计划读回薄封装
 * [POS]: P-ship-D features/templates/templatesApi.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as chatApi from "../chat/chatApi.js";

/**
 * Persist plan markdown. Prefers window.ccoChat.savePlan when main has wired it.
 * Does NOT call confirm_start / start_run.
 * @param {Record<string, unknown>} args chat_save_plan_cmd payload
 */
export function savePlan(args) {
  if (
    typeof window !== "undefined" &&
    window.ccoChat &&
    typeof window.ccoChat.savePlan === "function"
  ) {
    return window.ccoChat.savePlan(args);
  }
  return chatApi.savePlan(args);
}

/** Read plan markdown via gateway (chatApi → gateway.readPlanMd). */
export function readPlanMd(project, plan) {
  return chatApi.readPlanMd(project, plan);
}
