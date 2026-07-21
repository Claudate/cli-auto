/**
 * [INPUT]: module registrations
 * [OUTPUT]: host bag — runtime cross-calls without circular import deadlocks
 * [POS]: A5-2a features/chat/host.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/** @type {Record<string, any>} */
export const host = {};

export function register(partial) {
  Object.assign(host, partial);
}
