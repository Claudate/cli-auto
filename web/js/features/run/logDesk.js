/**
 * [INPUT]: log* modules
 * [OUTPUT]: public desk API → window.ccoLog / classic facade
 * [POS]: A5-2c features/run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as logFilter from "./logFilter.js";
import * as logRender from "./logRender.js";
import * as logVirtual from "./logVirtual.js";
import * as logContent from "./logContent.js";
import * as logActions from "./logActions.js";
import * as logBoard from "./logBoard.js";

/**
 * Full log surface for window.ccoLog (classic log.js facade).
 */
export function createLogDesk() {
  return {
    // board / primary paint
    renderCliBoard: logBoard.renderCliBoard,
    renderTaskList: logBoard.renderTaskList,
    renderDetailLog: logBoard.renderDetailLog,
    stallStripText: logBoard.stallStripText,
    // content
    fillPlannerLog: logContent.fillPlannerLog,
    fillPanelLogBody: logContent.fillPanelLogBody,
    panelLogContent: logContent.panelLogContent,
    panelLogHtml: logContent.panelLogHtml,
    aiLogPlainText: logContent.aiLogPlainText,
    renderLogConsoleHtml: logContent.renderLogConsoleHtml,
    logPanelSignature: logContent.logPanelSignature,
    // virtual
    mountVirtualLog: logVirtual.mountVirtualLog,
    paintVirtualLogWindow: logVirtual.paintVirtualLogWindow,
    isNearBottom: logVirtual.isNearBottom,
    LOG_VIRTUAL_THRESHOLD: logVirtual.LOG_VIRTUAL_THRESHOLD,
    // filter / render
    isAiInteractionEvent: logFilter.isAiInteractionEvent,
    eventPassesFilter: logFilter.eventPassesFilter,
    ansiToHtml: logFilter.ansiToHtml,
    isNoiseText: logFilter.isNoiseText,
    renderLogEvent: logRender.renderLogEvent,
    renderTranscriptLine: logRender.renderTranscriptLine,
    // actions (ccoRun/ccoResult; no invoke fallback)
    cancelTask: logActions.cancelTask,
    stopAll: logActions.stopAll,
    resumeRun: logActions.resumeRun,
    retryTask: logActions.retryTask,
    startReworkWave: logActions.startReworkWave,
    acceptRunResidual: logActions.acceptRunResidual,
    openExternalTerminal: logActions.openExternalTerminal,
    exportBoardLogsMd: logActions.exportBoardLogsMd,
    openHandoffLedger: logActions.openHandoffLedger,
    renderHandoffBoardStrip: logActions.renderHandoffBoardStrip,
    loadDoctor: logActions.loadDoctor,
  };
}

export default createLogDesk;
