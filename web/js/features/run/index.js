/**
 * [INPUT]: RunViewModel / runApi / RunView / logPanel / runBuckets / log*
 * [OUTPUT]: run feature 公共出口
 * [POS]: A4 + A5-2c features/run 桶文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图（源码边界 · 非第二套阶段表）:
 *   runApi        → gateway（stopRun / resumeRun / stopTask / openTaskTerminal）
 *   runBuckets    → 五态展示桶（读 DTO；不写策略）
 *   RunViewModel  → 意图与展示状态
 *   RunView       → 进度台 DOM · 发意图
 *   logPanel      → 日志次级折叠 chrome
 *   logVirtual    → P2-3 虚拟列表算法（A5-2c 自 log.js）
 *   logBoard      → CLI 多窗看板壳
 *   logBoardCard  → 单窗 chrome/body（P-ship-D 纵切）
 *   logBoardEvents→ 窗内 click/drag 重绑（P-ship-D 纵切）
 *   logActions    → stop/resume/export（经 ccoRun/ccoResult/gateway）
 *   loadLive      → workspace 轮询壳（A5-2b 自 plan.js）
 */

export { createRunViewModel } from "./RunViewModel.js";
export { bindRunView } from "./RunView.js";
export * as runApi from "./runApi.js";
export * as runBuckets from "./runBuckets.js";
export {
  syncMonitorLogsFold,
  paintLogSecondaryVisibility,
} from "./logPanel.js";
/** A5-2b: workspace live poll shell (was plan.js) */
export { loadLive, ensureSelectedTask, pickSelectedTaskId } from "./loadLive.js";
/** A5-2c: log desk (virtual list + board + actions) */
export { createLogDesk } from "./logDesk.js";
