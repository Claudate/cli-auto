/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: confirm desk → ccoSplit; replan/sanitize via gateway
 * [POS]: A5-2b-fin features/project/confirmActions.js
 * note: confirm desk → ccoSplit; replan/sanitize via gateway
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

/**
 * A5-2b：确认台只委托 ccoSplit（fillMeta + 三栏 + 开跑）。
 * main.js 未就绪时：仅 toast，不在本文件堆 classic 三栏逻辑。
 */
export function renderConfirmPanel() {
  const job = state.planJob;
  if (!job) return;

  if (window.ccoSplit && typeof window.ccoSplit.render === "function") {
    try {
      if (window.ccoSplit.vm && typeof window.ccoSplit.vm.setJob === "function") {
        window.ccoSplit.vm.setJob(job, {
          jobId: state.planJobId,
          selectedTaskId: state.confirmTaskId,
          editing: state.confirmEditing,
        });
      }
      window.ccoSplit.render();
      return;
    } catch (e) {
      console.error("[renderConfirmPanel] ccoSplit", e);
    }
  }

  // Pre-module fallback: keep title readable; no start_run / no duplicate desk
  const st = String(job.status || "").toLowerCase();
  const reused = st === "confirmed";
  const titleEl = $("#confirm-title");
  if (titleEl) {
    titleEl.textContent = job.plan_name
      ? `${reused ? "历史拆分" : "拆分结果"} · ${job.plan_name}`
      : reused
        ? "历史拆分（可再次确认并开始）"
        : "拆分结果";
  }
  const waves = $("#confirm-waves");
  if (waves && !waves.dataset.ccoAwaitSplit) {
    waves.dataset.ccoAwaitSplit = "1";
    waves.innerHTML =
      "<p class='muted'>拆分台加载中…若长时间空白请刷新窗口</p>";
  }
  host.updateSplitPlanChip();
}

/** A5-2b: one-line → features/split（无 classic fallback 业务） */
export function beginConfirmEdit() {
  if (window.ccoSplit && typeof window.ccoSplit.beginEdit === "function") {
    return window.ccoSplit.beginEdit();
  }
  toast("拆分台未就绪，请稍候再编辑");
}

export function cancelConfirmEdit() {
  if (window.ccoSplit && typeof window.ccoSplit.cancelEdit === "function") {
    return window.ccoSplit.cancelEdit();
  }
  state.confirmEditing = false;
  renderConfirmPanel();
}

export async function saveConfirmEdit() {
  if (window.ccoSplit && typeof window.ccoSplit.saveEdit === "function") {
    return window.ccoSplit.saveEdit();
  }
  toast("拆分台未就绪，无法保存");
}

/** P2-1: delete selected task — only via ccoSplit (gateway.removePlanTask). */
export async function deleteConfirmTask() {
  if (window.ccoSplit && typeof window.ccoSplit.deleteTask === "function") {
    return window.ccoSplit.deleteTask();
  }
  toast("拆分台未就绪，无法删除");
}

/**
 * Only from confirm phase — starts workers.
 * A5-2b: **唯一**路径 ccoSplit.confirmAndStart → gateway.confirmStart（无 invoke confirm_start 旁路）。
 */
export async function confirmAndStart() {
  if (window.ccoSplit && typeof window.ccoSplit.confirmAndStart === "function") {
    return window.ccoSplit.confirmAndStart({
      ensureDoctor:
        typeof host.ensureDoctor === "function" ? () => host.ensureDoctor(true) : undefined,
    });
  }
  const err = $("#confirm-error");
  if (err) {
    err.textContent = "拆分台未就绪，请刷新后重试（不会旁路开跑）";
    err.hidden = false;
  }
  toast("拆分台未就绪，无法确认开跑");
}

export function cancelPlanning() {
  host.stopPlanJobPoll();
  host.setAssignBusy(false);
  host.clearPlanSession(state.selectedPath);
  state.phase = "pick";
  state.planJobId = null;
  state.planJob = null;
  host.renderPhasePanels();
  host.renderPlanPicker();
  host.updateBgPlanBanner();
}

/**
 * Confirm-screen re-split: keep current plan path and start a fresh plan job
 * (one click — no need to re-pick the file). Falls back to chooser if no plan.
 * P2-2: pass preserve_from_job_id so human title/prompt/deps/deletes re-apply.
 */
export async function replanFromConfirm() {
  if (hasActiveRun()) {
    toastRunLocked("重新拆分");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  const mode =
    state.planJob?.digest_mode || state.planJob?.digestMode || "";
  const modeHint =
    typeof flowModeLabel === "function" && mode
      ? `「${flowModeLabel(mode)}」`
      : "";
  const planPath =
    state.selectedPlan ||
    state.planJob?.plan_path ||
    state.planJob?.planPath ||
    null;
  if (planPath && !state.selectedPlan) {
    state.selectedPlan =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(planPath, state.selectedPath) || planPath
        : planPath;
  }

  // P2-2: remember current job so the next start_plan_job can re-apply edits.
  const preserveFrom =
    state.planJobId ||
    state.planJob?.job_id ||
    state.planJob?.jobId ||
    null;
  state.preserveFromJobId = preserveFrom;
  // A3-3: stay on split phase; backend preserve_from_job_id keeps edits
  try {
    if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
      window.ccoApp.goSplit();
    }
  } catch (_) {}

  host.stopPlanJobPoll();
  host.setAssignBusy(false);
  host.clearPlanSession(state.selectedPath);
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.confirmEditing = false;
  state.returnPhaseAfterConfirm = null;
  state.phase = "pick";
  host.renderPhasePanels();
  host.renderPlanPicker();
  host.updateSplitPlanChip();
  host.updateBgPlanBanner();

  if (!state.selectedPlan || !state.selectedPath) {
    host.openPlanChooser(true);
    toast("请选择计划后再次拆分");
    return;
  }

  toast(
    modeHint
      ? `按当前计划重新拆分（保留人工修改 · 上次：${modeHint}）…`
      : preserveFrom
        ? "按当前计划重新拆分（保留人工修改）…"
        : "按当前计划重新拆分…"
  );
  // Same entry as「开始拆分」— keeps chooser options (并发 / 通道)
  if (typeof host.analyzePlanFromPicker === "function") {
    await host.analyzePlanFromPicker();
  } else {
    host.openPlanChooser(true);
  }
}

/**
 * Confirm-screen CTA when critic notes missing inspect tail:
 * enable settings.post_inspect_enabled, then re-split current plan.
 */
export async function enablePostInspectAndResplit() {
  if (hasActiveRun()) {
    toastRunLocked("开启巡检");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  const btn = $("#btn-enable-post-inspect");
  if (btn) {
    btn.disabled = true;
    btn.textContent = "开启中…";
  }
  try {
    const setS =
      window.ccoSettings?.setSettings ||
      ((u) => requireGateway().setSettings(u));
    await setS({ post_inspect_enabled: true });
    // Keep settings page in sync if open
    if ($("#s-post-inspect")) $("#s-post-inspect").checked = true;
    toast("已开启「拆分后附加：任务巡检」· 正在按当前计划重拆…");
    if (typeof replanFromConfirm === "function") {
      await replanFromConfirm();
    }
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "开启巡检并重拆";
    }
  }
}

/**
 * Confirm-screen CTA: enable settings.planner_critic_enabled, then re-split.
 */
export async function enablePlannerCriticAndResplit() {
  if (hasActiveRun()) {
    toastRunLocked("开启智能校对");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  const btn = $("#btn-enable-planner-critic");
  if (btn) {
    btn.disabled = true;
    btn.textContent = "开启中…";
  }
  try {
    const setS =
      window.ccoSettings?.setSettings ||
      ((u) => requireGateway().setSettings(u));
    await setS({ planner_critic_enabled: true });
    if ($("#s-planner-critic")) $("#s-planner-critic").checked = true;
    toast("已开启「智能第二跳校对」· 正在按当前计划重拆…");
    if (typeof replanFromConfirm === "function") {
      await replanFromConfirm();
    }
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "开启智能校对并重拆";
    }
  }
}

/** Confirm-screen: drop unmotivated depends_on edges. */
export async function sanitizeDepsFromConfirm() {
  if (hasActiveRun()) {
    toastRunLocked("让可并行的真正并行");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  if (!state.planJobId) {
    toast("没有可清理的拆分结果");
    return;
  }
  const btn = $("#btn-sanitize-deps");
  const label =
    typeof flowSanitizeDepsLabel === "function"
      ? flowSanitizeDepsLabel()
      : "让可并行的真正并行";
  if (btn) {
    btn.disabled = true;
    btn.textContent = "处理中…";
  }
  try {
    const resp = await requireGateway().sanitizePlanDeps(state.planJobId);
    const removed = resp?.removed ?? resp?.Removed ?? 0;
    const view = resp?.view || resp;
    if (view) {
      state.planJob = view;
      state.planJobId = view.job_id || view.jobId || state.planJobId;
      host.stashPlanSession(state.selectedPath);
    }
    if (removed > 0) {
      toast(`已去掉 ${removed} 条可疑依赖 · 可并行步骤更多了`);
    } else {
      toast("已经够并行 · 没有可再清的依赖边");
    }
    renderConfirmPanel();
    host.renderPlanPicker();
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = label;
    }
  }
}
