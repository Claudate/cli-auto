/**
 * [INPUT]: window.ccoResult（main.js ESM 安装 · features/result）
 * [OUTPUT]: 经典全局 renderResultDesk / finishRunRound（doctor/plan/RunView 调用）
 * [POS]: A5-2f-thin D1 — result.js ≤80 facade；逻辑在 features/result/*
 * note: 禁止堆新功能；禁止 invoke/start_run；回补只经 ccoResult
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — result classic facade (A5-2f D1 strangler) */

function _ccoResult() {
  return typeof window !== "undefined" ? window.ccoResult : null;
}

function _resultCall(name, ...args) {
  const d = _ccoResult();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[result facade] ccoResult." + name + " not ready");
  return undefined;
}

/**
 * R3: fill #result-desk when run finished; hide while running.
 * A4+: only window.ccoResult (features/result).
 */
function renderResultDesk(live, tasks, ctx) {
  return _resultCall("renderResultDesk", live, tasks, ctx);
}

/**
 * Soft end: dismiss run UI focus and go chat.
 * A4+: only ccoResult.finishRound.
 */
function finishRunRound() {
  return _resultCall("finishRound");
}
