/**
 * [INPUT]: window.ccoLog（main.js ESM 安装 · features/run/log*）
 * [OUTPUT]: 经典全局函数名兼容（doctor/plan/monitor 调用）
 * [POS]: A5-2c log.js ≤200 facade — 虚拟列表/看板真源 features/run/log*
 * note: 禁止堆新功能；禁止 invoke/start_run；stop/resume 只走 ccoRun
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — log classic facade (A5-2c strangler) */

function _ccoLog() {
  return typeof window !== "undefined" ? window.ccoLog : null;
}

function _logCall(name, ...args) {
  const d = _ccoLog();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[log facade] ccoLog." + name + " not ready");
  return undefined;
}

function renderCliBoard(...a) {
  return _logCall("renderCliBoard", ...a);
}
function renderTaskList(...a) {
  return _logCall("renderTaskList", ...a);
}
function renderDetailLog(...a) {
  return _logCall("renderDetailLog", ...a);
}
function fillPlannerLog(...a) {
  return _logCall("fillPlannerLog", ...a);
}
function fillPanelLogBody(...a) {
  return _logCall("fillPanelLogBody", ...a);
}
function panelLogContent(...a) {
  return _logCall("panelLogContent", ...a);
}
function panelLogHtml(...a) {
  return _logCall("panelLogHtml", ...a);
}
function aiLogPlainText(...a) {
  return _logCall("aiLogPlainText", ...a);
}
function renderLogConsoleHtml(...a) {
  return _logCall("renderLogConsoleHtml", ...a);
}
function logPanelSignature(...a) {
  return _logCall("logPanelSignature", ...a);
}
function mountVirtualLog(...a) {
  return _logCall("mountVirtualLog", ...a);
}
function paintVirtualLogWindow(...a) {
  return _logCall("paintVirtualLogWindow", ...a);
}
function isNearBottom(...a) {
  return _logCall("isNearBottom", ...a);
}
function isAiInteractionEvent(...a) {
  return _logCall("isAiInteractionEvent", ...a);
}
function eventPassesFilter(...a) {
  return _logCall("eventPassesFilter", ...a);
}
function ansiToHtml(...a) {
  return _logCall("ansiToHtml", ...a);
}
function isNoiseText(...a) {
  return _logCall("isNoiseText", ...a);
}
function renderLogEvent(...a) {
  return _logCall("renderLogEvent", ...a);
}
function renderTranscriptLine(...a) {
  return _logCall("renderTranscriptLine", ...a);
}
function cancelTask(...a) {
  return _logCall("cancelTask", ...a);
}
function stopAll(...a) {
  return _logCall("stopAll", ...a);
}
function resumeRun(...a) {
  return _logCall("resumeRun", ...a);
}
function startReworkWave(...a) {
  return _logCall("startReworkWave", ...a);
}
function acceptRunResidual(...a) {
  return _logCall("acceptRunResidual", ...a);
}
function openExternalTerminal(...a) {
  return _logCall("openExternalTerminal", ...a);
}
function exportBoardLogsMd(...a) {
  return _logCall("exportBoardLogsMd", ...a);
}
function openHandoffLedger(...a) {
  return _logCall("openHandoffLedger", ...a);
}
function renderHandoffBoardStrip(...a) {
  return _logCall("renderHandoffBoardStrip", ...a);
}
function loadDoctor(...a) {
  return _logCall("loadDoctor", ...a);
}
function stallStripText(...a) {
  return _logCall("stallStripText", ...a);
}
