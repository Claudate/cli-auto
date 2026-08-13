/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: add/remove project · dismiss run · doctor bridge
 * [POS]: A5-2b-fin features/project/projectCrud.js
 * note: add/remove project · dismiss run · doctor bridge
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { confirmDialog } from "../../shared/confirmDialog.js";
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
    // C4: consume the pending template BEFORE closeModal — closing the modal
    // (incl. user cancel) always clears the stash so it can't fire days later.
    let pending = null;
    try {
      pending = sessionStorage.getItem("cco.pendingPlanTemplate");
    } catch (_) {}
    closeModal();
    await loadProjects();
    await host.selectProject(path);
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

/**
 * shell-chrome B1：从 cco 列表移除项目（不删磁盘文件夹）。
 * @param {string} [pathArg] 侧栏行路径；默认当前选中
 * @param {{ skipConfirm?: boolean }} [opts]
 */
export async function removeSelectedProject(pathArg, opts = {}) {
  const path = pathArg || state.selectedPath;
  if (!path) return;
  const isCurrent = path === state.selectedPath;
  // 运行中：当前项目用 hasActiveRun；其它行看 projects 上的 active_status
  if (isCurrent && hasActiveRun()) {
    toastRunLocked("移除项目");
    return;
  }
  if (!isCurrent) {
    const p = (state.projects || []).find((x) => x.path === path);
    const stt = String(p?.active_status || "").toLowerCase();
    const live =
      (p?.running_tasks > 0) ||
      ["running", "starting", "queued", "validated", "init", "resuming"].includes(
        stt
      );
    if (live) {
      toast("该项目还在执行，请先停止后再从列表移除");
      return;
    }
  }
  const proj =
    (state.projects || []).find((x) => x.path === path) || null;
  const name =
    proj?.name ||
    String(path).split(/[/\\]/).filter(Boolean).pop() ||
    path;
  if (!opts.skipConfirm) {
    const ok = await confirmDialog({
      title: "移除项目",
      body: `从 cco 列表移除「${name}」？不会删除电脑上的文件夹。`,
      okLabel: "移除",
      danger: true,
    });
    if (!ok) return;
  }
  try {
    await requireGateway().removeProject(path);
    toast("已从列表移除（文件夹仍在）");
    host.clearPlanSession?.(path);
    if (isCurrent) {
      host.stopPlanJobPoll?.();
      host.setAssignBusy?.(false);
      state.planJobId = null;
      state.planJob = null;
      state.phase = "pick";
      state.selectedPath = null;
      state.live = null;
    }
    if (state.planSessions && state.planSessions[path]) {
      try {
        delete state.planSessions[path];
      } catch (_) {}
    }
    await loadProjects();
    if (isCurrent) goHome();
    else renderProjectList();
  } catch (e) {
    toast(String(e));
  }
}

/**
 * 结束本轮 / 收起运行视图。
 * SoT = SQLite project_ui_prefs.dismissed_run_id（gateway only）。
 * project_live_view 服务端过滤；禁止 localStorage 业务态。
 */
export async function dismissRun() {
  const path = state.selectedPath;
  const rid =
    state.live?.run_id ||
    (path && state.lastRunIdByProject?.[path]) ||
    null;
  if (path && rid) {
    try {
      await requireGateway().projectDismissRun(path, String(rid));
    } catch (e) {
      console.warn("[dismissRun] sqlite", e);
      toast(String(e?.message || e) || "结束本轮写入失败");
      // Still clear UI so user is not stuck on result desk
    }
  }
  try {
    host.clearPlanSession?.(path);
  } catch (_) {}
  state.live = null;
  state.selectedTaskId = null;
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.phase = "pick";
  state.planCollapsed = false;
  state.planStartedAt = 0;
  try {
    host.stopPlanJobPoll?.();
  } catch (_) {}
  try {
    host.setAssignBusy?.(false);
  } catch (_) {}
  try {
    if (typeof loadProjects === "function") await loadProjects();
  } catch (_) {}
  try {
    if (typeof openChatPage === "function") await openChatPage();
    else showPage("chat");
  } catch (_) {
    try {
      showPage("chat");
    } catch (__) {}
  }
  try {
    renderWorkspace();
  } catch (_) {}
  try {
    host.updateBgPlanBanner?.();
  } catch (_) {}
  try {
    renderProjectList();
  } catch (_) {}
  try {
    host.renderPlanPicker?.();
  } catch (_) {}
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
