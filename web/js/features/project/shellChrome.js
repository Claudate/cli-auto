/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: phase panels · flow strips · chips · top title
 * [POS]: A5-2b-fin features/project/shellChrome.js
 * note: phase panels · flow strips(no-op) · chips · top title；shell-chrome A1 去阶段条
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

/**
 * shell-chrome A1：顶栏/页内阶段条一律清空并 hidden（no-op 写 strip）。
 * 仍刷新 mode badge（智能/本地规则）。
 */
export function refreshFlowStrips(_phaseOverride) {
  const hostGlobal = $("#flow-strip-global");
  if (hostGlobal) {
    hostGlobal.innerHTML = "";
    hostGlobal.hidden = true;
    hostGlobal.setAttribute("aria-hidden", "true");
  }
  for (const id of [
    "#flow-strip-planning",
    "#flow-strip-confirm",
    "#flow-strip-running",
  ]) {
    const el = $(id);
    if (!el) continue;
    el.innerHTML = "";
    el.hidden = true;
    el.setAttribute("aria-hidden", "true");
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
  const splitFailed = ph === "plan_failed";
  // plan_failed reuses planning shell as a failure surface (no historical run desk).
  planning.hidden = ph !== "planning" && !splitFailed;
  confirm.hidden = ph !== "confirm";

  try {
    refreshFlowStrips(ph);
  } catch (_) {}

  paintPlanningFailSurface(splitFailed);

  if (ph === "planning" || splitFailed) {
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

/** Switch planning panel between spinner and failure (plan_failed). */
function paintPlanningFailSurface(failed) {
  const panel = $("#plan-phase-planning");
  if (!panel) return;
  panel.classList.toggle("is-plan-failed", !!failed);
  const spinner = panel.querySelector(".planning-title-row .spinner");
  const h3 = panel.querySelector(".planning-title-row h3");
  const sub = $("#planning-sub");
  const bar = panel.querySelector(".planning-progress");
  const cancelBtn = $("#btn-cancel-planning");
  const retryBtn = $("#btn-retry-planning");
  const errBox = $("#planning-fail-detail");
  const job = state.planJob;
  const errText = String(job?.error || "").trim();

  if (spinner) spinner.hidden = !!failed;
  if (bar) bar.hidden = !!failed;
  if (h3) {
    h3.textContent = failed
      ? "拆分没有成功"
      : "正在把计划拆成可执行步骤…";
  }
  if (sub) {
    if (failed) {
      if (/hard timeout|301s|did not finish/i.test(errText)) {
        sub.textContent =
          "智能拆分超时未完成。不会进入执行；可再拆一次，或改用本地规则拆分。";
      } else if (errText) {
        sub.textContent = errText.length > 200 ? errText.slice(0, 198) + "…" : errText;
      } else {
        sub.textContent = "拆分失败，不会进入执行台。请再拆一次或换拆分方式。";
      }
    }
    // when !failed, jobPoll keeps updating planning-sub
  }
  if (errBox) {
    if (failed && errText) {
      errBox.hidden = false;
      errBox.textContent = errText;
    } else {
      errBox.hidden = true;
      errBox.textContent = "";
    }
  }
  if (cancelBtn) {
    cancelBtn.textContent = failed ? "回到计划" : "取消回计划";
  }
  if (retryBtn) {
    retryBtn.hidden = !failed;
  }
}

export function renderWorkspaceShell() {
  const body = $("#workspace-body");
  if (!body) return;
  body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
  if (
    state.phase === "planning" ||
    state.phase === "confirm" ||
    state.phase === "plan_failed"
  ) {
    body.classList.add("mode-plan");
  } else if (isLiveStatus(state.live?.run_status)) body.classList.add("mode-running");
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
    ? "查看拆分结果（运行中只读；停止后可重新规划）"
    : "查看拆分结果";
  const labelEl = chip.querySelector(".split-plan-chip-label");
  if (labelEl) labelEl.textContent = "查看拆分结果";
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
