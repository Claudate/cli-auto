/**
 * [INPUT]: ResultViewModel / resultApi / ResultView / inspectCopy
 * [OUTPUT]: result feature 公共出口
 * [POS]: A4 features/result 桶文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图（源码边界 · 非第二套阶段表）:
 *   resultApi     → gateway（startRework / acceptResidual）
 *   inspectCopy   → inspect_loop DTO 人话（无裸 VERDICT 主路径）
 *   ResultViewModel → 意图
 *   ResultView    → 结果台 DOM · 发意图
 */

export { createResultViewModel } from "./ResultViewModel.js";
export { bindResultView } from "./ResultView.js";
export * as resultApi from "./resultApi.js";
export * as inspectCopy from "./inspectCopy.js";
