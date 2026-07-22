/**
 * [INPUT]: #monitor · run 上下文
 * [OUTPUT]: 运行端（CLI 看板）可见性；卡内详细日志按需展开
 * [POS]: A4-2 features/run；虚拟列表见 logVirtual.js（A5-2c）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function $(id) {
  return document.getElementById(id);
}

/**
 * Legacy no-op: 整板不再用 details 折叠；运行端始终在 #monitor 内可见。
 * 保留导出名，避免 classic/facade 调用炸掉。
 */
export function syncMonitorLogsFold(_bridge = {}) {
  // intentionally empty — CLI board is always shown while #monitor is visible
}

/**
 * Show/hide monitor board (CLI 运行端) under the progress card.
 * @param {{ planning?: boolean, hasTasks?: boolean, hasRun?: boolean }} ctx
 */
export function paintLogSecondaryVisibility(ctx) {
  const monitor = $("monitor");
  const cliEmpty = $("cli-empty");
  if (ctx.planning) {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = true;
    return;
  }
  if (!ctx.hasTasks) {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = !!ctx.hasRun;
    return;
  }
  if (monitor) monitor.hidden = false;
  if (cliEmpty) cliEmpty.hidden = true;
}
