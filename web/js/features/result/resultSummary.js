/**
 * [INPUT]: live DTO 费用字段（planner_cost_usd / exec_cost_usd）
 * [OUTPUT]: 结果台计划行人话费用句；无 cost 不伪装 $0
 * [POS]: P0-1 · P0-4 features/result 纯展示 helper；无 DOM / 无 gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * P0-4 决策：费用句规则仍本地拼接（两侧有无 + 「费用未汇总」），
 * 与 report「合计: 费用未汇总」同词；不另开 live.result_cost_note。
 */

/**
 * @typedef {{
 *   planner_cost_usd?: number|null,
 *   exec_cost_usd?: number|null,
 * }} LiveCostFields
 */

/**
 * @typedef {{
 *   kind: "known"|"partial"|"unknown",
 *   total_usd?: number,
 *   planner_usd?: number,
 *   exec_usd?: number,
 *   note: string,
 * }} CostSummary
 */

/**
 * Parse a live cost field: null/undefined/NaN → absent (not $0).
 * @param {unknown} v
 * @returns {number|null}
 */
export function parseCostUsd(v) {
  if (v == null || v === "") return null;
  const n = Number(v);
  if (!Number.isFinite(n)) return null;
  return n;
}

/**
 * USD for human copy. Tiny non-zero values keep extra digits so we never
 * print `$0.00` when a real micro-cost exists.
 * @param {number} n
 * @returns {string}
 */
export function formatUsd(n) {
  const abs = Math.abs(n);
  if (abs > 0 && abs < 0.01) return `$${n.toFixed(4)}`;
  return `$${n.toFixed(2)}`;
}

/**
 * Structured cost summary from live fields.
 * @param {LiveCostFields|null|undefined} live
 * @returns {CostSummary}
 */
export function summarizeLiveCost(live) {
  const planner = parseCostUsd(live?.planner_cost_usd);
  const exec = parseCostUsd(live?.exec_cost_usd);
  const hasP = planner != null;
  const hasE = exec != null;

  if (!hasP && !hasE) {
    return { kind: "unknown", note: "费用未汇总" };
  }

  if (hasP && hasE) {
    const total = planner + exec;
    return {
      kind: "known",
      total_usd: total,
      planner_usd: planner,
      exec_usd: exec,
      note: `约 ${formatUsd(total)}（规划 ${formatUsd(planner)} · 执行 ${formatUsd(exec)}）`,
    };
  }

  if (hasP) {
    return {
      kind: "partial",
      total_usd: planner,
      planner_usd: planner,
      note: `约 ${formatUsd(planner)}（仅规划）`,
    };
  }

  return {
    kind: "partial",
    total_usd: exec,
    exec_usd: exec,
    note: `约 ${formatUsd(exec)}（仅执行）`,
  };
}

/**
 * One phrase for planLine: always a non-empty human string.
 * Missing both sides → 「费用未汇总」; never invents $0.00.
 * @param {LiveCostFields|null|undefined} live
 * @returns {string}
 */
export function formatLiveCostPhrase(live) {
  return summarizeLiveCost(live).note;
}
