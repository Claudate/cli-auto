/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: confirm desk → ccoSplit; replan/sanitize via gateway
 * [POS]: A5-2b-fin features/project/confirmActions.js
 * note: confirm desk → ccoSplit; replan/sanitize via gateway
 * note: 无 job/无 tasks 必 paintConfirmEmptyState（CTA）；setJob force + post-paint 空白守卫
 * note: 无单份 job 但本项目是一波多计划 → 升级为本波总览（splitWaveLanding · 复用 chat wave 模块）
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
import {
  getBoundPlanJob,
  setBoundPlanJob,
  scrubForeignPlanJob,
  clearSplitUiBinding,
  rebindSplitToOpenProject,
  stampSplitDeskProject,
} from "./projectScope.js";
import { paintWaveLanding, toggleWaveBackLink } from "./splitWaveLanding.js";

/**
 * Explicit empty/error surface for confirm desk — never leave #confirm-waves blank
 * with no copy when the user is on phase=confirm.
 * @param {string} message
 */
function paintConfirmEmptyState(message) {
  const waves = $("#confirm-waves");
  const msg = String(message || "这里还没有拆分结果。").trim();
  if (waves) {
    waves.innerHTML =
      `<div class="split-empty-state" role="status">` +
      `<p class="muted">${esc(msg)}</p>` +
      `<p class="split-empty-actions">` +
      `<button type="button" class="btn primary sm" id="btn-split-empty-replan">拆成步骤</button>` +
      `</p>` +
      `</div>`;
    delete waves.dataset.sig;
    delete waves.dataset.ccoAwaitSplit;
    const btn = waves.querySelector("#btn-split-empty-replan");
    if (btn && !btn.dataset.ccoWired) {
      btn.dataset.ccoWired = "1";
      btn.onclick = () => {
        try {
          if (typeof host.replanFromConfirm === "function") {
            host.replanFromConfirm();
            return;
          }
        } catch (_) {}
        try {
          if (typeof host.openPlanChooser === "function") {
            host.openPlanChooser(true);
            return;
          }
        } catch (_) {}
        toast("请从计划列表选择计划后拆成步骤");
      };
    }
  }
  const titleEl = $("#confirm-title");
  if (titleEl) titleEl.textContent = "拆分结果";
  const meta = $("#confirm-meta");
  if (meta) meta.textContent = "";
}

/**
 * A5-2b：确认台只委托 ccoSplit（fillMeta + 三栏 + 开跑）。
 * main.js 未就绪时：仅 toast，不在本文件堆 classic 三栏逻辑。
 */
export function renderConfirmPanel() {
  // Drop foreign/stale job before paint (tab switch / delayed restore race).
  try {
    scrubForeignPlanJob(state.selectedPath);
  } catch (_) {}

  const job = getBoundPlanJob(state.selectedPath);
  if (!job) {
    // No bound job for this project — must clear desk so other project's
    // steps cannot linger in #confirm-waves after ring/tab navigation.
    // When user is on confirm phase, never leave a silent blank: explain + CTA.
    try {
      clearSplitUiBinding({ scrubState: false });
    } catch (_) {}
    if (state.phase === "confirm") {
      const root = state.selectedPath;
      // 先画保底空态（永不空白），再异步升级为本波总览（若本项目是一波多计划）
      paintConfirmEmptyState(
        root
          ? "当前项目还没有可展示的拆分结果。请先「拆成步骤」，或从计划列表打开已有拆分。"
          : "请先选择项目，再拆计划。"
      );
      toggleWaveBackLink(null);
      paintWaveLanding(root).catch(() => {});
    }
    return;
  }

  const tasks = Array.isArray(job.tasks) ? job.tasks : [];
  if (!tasks.length) {
    // planned/confirmed meta without task payloads — not a usable desk
    const metaN = Number(job.task_count || job.taskCount || 0) || 0;
    paintConfirmEmptyState(
      metaN > 0
        ? `拆分记录有 ${metaN} 步，但步骤明细没加载出来。请点「重新规划」或稍后再打开。`
        : "这份拆分没有步骤可展示。请重新规划，或改用快速拆分。"
    );
    toggleWaveBackLink(
      job.plan_path || job.planPath || state.selectedPlan || null,
      renderConfirmPanel
    );
    const err = $("#confirm-error");
    if (err) {
      err.textContent =
        metaN > 0
          ? "步骤明细缺失"
          : "没有可执行步骤";
      err.hidden = false;
    }
    try {
      stampSplitDeskProject(state.selectedPath);
    } catch (_) {}
    return;
  }

  if (window.ccoSplit && typeof window.ccoSplit.render === "function") {
    try {
      if (window.ccoSplit.vm && typeof window.ccoSplit.vm.setJob === "function") {
        window.ccoSplit.vm.setJob(job, {
          jobId: state.planJobId,
          selectedTaskId: state.confirmTaskId,
          editing: state.confirmEditing,
          force: true, // confirm paint always applies — avoids DOM-wipe no-op blank
        });
      }
      window.ccoSplit.render();
      stampSplitDeskProject(state.selectedPath);
      toggleWaveBackLink(
        job.plan_path || job.planPath || state.selectedPlan || null,
        renderConfirmPanel
      );
      // Post-paint guard: if waves still empty despite tasks, surface instead of silent blank
      try {
        const waves = $("#confirm-waves");
        const html = String(waves?.innerHTML || "").trim();
        if (waves && (!html || html.includes("暂无步骤"))) {
          paintConfirmEmptyState(
            "步骤已绑定但界面未画出卡片。请点「重新规划」，或切换到聊天再回到拆分台。"
          );
        }
      } catch (_) {}
      return;
    } catch (e) {
      console.error("[renderConfirmPanel] ccoSplit", e);
      paintConfirmEmptyState(
        `拆分台渲染失败：${e?.message || e}。请刷新窗口后重试。`
      );
      return;
    }
  }

  // Pre-module fallback: keep title readable; no start_run / no duplicate desk
  const st = String(job.status || "").toLowerCase();
  const reused = st === "confirmed";
  // 与 splitFillMeta 一致：计划名只在顶栏，不在 h3 再叠一遍
  const titleEl = $("#confirm-title");
  if (titleEl) {
    titleEl.textContent = reused
      ? "历史拆分（可再次执行规划）"
      : "拆分结果";
  }
  const waves = $("#confirm-waves");
  if (waves && !waves.dataset.ccoAwaitSplit) {
    waves.dataset.ccoAwaitSplit = "1";
    waves.innerHTML =
      "<p class='muted'>拆分台加载中…若长时间空白请刷新窗口</p>";
  }
  stampSplitDeskProject(state.selectedPath);
  host.updateSplitPlanChip();
  toggleWaveBackLink(
    job.plan_path || job.planPath || state.selectedPlan || null,
    renderConfirmPanel
  );
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
  // C2：用户取消规划 / 拆分失败后「回到计划」— 清 session，不进历史执行台
  host.clearPlanSession(state.selectedPath);
  state.phase = "pick";
  setBoundPlanJob(null, { projectPath: state.selectedPath });
  try {
    clearSplitUiBinding({ scrubState: false });
  } catch (_) {}
  host.renderPhasePanels();
  host.renderPlanPicker();
  host.updateBgPlanBanner();
  // 清完后若仍有历史 live，留给项目档案；本轮 phase=pick 不自动 goResult
  try {
    if (typeof host.renderWorkspace === "function") host.renderWorkspace();
  } catch (_) {}
}

/**
 * Confirm-screen re-split: keep current plan path and start a fresh plan job
 * (one click — no need to re-pick the file). Falls back to chooser if no plan.
 * P2-2: pass preserve_from_job_id so human title/prompt/deps/deletes re-apply.
 *
 * UX: do **not** clear the desk to phase=pick before analyze — that only
 * flashed an empty screen. Keep split/planning chrome until the new job lands.
 */
export async function replanFromConfirm() {
  if (hasActiveRun()) {
    toast(
      "本轮还在执行，请先停止运行，再重新规划（否则会改到正在跑的任务图）"
    );
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

  if (!state.selectedPlan || !state.selectedPath) {
    host.openPlanChooser(true);
    toast("请选择计划后再次拆分");
    return;
  }

  // P2-2: remember current job so the next start_plan_job can re-apply edits.
  // Prefer the job on the desk (human edits). If desk is thin direct/raw-single
  // but a multi-step split exists for this plan, backend restore ranking still
  // prefers multi-step on fail; preserve still copies edits from the desk job.
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

  // Soft reset only — leave cards visible until analyze swaps in the new job.
  host.stopPlanJobPoll();
  state.confirmEditing = false;
  state.returnPhaseAfterConfirm = null;
  // analyzePlanFromPicker sets phase=planning and replaces planJob.

  let revisionPreview = "";
  try {
    const el = $("#split-revision-notes");
    revisionPreview = el && String(el.value || "").trim();
  } catch (_) {
    revisionPreview = "";
  }
  toast(
    revisionPreview
      ? `按反馈重新规划：${revisionPreview.slice(0, 40)}${
          revisionPreview.length > 40 ? "…" : ""
        }`
      : modeHint
        ? `按当前计划重新规划（保留人工修改 · 上次：${modeHint}）…`
        : preserveFrom
          ? "按当前计划重新规划（保留人工修改）…"
          : "按当前计划重新规划…"
  );
  // Same entry as「开始拆分」— keeps chooser options (并发 / 通道)
  // Pass explicit path so replan cannot pick up a stale selectedPlan.
  // revision_notes is read again inside analyzePlanFromPicker → startPlanJob.
  if (typeof host.analyzePlanFromPicker === "function") {
    await host.analyzePlanFromPicker(state.selectedPlan);
  } else {
    host.openPlanChooser(true);
  }
  // Clear on successful start only (jobPoll); keep text if analyze failed.
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
    toast("已开启「拆分后附加：任务巡检」· 正在按当前计划重新规划…");
    if (typeof replanFromConfirm === "function") {
      await replanFromConfirm();
    }
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "开启巡检并重新规划";
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
    toast("已开启「智能第二跳校对」· 正在按当前计划重新规划…");
    if (typeof replanFromConfirm === "function") {
      await replanFromConfirm();
    }
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "开启智能校对并重新规划";
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
      setBoundPlanJob(view, {
        projectPath: state.selectedPath,
        allowMissingProjectField: true,
        keepConfirmTask: true,
      });
      host.stashPlanSession(state.selectedPath);
      rebindSplitToOpenProject();
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
