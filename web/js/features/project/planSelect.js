/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: confirm open · max parallel · selectPlan · pick file · default · quickSplit
 * [POS]: A5-2b-fin features/project/planSelect.js
 * note: confirm open · max parallel · selectPlan · pick file · default
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

export function showSplitPlanConfirm(opts = {}) {
  if (!state.planJob) {
    toast("还没有拆分结果");
    return;
  }
  const wantEdit = !!opts.edit;
  // Remember where to return when viewing split during a live/paused run.
  if ((hasActiveRun() || isRunPaused() || state.phase === "running" || state.phase === "done") &&
      state.phase !== "confirm") {
    state.returnPhaseAfterConfirm = state.phase || (hasActiveRun() ? "running" : "done");
  } else if (!opts.keepReturn) {
    state.returnPhaseAfterConfirm = null;
  }
  state.phase = "confirm";
  state.confirmEditing = false;
  if (!state.confirmTaskId && state.planJob.tasks?.length) {
    // Prefer first pending (not-yet-run) task when editing after pause.
    const pending =
      (state.planJob.tasks || []).find((t) => canEditSelectedTask(t.id)) ||
      state.planJob.tasks[0];
    state.confirmTaskId = pending.id;
  }
  // Plan list / rail reopen: leave plans|chat page so split desk is visible
  if (state.page !== "workspace") {
    showPage("workspace");
  }
  host.renderPhasePanels();
  host.renderPlanPicker();
  renderWorkspace();
  host.updateSplitPlanChip();
  if (wantEdit) {
    if (canEditSelectedTask(state.confirmTaskId)) {
      host.beginConfirmEdit();
    } else if (hasActiveRun()) {
      toast("运行中不可编辑，请先停止或待计划暂停");
    } else if (isRunPaused()) {
      toast("当前任务已执行过，请选左侧未执行任务再编辑");
    } else {
      toast("当前不可编辑");
    }
  }
}

/** Top-bar「编辑计划」：进确认页；仅暂停后、未执行任务可改。 */
export function openEditPlan() {
  if (!state.planJob) {
    toast("还没有拆分结果，请先点「拆成步骤」");
    return;
  }
  if (hasActiveRun()) {
    toast("运行中不可编辑，请先停止或待计划暂停");
    return;
  }
  showSplitPlanConfirm({ edit: true });
}

export function backFromConfirmToMonitor() {
  state.confirmEditing = false;
  state.phase = state.returnPhaseAfterConfirm || (hasActiveRun() ? "running" : "done");
  state.returnPhaseAfterConfirm = null;
  host.renderPhasePanels();
  host.renderPlanPicker();
  renderWorkspace();
  host.updateSplitPlanChip();
}

/** Concurrent workers chosen at plan-split time (1–32). */
export function readSplitMaxParallel() {
  // Empty / mid-edit → keep last committed hidden value, never force "2" into the field.
  const fromChooser = parseInt($("#chooser-max-parallel")?.value, 10);
  const fromHidden = parseInt($("#pp-max-parallel")?.value, 10);
  const fromSettings = parseInt($("#s-max-parallel")?.value, 10);
  const n = Number.isFinite(fromChooser) && fromChooser > 0
    ? fromChooser
    : Number.isFinite(fromHidden) && fromHidden > 0
      ? fromHidden
      : Number.isFinite(fromSettings) && fromSettings > 0
        ? fromSettings
        : 2;
  return Math.max(1, Math.min(32, n));
}

/** Commit/clamp concurrency into both inputs. Skip the field the user is typing in. */
export function syncSplitMaxParallelInputs(sourceId, { force = false } = {}) {
  const chooser = $("#chooser-max-parallel");
  const hidden = $("#pp-max-parallel");
  const active = document.activeElement;
  // While the user is editing (empty / partial), do not rewrite the visible field.
  if (
    !force &&
    chooser &&
    (active === chooser || chooser.dataset.editing === "1")
  ) {
    // Still mirror a valid number into hidden if present.
    const typed = parseInt(chooser.value, 10);
    if (Number.isFinite(typed) && typed > 0 && hidden) {
      hidden.value = String(Math.max(1, Math.min(32, typed)));
    }
    return readSplitMaxParallel();
  }
  const n = readSplitMaxParallel();
  if (chooser && sourceId !== "chooser-max-parallel") {
    if (force || active !== chooser) chooser.value = String(n);
  }
  if (hidden) hidden.value = String(n);
  return n;
}

/** Clamp concurrency on blur / assign; allow empty mid-edit. */
export function commitSplitMaxParallel(inputEl) {
  if (!inputEl) return readSplitMaxParallel();
  inputEl.dataset.touched = "1";
  inputEl.dataset.editing = "0";
  const n = Math.max(1, Math.min(32, parseInt(inputEl.value, 10) || 2));
  inputEl.value = String(n);
  const hidden = $("#pp-max-parallel");
  if (hidden) hidden.value = String(n);
  const chooser = $("#chooser-max-parallel");
  if (chooser && chooser !== inputEl) chooser.value = String(n);
  return n;
}

export async function selectPlan(planPath, opts = {}) {
  const keepSession = !!opts.keepSession;
  const next =
    normalizePlanPath(planPath, state.selectedPath) || planPath || null;
  const cur =
    normalizePlanPath(state.selectedPlan, state.selectedPath) ||
    state.selectedPlan ||
    null;
  const samePlan = !!(next && cur && next === cur);

  // 运行中禁止换源计划（可 keepSession 只用于恢复当前）
  if (hasActiveRun() && !keepSession && !samePlan && !opts.force) {
    toastRunLocked("切换计划");
    return;
  }

  // 规划/确认进行中：默认不销毁会话（后台继续）
  // force=true（聊天/计划管理「拆成步骤」换文件）允许切换；调用方须先 clear session。
  if (host.isPlanSessionActive() && !opts.force) {
    if (samePlan || keepSession) {
      state.selectedPlan = next || state.selectedPlan;
      host.renderPlanPicker();
      host.updateTopPlanInfo();
      if (state.planChooserOpen) host.updateChooserAssignState();
      return;
    }
    // 换了另一份计划：提示并拒绝静默清空
    toast("规划进行中：请先「返回选计划/重新规划」，或等待完成");
    return;
  }

  state.selectedPlan = next;
  if (next) state.chatDraftPlan = next;
  // Re-click of the already-selected plan with preview cached → repaint only,
  // no previewPlan IPC (switch-latency; also absorbs the double selectPlan call
  // from startExecuteFromSelection → selectPlanRailItem).
  if (samePlan && state.planPreview && !opts.force) {
    host.renderPhasePanels();
    host.renderPlanPicker();
    host.updateTopPlanInfo();
    if (state.planChooserOpen) host.updateChooserAssignState();
    return;
  }
  state.planPreview = null;
  host.renderPhasePanels();
  host.renderPlanPicker();
  if (!planPath) return;
  try {
    state.planPreview = await requireGateway().previewPlan(
      state.selectedPath,
      planPath
    );
  } catch (e) {
    console.warn("preview failed", e);
    state.planPreview = {
      name: planDisplayName(state.selectedPlan || planPath),
      task_count: "?",
      max_parallel: "?",
    };
  }
  host.renderPlanPicker();
  host.updateTopPlanInfo();
  if (state.planChooserOpen) host.updateChooserAssignState();
}

export async function pickPlanFileForPicker() {
  try {
    const proj = state.selectedPath;
    if (!proj) {
      toast("请先选择项目");
      return;
    }
    const root = String(proj).replace(/[/\\]+$/, "");
    // 默认落到当前项目根；管理页「打开文件」同源
    const selected = await openNativeDialog({
      multiple: false,
      directory: false,
      defaultPath: root,
      title: "打开计划文件（须在当前项目内）",
      filters: [{ name: "Plan", extensions: ["md", "yaml", "yml", "json"] }],
    });
    // 取消：dialog 返回 null / undefined / "" / []
    if (selected == null || selected === false || selected === "") return;
    const abs = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
    if (!abs) return;
    if (!host.isPlanUnderProject(abs, root)) {
      toast("请选择当前项目目录内的计划文件，不要选其它项目");
      return;
    }
    const rel = normalizePlanPath(abs, root) || abs;
    if (!rel || rel === root) {
      toast("请选择计划文件，而不是目录");
      return;
    }
    if (!state.plans.includes(rel)) state.plans = [rel, ...state.plans];
    await selectPlan(rel);
    state.selectedPlan = rel;
    state.chatDraftPlan = rel;
    // 留在弹窗内，方便直接点「开始拆分」
    if (state.planChooserOpen) {
      host.renderPlanChooser();
      host.updateChooserAssignState();
    }
    // 计划管理页：刷新列表与详情
    if (state.page === "plans" && typeof renderPlansMgmtPage === "function") {
      try {
        await loadPlanItems();
      } catch (_) {}
      try {
        if (typeof selectPlanRailItem === "function") selectPlanRailItem(rel);
      } catch (_) {}
      renderPlansMgmtPage();
    }
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/**
 * 快速拆分：拿一个计划**文件路径**（聊天里给的 / 拆分台输入的）→ 先校验可读，
 * 再走既有 startExecuteFromSelection（analyze → start_plan_job → 拆分台；禁止 start_run）。
 * 只对**当前项目内**可读的计划生效——项目外/读不到不静默进坏状态。
 * @param {string} rawPath 计划路径（项目相对或项目内绝对；可带引号/反引号）
 * @param {{source?:string, silentFail?:boolean}} [opts]
 *   silentFail=true：读不到时不弹提示、返回 false（供裸路径识别回退到普通聊天）
 * @returns {Promise<boolean>} 是否已路由到拆分台
 */
export async function quickSplitFromPath(rawPath, opts = {}) {
  if (!state.selectedPath) {
    if (!opts.silentFail) toast("请先选择项目");
    return false;
  }
  if (hasActiveRun()) {
    if (!opts.silentFail) toastRunLocked("拆分计划");
    return false;
  }
  const cleaned = String(rawPath || "")
    .trim()
    .replace(/^['"`]+|['"`]+$/g, "")
    .trim();
  if (!cleaned) {
    if (!opts.silentFail) toast("请提供计划文件路径");
    return false;
  }
  const rel =
    (typeof normalizePlanPath === "function"
      ? normalizePlanPath(cleaned, state.selectedPath)
      : null) || cleaned;
  // 校验可读：读不到（不存在 / 项目外 / 空）就不进拆分台。
  let readable = false;
  try {
    const md = await requireGateway().readPlanMd(state.selectedPath, rel);
    readable = !!(md && String(md).trim());
  } catch (_) {
    readable = false;
  }
  if (!readable) {
    if (!opts.silentFail) {
      toast(`读不到计划文件：${cleaned}（请确认是当前项目内的计划路径）`);
    }
    return false;
  }
  // startExecuteFromSelection 真源在 projectPicker，经 host 聚合调用避免环形依赖。
  await host.startExecuteFromSelection(rel, {
    source: opts.source || "quick-split",
    direct: true,
  });
  return true;
}

/** 拆分台/选计划弹窗：路径输入框回车 = 校验可读后直接进拆分台（一次性绑定）。 */
export function bindChooserQuickSplitInput() {
  const el = $("#chooser-path-input");
  if (!el || el.dataset.qsWired === "1") return;
  el.dataset.qsWired = "1";
  el.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || e.isComposing) return;
    e.preventDefault();
    const v = (el.value || "").trim();
    if (!v) {
      toast("请输入计划文件路径");
      return;
    }
    quickSplitFromPath(v, { source: "chooser" });
  });
}

export async function setDefaultPlan() {
  if (!state.selectedPath || !state.selectedPlan) return;
  try {
    await requireGateway().setProjectDefaultPlan(
      state.selectedPath,
      state.selectedPlan
    );
    const proj = state.projects.find((p) => p.path === state.selectedPath);
    if (proj) proj.default_plan = state.selectedPlan;
    toast("已设为默认计划");
  } catch (e) {
    toast(String(e));
  }
}
