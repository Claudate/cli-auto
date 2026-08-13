/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: plan session stash + entry route (A1 confirm desk) + selectProject + bg banner
 * [POS]: A5-2b-fin features/project/sessionEntry.js
 * note: 打开项目默认 chat；仅活动 run/暂停 → workspace 运行页；拆分台不默认抢入口
 * note: goToPlanMonitor 活动→running、终态→done（勿停 pick 空引导 / 历史拆分只读）
 * note: P0-B selectProject 时 best-effort 恢复项目 persona/芯片（chatPersonaSync）
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
import { restorePersonaForProject } from "../chat/chatPersonaSync.js";

export function isPlanSessionActive(phase = state.phase) {
  return (
    phase === "planning" ||
    phase === "confirm" ||
    phase === "plan_failed"
  );
}

/**
 * project_live 返回的是「项目最近一次 run」（含历史 completed / 其它计划的 paused）。
 * 打开拆分会话且尚未/不匹配本 job 的 run 时，不得把旧 run 当成「本轮结果」。
 *
 * 绑定规则（本轮 round）：
 * 1. dismissed 的 run 永不回绑。
 * 2. 真在跑 / 暂停 → 本轮（执行台优先，不被残留 planned job 解绑）。
 * 3. phase=running|done → 本轮（完成/失败后仍要画结果台；仅「待确认新图且无 run_id」除外）。
 * 4. 人在拆分台（planning/confirm/plan_failed）时：
 *    - job 已有 run_id → 仅同 id 算本轮；
 *    - job 尚无 run_id（待确认新图）→ 历史 live 一律不算本轮。
 * 5. pick / 冷启动：历史终态不算本轮（打开项目默认 chat）。
 * 6. job 已 confirmed 且带 run_id：同 id 终态也算本轮（返回结果，不要求 phase 已是 done）。
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

  const job = state.planJob;
  const jobSt = String(job?.status || "").toLowerCase();
  const jrid = job?.run_id || job?.runId || null;
  const phase = state.phase;
  const onSplitUi =
    phase === "confirm" ||
    phase === "planning" ||
    phase === "plan_failed";
  // 待确认新图（无 run_id）：只在拆分 UI / planned 会话上挡历史 live
  const blockingNewSplit =
    !!job &&
    !jrid &&
    (onSplitUi || jobSt === "planned" || jobSt === "planning" || jobSt === "plan_failed");

  // 真在执行 / 暂停：优先认本轮（勿被 jobSt=planned 残留解绑执行台）
  if (typeof isLiveStatus === "function" && isLiveStatus(live.run_status)) {
    if (blockingNewSplit && onSplitUi) return false;
    return true;
  }
  if (typeof isRunPaused === "function" && isRunPaused()) {
    if (blockingNewSplit && onSplitUi) return false;
    return true;
  }
  const rs = String(live.run_status || "").toLowerCase();
  if (rs === "paused") {
    if (blockingNewSplit && onSplitUi) return false;
    return true;
  }

  // 已在执行/结果台：终态 live 必须能画结果；仅「拆分台上看新图」才挡
  if (phase === "running" || phase === "done") {
    if (blockingNewSplit && onSplitUi) return false;
    if (jrid) return String(jrid) === String(live.run_id);
    return true;
  }

  // 拆分台 UI：严格按 job.run_id 绑定
  if (onSplitUi) {
    if (jobSt === "plan_failed" || jobSt === "planning" || phase === "plan_failed" || phase === "planning") {
      return !!(jrid && String(jrid) === String(live.run_id));
    }
    if (jobSt === "planned" || phase === "confirm") {
      if (!jrid) return false;
      return String(jrid) === String(live.run_id);
    }
    if (jobSt === "confirmed") {
      if (jrid) return String(jrid) === String(live.run_id);
    }
    return false;
  }

  // job 已确认并绑定本 run：聊天页点「查看结果/返回执行」时 phase 可能仍是 pick
  if (job && jobSt === "confirmed" && jrid && String(jrid) === String(live.run_id)) {
    return true;
  }

  // 打开项目默认 chat：pick 下历史终态 live 不算「本轮」
  if (phase === "pick" || !phase) {
    return false;
  }
  if (!job) return true;
  if (jrid) return String(jrid) === String(live.run_id);
  return false;
}

/** 历史 live 仅作项目档案，不驱动 phase / 本轮结果台 */
export function hasCurrentRoundLive() {
  // 只认「属于本轮」的 live；拆分台上的外国 paused 不得当本轮
  return liveBelongsToOpenPlan();
}

/**
 * 仅在 job 已 confirmed 且缺 run_id、且 live 属于本轮时回填。
 * planned 待确认图禁止把项目历史 paused run 写进 job.run_id。
 */
function stampJobRunIdFromLiveIfSafe() {
  if (!state.planJob || !state.live?.run_id) return;
  const jrid = state.planJob.run_id || state.planJob.runId || null;
  if (jrid) return;
  const st = String(state.planJob.status || "").toLowerCase();
  if (st !== "confirmed") return;
  if (!liveBelongsToOpenPlan()) return;
  state.planJob = { ...state.planJob, run_id: state.live.run_id };
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
  // Elapsed/timeout clock = job creation time, not restore time — restoring an
  // old planning job must not silently grant it another fresh 12 minutes.
  const createdMs = Date.parse(view.created_at || view.createdAt || "");
  state.planStartedAt = Number.isFinite(createdMs) ? createdMs : Date.now();
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
      // also index by raw path variant (NOT bare basename — cross-directory
      // same-named plans must not borrow each other's 已拆分 badge)
      by[raw] = entry;
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
  return by[key] || by[planPath] || null;
}

/** 离开 workspace 后仍可回看：规划中 / 待确认 / 运行中 / 暂停 / 刚结束 */
export function hasMonitorableActivity() {
  if (hasActiveRun() || isRunPaused()) return true;
  if (isPlanSessionActive() && state.planJobId) return true;
  if (state.live?.run_id && (state.phase === "running" || state.phase === "done")) return true;
  // 终态 live 在 phase 仍是 pick/confirm 残留时也要能回结果台（完成/失败后常见）
  if (state.live?.run_id && liveBelongsToOpenPlan()) return true;
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
  // 1) 本轮真在跑 / 本轮暂停 → 运行页（外国历史 paused 不抢入口）
  if (liveBelongsToOpenPlan()) {
    const rs = String(state.live?.run_status || "").toLowerCase();
    const live =
      typeof isLiveStatus === "function"
        ? isLiveStatus(state.live?.run_status)
        : ["running", "starting", "queued", "validated", "init", "resuming"].includes(rs);
    const paused =
      typeof isRunPaused === "function"
        ? isRunPaused()
        : rs === "paused";
    if (live || paused) {
      return { page: "workspace", phaseHint: "running" };
    }
  } else if (typeof hasActiveRun === "function" && hasActiveRun() && !state.planJob) {
    return { page: "workspace", phaseHint: "running" };
  } else if (
    typeof isRunPaused === "function" &&
    isRunPaused() &&
    !state.planJob
  ) {
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

/** Terminal run statuses that still have a result desk (not actively executing). */
function isTerminalRunStatus(st) {
  return ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
    String(st || "").toLowerCase()
  );
}

/**
 * Enter CLI/result desk from chat etc. Lifts phase for active OR finished live.
 * Bugfix: only lifting active/paused left completed/failed on pick → empty #cli-empty.
 */
function liftWorkspacePhaseForLive() {
  const jobSt = String(state.planJob?.status || "").toLowerCase();
  const jrid = state.planJob?.run_id || state.planJob?.runId || null;
  const okRound =
    typeof liveBelongsToOpenPlan === "function"
      ? liveBelongsToOpenPlan() || !state.planJob
      : true;
  // 待确认新图且 live 不属于本轮：勿用外国 run 劫持拆分台
  const blockingNewSplit =
    (jobSt === "planned" || state.phase === "confirm") &&
    !jrid &&
    !okRound;
  if (blockingNewSplit) return;

  const runLive =
    (typeof hasActiveRun === "function" && hasActiveRun()) ||
    (typeof isRunPaused === "function" && isRunPaused()) ||
    (typeof isLiveStatus === "function" &&
      isLiveStatus(state.live?.run_status));

  if (runLive) {
    state.phase = "running";
  } else if (state.live?.run_id && isTerminalRunStatus(state.live?.run_status)) {
    // 完成/失败/中止：抬到 done，画结果台（勿停在 pick 导致空引导）
    // confirmed job 同 run_id 或无挡新图时都算本轮
    const sameJob =
      !jrid || String(jrid) === String(state.live.run_id);
    if (sameJob || okRound || !state.planJob) {
      state.phase = "done";
    } else {
      return;
    }
  } else {
    return;
  }

  state.planCollapsed = true;
  state.confirmEditing = false;
  stampJobRunIdFromLiveIfSafe();
  try {
    if (typeof host.setPlanCollapsed === "function") {
      host.setPlanCollapsed(true);
    }
  } catch (_) {}
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

  // 活动/暂停 → running；完成/失败/中止 → done。
  // 聊天页点「返回执行/查看结果」时 phase 常仍是 confirm/pick，
  // 若不抬 phase：要么拆分台挡住 CLI，要么 liveBelongs 在 pick 下拒终态 → #cli-empty。
  liftWorkspacePhaseForLive();

  if (isPlanSessionActive() && state.phase !== "running" && state.phase !== "done") {
    host.renderPhasePanels();
    host.renderPlanPicker();
    if (state.phase === "planning" && state.planJobId) {
      host.startPlanJobPoll();
      host.refreshPlanJob().catch(() => {});
    }
  } else {
    // running / done：隐藏 planning/confirm 面板，露出 #monitor CLI 看板
    host.renderPhasePanels();
    host.renderPlanPicker();
  }
  renderWorkspace();
  host.updateTopPlanInfo();
  updateBgPlanBanner();
  try {
    if (typeof host.renderPlanPicker === "function") host.renderPlanPicker();
  } catch (_) {}
  // 刷新 live，避免聊天页停留期间任务条过期；刷新后再抬一次 phase（live 可能刚到位）
  try {
    if (typeof host.loadLive === "function") {
      host.loadLive()
        .then(() => {
          const before = state.phase;
          liftWorkspacePhaseForLive();
          if (state.phase !== before) {
            try {
              host.renderPhasePanels();
            } catch (_) {}
            try {
              renderWorkspace();
            } catch (_) {}
          }
        })
        .catch(() => {});
    }
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
  // P0-B: best-effort 恢复本项目 persona/芯片（无项目/无存储不报错）
  try {
    restorePersonaForProject(path).catch(() => {});
  } catch (_) {}
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
      // 仅当 job 已 confirmed 且缺 run_id 时回填；planned 新图禁止偷绑历史 live
      stampJobRunIdFromLiveIfSafe();
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
    // 拆分台 planned 新图 + 外国 paused：不要被推进 running
    if (liveBelongsToOpenPlan() || !state.planJob) {
      state.planCollapsed = true;
      state.phase = "running";
      stampJobRunIdFromLiveIfSafe();
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
