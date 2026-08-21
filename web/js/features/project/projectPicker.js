/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: assign busy · execute · chooser · renderPlanPicker · max parallel partial
 * [POS]: A5-2b-fin features/project/projectPicker.js
 * note: assign busy · execute · chooser · renderPlanPicker；系统页隐藏业务顶栏 CTA
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
import { setBoundPlanJob, clearSplitUiBinding } from "./projectScope.js";

export function defaultAssignLabel(btnId) {
  // S10/F2：主 CTA 统一「拆成步骤」；选项层确认后也是同一动作
  if (btnId === "btn-chooser-assign") return "拆成步骤";
  return "拆成步骤";
}

export function setAssignBusy(busy) {
  state.assigning = !!busy;
  const ids = ["btn-chooser-assign", "btn-pp-analyze", "btn-plans-assign", "btn-chat-assign"];
  for (const id of ids) {
    const btn = document.getElementById(id);
    if (!btn) continue;
    if (busy) {
      btn.disabled = true;
      btn.classList.add("is-busy");
      if (!btn.dataset.label) btn.dataset.label = btn.textContent || defaultAssignLabel(id);
      btn.innerHTML = '<span class="spinner sm" aria-hidden="true"></span><span>拆分中…</span>';
    } else {
      btn.classList.remove("is-busy");
      const active = isLiveStatus(state.live?.run_status);
      const label = btn.dataset.label || defaultAssignLabel(id);
      btn.textContent = active ? "运行中…" : label;
      delete btn.dataset.label;
      if (btn.id === "btn-chooser-assign") {
        btn.disabled = !state.selectedPlan || !!active;
      } else if (btn.id === "btn-chat-assign") {
        btn.disabled = !state.chatDraftPlan || !!active;
      } else if (btn.id === "btn-plans-assign") {
        btn.disabled = !state.selectedPlan || !!active;
      } else {
        btn.disabled = !!active;
      }
    }
  }
  // Plan-card CTAs: busy/live only — never require chatDraftPlan (unsaved
  // fences clear that path on purpose; card body still saves on click).
  document
    .querySelectorAll(".btn-chat-plan-assign, .btn-chat-plan-direct")
    .forEach((btn) => {
      const def = btn.classList.contains("btn-chat-plan-direct")
        ? "直接执行"
        : "拆成步骤";
      if (busy) {
        btn.disabled = true;
        btn.classList.add("is-busy");
        if (!btn.dataset.label) btn.dataset.label = btn.textContent || def;
        btn.innerHTML =
          '<span class="spinner sm" aria-hidden="true"></span><span>处理中…</span>';
        return;
      }
      btn.classList.remove("is-busy");
      const live = isLiveStatus(state.live?.run_status);
      btn.textContent = live ? "运行中…" : btn.dataset.label || def;
      delete btn.dataset.label;
      btn.disabled = !!live;
    });
}

/**
 * E1 统一执行入口：带走已选计划 → workspace → 执行选项（薄层仍用 plan-chooser，列表可换文件）。
 * 管理页 / 聊天就绪条 / 全文 modal 共用，避免「再选一遍同一文件」。
 */
export async function startExecuteFromSelection(planPath, opts = {}) {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  const raw =
    planPath ||
    state.selectedPlan ||
    state.chatDraftPlan ||
    null;
  const path =
    (typeof normalizePlanPath === "function"
      ? normalizePlanPath(raw, state.selectedPath)
      : null) || raw;
  if (!path) {
    toast("请先选中一份计划");
    if (state.page !== "plans") {
      openPlanChooser(true);
      try {
        await host.loadPlansForPicker();
        renderPlanChooser();
        updateChooserAssignState();
      } catch (_) {}
    }
    return;
  }
  if (opts.fakeNote || state.chatFake) {
    toast("注意：当前计划来自本地模板（非真实小叶），确认后仍将进入执行");
  }

  // Explicit path from chat / full-view / plans: this is the plan to split.
  // If a prior planning/confirm session still holds another plan, tear it down
  // so selectPlan cannot refuse the switch (toast said chat-*.md while job
  // kept pilotdeck).
  const prev = state.selectedPlan;
  const prevKey =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(prev, state.selectedPath) || prev
      : prev;
  const nextKey =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(path, state.selectedPath) || path
      : path;
  const switching = !!(prevKey && nextKey && prevKey !== nextKey);
  const forceSwitch =
    !!opts.force ||
    opts.source === "chat" ||
    opts.source === "chat-direct" ||
    opts.source === "full-view" ||
    opts.source === "plans" ||
    switching;
  if (forceSwitch && (host.isPlanSessionActive?.() || state.planJobId)) {
    try {
      host.stopPlanJobPoll?.();
    } catch (_) {}
    try {
      host.clearPlanSession(state.selectedPath);
    } catch (_) {}
    setBoundPlanJob(null, { projectPath: state.selectedPath });
    state.phase = "pick";
    state.confirmEditing = false;
    try {
      clearSplitUiBinding({ scrubState: false });
    } catch (_) {}
    try {
      host.setAssignBusy?.(false);
    } catch (_) {}
  }

  state.chatDraftPlan = path;
  state.selectedPlan = path;
  if (typeof selectPlanRailItem === "function") {
    try {
      selectPlanRailItem(path);
    } catch (_) {}
  }
  try {
    await host.selectPlan(path, { force: forceSwitch });
  } catch (e) {
    // Drop one-shot direct flags so a failed select cannot auto-start later.
    state.forcePlanModeDirect = false;
    state.forceAutoStartAfterPlan = false;
    toast(String(e?.message || e));
    return;
  }
  // selectPlan may no-op under race; re-assert identity.
  state.selectedPlan = path;
  state.chatDraftPlan = path;

  // A2：默认跳过选项层直开拆分（仍走 analyze → start_plan_job → confirm_start；禁止 start_run）。
  const direct =
    opts.direct === true ||
    (opts.direct !== false &&
      typeof chatAssignDirectEnabled === "function" &&
      chatAssignDirectEnabled());
  if (direct && typeof host.analyzePlanFromPicker === "function") {
    if (state.page !== "workspace") showPage("workspace");
    openPlanChooser(false);
    renderPlanPicker();
    const name =
      typeof planDisplayName === "function" ? planDisplayName(path) : path;
    const isDirectExec =
      !!state.forcePlanModeDirect || opts.source === "chat-direct";
    toast(
      isDirectExec
        ? `正在准备直接执行…「${name}」`
        : `正在拆成步骤…「${name}」`
    );
    // Pass path explicitly — never re-read a possibly stale selectedPlan alone.
    await host.analyzePlanFromPicker(path);
    return;
  }
  // 设置「先确认选项」或 opts.direct===false：打开选项层
  if (state.page !== "workspace") showPage("workspace");
  openPlanChooser(true, { fromExecute: true, expandList: false });
  try {
    await host.loadPlansForPicker();
  } catch (_) {}
  renderPlanChooser();
  updateChooserAssignState();
  renderPlanPicker();
  const name =
    typeof planDisplayName === "function" ? planDisplayName(path) : path;
  toast(`将拆分：${name} · 确认选项后点「拆成步骤」`);
}

export function openPlanChooser(open = true, opts = {}) {
  if (open && hasActiveRun()) {
    toastRunLocked("切换/选择计划");
    return;
  }
  state.planChooserOpen = open;
  // E1：从「执行此计划」进来默认折叠列表；从「选择计划」进来展开
  if (open) {
    if (opts.expandList != null) {
      state.chooserListExpanded = !!opts.expandList;
    } else if (opts.fromExecute && state.selectedPlan) {
      state.chooserListExpanded = false;
    } else if (state.chooserListExpanded == null) {
      state.chooserListExpanded = !state.selectedPlan;
    }
  }
  const sheet = $("#plan-chooser");
  if (!sheet) return;
  sheet.hidden = !open;
  if (open) {
    renderPlanChooser();
    updateChooserAssignState();
  }
}

export function setChooserListExpanded(expanded) {
  state.chooserListExpanded = !!expanded;
  if (state.planChooserOpen) renderPlanChooser();
}

export function updateChooserAssignState() {
  const btn = $("#btn-chooser-assign");
  const label = $("#chooser-selected-label");
  const active = isLiveStatus(state.live?.run_status);
  const plan = state.selectedPlan;
  if (label) {
    label.textContent = plan ? `将执行：${planDisplayName(plan)}` : "未选择计划";
    label.title = plan || "";
  }
  if (btn && !state.assigning) {
    btn.disabled = !plan || !!active;
    btn.textContent = active ? "运行中…" : "拆成步骤";
  }
}

export function renderPlanChooser() {
  const list = $("#chooser-list");
  const empty = $("#chooser-empty");
  const toggle = $("#btn-chooser-toggle-list");
  const sub = $("#chooser-sub");
  if (!list) return;
  host.syncShowExecutedToggles();

  const hasSelected = !!state.selectedPlan;
  const expanded = state.chooserListExpanded != null
    ? !!state.chooserListExpanded
    : !hasSelected;

  if (toggle) {
    toggle.hidden = !hasSelected;
    toggle.textContent = expanded ? "收起列表" : "换一份计划…";
  }
  if (sub) {
    if (typeof flowChooserSub === "function") {
      sub.textContent = flowChooserSub(hasSelected);
    } else {
      sub.textContent = hasSelected
        ? "已选计划 · 确认同时进行几步后点「拆成步骤」"
        : "确认同时进行几步后点「拆成步骤」；可换一份计划";
    }
  }

  // E1 薄层：已有选中且未展开 → 不铺全量列表
  if (hasSelected && !expanded) {
    if (empty) empty.hidden = true;
    list.innerHTML = "";
    list.hidden = true;
    updateChooserAssignState();
    return;
  }
  list.hidden = false;

  if (state.plansLoading) {
    if (empty) empty.hidden = true;
    list.innerHTML =
      '<div class="chooser-loading"><span class="spinner sm" aria-hidden="true"></span>正在扫描计划…</div>';
    updateChooserAssignState();
    return;
  }
  if (!state.plans.length) {
    if (empty) empty.hidden = false;
    list.innerHTML = "";
    updateChooserAssignState();
    return;
  }
  if (empty) empty.hidden = true;

  // H2: build meta-backed rows; pin selected + draft so they always show
  const root = state.selectedPath;
  const pinPaths = [
    state.selectedPlan,
    state.chatDraftPlan,
    state.planFull?.path,
  ]
    .filter(Boolean)
    .map((p) => normalizePlanPath(p, root) || p);

  const items = state.plans.map((p) => {
    const path = normalizePlanPath(p, root) || p;
    const meta = host.planMetaForPath(path, root);
    return {
      ...meta,
      path,
      title: meta.title || planDisplayName(path),
    };
  });
  // Ensure pin-only paths (manual pick) appear even if filtered later
  for (const pin of pinPaths) {
    if (pin && !items.some((it) => it.path === pin)) {
      const meta = host.planMetaForPath(pin, root);
      items.unshift({
        ...meta,
        path: pin,
        title: meta.title || planDisplayName(pin),
      });
    }
  }

  const parts = host.partitionPlanItems(items, {
    showExecuted: !!state.showExecutedPlans,
    pinPaths,
  });

  const rows = [];
  for (const it of parts.visible) {
    const path = it.path;
    const selected = path === state.selectedPlan;
    const title = it.title || planDisplayName(path);
    const badge = host.planExecBadgeInfo(it);
    rows.push(
      `<button type="button" class="plan-item${selected ? " selected" : ""}" data-plan="${esc(path)}">` +
        `<div class="plan-item-title-row">` +
        `<div class="plan-item-title">${esc(title)}</div>` +
        `<span class="plan-rail-badge ${badge.cls}">${esc(badge.label)}</span>` +
        `</div>` +
        `<div class="plan-item-path">${esc(path)}</div>` +
        `</button>`
    );
  }
  if (parts.historyHidden) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">` +
        `已隐藏 ${parts.historyCount} 份已执行计划 · 勾选上方「显示已执行」可展开` +
        `</div>`
    );
  }

  list.innerHTML = rows.join("");
  updateChooserAssignState();
}

export function renderPlanPicker() {
  const pp = $("#plan-picker");
  const btnChoose = $("#btn-plan-choose");
  const btnAssign = $("#btn-pp-analyze");
  const btnMonitor = $("#btn-monitor-plan");

  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const inChat = !!state.selectedPath && state.page === "chat";
  const inPlans = !!state.selectedPath && state.page === "plans";
  // 系统页：顶栏不展示业务 CTA / 阶段相关入口
  const isSystemPage =
    state.page === "settings" ||
    state.page === "doctor" ||
    state.page === "help";
  // 选择计划：workspace；执行此计划：仅 workspace（聊天/计划管理用各自 CTA）
  const hideForPhase = state.phase === "planning" || state.phase === "confirm";
  const runActive = hasActiveRun();
  const hasSplit =
    !!state.planJob &&
    ["planned", "confirmed"].includes(String(state.planJob.status || "").toLowerCase());

  // 顶栏 icon 已撤：聊天走 view-ring；计划管理页仍可由程序 openPlanManagement；刷新无入口
  // 兼容旧 DOM（若缓存页仍有节点则强制 hidden）
  for (const id of ["btn-open-chat", "btn-plan-mgmt", "btn-refresh"]) {
    const el = document.getElementById(id);
    if (el) el.hidden = true;
  }

  // A4：有待确认且在 chat →「继续核对拆分」；有活动 run →「返回执行」；终态 →「查看结果」
  // 系统页不展示（设置/环境检查/帮助与执行态无关）
  if (btnMonitor) {
    const jobSt = String(state.planJob?.status || "").toLowerCase();
    const jobRunId = state.planJob?.run_id || state.planJob?.runId || null;
    // confirmed 已 spawn run 的不算「待确认新图」——应回执行/结果台
    const pendingSplit =
      hasSplit &&
      !runActive &&
      !isRunPaused() &&
      !jobRunId &&
      (jobSt === "planned" ||
        (jobSt === "confirmed" && !state.live?.run_id) ||
        state.phase === "confirm");
    const finishedLive =
      !!state.live?.run_id &&
      !runActive &&
      !isRunPaused() &&
      (typeof host.liveBelongsToOpenPlan === "function"
        ? host.liveBelongsToOpenPlan()
        : true) &&
      ["completed", "done", "failed", "aborted", "stopped"].includes(
        String(state.live?.run_status || "").toLowerCase()
      );
    const showMon =
      !isSystemPage &&
      !!state.selectedPath &&
      state.page !== "workspace" &&
      state.page !== "welcome" &&
      (host.hasMonitorableActivity() ||
        pendingSplit ||
        finishedLive ||
        host.isPlanSessionActive());
    btnMonitor.hidden = !showMon;
    if (showMon) {
      const urgent = runActive || pendingSplit || state.phase === "planning";
      btnMonitor.classList.toggle("primary", urgent && !inChat);
      // chat 页：待确认时用 ghost 主文案，避免双 primary
      if (inChat && pendingSplit) {
        btnMonitor.classList.remove("primary");
        btnMonitor.classList.add("ghost");
      } else {
        btnMonitor.classList.toggle("ghost", !urgent || inChat);
      }
      if (runActive) {
        btnMonitor.textContent = "返回执行";
        btnMonitor.title =
          typeof flowRunningMonitorTitle === "function"
            ? flowRunningMonitorTitle()
            : "返回工作区查看执行进度";
      } else if (isRunPaused()) {
        btnMonitor.textContent = "返回执行";
        btnMonitor.title = "返回工作区查看已暂停的计划";
      } else if (
        finishedLive ||
        state.phase === "done" ||
        (jobRunId && state.live?.run_id)
      ) {
        // 完成/失败/中止：优先「查看结果」，勿被 confirmed job 文案盖成「继续核对拆分」
        btnMonitor.textContent = "查看结果";
        btnMonitor.title = "返回工作区查看运行结果";
      } else if (pendingSplit || state.phase === "confirm") {
        btnMonitor.textContent = "继续核对拆分";
        btnMonitor.title = "回到拆分台核对后点「执行规划」";
      } else if (host.isPlanSessionActive()) {
        btnMonitor.textContent =
          state.phase === "planning" ? "查看规划" : "继续核对拆分";
        btnMonitor.title =
          state.phase === "planning"
            ? "返回工作区查看拆分进度"
            : "返回工作区确认拆分结果";
      } else {
        btnMonitor.textContent = "查看结果";
        btnMonitor.title = "返回工作区查看运行结果";
      }
    } else {
      btnMonitor.classList.remove("primary");
      btnMonitor.classList.add("ghost");
    }
  }

  // 顶栏「选择计划」/「拆成步骤」：执行/结果台不露出（红框收口）；仅 author 相位 workspace
  const hideTopPlanChrome =
    hideForPhase ||
    state.phase === "running" ||
    state.phase === "done" ||
    !!runActive ||
    !!isLiveStatus(state.live?.run_status);
  if (btnChoose) {
    btnChoose.hidden = isSystemPage || !inWorkspace || hideTopPlanChrome;
    btnChoose.disabled = !!runActive;
    btnChoose.title = runActive ? "运行中，请先停止后再切换计划" : "选择计划";
  }
  // 顶栏主 CTA「拆成步骤」：仅 workspace 且未在拆分/执行中
  if (btnAssign) {
    btnAssign.hidden = isSystemPage || !inWorkspace || hideTopPlanChrome;
    if (!hideTopPlanChrome && !state.assigning) {
      btnAssign.textContent = runActive ? "运行中…" : "拆成步骤";
      btnAssign.title = runActive
        ? "运行中，请先停止后再拆分新计划"
        : "把当前计划拆成可执行步骤";
    }
  }
  try {
    document.body.dataset.ccoPhase = state.phase || "pick";
    document.body.classList.toggle("cco-run-active", !!runActive);
    if (typeof host.refreshFlowStrips === "function") host.refreshFlowStrips();
  } catch (_) {}

  // shell-chrome A3：顶栏「编辑任务」已撤（若旧 DOM 仍在则强制 hidden）
  const btnEditLegacy = $("#btn-edit-plan");
  if (btnEditLegacy) btnEditLegacy.hidden = true;

  // plan-picker 仅用于错误条 / 隐藏钩子
  if (pp) {
    const err = $("#pp-error");
    const hasErr = err && !err.hidden && err.textContent;
    pp.hidden = !inWorkspace || hideForPhase || !hasErr;
    pp.classList.add("headless", "compact");
  }

  // 非 workspace：chat/plans 保留已开的 plan-chooser；其它页关掉（E0 修 plans 误关）
  if (!inWorkspace) {
    if (!inChat && !inPlans && state.planChooserOpen && !runActive) {
      openPlanChooser(false);
    } else if ((inChat || inPlans) && state.planChooserOpen) {
      renderPlanChooser();
      updateChooserAssignState();
    }
    host.updateSplitPlanChip();
    host.updateTopPlanInfo();
    return;
  }

  const active = runActive || isLiveStatus(state.live?.run_status);
  if (btnAssign && !state.assigning) {
    // 弹窗化后无计划也可点开选计划；仅运行中禁用
    btnAssign.disabled = !!active;
    btnAssign.textContent = active ? "运行中…" : "拆成步骤";
    btnAssign.title = active
      ? "运行中，请先停止后再拆分新计划"
      : "打开选项并将计划拆成步骤";
  }
  host.updateSplitPlanChip();

  // S0：高级「拆分后自动开始」；checked = autoStartAfterPlan
  const autoEl = $("#pp-auto-start") || $("#pp-pause-confirm");
  if (autoEl) {
    autoEl.checked = !!state.autoStartAfterPlan;
  }

  try {
    const defP = $("#s-default-provider")?.value;
    if (defP && $("#pp-provider") && !$("#pp-provider").dataset.touched) {
      $("#pp-provider").value = defP;
    }
  } catch (_) {}
  // Only seed concurrency when the user is not mid-edit (never clamp-while-typing).
  const chooserMp = $("#chooser-max-parallel");
  if (
    chooserMp &&
    document.activeElement !== chooserMp &&
    chooserMp.dataset.editing !== "1"
  ) {
    host.syncSplitMaxParallelInputs(null, { force: false });
  }

  if (state.planChooserOpen) renderPlanChooser();
  host.updateSplitPlanChip();
  host.updateTopPlanInfo();
}
