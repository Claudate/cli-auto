/**
 * [INPUT]: none（三栏真源 = window.ccoSplit / features/split）
 * [OUTPUT]: 空壳占位 — classic 全局名已废弃；main 装 ccoSplit 后无需本文件逻辑
 * [POS]: A5-2f-thin D3 — split.js ≤50 空壳；禁止与 ccoSplit 双轨三栏
 * note: 删除序允许整删 script；保留文件便于旧书签/缓存命中不 404
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — split classic shell (A5-2f D3 · no dual-track paint) */

// Intentionally empty: timeline / cards / quality open live in features/split.
// main.js ccoSplit.render() guards refreshSplitQualityOpen with typeof.
// splitRender oneLiner/roleBadge have built-in fallbacks (no classic globals).
