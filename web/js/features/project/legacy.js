/**
 * [INPUT]: window globals from classic scripts
 * [OUTPUT]: state proxy + classic helpers for features/project
 * [POS]: A5-2b-fin features/project/legacy.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/** Live view of classic `state`. */
export const state = new Proxy(
  {},
  {
    get(_t, prop) {
      if (prop === Symbol.toStringTag) return "StateProxy";
      const s = typeof window !== "undefined" ? window.state : null;
      if (!s) return undefined;
      const v = s[prop];
      return typeof v === "function" ? v.bind(s) : v;
    },
    set(_t, prop, val) {
      const s = typeof window !== "undefined" ? window.state : null;
      if (!s) return false;
      s[prop] = val;
      return true;
    },
    has(_t, prop) {
      const s = typeof window !== "undefined" ? window.state : null;
      return !!(s && prop in s);
    },
  }
);

export function $(sel, el = document) {
  if (typeof window !== "undefined" && typeof window.$ === "function") {
    return window.$(sel, el);
  }
  return el && el.querySelector ? el.querySelector(sel) : null;
}

function call(name, ...args) {
  const fn = typeof window !== "undefined" ? window[name] : null;
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

export const toast = (...a) => call("toast", ...a);
export const showPage = (...a) => call("showPage", ...a);
export const hasActiveRun = (...a) => !!call("hasActiveRun", ...a);
export const isRunPaused = (...a) => !!call("isRunPaused", ...a);
export const isLiveStatus = (...a) => !!call("isLiveStatus", ...a);
export const isFailedStatus = (...a) => !!call("isFailedStatus", ...a);
export const toastRunLocked = (...a) => call("toastRunLocked", ...a);
export const normalizePlanPath = (...a) => call("normalizePlanPath", ...a);
export const planDisplayName = (...a) => call("planDisplayName", ...a);
export const fillPlannerLog = (...a) => call("fillPlannerLog", ...a);
export const canEditSelectedTask = (...a) => call("canEditSelectedTask", ...a);
export const openNativeDialog = (...a) => call("openNativeDialog", ...a);
export const loadProjects = (...a) => call("loadProjects", ...a);
export const renderProjectList = (...a) => call("renderProjectList", ...a);
export const renderWorkspace = (...a) => call("renderWorkspace", ...a);
export const goHome = (...a) => call("goHome", ...a);
export const closeModal = (...a) => call("closeModal", ...a);
export const openChatPage = (...a) => call("openChatPage", ...a);
export const stashChatSession = (...a) => call("stashChatSession", ...a);
export const restoreChatSession = (...a) => call("restoreChatSession", ...a);
export const stopChatWaitTicker = (...a) => call("stopChatWaitTicker", ...a);
export const loadPlanRail = (...a) => call("loadPlanRail", ...a);
export const renderPlanRail = (...a) => call("renderPlanRail", ...a);
export const selectPlanRailItem = (...a) => call("selectPlanRailItem", ...a);
export const renderPlansMgmtPage = (...a) => call("renderPlansMgmtPage", ...a);
export const chatAssignDirectEnabled = (...a) => call("chatAssignDirectEnabled", ...a);
export const flowModeLabel = (...a) => call("flowModeLabel", ...a);
export const flowModeHint = (...a) => call("flowModeHint", ...a);
export const flowStageStripHtml = (...a) => call("flowStageStripHtml", ...a);
export const flowChooserSub = (...a) => call("flowChooserSub", ...a);
export const flowJoinSeriousFun = (...a) => call("flowJoinSeriousFun", ...a);
export const flowPickBlurb = (...a) => call("flowPickBlurb", ...a);
export const flowPlanHowLabel = (...a) => call("flowPlanHowLabel", ...a);
export const flowPlanningSub = (...a) => call("flowPlanningSub", ...a);
export const flowSanitizeDepsLabel = (...a) => call("flowSanitizeDepsLabel", ...a);
export const flowRunningMonitorTitle = (...a) => call("flowRunningMonitorTitle", ...a);
export const esc = (...a) =>
  typeof call("esc", ...a) !== "undefined"
    ? call("esc", ...a)
    : String(a[0] ?? "")
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");

/** Prefer gateway; throw if neither classic nor main ready. */
export function requireGateway() {
  if (typeof window !== "undefined" && typeof window.requireGateway === "function") {
    return window.requireGateway();
  }
  if (typeof window !== "undefined" && window.ccoGateway) return window.ccoGateway;
  throw new Error("gateway not ready");
}
