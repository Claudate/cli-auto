/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: assign busy · execute · chooser · renderPlanPicker · max parallel partial
 * [POS]: A5-2b-fin features/project/projectPicker.js
 * note: assign busy · execute · chooser · renderPlanPicker · max parallel partial
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
        btn.disabled = !(state.planRailSelected || state.selectedPlan) || !!active;
      } else {
        btn.disabled = !!active;
      }
    }
  }
  // Dynamic plan-card CTAs (chat reply footer) — same busy lock as sticky assign
  const cardAssigns = document.querySelectorAll(".btn-chat-plan-assign");
  for (const btn of cardAssigns) {
    if (busy) {
      btn.disabled = true;
      btn.classList.add("is-busy");
      if (!btn.dataset.label) btn.dataset.label = btn.textContent || "拆成步骤";
      btn.innerHTML = '<span class="spinner sm" aria-hidden="true"></span><span>拆分中…</span>';
    } else {
      btn.classList.remove("is-busy");
      const active = isLiveStatus(state.live?.run_status);
      const label = btn.dataset.label || "拆成步骤";
      btn.textContent = active ? "运行中…" : label;
      delete btn.dataset.label;
      btn.disabled = !state.chatDraftPlan || !!active;
    }
  }
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
  const path =
    planPath ||
    state.selectedPlan ||
    state.chatDraftPlan ||
    state.planRailSelected ||
    null;
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
    toast("注意：当前计划来自本地模板（非真实 AI），确认后仍将进入执行");
  }
  state.chatDraftPlan = path;
  if (typeof selectPlanRailItem === "function") {
    try {
      selectPlanRailItem(path);
    } catch (_) {}
  }
  try {
    await host.selectPlan(path);
  } catch (e) {
    toast(String(e?.message || e));
    return;
  }
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
    toast(`正在拆成步骤…「${name}」`);
    await host.analyzePlanFromPicker();
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
  const btnEdit = $("#btn-edit-plan");
  const btnChat = $("#btn-open-chat");
  const btnMonitor = $("#btn-monitor-plan");
  const btnPlanMgmt = $("#btn-plan-mgmt");

  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const inChat = !!state.selectedPath && state.page === "chat";
  const inPlans = !!state.selectedPath && state.page === "plans";
  // 选择计划：workspace；执行此计划：仅 workspace（聊天/计划管理用各自 CTA）
  const hideForPhase = state.phase === "planning" || state.phase === "confirm";
  const runActive = hasActiveRun();
  const hasSplit =
    !!state.planJob &&
    ["planned", "confirmed"].includes(String(state.planJob.status || "").toLowerCase());

  // 顶栏「聊天」：已选项目常驻；chat 页隐藏自指
  if (btnChat) {
    btnChat.hidden =
      !state.selectedPath ||
      state.page === "welcome" ||
      state.page === "chat";
    btnChat.disabled = false;
    btnChat.title = "与 AI 共建计划文档";
  }

  // A4：计划管理只在「更多」里，永不 primary（主路径不抢戏）
  if (btnPlanMgmt) {
    const showMgmt =
      !!state.selectedPath &&
      state.page !== "welcome" &&
      state.page !== "plans";
    btnPlanMgmt.hidden = !showMgmt;
    btnPlanMgmt.disabled = false;
    btnPlanMgmt.textContent = "管理计划文件";
    btnPlanMgmt.title = "进阶：选中 / 预览 / 编辑计划文件";
    btnPlanMgmt.classList.remove("primary");
    btnPlanMgmt.classList.add("ghost");
  }

  // A4：有待确认且在 chat →「继续核对拆分」；有活动 run →「返回执行」
  if (btnMonitor) {
    const pendingSplit =
      hasSplit &&
      !runActive &&
      ["planned", "confirmed"].includes(
        String(state.planJob?.status || "").toLowerCase()
      );
    const showMon =
      !!state.selectedPath &&
      state.page !== "workspace" &&
      state.page !== "welcome" &&
      (host.hasMonitorableActivity() || pendingSplit || host.isPlanSessionActive());
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
      } else if (pendingSplit || state.phase === "confirm") {
        btnMonitor.textContent = "继续核对拆分";
        btnMonitor.title = "回到拆分台核对波次后确认并开始";
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

  // 顶栏「选择计划」：进「更多」；workspace 非拆分相位显示
  if (btnChoose) {
    btnChoose.hidden = !inWorkspace || hideForPhase;
    btnChoose.disabled = !!runActive;
    btnChoose.title = runActive ? "运行中，请先停止后再切换计划" : "选择计划";
  }
  // 顶栏主 CTA「拆成步骤」：仅 workspace 且未在拆分中；主路径最多 1 个 primary
  if (btnAssign) {
    btnAssign.hidden = !inWorkspace || hideForPhase;
    if (!hideForPhase && !state.assigning) {
      btnAssign.textContent = runActive ? "运行中…" : "拆成步骤";
      btnAssign.title = runActive
        ? "运行中，请先停止后再拆分新计划"
        : "把当前计划拆成可执行步骤";
    }
  }
  // F5：更多菜单在无次要入口时隐藏
  const topMore = $("#top-more");
  if (topMore) {
    const hasSecondary =
      (btnPlanMgmt && !btnPlanMgmt.hidden) ||
      (btnChoose && !btnChoose.hidden) ||
      (btnEdit && !btnEdit.hidden) ||
      !!$("#budget-chip:not([hidden])");
    // 刷新始终可用；有项目时显示更多
    topMore.hidden = !state.selectedPath && state.page === "welcome";
    if (!hasSecondary && state.page === "welcome") topMore.hidden = true;
  }
  try {
    document.body.dataset.ccoPhase = state.phase || "pick";
    document.body.classList.toggle("cco-run-active", !!runActive);
    if (typeof host.refreshFlowStrips === "function") host.refreshFlowStrips();
  } catch (_) {}

  // 「编辑任务」：仅在有拆分结果时显示；运行中禁用，暂停后可进确认页改未执行任务
  if (btnEdit) {
    const showEdit = inWorkspace && hasSplit;
    btnEdit.hidden = !showEdit;
    if (showEdit) {
      btnEdit.textContent = "编辑任务";
      const editableNow = canEditSelectedTask(state.confirmTaskId || state.planJob?.tasks?.[0]?.id);
      btnEdit.disabled = !!runActive && !isRunPaused();
      if (runActive && !isRunPaused()) {
        btnEdit.title = "运行中不可编辑任务，请先停止或待计划暂停";
      } else if (isRunPaused()) {
        btnEdit.title = "计划已暂停：仅可编辑尚未执行的任务";
      } else if (state.phase === "confirm" || editableNow) {
        btnEdit.title = "编辑拆分后的任务说明（仅未执行任务）";
      } else {
        btnEdit.title = "打开拆分结果；仅暂停后、未执行任务可编辑";
      }
    }
  }

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
