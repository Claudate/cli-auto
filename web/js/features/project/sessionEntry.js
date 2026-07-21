/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: plan session stash + H0 entry route + selectProject + bg banner
 * [POS]: A5-2b-fin features/project/sessionEntry.js
 * note: plan session stash + H0 entry route + selectProject + bg banner
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

export function isPlanSessionActive(phase = state.phase) {
  return phase === "planning" || phase === "confirm";
}

export function stashPlanSession(projectPath = state.selectedPath) {
  if (!projectPath) return;
  if (!isPlanSessionActive() && !state.planJobId) {
    if (!state.planJobId) delete state.planSessions[projectPath];
    return;
  }
  state.planSessions[projectPath] = {
    phase: state.phase,
    planJobId: state.planJobId,
    planJob: state.planJob,
    selectedPlan: state.selectedPlan,
    confirmTaskId: state.confirmTaskId,
    planStartedAt: state.planStartedAt,
    assigning: !!state.assigning,
  };
}

export function restorePlanSession(projectPath) {
  const s = state.planSessions[projectPath];
  if (!s) return false;
  state.phase = s.phase || "pick";
  state.planJobId = s.planJobId || null;
  state.planJob = s.planJob || null;
  state.selectedPlan = s.selectedPlan || state.selectedPlan;
  state.confirmTaskId = s.confirmTaskId || null;
  state.planStartedAt = s.planStartedAt || 0;
  if (s.assigning) host.setAssignBusy(true);
  if (state.phase === "planning" && state.planJobId) host.startPlanJobPoll();
  return true;
}

export function clearPlanSession(projectPath = state.selectedPath) {
  if (projectPath) delete state.planSessions[projectPath];
}

/** 把磁盘/API 返回的 plan job 接到 UI（不自动开跑） */
export function applyRestoredPlanJob(view, { resumePoll = true } = {}) {
  if (!view) return false;
  const status = String(view.status || "").toLowerCase();
  state.planJob = view;
  state.planJobId = view.job_id || view.jobId || null;
  state.selectedPlan =
    normalizePlanPath(view.plan_path || view.planPath) ||
    state.selectedPlan;
  state.confirmTaskId = view.tasks?.[0]?.id || state.confirmTaskId || null;
  state.planStartedAt = Date.now();
  state.planPollFails = 0;

  if (status === "planning") {
    state.phase = "planning";
    if (resumePoll) host.startPlanJobPoll();
  } else if (status === "planned" || status === "confirmed") {
    // confirmed 也可再次「开始运行」，不必重拆
    state.phase = "confirm";
    host.stopPlanJobPoll();
    host.setAssignBusy(false);
  } else {
    return false;
  }
  stashPlanSession(state.selectedPath);
  return true;
}

export async function tryRestorePersistedPlanJob(projectPath) {
  if (!projectPath) return false;
  try {
    const view = await requireGateway().latestPlanJob(projectPath);
    if (!view) return false;
    const ok = applyRestoredPlanJob(view);
    if (!ok) return false;
    const status = String(view.status || "").toLowerCase();
    const n = view.task_count || view.tasks?.length || 0;
    if (status === "planning") {
      toast("已接上未完成的规划任务");
    } else if (status === "planned" || status === "confirmed") {
      // 不暗示「已进入执行页」：H0 仍落 chat；顶栏可「返回确认」
      toast(
        n
          ? `已记住上次拆分（${n} 任务）· 顶栏「返回确认」可继续执行`
          : "已记住上次拆分 · 顶栏「返回确认」可继续执行"
      );
    }
    return true;
  } catch (e) {
    console.warn("restore persisted plan job", e);
    return false;
  }
}

/** 离开 workspace 后仍可回看：规划中 / 待确认 / 运行中 / 暂停 / 刚结束 */
export function hasMonitorableActivity() {
  if (hasActiveRun() || isRunPaused()) return true;
  if (isPlanSessionActive() && state.planJobId) return true;
  if (state.live?.run_id && (state.phase === "running" || state.phase === "done")) return true;
  return false;
}

/**
 * H0 入口路由（对齐用户主路径）：
 * 1. 有活动 run → workspace 执行面板
 * 2. AI 正在拆分中（planning）→ workspace 看规划进度
 * 3. 其它一律 chat 主窗（含 planned/confirmed 待确认、done、无 live）
 *
 * 待确认不抢主窗：planJob 仍保留在内存，顶栏「返回确认」/ banner 可进 workspace。
 * 不改 Mode B confirm_start。
 */
export function resolveEntryRoute() {
  // 1) 真的有任务在跑 → 执行面板
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    return { page: "workspace", phaseHint: "running" };
  }
  // 2) 仅 AI 拆分「进行中」→ 看规划进度；planned/confirmed/failed 不抢主窗
  const st = String(state.planJob?.status || "").toLowerCase();
  if (state.planJobId && (state.phase === "planning" || st === "planning") && st === "planning") {
    return { page: "workspace", phaseHint: "planning" };
  }
  // 3) 默认聊天主窗（打开软件 / 选项目 / 待确认 / 已结束）
  return { page: "chat", phaseHint: null };
}

/**
 * 按 resolveEntryRoute 落地页面。
 * workspace：渲染看板/规划相位；chat：走 openChatPage（主窗）。
 */
export async function applyEntryRoute() {
  const route = resolveEntryRoute();

  if (route.page === "workspace") {
    if (route.phaseHint === "running") {
      // 活动 run：看板优先；confirm 会话可 stash，顶栏「返回确认」仍可回
      state.phase = "running";
      state.planCollapsed = true;
    } else if (route.phaseHint === "planning") {
      state.phase = "planning";
    }
    showPage("workspace");
    host.renderPhasePanels();
    host.renderPlanPicker();
    renderWorkspace();
    host.updateTopPlanInfo();
    updateBgPlanBanner();
    if (state.phase === "planning" && state.planJobId) {
      host.startPlanJobPoll();
    }
    return route;
  }

  // 默认聊天主窗（H0）
  // 保留 planJob 内存态（planned/confirmed），便于顶栏「返回确认」；不 showPage workspace
  if (typeof openChatPage === "function") {
    await openChatPage();
  } else {
    showPage("chat");
  }
  try {
    if (typeof host.renderPlanPicker === "function") host.renderPlanPicker();
  } catch (_) {}
  try {
    host.updateTopPlanInfo();
  } catch (_) {}
  try {
    updateBgPlanBanner();
  } catch (_) {}
  return route;
}

/** 从聊天/设置等页回到 workspace 监视（规划相位或 CLI 看板） */
export function goToPlanMonitor() {
  const path = state.selectedPath || state.lastWorkspacePath;
  if (!path) {
    toast("请先选择项目");
    return;
  }
  if (path !== state.selectedPath) {
    Promise.resolve(selectProject(path)).catch((e) => toast(String(e)));
    return;
  }
  // 用户主动点「监控」：强制进 workspace（不是 H0 默认入口）
  showPage("workspace");
  if (isPlanSessionActive()) {
    host.renderPhasePanels();
    host.renderPlanPicker();
    if (state.phase === "planning" && state.planJobId) {
      host.startPlanJobPoll();
      host.refreshPlanJob().catch(() => {});
    }
  }
  renderWorkspace();
  host.updateTopPlanInfo();
  updateBgPlanBanner();
  try {
    if (typeof host.renderPlanPicker === "function") host.renderPlanPicker();
  } catch (_) {}
}

/** 后台 banner 关断签名：phase / job / run 变化后重新显示 */
export const BG_BANNER_DISMISS_KEY = "cco.bgBannerDismissSig";

export function bgBannerActivitySig() {
  return [
    state.phase || "",
    state.planJobId || "",
    state.live?.run_id || "",
    state.selectedPlan || state.live?.plan_path || "",
  ].join("|");
}

export function isBgBannerDismissed() {
  try {
    return localStorage.getItem(BG_BANNER_DISMISS_KEY) === bgBannerActivitySig();
  } catch (_) {
    return false;
  }
}

export function dismissBgPlanBanner() {
  try {
    localStorage.setItem(BG_BANNER_DISMISS_KEY, bgBannerActivitySig());
  } catch (_) {}
  const bar = document.getElementById("bg-plan-banner");
  if (bar) bar.hidden = true;
}

export function updateBgPlanBanner() {
  let bar = document.getElementById("bg-plan-banner");
  if (!bar) {
    bar = document.createElement("div");
    bar.id = "bg-plan-banner";
    bar.className = "bg-plan-banner";
    bar.hidden = true;
    bar.setAttribute("role", "status");
    bar.innerHTML =
      '<span class="spinner sm" id="bg-plan-banner-spin" aria-hidden="true"></span>' +
      '<span id="bg-plan-banner-text">规划在后台进行中</span>' +
      '<button type="button" class="btn ghost sm" id="btn-bg-plan-back">查看监控</button>' +
      '<button type="button" class="btn ghost sm bg-plan-banner-dismiss" id="btn-bg-plan-dismiss" aria-label="关闭提示" title="关闭">×</button>';
    document.body.appendChild(bar);
    bar.querySelector("#btn-bg-plan-back")?.addEventListener("click", () => {
      goToPlanMonitor();
    });
    bar.querySelector("#btn-bg-plan-dismiss")?.addEventListener("click", (e) => {
      e.stopPropagation();
      dismissBgPlanBanner();
    });
  } else {
    // 热更新/旧 DOM：保证 ghost 钮与关闭钮存在
    const back = bar.querySelector("#btn-bg-plan-back");
    if (back) {
      back.classList.remove("primary");
      back.classList.add("ghost");
    }
    if (!bar.querySelector("#btn-bg-plan-dismiss")) {
      const d = document.createElement("button");
      d.type = "button";
      d.id = "btn-bg-plan-dismiss";
      d.className = "btn ghost sm bg-plan-banner-dismiss";
      d.setAttribute("aria-label", "关闭提示");
      d.title = "关闭";
      d.textContent = "×";
      d.addEventListener("click", (e) => {
        e.stopPropagation();
        dismissBgPlanBanner();
      });
      bar.appendChild(d);
    }
  }
  const away = state.page !== "workspace" || !state.selectedPath;
  const planning = isPlanSessionActive() && !!state.planJobId;
  const running = hasActiveRun();
  const paused = isRunPaused();
  const finished =
    !running &&
    !paused &&
    !planning &&
    !!state.live?.run_id &&
    (state.phase === "running" || state.phase === "done");
  // 顶栏已有监控入口时隐藏 banner，避免 chat/设置 双入口抢注意力
  const topMonVisible =
    !!state.selectedPath &&
    state.page !== "workspace" &&
    state.page !== "welcome" &&
    hasMonitorableActivity();
  const show =
    away &&
    (planning || running || paused || finished) &&
    !topMonVisible &&
    !isBgBannerDismissed();
  if (!show) {
    bar.hidden = true;
    return;
  }
  const name =
    planDisplayName(state.selectedPlan || state.live?.plan_path || "") || "当前计划";
  const projPath = state.selectedPath || state.lastWorkspacePath || "";
  const proj = (state.projects || []).find((p) => p.path === projPath);
  const projLabel =
    proj?.name ||
    (projPath ? String(projPath).split(/[/\\]/).filter(Boolean).pop() : "");
  const prefix = projLabel ? `${projLabel} · ` : "";
  const txt = document.getElementById("bg-plan-banner-text");
  const spin = document.getElementById("bg-plan-banner-spin");
  const btn = document.getElementById("btn-bg-plan-back");
  if (txt) {
    if (planning && state.phase === "planning") {
      txt.textContent = `${prefix}正在后台拆分「${name}」…`;
    } else if (planning) {
      txt.textContent = `${prefix}「${name}」待确认`;
    } else if (running) {
      txt.textContent = `${prefix}「${name}」正在运行`;
    } else if (paused) {
      txt.textContent = `${prefix}「${name}」已暂停`;
    } else {
      txt.textContent = `${prefix}「${name}」运行结束`;
    }
  }
  if (spin) spin.hidden = !(planning && state.phase === "planning") && !running;
  if (btn) {
    btn.className = "btn ghost sm";
    btn.textContent =
      planning && state.phase === "confirm" ? "返回确认" : "查看监控";
  }
  bar.hidden = false;
}

export async function selectProject(path) {
  // 同项目再点：H0 按活动态路由（禁止无条件 workspace）
  if (path && path === state.selectedPath) {
    renderProjectList();
    await applyEntryRoute();
    if (state.phase === "planning" && state.planJobId) {
      host.refreshPlanJob().catch(() => {});
    }
    return;
  }

  // 多项目可并行：离开旧项目只缓存会话，不停止其后台 CLI / run
  if (state.selectedPath) {
    state.lastWorkspacePath = state.selectedPath;
  }
  stashPlanSession(state.selectedPath);
  host.stopPlanJobPoll();
  host.setAssignBusy(false);

  state.selectedPath = path;
  state.logStick = true;
  state.planPreview = null;
  state.selectedTaskId = null;
  state.planCollapsed = false;
  state.filterFailedOnly = false;
  state.cliStatusFilter = "all";
  state.closedPanels = {};
  state.selectedPlan = null;
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.phase = "pick";
  // 聊天按项目隔离：先 stash 旧项目，再切到新项目缓存（或空会话）
  try {
    if (typeof stashChatSession === "function" && state.chatProjectPath) {
      stashChatSession(state.chatProjectPath);
    }
  } catch (_) {}
  state.chatBusy = false;
  state.chatWaitStartedAt = 0;
  try {
    if (typeof stopChatWaitTicker === "function") stopChatWaitTicker();
  } catch (_) {}
  if (typeof restoreChatSession === "function" && restoreChatSession(path)) {
    /* 新项目已有缓存 */
  } else {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null };
    state.chatDraftPlan = null;
    state.chatProjectPath = path || null;
  }

  const restoredMem = restorePlanSession(path);

  // H0：先拉 live / plans，再 applyEntryRoute（禁止提前 showPage("workspace")）
  renderProjectList();
  await Promise.all([host.loadLive(), host.loadPlansForPicker(), host.ensureDoctor()]);

  if (restoredMem) {
    // 活动 run 优先看板；否则保留内存 plan 会话 phase（待确认不抢 chat）
    if (hasActiveRun()) {
      state.phase = "running";
      state.planCollapsed = true;
    }
    await applyEntryRoute();
    if (state.phase === "planning" && state.planJobId) {
      await host.refreshPlanJob().catch(() => {});
    }
    // 仅在真的落在 workspace 规划中时提示；chat 主窗用顶栏「返回确认」即可
    if (
      state.page === "workspace" &&
      state.phase === "planning" &&
      !hasActiveRun()
    ) {
      toast("已回到后台规划");
    } else if (
      state.page === "chat" &&
      isPlanSessionActive() &&
      state.phase === "confirm" &&
      !hasActiveRun()
    ) {
      toast("有待确认的拆分 · 点顶栏「返回确认」可继续执行");
    }
    return;
  }

  // 内存无会话 → 从磁盘接上该项目最近一次拆分（避免每次重拆）
  // 活动 run 优先；planned/confirmed 会恢复到内存，但 H0 仍落 chat
  const activeRun = hasActiveRun();
  if (!activeRun) {
    const restoredDisk = await tryRestorePersistedPlanJob(path);
    if (restoredDisk) {
      await applyEntryRoute();
      if (state.phase === "planning" && state.planJobId) {
        await host.refreshPlanJob().catch(() => {});
      }
      if (
        state.page === "chat" &&
        state.phase === "confirm" &&
        state.planJobId
      ) {
        const n = state.planJob?.task_count || state.planJob?.tasks?.length || 0;
        if (n) {
          toast(`已恢复历史拆分（${n} 任务）· 聊天主窗可改计划，顶栏「返回确认」可执行`);
        }
      }
      return;
    }
  }

  if (hasActiveRun()) {
    state.planCollapsed = true;
    state.phase = "running";
  } else if (
    state.live?.run_id &&
    ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
      String(state.live?.run_status || "").toLowerCase()
    )
  ) {
    state.phase = "done";
  }
  const proj = state.projects.find((p) => p.path === path);
  const rawCandidate =
    state.live?.plan_path || proj?.default_plan || proj?.last_plan || state.plans[0] || null;
  const candidate = normalizePlanPath(rawCandidate, path) || rawCandidate;
  if (candidate) {
    try {
      await host.selectPlan(candidate, { keepSession: true });
    } catch (e) {
      console.warn("restore plan failed", e);
      state.selectedPlan = candidate;
    }
  }
  // H0 最终落点：有活动 run / 拆分中 → workspace；否则 chat
  await applyEntryRoute();
  // 双保险：无活动 run 且不在 planning 时绝不能停在 workspace（防历史分支漏改）
  if (
    state.page === "workspace" &&
    !(typeof hasActiveRun === "function" && hasActiveRun())
  ) {
    const st2 = String(state.planJob?.status || "").toLowerCase();
    if (state.phase !== "planning" && st2 !== "planning") {
      if (typeof openChatPage === "function") await openChatPage();
      else showPage("chat");
      try {
        host.renderPlanPicker();
      } catch (_) {}
    }
  }
}
