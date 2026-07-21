/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: phase panels · flow strips · chips · top title
 * [POS]: A5-2b-fin features/project/shellChrome.js
 * note: phase panels · flow strips · chips · top title
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

export function applyFlowModeBadge(rowId, badgeId, hintId, mode) {
  const row = $(rowId);
  const badge = $(badgeId);
  const hint = $(hintId);
  const label =
    typeof flowModeLabel === "function" ? flowModeLabel(mode) : "";
  if (!row || !badge) return;
  if (!label) {
    row.hidden = true;
    return;
  }
  row.hidden = false;
  badge.textContent = label;
  badge.className = `flow-mode-badge is-${String(mode || "").toLowerCase() || "mixed"}`;
  if (hint) {
    hint.textContent =
      typeof flowModeHint === "function" ? flowModeHint(mode) : "";
  }
}

/** Map app state → flow strip phase for F1 global bar. */
export function resolveFlowPhaseForStrip() {
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    const liveSt = String(state.live?.run_status || "").toLowerCase();
    if (["failed", "aborted", "error"].includes(liveSt)) return "fail";
    return "running";
  }
  if (state.phase === "planning") return "planning";
  if (state.phase === "confirm") return "confirm";
  if (state.phase === "done") return "done";
  if (state.phase === "running") return "running";
  const liveSt = String(state.live?.run_status || "").toLowerCase();
  if (["completed", "done", "success"].includes(liveSt)) return "done";
  if (["failed", "aborted", "error"].includes(liveSt)) return "fail";
  // 写计划：chat / plans / welcome / pick
  if (
    state.page === "chat" ||
    state.page === "plans" ||
    state.page === "welcome" ||
    state.phase === "pick" ||
    !state.phase
  ) {
    return "idle";
  }
  return state.phase || "idle";
}

export function refreshFlowStrips(phaseOverride) {
  if (typeof flowStageStripHtml !== "function") return;
  const ph = phaseOverride || state.phase;
  const globalPh =
    phaseOverride != null ? phaseOverride : resolveFlowPhaseForStrip();
  const hostGlobal = $("#flow-strip-global");
  const hasGlobal = !!hostGlobal;
  if (hostGlobal) {
    // 顶栏：唯一完整阶段点（写计划→拆分→执行→结果）
    hostGlobal.innerHTML = flowStageStripHtml(globalPh, { compact: true });
    hostGlobal.hidden = false;
  }
  // 页内条：有全局条时只留副文案（lineOnly），禁止再画一遍阶段点
  const pageOpts = hasGlobal
    ? { lineOnly: true }
    : { compact: false };
  const hostPlan = $("#flow-strip-planning");
  const hostConfirm = $("#flow-strip-confirm");
  const hostRun = $("#flow-strip-running");
  if (hostPlan) {
    if (ph === "planning") {
      const html = flowStageStripHtml("planning", pageOpts);
      hostPlan.innerHTML = html;
      hostPlan.hidden = !html;
    } else {
      hostPlan.hidden = true;
    }
  }
  if (hostConfirm) {
    if (ph === "confirm") {
      const html = flowStageStripHtml("confirm", pageOpts);
      hostConfirm.innerHTML = html;
      hostConfirm.hidden = !html;
    } else {
      hostConfirm.hidden = true;
    }
  }
  if (hostRun) {
    const runActive =
      ph === "running" ||
      (typeof hasActiveRun === "function" && hasActiveRun());
    if (runActive && state.page === "workspace") {
      const liveSt = String(state.live?.run_status || "").toLowerCase();
      const done =
        ["completed", "done", "success"].includes(liveSt) ||
        (state.live && !runActive);
      const fail = ["failed", "aborted", "error"].includes(liveSt);
      const html = flowStageStripHtml(
        fail ? "fail" : done ? "done" : "running",
        pageOpts
      );
      hostRun.innerHTML = html;
      hostRun.hidden = !html;
    } else {
      hostRun.hidden = true;
    }
  }
  const mode =
    state.planJob?.digest_mode ||
    state.planJob?.digestMode ||
    null;
  applyFlowModeBadge(
    "#planning-mode-row",
    "#planning-mode-badge",
    "#planning-mode-hint",
    mode
  );
  applyFlowModeBadge(
    "#confirm-mode-row",
    "#confirm-mode-badge",
    "#confirm-mode-hint",
    mode
  );
}

export function renderPhasePanels() {
  const planning = $("#plan-phase-planning");
  const confirm = $("#plan-phase-confirm");
  if (!planning || !confirm) return;

  const ph = state.phase;
  planning.hidden = ph !== "planning";
  confirm.hidden = ph !== "confirm";

  try {
    refreshFlowStrips(ph);
  } catch (_) {}

  if (ph === "planning") {
    if (state.planJob) {
      fillPlannerLog(state.planJob);
    } else {
      const log = $("#planner-log");
      if (log && !log.dataset.sig) {
        log.innerHTML = '<div class="cli-empty-ai muted">正在理解计划并拆分步骤…</div>';
      }
    }
  }
  if (ph === "confirm") {
    host.renderConfirmPanel();
  }
  try { host.updateBgPlanBanner(); } catch (_) {}
}

export function renderWorkspaceShell() {
  const body = $("#workspace-body");
  if (!body) return;
  body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
  if (state.phase === "planning" || state.phase === "confirm") body.classList.add("mode-plan");
  else if (isLiveStatus(state.live?.run_status)) body.classList.add("mode-running");
  else if (state.phase === "done") body.classList.add("mode-done");
  else body.classList.add("mode-idle");
}

export function setPlanCollapsed(collapsed) {
  // 新 UX：计划区永远紧凑；collapsed 语义保留给兼容
  state.planCollapsed = true;
  const pp = $("#plan-picker");
  if (pp) pp.classList.add("compact", "collapsed");
}

/** Top-bar summary of the latest split plan (right of 执行此计划). */
export function updateSplitPlanChip() {
  const chip = $("#split-plan-chip");
  if (!chip) return;
  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const job = state.planJob;
  const st = String(job?.status || "").toLowerCase();
  // 已在拆分台时不显示 chip（标题/meta 已在页内；chip 只作他页回跳）
  const onConfirmDesk = state.phase === "confirm";
  const show =
    inWorkspace &&
    !onConfirmDesk &&
    job &&
    (state.phase === "running" ||
      state.phase === "done" ||
      st === "planned" ||
      st === "confirmed");
  chip.hidden = !show;
  if (!show) return;
  const name = job.plan_name || planDisplayName(job.plan_path) || "已拆分";
  const n = job.task_count || job.tasks?.length || 0;
  const waves = (job.layers || []).length;
  const mp = job.max_parallel ?? job.maxParallel ?? "—";
  const layers = job.layers || [];
  const widest = layers.reduce((m, l) => Math.max(m, (l || []).length), 0);
  const runHint = hasActiveRun() ? " · 运行中" : "";
  const capHint =
    typeof mp === "number" && widest > 0 && widest < mp
      ? ` · 最宽波 ${widest}`
      : "";
  $("#split-plan-chip-name").textContent = name;
  $("#split-plan-chip-meta").textContent = `${n} 任务 · 并发上限 ${mp}${capHint} · ${waves || "—"} 波${runHint}`;
  chip.title = hasActiveRun()
    ? "查看拆分结果（运行中只读；停止后可编辑/重拆）"
    : "点击查看/编辑拆分结果";
  updateBudgetChip();
}

/** P1-5: 顶栏「规划 $x · 执行 $y」简版 */
export function updateBudgetChip() {
  const chip = $("#budget-chip");
  const text = $("#budget-chip-text");
  if (!chip || !text) return;
  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const live = state.live;
  const job = state.planJob;
  const liveOk =
    !state.planJobId ||
    (typeof host.liveBelongsToOpenPlan === "function"
      ? host.liveBelongsToOpenPlan()
      : true);
  const planCost =
    liveOk && live?.planner_cost_usd != null
      ? Number(live.planner_cost_usd)
      : job?.planner_cost_usd != null
        ? Number(job.planner_cost_usd)
        : null;
  const execCost = !liveOk
    ? null
    : live?.exec_cost_usd != null
      ? Number(live.exec_cost_usd)
      : live?.tasks
        ? live.tasks.reduce((s, t) => s + (t.cost_usd != null ? Number(t.cost_usd) : 0), 0)
        : null;
  const hasPlan = planCost != null && !Number.isNaN(planCost);
  const hasExec =
    execCost != null && !Number.isNaN(execCost) && (execCost > 0 || (live?.tasks || []).some((t) => t.cost_usd != null));
  const show = inWorkspace && (hasPlan || hasExec);
  chip.hidden = !show;
  if (!show) return;
  const fmt = (n) => `$${Number(n).toFixed(2)}`;
  const bits = [];
  bits.push(`规划 ${hasPlan ? fmt(planCost) : "—"}`);
  bits.push(`执行 ${hasExec ? fmt(execCost) : "—"}`);
  text.textContent = bits.join(" · ");
  chip.title = "规划成本（AI 拆分）与执行成本（worker）分栏";
}

export function updateTopPlanInfo() {
  // 红框1：顶栏只显示计划名，不显示路径
  const title = $("#page-title");
  const sub = $("#page-sub");
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  // 打开拆分会话时以 job 计划为准，勿被项目历史 live.plan_path 顶替
  const jobPlan = state.planJob?.plan_path || state.planJob?.planPath || null;
  const allowLivePlan =
    !state.planJobId ||
    (typeof host.liveBelongsToOpenPlan === "function"
      ? host.liveBelongsToOpenPlan()
      : true);
  let plan =
    state.selectedPlan ||
    normalizePlanPath(jobPlan) ||
    (allowLivePlan ? normalizePlanPath(state.live?.plan_path) : null) ||
    normalizePlanPath(proj?.default_plan) ||
    normalizePlanPath(proj?.last_plan) ||
    null;
  if (plan && !state.selectedPlan) state.selectedPlan = plan;

  if (state.page === "workspace" && state.selectedPath) {
    const name =
      (state.planPreview && state.planPreview.name) ||
      (plan ? planDisplayName(plan) : "未选择计划");
    if (title) {
      title.textContent = name;
      title.title = plan || "";
    }
    if (sub) {
      sub.textContent = "";
      sub.title = plan || "";
      sub.hidden = true;
    }
  } else if (sub) {
    sub.hidden = false;
  }

  const btnAssign = $("#btn-pp-analyze");
  if (btnAssign && state.page === "workspace") {
    const active = isLiveStatus(state.live?.run_status);
    btnAssign.disabled = !!active;
  }

  const nameEl = $("#top-plan-name");
  const pathEl = $("#top-plan-path");
  const box = $("#top-plan-info");
  if (box) box.hidden = true;
  if (nameEl) nameEl.textContent = plan ? planDisplayName(plan) : "";
  if (pathEl) pathEl.textContent = "";
}

export function renderPlanPreview() {
  // 紧凑模式不再展示大预览；保留函数避免旧调用报错
  return;
}
