/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: plan session stash + entry route (A1 confirm desk) + selectProject + bg banner
 * [POS]: A5-2b-fin features/project/sessionEntry.js
 * note: 打开项目默认 chat；仅活动 run/暂停 → workspace 运行页；拆分台不默认抢入口
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
  return (
    phase === "planning" ||
    phase === "confirm" ||
    phase === "plan_failed"
  );
}

/**
 * project_live 返回的是「项目最近一次 run」（含历史 completed）。
 * 打开拆分会话且尚未/不匹配本 job 的 run 时，不得把旧 run 当成「本轮结果」。
 * plan_failed 同样不得用历史 completed 当「本轮结果」。
 *
 * 例外：live 仍在跑 / 暂停时一律算本轮——重开项目、单任务再跑、续跑都不能因
 * planJob 缺 run_id 而把执行台刷成空白（cli-empty）。
 */
export function liveBelongsToOpenPlan() {
  const live = state.live;
  if (!live?.run_id) return false;
  // dismiss SoT = SQLite last_dismissed；已结束本轮不回绑结果/运行台
  try {
    const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
    const last = proj?.last_run_id || proj?.lastRunId || null;
    if (
      proj &&
      (proj.last_dismissed || proj.lastDismissed) &&
      last &&
      String(last) === String(live.run_id)
    ) {
      return false;
    }
  } catch (_) {}
  // 真在执行或暂停：进度台必须可见（isRunPaused 已排除 dismissed）
  if (typeof isLiveStatus === "function" && isLiveStatus(live.run_status)) {
    return true;
  }
  if (typeof isRunPaused === "function" && isRunPaused()) {
    return true;
  }
  const rs = String(live.run_status || "").toLowerCase();
  // paused only if not dismissed (above); still allow when no project row yet
  if (rs === "paused") return true;
  // 拆分失败/进行中：没有本 job 的 run，历史 live 一律不算本轮
  if (state.phase === "plan_failed" || state.phase === "planning") {
    return false;
  }
  // 打开项目默认 chat：pick 下历史终态 live 不算「本轮」
  if (state.phase === "pick" || !state.phase) {
    return false;
  }
  // 本轮已进入执行/结果：允许用项目 live 画台
  if (state.phase === "running" || state.phase === "done") {
    return true;
  }
  const job = state.planJob;
  if (!job) return true;
  const st = String(job.status || "").toLowerCase();
  if (st === "plan_failed" || st === "planning") return false;
  if (st !== "planned" && st !== "confirmed") return true;
  const jrid = job.run_id || job.runId || null;
  if (!jrid) {
    if (state.phase === "confirm") return false;
    return true;
  }
  return String(jrid) === String(live.run_id);
}

/** 历史 live 仅作项目档案，不驱动 phase / 本轮结果台 */
export function hasCurrentRoundLive() {
  if (typeof hasActiveRun === "function" && hasActiveRun()) return true;
  if (typeof isRunPaused === "function" && isRunPaused()) return true;
  return liveBelongsToOpenPlan();
}

/**
 * 内存会话：仅缓存「拆分进行中 / 待确认」。
 * done/running/pick 不 stash（避免再点项目回结果台）。
 * 跨重启仍以 SQLite cco_split / plan_jobs 为准。
 */
export function stashPlanSession(projectPath = state.selectedPath) {
  if (!projectPath) return;
  if (!isPlanSessionActive() || !state.planJobId) {
    delete state.planSessions[projectPath];
    return;
  }
  // 只保留 planning / confirm / plan_failed
  const phase = state.phase;
  if (phase !== "planning" && phase !== "confirm" && phase !== "plan_failed") {
    delete state.planSessions[projectPath];
    return;
  }
  state.planSessions[projectPath] = {
    phase,
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
  let phase = s.phase || "pick";
  // 禁止恢复成结果台 / 运行台（入口默认 chat，除非 hasActiveRun）
  if (phase === "done" || phase === "running" || phase === "pick") {
    delete state.planSessions[projectPath];
    return false;
  }
  if (phase !== "planning" && phase !== "confirm" && phase !== "plan_failed") {
    delete state.planSessions[projectPath];
    return false;
  }
  state.phase = phase;
  state.planJobId = s.planJobId || null;
  state.planJob = s.planJob || null;
  state.selectedPlan = s.selectedPlan || state.selectedPlan;
  state.confirmTaskId = s.confirmTaskId || null;
  state.planStartedAt = s.planStartedAt || 0;
  if (s.assigning) host.setAssignBusy(true);
  if (state.phase === "planning" && state.planJobId) host.startPlanJobPoll();
  return true;
}

/**
 * shell-chrome C2：仅允许这些路径清 session，禁止旁路静默丢拆分结果：
 * - 用户取消规划（cancelPlanning）
 * - 新开拆分任务（analyze 开 planning，旧 job 被 supersede）
 * - 从 cco 列表移除项目（removeProject）
 * 禁止：仅切页 / 打开计划管理 / 点 chip 回看 时 clear。
 */
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
    // confirmed 也可再次「执行规划」，不必重新规划
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
      toast(
        n
          ? `已回到拆分台（${n} 任务），核对后可执行规划`
          : "已回到拆分台，核对后可执行规划"
      );
    }
    return true;
  } catch (e) {
    console.warn("restore persisted plan job", e);
    return false;
  }
}

/**
 * Normalize plan path for split-index lookup (slash-unified, strip leading ./).
 * @param {string} planPath
 * @param {string} [projectRoot]
 */
export function planPathLookupKey(planPath, projectRoot = state.selectedPath) {
  if (!planPath) return "";
  let p =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(planPath, projectRoot) || planPath
      : planPath;
  p = String(p).trim().replace(/\\/g, "/").replace(/^file:\/\//, "");
  while (p.startsWith("./")) p = p.slice(2);
  return p.replace(/^\/+/, "");
}

/**
 * Restore restorable split for a **specific plan path** (plan list → 查看拆分结果).
 * Uses SQLite plan_jobs index + disk job_view; does not auto-start.
 * @param {string} planPath
 * @param {{ silent?: boolean, projectPath?: string }} [opts]
 * @returns {Promise<boolean>}
 */
export async function tryRestorePlanJobForPlan(planPath, opts = {}) {
  const projectPath = opts.projectPath || state.selectedPath;
  if (!projectPath || !planPath) return false;
  const silent = !!opts.silent;
  try {
    const view = await requireGateway().latestPlanJobForPlan(
      projectPath,
      planPath
    );
    if (!view) {
      if (!silent) toast("还没有这份计划的拆分结果，请先点「拆成步骤」");
      return false;
    }
    const ok = applyRestoredPlanJob(view, { resumePoll: true });
    if (!ok) {
      if (!silent) toast("这份计划的拆分结果已无法恢复，请重新规划");
      return false;
    }
    const status = String(view.status || "").toLowerCase();
    const n = view.task_count || view.tasks?.length || 0;
    if (!silent) {
      if (status === "planning") {
        toast("已接上该计划未完成的拆分");
      } else {
        toast(
          n
            ? `已打开拆分结果（${n} 步），可继续核对或执行规划`
            : "已打开拆分结果"
        );
      }
    }
    return true;
  } catch (e) {
    console.warn("restore plan job for plan", e);
    if (!silent) toast(String(e?.message || e || "无法打开拆分结果"));
    return false;
  }
}

/**
 * Load SQLite split index into state.planSplitByPath for list badges.
 * @param {string} [projectPath]
 */
export async function loadPlanSplitIndex(projectPath = state.selectedPath) {
  if (!projectPath) {
    state.planSplitByPath = {};
    return {};
  }
  try {
    const rows = (await requireGateway().listPlanSplitIndex(projectPath)) || [];
    const by = {};
    for (const r of Array.isArray(rows) ? rows : []) {
      const raw = r.plan_path || r.planPath || "";
      const key = planPathLookupKey(raw, projectPath);
      if (!key) continue;
      // first row wins (query is newest-first)
      if (by[key]) continue;
      const entry = {
        job_id: r.job_id || r.jobId || null,
        status: String(r.status || "").toLowerCase(),
        task_count: r.task_count ?? r.taskCount ?? null,
        plan_name: r.plan_name || r.planName || null,
        updated_at: r.updated_at || r.updatedAt || null,
        plan_path: key,
      };
      by[key] = entry;
      // also index by raw path variants
      by[raw] = entry;
      const base = key.split("/").pop();
      if (base && !by[base]) by[base] = entry;
    }
    state.planSplitByPath = by;
    return by;
  } catch (e) {
    console.warn("loadPlanSplitIndex", e);
    state.planSplitByPath = state.planSplitByPath || {};
    return state.planSplitByPath;
  }
}

/** Lookup restorable split index row for a plan path. */
export function planSplitForPath(planPath, projectRoot = state.selectedPath) {
  if (!planPath) return null;
  const by = state.planSplitByPath || {};
  const key = planPathLookupKey(planPath, projectRoot);
  return by[key] || by[planPath] || by[key.split("/").pop()] || null;
}

/** 离开 workspace 后仍可回看：规划中 / 待确认 / 运行中 / 暂停 / 刚结束 */
export function hasMonitorableActivity() {
  if (hasActiveRun() || isRunPaused()) return true;
  if (isPlanSessionActive() && state.planJobId) return true;
  if (state.live?.run_id && (state.phase === "running" || state.phase === "done")) return true;
  return false;
}

/**
 * 打开项目入口路由：
 * 1. 任务在跑（含 starting/queued）或暂停 → workspace 运行页
 * 2. 其它一律 chat（写计划主窗；拆分台不默认抢入口，可经「继续核对拆分/查看监控」进入）
 *
 * 不改 Mode B confirm_start。
 */
export function resolveEntryRoute() {
  // 1) 真的有任务在跑，或暂停可续跑 → 运行页
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    return { page: "workspace", phaseHint: "running" };
  }
  if (typeof isRunPaused === "function" && isRunPaused()) {
    return { page: "workspace", phaseHint: "running" };
  }
  // 2) 默认聊天主窗（planning / confirm / done / 冷启动）
  return { page: "chat", phaseHint: null };
}

/**
 * 按 resolveEntryRoute 落地页面。
 * workspace：渲染看板/规划/拆分台；chat：走 openChatPage。
 */
export async function applyEntryRoute() {
  const route = resolveEntryRoute();

  if (route.page === "workspace") {
    if (route.phaseHint === "running") {
      state.phase = "running";
      state.planCollapsed = true;
    } else if (route.phaseHint === "planning") {
      state.phase = "planning";
    } else if (route.phaseHint === "confirm") {
      state.phase = "confirm";
      if (!state.confirmTaskId && state.planJob?.tasks?.length) {
        state.confirmTaskId = state.planJob.tasks[0].id;
      }
    }
    showPage("workspace");
    host.renderPhasePanels();
    host.renderPlanPicker();
    if (state.phase === "confirm" && typeof host.renderConfirmPanel === "function") {
      try {
        host.renderConfirmPanel();
      } catch (_) {}
    }
    renderWorkspace();
    host.updateTopPlanInfo();
    updateBgPlanBanner();
    if (state.phase === "planning" && state.planJobId) {
      host.startPlanJobPoll();
    }
    return route;
  }

  // 默认聊天主窗；planJob 仍可保留在内存
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
      `<button type="button" class="icon-btn sm bg-plan-banner-dismiss" id="btn-bg-plan-dismiss" aria-label="关闭提示" title="关闭">${typeof window.ccoIcon === "function" ? window.ccoIcon("x", { size: 14 }) : "×"}</button>`;
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
      d.className = "icon-btn sm bg-plan-banner-dismiss";
      d.setAttribute("aria-label", "关闭提示");
      d.title = "关闭";
      if (typeof window.ccoIcon === "function") {
        d.innerHTML = window.ccoIcon("x", { size: 14 });
      } else {
        d.textContent = "×";
      }
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
    liveBelongsToOpenPlan() &&
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
  // 计划管理页作用域随项目重置
  state.plansMgmtScopeDir = null;
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
      if (state.planJob && state.live?.run_id) {
        const jrid = state.planJob.run_id || state.planJob.runId || null;
        if (!jrid) {
          state.planJob = { ...state.planJob, run_id: state.live.run_id };
        }
      }
    }
    await applyEntryRoute();
    if (state.phase === "planning" && state.planJobId) {
      await host.refreshPlanJob().catch(() => {});
    }
    // workspace 规划/拆分台已落地；不 toast 冲回 chat
    if (
      state.page === "workspace" &&
      state.phase === "planning" &&
      !hasActiveRun()
    ) {
      toast("已回到后台规划");
    }
    return;
  }

  // 内存无会话 → 从磁盘接上该项目最近一次拆分（避免每次重新规划）
  // 活动 run 优先；planned/confirmed 恢复后 A1 落拆分台
  const activeRun = hasActiveRun();
  if (!activeRun) {
    const restoredDisk = await tryRestorePersistedPlanJob(path);
    if (restoredDisk) {
      await applyEntryRoute();
      if (state.phase === "planning" && state.planJobId) {
        await host.refreshPlanJob().catch(() => {});
      }
      return;
    }
  }

  if (hasActiveRun() || (typeof isRunPaused === "function" && isRunPaused())) {
    state.planCollapsed = true;
    state.phase = "running";
    // 活动 run 已接管本轮：把 job.run_id 补上，避免后续 liveBelongs 再误判
    if (state.planJob && state.live?.run_id) {
      const jrid = state.planJob.run_id || state.planJob.runId || null;
      if (!jrid) {
        state.planJob = { ...state.planJob, run_id: state.live.run_id };
      }
    }
  }
  // 终态历史 run 不再设 phase=done：打开项目默认 chat（用户可再进结果/监控）
  const proj = state.projects.find((p) => p.path === path);
  // 打开拆分会话时顶栏计划跟 job，勿被历史 live.plan_path 冲掉
  const preferLivePlan =
    !state.planJobId || liveBelongsToOpenPlan() || !state.selectedPlan;
  const rawCandidate = preferLivePlan
    ? state.live?.plan_path ||
      proj?.default_plan ||
      proj?.last_plan ||
      state.plans[0] ||
      null
    : state.selectedPlan ||
      proj?.default_plan ||
      proj?.last_plan ||
      state.plans[0] ||
      null;
  const candidate = normalizePlanPath(rawCandidate, path) || rawCandidate;
  if (candidate) {
    try {
      await host.selectPlan(candidate, { keepSession: true });
    } catch (e) {
      console.warn("restore plan failed", e);
      state.selectedPlan = candidate;
    }
  }
  // 最终落点：仅活动 run / 暂停 → workspace；否则 chat
  await applyEntryRoute();
  // 双保险：无在跑/暂停任务时不得停在 workspace
  if (state.page === "workspace") {
    const running =
      (typeof hasActiveRun === "function" && hasActiveRun()) ||
      (typeof isRunPaused === "function" && isRunPaused());
    if (!running) {
      if (typeof openChatPage === "function") await openChatPage();
      else showPage("chat");
      try {
        host.renderPlanPicker();
      } catch (_) {}
    }
  }
}
