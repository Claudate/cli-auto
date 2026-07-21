/**
 * [INPUT]: catalog / splitSummary / templatesApi / actions / install
 * [OUTPUT]: templates feature 公共出口
 * [POS]: P-ship-D features/templates 桶文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图:
 *   catalog        → 内置模板 + 空态 HTML
 *   splitSummary   → S14 摘要块纯函数
 *   templatesApi   → chatApi/gateway（savePlan · readPlanMd）
 *   templatesActions → apply / writeback CTA
 *   installTemplates → window.ccoTemplates + classic 全局
 */

export {
  PLAN_TEMPLATES,
  planTemplateById,
  planTemplateChatEmptyHtml,
  planTemplateWelcomeHtml,
} from "./catalog.js";
export {
  SPLIT_SUMMARY_START,
  SPLIT_SUMMARY_END,
  buildSplitSummaryBlock,
  mergeSplitSummaryIntoMarkdown,
} from "./splitSummary.js";
export * as templatesApi from "./templatesApi.js";
export {
  applyPlanTemplate,
  writeSplitSummaryToPlan,
  refreshSplitWritebackBtn,
} from "./templatesActions.js";
export {
  createTemplatesDesk,
  installTemplatesHost,
} from "./installTemplates.js";
