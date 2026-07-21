/**
 * [INPUT]: #monitor-logs-fold · state.monitorLogsOpen
 * [OUTPUT]: 日志次级面板默认折叠；不挡主进度
 * [POS]: A4-2 features/run；虚拟列表见 logVirtual.js（A5-2c）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function $(id) {
  return document.getElementById(id);
}

/**
 * R1: keep logs fold closed by default; remember user open.
 * Does not reimplement virtual list — only fold chrome.
 * @param {{ getOpen?: () => boolean, setOpen?: (v: boolean) => void }} [bridge]
 */
export function syncMonitorLogsFold(bridge = {}) {
  const fold = $("monitor-logs-fold");
  if (!fold) return;

  const getOpen =
    typeof bridge.getOpen === "function"
      ? bridge.getOpen
      : () => {
          const s = g("state");
          return !!(s && s.monitorLogsOpen);
        };
  const setOpen =
    typeof bridge.setOpen === "function"
      ? bridge.setOpen
      : (v) => {
          const s = g("state");
          if (s) s.monitorLogsOpen = !!v;
          try {
            localStorage.setItem("cco.monitorLogsOpen", v ? "1" : "0");
          } catch (_) {}
        };

  if (fold.dataset.ccoA4Bound !== "1") {
    fold.dataset.ccoA4Bound = "1";
    fold.addEventListener("toggle", () => {
      setOpen(!!fold.open);
    });
  }
  fold.open = !!getOpen();
}

/**
 * Show/hide monitor board as secondary (not main focus).
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
  syncMonitorLogsFold();
}
