/**
 * [INPUT]: ResultViewModel / resultApi / ResultView / inspectCopy / resultSummary
 * [OUTPUT]: result feature 公共出口
 * [POS]: A4 · P0-1/P0-4 features/result 桶文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图（源码边界 · 非第二套阶段表）:
 *   resultApi     → gateway（startRework / acceptResidual）
 *   inspectCopy   → inspect_loop 人话 · 与 report「对照计划」同词（P0-4）
 *   resultSummary → live 费用人话（无 DOM；不下沉 Rust）
 *   ResultViewModel → 意图
 *   ResultView    → 结果台 DOM · verification 副栏（P2-1）· 发意图
 */

export { createResultViewModel } from "./ResultViewModel.js";
export { bindResultView } from "./ResultView.js";
export * as resultApi from "./resultApi.js";
export * as inspectCopy from "./inspectCopy.js";
export * as resultSummary from "./resultSummary.js";
export {
  formatLiveCostPhrase,
  summarizeLiveCost,
  parseCostUsd,
  formatUsd,
} from "./resultSummary.js";
export {
  PLAN_COMPARE_COPY,
  planCompareKind,
  honestInspectCopy,
  inspectStripParts,
  inspectActionVisibility,
} from "./inspectCopy.js";
