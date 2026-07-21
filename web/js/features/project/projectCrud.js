/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: add/remove project · dismiss run · doctor bridge
 * [POS]: A5-2b-fin features/project/projectCrud.js
 * note: add/remove project · dismiss run · doctor bridge
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  isRunPaused,
  isLiveStatus,
  isFailedStatus,
  toastRunLocked,
  normalizePlanPath,
  planDisplayName,
  fillPlannerLog,
  canEditSelectedTask,
  openNativeDialog,
  loadProjects,
  renderProjectList,
  renderWorkspace,
  goHome,
  closeModal,
  openChatPage,
  stashChatSession,
  restoreChatSession,
  stopChatWaitTicker,
  loadPlanRail,
  renderPlanRail,
  selectPlanRailItem,
  renderPlansMgmtPage,
  chatAssignDirectEnabled,
  flowModeLabel,
  flowModeHint,
  flowStageStripHtml,
  flowChooserSub,
  flowJoinSeriousFun,
  flowPickBlurb,
  flowPlanHowLabel,
  flowPlanningSub,
  flowSanitizeDepsLabel,
  flowRunningMonitorTitle,
  esc,
  requireGateway,
} from "./legacy.js";
import { host } from "./host.js";

export async function addProjectFromModal() {
  const path = $("#m-project-path").value.trim();
  const name = $("#m-project-name").value.trim() || null;
  if (!path) return toast("请选择项目路径");
  try {
    await requireGateway().addProject(path, name);
    toast("已添加项目");
    closeModal();
    await loadProjects();
    await host.selectProject(path);
    // C4: resume welcome-template click after folder pick
    let pending = null;
    try {
      pending = sessionStorage.getItem("cco.pendingPlanTemplate");
      if (pending) sessionStorage.removeItem("cco.pendingPlanTemplate");
    } catch (_) {}
    if (pending && typeof window.applyPlanTemplate === "function") {
      await Promise.resolve(window.applyPlanTemplate(pending)).catch((e) =>
        toast(String(e?.message || e))
      );
    } else if (
      pending &&
      window.ccoTemplates &&
      typeof window.ccoTemplates.applyPlanTemplate === "function"
    ) {
      await Promise.resolve(
        window.ccoTemplates.applyPlanTemplate(pending)
      ).catch((e) => toast(String(e?.message || e)));
    }
  } catch (e) {
    toast(String(e));
  }
}

export async function pickFolderToModal() {
  try {
    const selected = await openNativeDialog({ directory: true, multiple: false });
    if (selected) $("#m-project-path").value = selected;
  } catch (e) {
    toast(String(e));
  }
}

export async function removeSelectedProject() {
  if (!state.selectedPath) return;
  if (hasActiveRun()) {
    toastRunLocked("关闭/移除项目");
    return;
  }
  try {
    const path = state.selectedPath;
    await requireGateway().removeProject(path);
    toast("已移除项目");
    host.clearPlanSession(path);
    host.stopPlanJobPoll();
    host.setAssignBusy(false);
    state.planJobId = null;
    state.planJob = null;
    state.phase = "pick";
    state.selectedPath = null;
    state.live = null;
    await loadProjects();
    goHome();
  } catch (e) {
    toast(String(e));
  }
}

/* 隐藏当前运行视图（不清除运行记录，不删除项目） */
export async function dismissRun() {
  // 只收起运行视图；若在规划/确认则保留
  state.live = null;
  state.selectedTaskId = null;
  if (!host.isPlanSessionActive()) {
    state.phase = "pick";
  }
  state.planCollapsed = false;
  renderWorkspace();
  host.updateBgPlanBanner();
}

/* ── Doctor gate (A5-2d → features/settings via ccoSettings) ── */
export async function ensureDoctor(force = false) {
  if (window.ccoSettings?.ensureDoctor) {
    return window.ccoSettings.ensureDoctor(force);
  }
  // pre-main fallback
  const now = Date.now();
  if (!force && state.doctorCache && now - state.doctorCache.at < 60_000) {
    return state.doctorCache;
  }
  try {
    const d = await requireGateway().doctor(state.selectedPath || null);
    state.doctorCache = { ok: !!d.ok, at: now, lines: d.lines || [] };
  } catch (e) {
    state.doctorCache = {
      ok: false,
      at: now,
      lines: [{ name: "doctor", ok: false, detail: String(e) }],
    };
  }
  if (window.ccoSettings?.renderDoctorWarn) window.ccoSettings.renderDoctorWarn();
  return state.doctorCache;
}

export function renderDoctorWarn() {
  if (window.ccoSettings?.renderDoctorWarn) {
    return window.ccoSettings.renderDoctorWarn();
  }
}
