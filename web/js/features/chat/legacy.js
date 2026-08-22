/**
 * [INPUT]: window globals from classic scripts
 * [OUTPUT]: state/$/toast/plan helpers for features/chat
 * [POS]: A5-2a features/chat/legacy.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/** Live view of classic `state` (property get/set only). */
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
  if (typeof sel === "string" && !sel.startsWith("#") && !sel.startsWith(".") && !sel.includes(" ")) {
    // classic chat sometimes passes bare id (syncPlansDirLabels)
    return (
      document.getElementById(sel) ||
      (el && el.querySelector ? el.querySelector(sel) : null)
    );
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
export const toastRunLocked = (...a) => call("toastRunLocked", ...a);
export const normalizePlanPath = (...a) => call("normalizePlanPath", ...a);
export const planDisplayName = (...a) => call("planDisplayName", ...a);
export const planExecBadgeInfo = (...a) => call("planExecBadgeInfo", ...a);
export const applyPlanMetaItems = (...a) => call("applyPlanMetaItems", ...a);
export const partitionPlanItems = (...a) => call("partitionPlanItems", ...a);
export const isPlanUnderProject = (...a) => call("isPlanUnderProject", ...a);
export const selectPlan = (...a) => call("selectPlan", ...a);
export const startExecuteFromSelection = (...a) =>
  call("startExecuteFromSelection", ...a);
export const quickSplitFromPath = (...a) => call("quickSplitFromPath", ...a);
export const loadPlansForPicker = (...a) => call("loadPlansForPicker", ...a);
export const openPlanChooser = (...a) => call("openPlanChooser", ...a);
export const updateChooserAssignState = (...a) =>
  call("updateChooserAssignState", ...a);
export const openNativeDialog = (...a) => call("openNativeDialog", ...a);
export const pickPlanFileForPicker = (...a) => call("pickPlanFileForPicker", ...a);
export const syncShowExecutedToggles = (...a) =>
  call("syncShowExecutedToggles", ...a);
export const renderPlanPicker = (...a) => call("renderPlanPicker", ...a);
export const planTemplateChatEmptyHtml = (...a) =>
  call("planTemplateChatEmptyHtml", ...a);
export const openDoctorPage = (...a) => call("openDoctorPage", ...a);
export const runDoctor = (...a) => call("runDoctor", ...a);
export const loadDoctor = (...a) => call("loadDoctor", ...a);
