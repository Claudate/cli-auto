/**
 * [INPUT]: module registrations
 * [OUTPUT]: host bag for cross-calls
 * [POS]: A5-2b-fin features/project/host.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/** @type {Record<string, any>} */
export const host = {};

export function register(partial) {
  Object.assign(host, partial);
}
