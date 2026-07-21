/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: start_plan_job · poll · optional gate · advance (no silent auto-start past optionals)
 * [POS]: A5-2b-fin features/project/jobPoll.js
 * note: start_plan_job · poll · optional gate · advance (no silent auto-start past optionals)
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

/** Mode B: analyze plan → plan job (does NOT start workers). */
/** 拆成步骤：AI 拆分后进入拆分台（可编辑）；入口文案统一为「拆成步骤」 */
export async function analyzePlanFromPicker() {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (state.assigning) return;
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  if (!state.selectedPlan) {
    host.openPlanChooser(true);
    toast("请先选择计划");
    return;
  }
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }

  // C6: default fast local split — avoid multi-minute Claude CLI planning spin.
  const planMode = $("#pp-plan-mode")?.value || "fast";
  const provider = $("#pp-provider")?.value || "claude";
  const mode = $("#pp-mode")?.value || "print";
  // Commit any in-progress concurrency edit before reading.
  const maxParallel = host.commitSplitMaxParallel($("#chooser-max-parallel") || $("#pp-max-parallel"));

  const doc = await host.ensureDoctor(true);
  if (doc && !doc.ok && provider !== "fake" && planMode !== "fake") {
    // 不硬拦死：提示 + 允许用户忽略后重试；首次仍阻止避免必败
    if (err) {
      err.textContent = "环境未就绪。可点上方「忽略」后重试，或到环境检查配置 Claude 路径";
      err.hidden = false;
    }
    host.renderDoctorWarn();
    // 若用户已忽略同类警告，允许继续
    const fails = (doc.lines || []).filter((l) => !l.ok);
    const key = fails.map((l) => l.name + ":" + l.detail).join("|");
    if (!(state.doctorDismissedKey && state.doctorDismissedKey === key)) {
      return;
    }
  }

  host.setAssignBusy(true);
  state.phase = "planning";
  state.planJob = null;
  state.planJobId = null;
  state.confirmEditing = false;
  host.clearPlanSession(state.selectedPath);
  stopPlanJobPoll();
  host.openPlanChooser(false);
  // 规划 UI 在 workspace；从聊天/其它页分配时先切回
  if (state.page !== "workspace") showPage("workspace");
  host.renderPhasePanels();
  host.renderPlanPicker();
  host.renderWorkspaceShell();
  const logEl0 = $("#planner-log");
  if (logEl0) {
    logEl0.dataset.sig = "";
    logEl0.innerHTML =
      '<div class="cli-empty-ai muted">正在理解计划并拆分步骤…</div>';
  }
  const sub0 = $("#planning-sub");
  if (sub0) {
    const name = planDisplayName(state.selectedPlan);
    sub0.textContent =
      typeof flowJoinSeriousFun === "function"
        ? flowJoinSeriousFun(
            `正在拆分「${name}」…（同时最多 ${maxParallel} 步）`,
            typeof flowPickBlurb === "function" ? flowPickBlurb("planning", name) : ""
          )
        : `正在拆分「${name}」…（同时最多 ${maxParallel} 步）`;
  }

  try {
    const preserveFrom = state.preserveFromJobId || null;
    // One-shot: clear so a later fresh assign doesn't accidentally inherit.
    state.preserveFromJobId = null;
    const view = await requireGateway().startPlanJob({
      req: {
        project: state.selectedPath,
        plan: state.selectedPlan,
        plan_mode: planMode,
        provider,
        mode,
        max_parallel: maxParallel,
        // P2-2: re-apply confirm-screen edits from previous job (by title).
        preserve_from_job_id: preserveFrom || null,
      },
    });
    state.planJob = view;
    // Tauri/serde 字段兼容
    state.planJobId = view.job_id || view.jobId || null;
    state.planStartedAt = Date.now();
    state.planPollFails = 0;
    host.stashPlanSession(state.selectedPath);
    fillPlannerLog(view);

    const status = String(view.status || "").toLowerCase();
    if (status === "planned") {
      await advancePlannedJob(view);
    } else if (status === "plan_failed") {
      state.phase = "pick";
      if (err) {
        err.textContent = view.error || "规划失败";
        err.hidden = false;
      }
      toast(view.error || "规划失败");
      host.renderPhasePanels();
      host.renderPlanPicker();
      host.setAssignBusy(false);
    } else {
      // async AI planning — keep busy + poll until planned/failed
      state.phase = "planning";
      host.renderPhasePanels();
      startPlanJobPoll();
      // 立即拉一次，避免只显示 started 第一行就干等
      await refreshPlanJob();
    }
  } catch (e) {
    state.phase = "pick";
    if (err) {
      err.textContent = String(e);
      err.hidden = false;
    }
    toast(String(e));
    host.renderPhasePanels();
    host.renderPlanPicker();
    host.setAssignBusy(false);
  }
}

export function stopPlanJobPoll() {
  if (state.planJobPollTimer) {
    clearInterval(state.planJobPollTimer);
    state.planJobPollTimer = null;
  }
}

export function startPlanJobPoll() {
  stopPlanJobPoll();
  state.planJobPollTimer = setInterval(() => {
    refreshPlanJob().catch((e) => console.warn("plan poll", e));
  }, 600);
}

export function planHasOptionalTasks(view) {
  const tasks = view?.tasks || [];
  return tasks.some((t) => !!t.optional);
}

export function isSystemPostTask(t) {
  if (!t) return false;
  const id = String(t.id || "");
  if (
    id === "sys-post-inspect" ||
    id === "sys-post-git-push" ||
    id === "sys-post-open-pr"
  )
    return true;
  if (id.startsWith("sys-post-")) return true;
  return String(t.group || "") === "系统收尾";
}

export function countOptionalIncluded(view) {
  const tasks = view?.tasks || [];
  return tasks.filter((t) => t.optional && t.include !== false).length;
}

/**
 * Whether confirm screen must wait for human before auto-start.
 * - Business optionals (非系统): always block（默认不跑，须人勾选）
 * - System post only（设置开启、默认勾选）: 全部 include 则可 auto-start
 */
export function planNeedsOptionalConfirm(view) {
  const tasks = view?.tasks || [];
  const businessOpt = tasks.filter((t) => !!t.optional && !isSystemPostTask(t));
  if (businessOpt.length > 0) return true;
  const sysOpt = tasks.filter((t) => !!t.optional && isSystemPostTask(t));
  if (!sysOpt.length) return false;
  // 系统收尾有未勾选 → 仍停一下让用户看到；全勾选则不挡 auto-start
  return sysOpt.some((t) => t.include === false);
}

export async function advancePlannedJob(view) {
  stopPlanJobPoll();
  state.planJob = view;
  if (!state.confirmTaskId && view.tasks?.length) {
    state.confirmTaskId = view.tasks[0].id;
  }
  host.stashPlanSession(state.selectedPath);
  host.updateBgPlanBanner();
  // E2：拆分完成必须回到执行面，禁止只 toast「请返回确认」而人还在 chat/plans
  if (state.page !== "workspace") {
    showPage("workspace");
  }
  const n = view.task_count || view.tasks?.length || 0;
  const adapter = view.adapter || "";
  const how =
    typeof flowPlanHowLabel === "function"
      ? flowPlanHowLabel(adapter)
      : adapter.includes("heuristic")
        ? "本地规则拆分"
        : adapter.includes("llm")
          ? "智能拆分"
          : "拆分完成";
  // 业务可选：必须人工确认。系统收尾默认勾选时可 auto-start。
  const needsOpt = planNeedsOptionalConfirm(view);
  const hasOptional = planHasOptionalTasks(view);
  if (state.autoStartAfterPlan && !needsOpt) {
    toast(`${how}：${n} 个任务，正在启动…`);
    state.phase = "confirm";
    try {
      if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
        window.ccoApp.goSplit();
      }
    } catch (_) {}
    host.renderPhasePanels();
    host.renderPlanPicker();
    host.setAssignBusy(false);
    await host.confirmAndStart();
  } else {
    const optHint = needsOpt
      ? "；含可选项，请确认勾选后再开始"
      : hasOptional
        ? "；含系统收尾（默认已勾选）"
        : "，请确认后开始";
    toast(`${how}：${n} 个任务${optHint}`);
    state.phase = "confirm";
    try {
      if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
        window.ccoApp.goSplit();
      }
    } catch (_) {}
    host.renderPhasePanels();
    host.renderPlanPicker();
    host.setAssignBusy(false);
  }
}

export async function refreshPlanJob() {
  if (!state.planJobId) return;
  try {
    const view = await requireGateway().getPlanJob(state.planJobId);
    state.planPollFails = 0;
    state.planJob = view;
    const status = String(view.status || "").toLowerCase();
    fillPlannerLog(view);

    if (status === "planned") {
      await advancePlannedJob(view);
    } else if (status === "plan_failed") {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      state.phase = "pick";
      const err = $("#pp-error");
      if (err) {
        err.textContent = view.error || "规划失败";
        err.hidden = false;
      }
      toast(view.error || "规划失败");
      host.renderPhasePanels();
      host.renderPlanPicker();
    } else if (status === "planning") {
      state.phase = "planning";
      // 超时保护：超过 12 分钟仍 planning
      if (state.planStartedAt && Date.now() - state.planStartedAt > 12 * 60 * 1000) {
        stopPlanJobPoll();
        host.setAssignBusy(false);
        state.phase = "pick";
        toast("拆分超时：智能拆分可能无响应。请检查环境，或在更多选项里改用「模拟拆分」。");
        host.renderPhasePanels();
        host.renderPlanPicker();
        return;
      }
      const sub = $("#planning-sub");
      if (sub) {
        const elapsed = state.planStartedAt
          ? Math.round((Date.now() - state.planStartedAt) / 1000)
          : 0;
        sub.textContent =
          typeof flowPlanningSub === "function"
            ? flowPlanningSub(elapsed)
            : `正在拆分计划步骤（已等待 ${elapsed}s）…`;
      }
      host.renderPhasePanels();
    } else if (status === "confirmed" && (view.run_id || view.runId)) {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      state.phase = "running";
      host.renderPhasePanels();
    } else {
      host.renderPhasePanels();
    }
  } catch (e) {
    state.planPollFails = (state.planPollFails || 0) + 1;
    console.warn("refreshPlanJob", e);
    if (state.planPollFails === 1 || state.planPollFails % 5 === 0) {
      toast(`规划状态刷新失败：${e}`);
    }
    // 5 次失败后尝试读本地日志提示
    if (state.planPollFails >= 8) {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      state.phase = "pick";
      toast("无法轮询规划任务。请点刷新重试，或用 CLI：cco plan --project ...");
      host.renderPhasePanels();
      host.renderPlanPicker();
    }
  }
}
