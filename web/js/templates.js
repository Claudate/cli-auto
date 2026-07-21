/**
 * [INPUT]: window.ccoTemplates（main.js ESM 安装 · features/templates）
 * [OUTPUT]: 经典全局 planTemplate* / applyPlanTemplate / writeSplit* / refresh*
 * [POS]: P-ship-D D7 — templates.js ≤80 facade；逻辑在 features/templates/*
 * note: 禁止堆新功能；禁止 invoke/confirm_start/start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — templates classic facade (P-ship-D D7 strangler) */

function _ccoTemplates() {
  return typeof window !== "undefined" ? window.ccoTemplates : null;
}

function _tplCall(name, ...args) {
  const d = _ccoTemplates();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[templates facade] ccoTemplates." + name + " not ready");
  return undefined;
}

function planTemplateById(...a) {
  return _tplCall("planTemplateById", ...a);
}
function planTemplateChatEmptyHtml(...a) {
  return _tplCall("planTemplateChatEmptyHtml", ...a);
}
function planTemplateWelcomeHtml(...a) {
  return _tplCall("planTemplateWelcomeHtml", ...a);
}
function applyPlanTemplate(...a) {
  return _tplCall("applyPlanTemplate", ...a);
}
function writeSplitSummaryToPlan(...a) {
  return _tplCall("writeSplitSummaryToPlan", ...a);
}
function refreshSplitWritebackBtn(...a) {
  return _tplCall("refreshSplitWritebackBtn", ...a);
}
function buildSplitSummaryBlock(...a) {
  return _tplCall("buildSplitSummaryBlock", ...a);
}
function mergeSplitSummaryIntoMarkdown(...a) {
  return _tplCall("mergeSplitSummaryIntoMarkdown", ...a);
}
