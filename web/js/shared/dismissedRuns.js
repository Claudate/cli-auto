/**
 * [INPUT]: —
 * [OUTPUT]: no-op stubs (migration empty shell)
 * [POS]: **deprecated** — SoT is SQLite `project_ui_prefs.dismissed_run_id`
 *         via gateway.projectDismissRun / project_live_view server filter.
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * Do NOT write localStorage for dismissed runs. Call gateway only from dismissRun.
 */

/** @deprecated */
export function syncDismissedToState() {}
/** @deprecated */
export function getDismissedRunId() {
  return null;
}
/** @deprecated */
export function setDismissedRun() {}
/** @deprecated */
export function clearDismissedRun() {}
/** @deprecated — server filters live */
export function shouldHideLiveRun() {
  return false;
}
/** @deprecated */
export function noteLiveRun(_projectPath, live) {
  return live;
}
/** @deprecated — use gateway.projectDismissRun */
export function dismissProjectRun() {
  return null;
}

export default {
  getDismissedRunId,
  setDismissedRun,
  clearDismissedRun,
  shouldHideLiveRun,
  noteLiveRun,
  dismissProjectRun,
  syncDismissedToState,
};
