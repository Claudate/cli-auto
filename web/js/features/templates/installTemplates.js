/**
 * [INPUT]: templates modules
 * [OUTPUT]: window.ccoTemplates + classic global names（strangler）
 * [POS]: P-ship-D features/templates/installTemplates.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  planTemplateById,
  planTemplateChatEmptyHtml,
  planTemplateWelcomeHtml,
} from "./catalog.js";
import {
  buildSplitSummaryBlock,
  mergeSplitSummaryIntoMarkdown,
} from "./splitSummary.js";
import * as templatesApi from "./templatesApi.js";
import {
  applyPlanTemplate,
  writeSplitSummaryToPlan,
  refreshSplitWritebackBtn,
} from "./templatesActions.js";

/** Public desk for window.ccoTemplates (classic templates.js is facade). */
export function createTemplatesDesk() {
  return {
    api: templatesApi,
    planTemplateById,
    planTemplateChatEmptyHtml,
    planTemplateWelcomeHtml,
    applyPlanTemplate,
    writeSplitSummaryToPlan,
    refreshSplitWritebackBtn,
    buildSplitSummaryBlock,
    mergeSplitSummaryIntoMarkdown,
  };
}

/**
 * Install ccoTemplates + classic globals used by state/chat/settings/main.
 * @returns {ReturnType<typeof createTemplatesDesk>}
 */
export function installTemplatesHost() {
  const desk = createTemplatesDesk();
  window.ccoTemplates = desk;

  window.planTemplateById = planTemplateById;
  window.planTemplateChatEmptyHtml = planTemplateChatEmptyHtml;
  window.planTemplateWelcomeHtml = planTemplateWelcomeHtml;
  window.applyPlanTemplate = applyPlanTemplate;
  window.writeSplitSummaryToPlan = writeSplitSummaryToPlan;
  window.refreshSplitWritebackBtn = refreshSplitWritebackBtn;
  window.buildSplitSummaryBlock = buildSplitSummaryBlock;
  window.mergeSplitSummaryIntoMarkdown = mergeSplitSummaryIntoMarkdown;

  return desk;
}
