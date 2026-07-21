/**
 * [INPUT]: SplitViewModel / splitApi / SplitView / splitDetail
 * [OUTPUT]: split feature 公共出口
 * [POS]: A3 features/split 桶文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图（源码边界 · 非第二套阶段表）:
 *   splitApi        → gateway（confirmStart / updatePlanTask / …）
 *   SplitViewModel  → 意图与展示状态
 *   splitRender     → 波次/卡片 HTML（无 IPC）
 *   splitDetail     → 详情 + 高级路由 paint
 *   SplitView       → DOM 绑定 · 发意图
 *   splitFillMeta   → 标题/meta/critic 条（A5-2b 自 plan.js）
 */

export { createSplitViewModel } from "./SplitViewModel.js";
export { bindSplitView, ensureAdvancedRouteDom } from "./SplitView.js";
export * as splitApi from "./splitApi.js";
export * as splitRender from "./splitRender.js";
/** A5-2b: confirm chrome meta (title / critic / optional hints) */
export { fillSplitMeta } from "./splitFillMeta.js";
