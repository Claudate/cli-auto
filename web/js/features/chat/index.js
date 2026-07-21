/**
 * [INPUT]: ChatViewModel / chatApi / installChat
 * [OUTPUT]: author feature 公共出口
 * [POS]: A5-2a features/chat/index.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export { createChatViewModel } from "./ChatViewModel.js";
export * as chatApi from "./chatApi.js";
export { createChatDesk, installChatHost } from "./installChat.js";
export { host } from "./host.js";
