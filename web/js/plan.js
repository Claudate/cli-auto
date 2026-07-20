/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: plan UI 片段 · 顶栏选择/分配可见性 · 全局 plan-chooser · H0 入口路由 · H2 meta/badge
 * [POS]: web/js D4 自 app.js 纵切；无构建器，顺序 script 共享全局
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 * 注：#plan-chooser 在 main 级（非 page-workspace）；聊天/管理页走 startExecuteFromSelection
 * H0：selectProject 经 resolveEntryRoute/applyEntryRoute（仅活动 run 或正在拆分 → workspace；待确认/历史拆分不抢主窗，落 chat + 顶栏「返回确认」）
 * H2：get_plan_meta → planExecBadgeInfo + partitionPlanItems；chooser/rail「显示已执行」共用
 * E0–E2：管理入口不弹层；统一 startExecuteFromSelection；拆完非 workspace 强制回跳
 */
/* cco desktop — plan */

function isPlanSessionActive(phase = state.phase) {
  return phase === "planning" || phase === "confirm";
}

function stashPlanSession(projectPath = state.selectedPath) {
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

function restorePlanSession(projectPath) {
  const s = state.planSessions[projectPath];
  if (!s) return false;
  state.phase = s.phase || "pick";
  state.planJobId = s.planJobId || null;
  state.planJob = s.planJob || null;
  state.selectedPlan = s.selectedPlan || state.selectedPlan;
  state.confirmTaskId = s.confirmTaskId || null;
  state.planStartedAt = s.planStartedAt || 0;
  if (s.assigning) setAssignBusy(true);
  if (state.phase === "planning" && state.planJobId) startPlanJobPoll();
  return true;
}

function clearPlanSession(projectPath = state.selectedPath) {
  if (projectPath) delete state.planSessions[projectPath];
}

/** 把磁盘/API 返回的 plan job 接到 UI（不自动开跑） */
function applyRestoredPlanJob(view, { resumePoll = true } = {}) {
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
    if (resumePoll) startPlanJobPoll();
  } else if (status === "planned" || status === "confirmed") {
    // confirmed 也可再次「开始运行」，不必重拆
    state.phase = "confirm";
    stopPlanJobPoll();
    setAssignBusy(false);
  } else {
    return false;
  }
  stashPlanSession(state.selectedPath);
  return true;
}

async function tryRestorePersistedPlanJob(projectPath) {
  if (!projectPath) return false;
  try {
    const view = await invoke("latest_plan_job_cmd", { project: projectPath });
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
function hasMonitorableActivity() {
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
function resolveEntryRoute() {
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
async function applyEntryRoute() {
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
    renderPhasePanels();
    renderPlanPicker();
    renderWorkspace();
    updateTopPlanInfo();
    updateBgPlanBanner();
    if (state.phase === "planning" && state.planJobId) {
      startPlanJobPoll();
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
    if (typeof renderPlanPicker === "function") renderPlanPicker();
  } catch (_) {}
  try {
    updateTopPlanInfo();
  } catch (_) {}
  try {
    updateBgPlanBanner();
  } catch (_) {}
  return route;
}

/** 从聊天/设置等页回到 workspace 监视（规划相位或 CLI 看板） */
function goToPlanMonitor() {
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
    renderPhasePanels();
    renderPlanPicker();
    if (state.phase === "planning" && state.planJobId) {
      startPlanJobPoll();
      refreshPlanJob().catch(() => {});
    }
  }
  renderWorkspace();
  updateTopPlanInfo();
  updateBgPlanBanner();
  try {
    if (typeof renderPlanPicker === "function") renderPlanPicker();
  } catch (_) {}
}

/** 后台 banner 关断签名：phase / job / run 变化后重新显示 */
const BG_BANNER_DISMISS_KEY = "cco.bgBannerDismissSig";

function bgBannerActivitySig() {
  return [
    state.phase || "",
    state.planJobId || "",
    state.live?.run_id || "",
    state.selectedPlan || state.live?.plan_path || "",
  ].join("|");
}

function isBgBannerDismissed() {
  try {
    return localStorage.getItem(BG_BANNER_DISMISS_KEY) === bgBannerActivitySig();
  } catch (_) {
    return false;
  }
}

function dismissBgPlanBanner() {
  try {
    localStorage.setItem(BG_BANNER_DISMISS_KEY, bgBannerActivitySig());
  } catch (_) {}
  const bar = document.getElementById("bg-plan-banner");
  if (bar) bar.hidden = true;
}

function updateBgPlanBanner() {
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

async function selectProject(path) {
  // 同项目再点：H0 按活动态路由（禁止无条件 workspace）
  if (path && path === state.selectedPath) {
    renderProjectList();
    await applyEntryRoute();
    if (state.phase === "planning" && state.planJobId) {
      refreshPlanJob().catch(() => {});
    }
    return;
  }

  // 多项目可并行：离开旧项目只缓存会话，不停止其后台 CLI / run
  if (state.selectedPath) {
    state.lastWorkspacePath = state.selectedPath;
  }
  stashPlanSession(state.selectedPath);
  stopPlanJobPoll();
  setAssignBusy(false);

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
  await Promise.all([loadLive(), loadPlansForPicker(), ensureDoctor()]);

  if (restoredMem) {
    // 活动 run 优先看板；否则保留内存 plan 会话 phase（待确认不抢 chat）
    if (hasActiveRun()) {
      state.phase = "running";
      state.planCollapsed = true;
    }
    await applyEntryRoute();
    if (state.phase === "planning" && state.planJobId) {
      await refreshPlanJob().catch(() => {});
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
        await refreshPlanJob().catch(() => {});
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
      await selectPlan(candidate, { keepSession: true });
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
        renderPlanPicker();
      } catch (_) {}
    }
  }
}

function applyFlowModeBadge(rowId, badgeId, hintId, mode) {
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

function refreshFlowStrips(phaseOverride) {
  if (typeof flowStageStripHtml !== "function") return;
  const ph = phaseOverride || state.phase;
  const hostPlan = $("#flow-strip-planning");
  const hostConfirm = $("#flow-strip-confirm");
  const hostRun = $("#flow-strip-running");
  if (hostPlan) {
    if (ph === "planning") {
      hostPlan.innerHTML = flowStageStripHtml("planning");
      hostPlan.hidden = false;
    } else {
      hostPlan.hidden = true;
    }
  }
  if (hostConfirm) {
    if (ph === "confirm") {
      hostConfirm.innerHTML = flowStageStripHtml("confirm");
      hostConfirm.hidden = false;
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
      hostRun.innerHTML = flowStageStripHtml(
        fail ? "fail" : done ? "done" : "running"
      );
      hostRun.hidden = false;
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

function renderPhasePanels() {
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
    renderConfirmPanel();
  }
  try { updateBgPlanBanner(); } catch (_) {}
}

async function addProjectFromModal() {
  const path = $("#m-project-path").value.trim();
  const name = $("#m-project-name").value.trim() || null;
  if (!path) return toast("请选择项目路径");
  try {
    await invoke("add_project_cmd", { path, name });
    toast("已添加项目");
    closeModal();
    await loadProjects();
    await selectProject(path);
  } catch (e) {
    toast(String(e));
  }
}

async function pickFolderToModal() {
  try {
    const selected = await openNativeDialog({ directory: true, multiple: false });
    if (selected) $("#m-project-path").value = selected;
  } catch (e) {
    toast(String(e));
  }
}

async function removeSelectedProject() {
  if (!state.selectedPath) return;
  if (hasActiveRun()) {
    toastRunLocked("关闭/移除项目");
    return;
  }
  try {
    const path = state.selectedPath;
    await invoke("remove_project_cmd", { path });
    toast("已移除项目");
    clearPlanSession(path);
    stopPlanJobPoll();
    setAssignBusy(false);
    state.planJobId = null;
    state.planJob = null;
    state.phase = "pick";
    state.selectedPath = null;
    state.live = null;
    await loadProjects();
    goHome();
  } catch (e) {
    toast(String(e));
  }
}

/* 隐藏当前运行视图（不清除运行记录，不删除项目） */
async function dismissRun() {
  // 只收起运行视图；若在规划/确认则保留
  state.live = null;
  state.selectedTaskId = null;
  if (!isPlanSessionActive()) {
    state.phase = "pick";
  }
  state.planCollapsed = false;
  renderWorkspace();
  updateBgPlanBanner();
}

/* ── Doctor gate ── */
async function ensureDoctor(force = false) {
  const now = Date.now();
  if (!force && state.doctorCache && now - state.doctorCache.at < 60_000) {
    renderDoctorWarn();
    return state.doctorCache;
  }
  try {
    const d = await invoke("doctor_cmd", { project: state.selectedPath || null });
    state.doctorCache = { ok: !!d.ok, at: now, lines: d.lines || [] };
  } catch (e) {
    state.doctorCache = {
      ok: false,
      at: now,
      lines: [{ name: "doctor", ok: false, detail: String(e) }],
    };
  }
  renderDoctorWarn();
  return state.doctorCache;
}

function renderDoctorWarn() {
  const bar = $("#doctor-warn");
  if (!bar || state.page !== "workspace") return;
  const d = state.doctorCache;
  if (!d || d.ok) {
    bar.hidden = true;
    return;
  }
  const fails = (d.lines || []).filter((l) => !l.ok);
  const key = fails.map((l) => l.name + ":" + l.detail).join("|");
  if (state.doctorDismissedKey && state.doctorDismissedKey === key) {
    bar.hidden = true;
    return;
  }
  const live = state.live;
  const st = String(live?.run_status || "").toLowerCase();
  const historyOk = live && ["completed", "done"].includes(st);
  // 历史已成功：默认不刷黄条，避免「明明跑完还骂环境」
  if (historyOk && !isLiveStatus(st)) {
    bar.hidden = true;
    return;
  }
  const detail = fails
    .map((l) => `${l.name}: ${l.detail}`)
    .slice(0, 2)
    .join(" · ");
  bar.classList.add("soft");
  const textEl = $("#doctor-warn-text");
  if (textEl) {
    textEl.textContent =
      detail || "环境检查未通过。若 Claude 已安装，点「重新检查」或设置 CCO_CLAUDE_BIN。";
  }
  bar.hidden = false;
}

/** 计划路径是否属于当前项目（相对路径，或绝对路径前缀为本项目） */
function isPlanUnderProject(planPath, projectRoot = state.selectedPath) {
  if (!planPath || !projectRoot) return false;
  const root = String(projectRoot).replace(/[/\\]+$/, "");
  let p = String(planPath).trim().replace(/^file:\/\//, "");
  if (!p) return false;
  // 绝对路径：必须落在当前项目下
  if (p.startsWith("/") || /^[A-Za-z]:[\\/]/.test(p)) {
    return p === root || p.startsWith(root + "/") || p.startsWith(root + "\\");
  }
  // 相对路径：拒绝跳出项目
  if (p === ".." || p.startsWith("../") || p.startsWith("..\\") || p.includes("/../") || p.includes("\\..\\")) {
    return false;
  }
  return true;
}

/* ══════════════════════════════════════════════
 * H2 — shared plan exec badge + history filter
 * chooser 与 plan-rail 共用；数据源 = list_plan_meta（非 mtime）
 * ══════════════════════════════════════════════ */

/** Badge from PlanMeta: 已执行 / 失败过 / 未执行 */
function planExecBadgeInfo(item) {
  if (!item) return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
  if (item.ever_completed || item.everCompleted) {
    return { label: "已执行", cls: "plan-rail-badge-done", kind: "done" };
  }
  const st = String(item.last_run_status || item.lastRunStatus || "").toLowerCase();
  if (st && ["failed", "aborted", "timeout", "stopped"].includes(st)) {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  if (st && st !== "completed" && st !== "done" && st !== "") {
    // had a non-success terminal/partial run
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
}

function planIsEverCompleted(item) {
  if (!item) return false;
  return !!(item.ever_completed || item.everCompleted);
}

/** Lookup meta for a path (relative preferred); empty stub if unknown. */
function planMetaForPath(path, root = state.selectedPath) {
  if (!path) return { path: "", title: null, ever_completed: false, last_run_status: null };
  const norm = (typeof normalizePlanPath === "function" ? normalizePlanPath(path, root) : null) || path;
  const by = state.planMetaByPath || {};
  return (
    by[norm] ||
    by[path] || {
      path: norm,
      title: null,
      ever_completed: false,
      last_run_status: null,
      last_run_id: null,
      last_run_finished_at: null,
    }
  );
}

/**
 * Split items into active (always shown) vs history (ever_completed, collapsible).
 * pinPaths always stay in active even if completed (draft/selected/manual).
 */
function partitionPlanItems(items, { showExecuted = false, pinPaths = [] } = {}) {
  const pins = new Set(
    (pinPaths || []).filter(Boolean).map((p) => String(p))
  );
  const active = [];
  const history = [];
  for (const it of items || []) {
    const path = it.path || it;
    const meta = typeof it === "string" ? planMetaForPath(it) : it;
    const completed = planIsEverCompleted(meta);
    const pinned = pins.has(path) || pins.has(meta.path);
    if (completed && !pinned) {
      history.push(typeof it === "string" ? { ...meta, path } : it);
    } else {
      active.push(typeof it === "string" ? { ...meta, path } : it);
    }
  }
  return {
    active,
    history,
    // When toggle on, show both; when off, only active (history collapsed/hidden)
    visible: showExecuted ? active.concat(history) : active,
    historyHidden: !showExecuted && history.length > 0,
    historyCount: history.length,
  };
}

function setShowExecutedPlans(on) {
  state.showExecutedPlans = !!on;
  try {
    localStorage.setItem("cco.showExecutedPlans", state.showExecutedPlans ? "1" : "0");
  } catch (_) {}
  syncShowExecutedToggles();
  if (state.planChooserOpen) renderPlanChooser();
  if (typeof renderPlanRail === "function") {
    try {
      renderPlanRail();
    } catch (_) {}
  }
  if (state.page === "plans" && typeof renderPlansMgmtPage === "function") {
    try {
      renderPlansMgmtPage();
    } catch (_) {}
  }
}

function syncShowExecutedToggles() {
  const on = !!state.showExecutedPlans;
  for (const id of [
    "chooser-show-executed",
    "plan-rail-show-executed",
    "plans-mgmt-show-executed",
  ]) {
    const el = document.getElementById(id);
    if (el && el.type === "checkbox") el.checked = on;
  }
}

/** Normalize get_plan_meta / fallback list into state.planMetaItems + byPath. */
function applyPlanMetaItems(items, root = state.selectedPath) {
  const list = (Array.isArray(items) ? items : [])
    .map((m) => {
      const path = normalizePlanPath(m.path || m, root) || m.path || m;
      return {
        path,
        title: m.title || null,
        ever_completed: !!(m.ever_completed || m.everCompleted),
        last_run_status: m.last_run_status || m.lastRunStatus || null,
        last_run_id: m.last_run_id || m.lastRunId || null,
        last_run_finished_at: m.last_run_finished_at || m.lastRunFinishedAt || null,
      };
    })
    .filter((m) => m.path && isPlanUnderProject(m.path, root));
  state.planMetaItems = list;
  const by = {};
  for (const m of list) by[m.path] = m;
  state.planMetaByPath = by;
  return list;
}

async function loadPlansForPicker() {
  if (!state.selectedPath) {
    state.plans = [];
    state.planMetaItems = [];
    state.planMetaByPath = {};
    state.plansLoading = false;
    if (state.planChooserOpen) renderPlanChooser();
    updateChooserAssignState();
    return [];
  }
  state.plansLoading = true;
  if (state.planChooserOpen) renderPlanChooser();
  try {
    const root = state.selectedPath;
    // H2: prefer list_plan_meta (path + ever_completed / last_run_*); fall back to paths
    let list = [];
    let metas = null;
    try {
      metas = await invoke("get_plan_meta", { project: root });
    } catch (_) {
      metas = null;
    }
    if (Array.isArray(metas) && metas.length) {
      const applied = applyPlanMetaItems(metas, root);
      list = applied.map((m) => m.path);
    } else {
      const plans = (await invoke("get_plans", { project: root })) || [];
      list = (Array.isArray(plans) ? plans : [])
        .map((p) => normalizePlanPath(p, root) || p)
        .filter((p) => isPlanUnderProject(p, root));
      applyPlanMetaItems(
        list.map((p) => ({
          path: p,
          title: null,
          ever_completed: false,
          last_run_status: null,
        })),
        root
      );
    }
    // 用户手动选的计划若不在扫描结果中，且仍属本项目，置顶保留
    const selected = normalizePlanPath(state.selectedPlan, root) || state.selectedPlan;
    if (selected && isPlanUnderProject(selected, root) && !list.includes(selected)) {
      list.unshift(selected);
      if (!state.planMetaByPath[selected]) {
        const stub = {
          path: selected,
          title: null,
          ever_completed: false,
          last_run_status: null,
          last_run_id: null,
          last_run_finished_at: null,
        };
        state.planMetaItems = [stub, ...(state.planMetaItems || [])];
        state.planMetaByPath[selected] = stub;
      }
    }
    // 若当前选中已不在本项目，清掉，避免列表/分配指向别的目录
    if (state.selectedPlan && !isPlanUnderProject(state.selectedPlan, root) && !isPlanUnderProject(selected, root)) {
      state.selectedPlan = null;
    } else if (selected && isPlanUnderProject(selected, root)) {
      state.selectedPlan = selected;
    }
    state.plans = list;
  } catch (e) {
    console.warn("loadPlansForPicker", e);
    toast(String(e));
  } finally {
    state.plansLoading = false;
  }
  if (state.planChooserOpen) renderPlanChooser();
  renderPlanPicker();
  updateChooserAssignState();
  // Keep rail in sync when chooser rescans
  if (typeof loadPlanRail === "function" && state.page === "chat") {
    try {
      // meta already in state — rail can re-render without re-fetch; still refresh for safety
      if (typeof renderPlanRail === "function") renderPlanRail();
    } catch (_) {}
  }
  return state.plans;
}

function defaultAssignLabel(btnId) {
  if (btnId === "btn-chooser-assign") return "开始拆分";
  return "执行此计划";
}

function setAssignBusy(busy) {
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
      if (!btn.dataset.label) btn.dataset.label = btn.textContent || "执行此计划";
      btn.innerHTML = '<span class="spinner sm" aria-hidden="true"></span><span>拆分中…</span>';
    } else {
      btn.classList.remove("is-busy");
      const active = isLiveStatus(state.live?.run_status);
      const label = btn.dataset.label || "执行此计划";
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
async function startExecuteFromSelection(planPath, opts = {}) {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("执行此计划");
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
        await loadPlansForPicker();
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
    await selectPlan(path);
  } catch (e) {
    toast(String(e?.message || e));
    return;
  }
  // C3 方案 B（可选）：跳过二次确认，沿用当前/上次并发等选项直开拆分。
  // 仍走 analyzePlanFromPicker → start_plan_job → confirm_start；禁止 start_run 旁路。
  const direct =
    opts.direct === true ||
    (opts.direct !== false &&
      typeof chatAssignDirectEnabled === "function" &&
      chatAssignDirectEnabled());
  if (direct && typeof analyzePlanFromPicker === "function") {
    if (state.page !== "workspace") showPage("workspace");
    openPlanChooser(false);
    renderPlanPicker();
    const name =
      typeof planDisplayName === "function" ? planDisplayName(path) : path;
    toast(`将执行：${name} · 直接拆分（方案 B）`);
    await analyzePlanFromPicker();
    return;
  }
  // 方案 A：始终在 workspace 打开选项层
  if (state.page !== "workspace") showPage("workspace");
  openPlanChooser(true, { fromExecute: true, expandList: false });
  try {
    await loadPlansForPicker();
  } catch (_) {}
  renderPlanChooser();
  updateChooserAssignState();
  renderPlanPicker();
  const name =
    typeof planDisplayName === "function" ? planDisplayName(path) : path;
  toast(`将执行：${name} · 确认选项后点「开始拆分」`);
}

function renderWorkspaceShell() {
  const body = $("#workspace-body");
  if (!body) return;
  body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
  if (state.phase === "planning" || state.phase === "confirm") body.classList.add("mode-plan");
  else if (isLiveStatus(state.live?.run_status)) body.classList.add("mode-running");
  else if (state.phase === "done") body.classList.add("mode-done");
  else body.classList.add("mode-idle");
}

function setPlanCollapsed(collapsed) {
  // 新 UX：计划区永远紧凑；collapsed 语义保留给兼容
  state.planCollapsed = true;
  const pp = $("#plan-picker");
  if (pp) pp.classList.add("compact", "collapsed");
}

function openPlanChooser(open = true, opts = {}) {
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

function setChooserListExpanded(expanded) {
  state.chooserListExpanded = !!expanded;
  if (state.planChooserOpen) renderPlanChooser();
}

function updateChooserAssignState() {
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
    btn.textContent = active ? "运行中…" : "开始拆分";
  }
}

function renderPlanChooser() {
  const list = $("#chooser-list");
  const empty = $("#chooser-empty");
  const toggle = $("#btn-chooser-toggle-list");
  const sub = $("#chooser-sub");
  if (!list) return;
  syncShowExecutedToggles();

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
        ? "已选计划 · 确认同时进行几步后点「开始拆分」"
        : "确认同时进行几步后点「开始拆分」；可换一份计划";
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
    const meta = planMetaForPath(path, root);
    return {
      ...meta,
      path,
      title: meta.title || planDisplayName(path),
    };
  });
  // Ensure pin-only paths (manual pick) appear even if filtered later
  for (const pin of pinPaths) {
    if (pin && !items.some((it) => it.path === pin)) {
      const meta = planMetaForPath(pin, root);
      items.unshift({
        ...meta,
        path: pin,
        title: meta.title || planDisplayName(pin),
      });
    }
  }

  const parts = partitionPlanItems(items, {
    showExecuted: !!state.showExecutedPlans,
    pinPaths,
  });

  const rows = [];
  for (const it of parts.visible) {
    const path = it.path;
    const selected = path === state.selectedPlan;
    const title = it.title || planDisplayName(path);
    const badge = planExecBadgeInfo(it);
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

function renderPlanPicker() {
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

  // 计划管理 = 独立页面（page=plans），不是聊天右栏
  // E2：running/confirm 时弱化为 ghost，不与「返回执行/继续确认」抢 primary
  if (btnPlanMgmt) {
    const runOrConfirm =
      runActive ||
      state.phase === "planning" ||
      state.phase === "confirm";
    const showMgmt =
      !!state.selectedPath &&
      state.page !== "welcome" &&
      state.page !== "plans";
    btnPlanMgmt.hidden = !showMgmt;
    btnPlanMgmt.disabled = false;
    btnPlanMgmt.textContent = "计划管理";
    btnPlanMgmt.title = "进入计划管理：选中 / 预览 / 编辑文档 / 执行";
    const makePrimary = inChat && !runOrConfirm;
    btnPlanMgmt.classList.toggle("primary", makePrimary);
    btnPlanMgmt.classList.toggle("ghost", !makePrimary);
  }

  // 顶栏「返回执行/继续确认」：仅有可监视活动时；chat 页与「计划管理」分钮（≠ 计划管理）
  // 与 banner 互斥（updateBgPlanBanner 见 topMonVisible），避免三连噪声
  if (btnMonitor) {
    const showMon =
      !!state.selectedPath &&
      state.page !== "workspace" &&
      state.page !== "welcome" &&
      hasMonitorableActivity();
    btnMonitor.hidden = !showMon;
    if (showMon) {
      // 活动 run / 待确认：primary 更显眼；其它 ghost（chat 有计划管理时保持 ghost 不抢）
      const urgent =
        (runActive || (isPlanSessionActive() && !!state.planJobId)) && !inChat;
      btnMonitor.classList.toggle("primary", urgent);
      btnMonitor.classList.toggle("ghost", !urgent);
      if (runActive) {
        btnMonitor.textContent = "返回执行";
        btnMonitor.title =
      typeof flowRunningMonitorTitle === "function"
        ? flowRunningMonitorTitle()
        : "返回工作区查看执行进度";
      } else if (isRunPaused()) {
        btnMonitor.textContent = "返回执行";
        btnMonitor.title = "返回工作区查看已暂停的计划";
      } else if (isPlanSessionActive()) {
        btnMonitor.textContent =
          state.phase === "planning" ? "查看规划" : "继续确认";
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

  // 顶栏「选择计划」：workspace；chat/plans 用各自入口，隐藏以免堆按钮
  if (btnChoose) {
    btnChoose.hidden = !inWorkspace || hideForPhase;
    btnChoose.disabled = !!runActive;
    btnChoose.title = runActive ? "运行中，请先停止后再切换计划" : "选择计划";
  }
  // 顶栏「执行此计划」：仅 workspace 显示；聊天/管理页用各自 CTA
  if (btnAssign) {
    btnAssign.hidden = !inWorkspace || hideForPhase;
    if (!hideForPhase && !state.assigning) {
      btnAssign.textContent = runActive ? "运行中…" : "执行此计划";
      btnAssign.title = runActive
        ? "运行中，请先停止后再执行新计划"
        : "打开执行选项并拆分";
    }
  }

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
    updateSplitPlanChip();
    updateTopPlanInfo();
    return;
  }

  const active = runActive || isLiveStatus(state.live?.run_status);
  if (btnAssign && !state.assigning) {
    // 弹窗化后无计划也可点开选计划；仅运行中禁用
    btnAssign.disabled = !!active;
    btnAssign.textContent = active ? "运行中…" : "执行此计划";
    btnAssign.title = active
      ? "运行中，请先停止后再执行新计划"
      : "打开执行选项并拆分";
  }
  updateSplitPlanChip();

  const pauseEl = $("#pp-pause-confirm");
  if (pauseEl) {
    // checked = 规划后暂停 = 不 auto-start
    pauseEl.checked = !state.autoStartAfterPlan;
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
    syncSplitMaxParallelInputs(null, { force: false });
  }

  if (state.planChooserOpen) renderPlanChooser();
  updateSplitPlanChip();
  updateTopPlanInfo();
}

/** Top-bar summary of the latest split plan (right of 执行此计划). */
function updateSplitPlanChip() {
  const chip = $("#split-plan-chip");
  if (!chip) return;
  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const job = state.planJob;
  const st = String(job?.status || "").toLowerCase();
  const show =
    inWorkspace &&
    job &&
    (state.phase === "confirm" ||
      state.phase === "running" ||
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
function updateBudgetChip() {
  const chip = $("#budget-chip");
  const text = $("#budget-chip-text");
  if (!chip || !text) return;
  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const live = state.live;
  const job = state.planJob;
  const planCost =
    live?.planner_cost_usd != null
      ? Number(live.planner_cost_usd)
      : job?.planner_cost_usd != null
        ? Number(job.planner_cost_usd)
        : null;
  const execCost =
    live?.exec_cost_usd != null
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

function showSplitPlanConfirm(opts = {}) {
  if (!state.planJob) {
    toast("还没有拆分结果");
    return;
  }
  const wantEdit = !!opts.edit;
  // Remember where to return when viewing split during a live/paused run.
  if ((hasActiveRun() || isRunPaused() || state.phase === "running" || state.phase === "done") &&
      state.phase !== "confirm") {
    state.returnPhaseAfterConfirm = state.phase || (hasActiveRun() ? "running" : "done");
  } else if (!opts.keepReturn) {
    state.returnPhaseAfterConfirm = null;
  }
  state.phase = "confirm";
  state.confirmEditing = false;
  if (!state.confirmTaskId && state.planJob.tasks?.length) {
    // Prefer first pending (not-yet-run) task when editing after pause.
    const pending =
      (state.planJob.tasks || []).find((t) => canEditSelectedTask(t.id)) ||
      state.planJob.tasks[0];
    state.confirmTaskId = pending.id;
  }
  renderPhasePanels();
  renderPlanPicker();
  renderWorkspace();
  updateSplitPlanChip();
  if (wantEdit) {
    if (canEditSelectedTask(state.confirmTaskId)) {
      beginConfirmEdit();
    } else if (hasActiveRun()) {
      toast("运行中不可编辑，请先停止或待计划暂停");
    } else if (isRunPaused()) {
      toast("当前任务已执行过，请选左侧未执行任务再编辑");
    } else {
      toast("当前不可编辑");
    }
  }
}

/** Top-bar「编辑计划」：进确认页；仅暂停后、未执行任务可改。 */
function openEditPlan() {
  if (!state.planJob) {
    toast("还没有拆分结果，请先执行此计划");
    return;
  }
  if (hasActiveRun()) {
    toast("运行中不可编辑，请先停止或待计划暂停");
    return;
  }
  showSplitPlanConfirm({ edit: true });
}

function backFromConfirmToMonitor() {
  state.confirmEditing = false;
  state.phase = state.returnPhaseAfterConfirm || (hasActiveRun() ? "running" : "done");
  state.returnPhaseAfterConfirm = null;
  renderPhasePanels();
  renderPlanPicker();
  renderWorkspace();
  updateSplitPlanChip();
}

/** Concurrent workers chosen at plan-split time (1–32). */
function readSplitMaxParallel() {
  // Empty / mid-edit → keep last committed hidden value, never force "2" into the field.
  const fromChooser = parseInt($("#chooser-max-parallel")?.value, 10);
  const fromHidden = parseInt($("#pp-max-parallel")?.value, 10);
  const fromSettings = parseInt($("#s-max-parallel")?.value, 10);
  const n = Number.isFinite(fromChooser) && fromChooser > 0
    ? fromChooser
    : Number.isFinite(fromHidden) && fromHidden > 0
      ? fromHidden
      : Number.isFinite(fromSettings) && fromSettings > 0
        ? fromSettings
        : 2;
  return Math.max(1, Math.min(32, n));
}

/** Commit/clamp concurrency into both inputs. Skip the field the user is typing in. */
function syncSplitMaxParallelInputs(sourceId, { force = false } = {}) {
  const chooser = $("#chooser-max-parallel");
  const hidden = $("#pp-max-parallel");
  const active = document.activeElement;
  // While the user is editing (empty / partial), do not rewrite the visible field.
  if (
    !force &&
    chooser &&
    (active === chooser || chooser.dataset.editing === "1")
  ) {
    // Still mirror a valid number into hidden if present.
    const typed = parseInt(chooser.value, 10);
    if (Number.isFinite(typed) && typed > 0 && hidden) {
      hidden.value = String(Math.max(1, Math.min(32, typed)));
    }
    return readSplitMaxParallel();
  }
  const n = readSplitMaxParallel();
  if (chooser && sourceId !== "chooser-max-parallel") {
    if (force || active !== chooser) chooser.value = String(n);
  }
  if (hidden) hidden.value = String(n);
  return n;
}

/** Clamp concurrency on blur / assign; allow empty mid-edit. */
function commitSplitMaxParallel(inputEl) {
  if (!inputEl) return readSplitMaxParallel();
  inputEl.dataset.touched = "1";
  inputEl.dataset.editing = "0";
  const n = Math.max(1, Math.min(32, parseInt(inputEl.value, 10) || 2));
  inputEl.value = String(n);
  const hidden = $("#pp-max-parallel");
  if (hidden) hidden.value = String(n);
  const chooser = $("#chooser-max-parallel");
  if (chooser && chooser !== inputEl) chooser.value = String(n);
  return n;
}

function updateTopPlanInfo() {
  // 红框1：顶栏只显示计划名，不显示路径
  const title = $("#page-title");
  const sub = $("#page-sub");
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  let plan =
    state.selectedPlan ||
    normalizePlanPath(state.live?.plan_path) ||
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

function renderPlanPreview() {
  // 紧凑模式不再展示大预览；保留函数避免旧调用报错
  return;
}

async function selectPlan(planPath, opts = {}) {
  const keepSession = !!opts.keepSession;
  const next = normalizePlanPath(planPath) || planPath || null;
  const samePlan = next && state.selectedPlan && next === state.selectedPlan;

  // 运行中禁止换源计划（可 keepSession 只用于恢复当前）
  if (hasActiveRun() && !keepSession && !samePlan && !opts.force) {
    toastRunLocked("切换计划");
    return;
  }

  // 规划/确认进行中：默认不销毁会话（后台继续）
  if (isPlanSessionActive() && !opts.force) {
    if (samePlan || keepSession) {
      state.selectedPlan = next || state.selectedPlan;
      renderPlanPicker();
      updateTopPlanInfo();
      if (state.planChooserOpen) updateChooserAssignState();
      return;
    }
    // 换了另一份计划：提示并拒绝静默清空
    toast("规划进行中：请先「返回选计划/重新规划」，或等待完成");
    return;
  }

  state.selectedPlan = next;
  state.planPreview = null;
  renderPhasePanels();
  renderPlanPicker();
  if (!planPath) return;
  try {
    state.planPreview = await invoke("preview_plan_cmd", {
      project: state.selectedPath,
      plan: planPath,
    });
  } catch (e) {
    console.warn("preview failed", e);
    state.planPreview = {
      name: planDisplayName(state.selectedPlan || planPath),
      task_count: "?",
      max_parallel: "?",
    };
  }
  renderPlanPicker();
  updateTopPlanInfo();
  if (state.planChooserOpen) updateChooserAssignState();
}

async function pickPlanFileForPicker() {
  try {
    const proj = state.selectedPath;
    if (!proj) {
      toast("请先选择项目");
      return;
    }
    const root = String(proj).replace(/[/\\]+$/, "");
    // 默认打开当前项目目录，避免系统文件框落在上次其它项目路径
    const selected = await openNativeDialog({
      multiple: false,
      defaultPath: root,
      title: "选择计划文件（当前项目内）",
      filters: [{ name: "Plan", extensions: ["md", "yaml", "yml", "json"] }],
    });
    if (!selected) return;
    const abs = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
    if (!abs) return;
    if (!isPlanUnderProject(abs, root)) {
      toast("请选择当前项目目录内的计划文件，不要选其它项目");
      return;
    }
    const rel = normalizePlanPath(abs, root) || abs;
    if (!rel || rel === root) {
      toast("请选择计划文件，而不是目录");
      return;
    }
    if (!state.plans.includes(rel)) state.plans = [rel, ...state.plans];
    await selectPlan(rel);
    // 留在弹窗内，方便直接点「开始拆分」
    if (state.planChooserOpen) {
      renderPlanChooser();
      updateChooserAssignState();
    }
  } catch (e) {
    toast(String(e));
  }
}

async function setDefaultPlan() {
  if (!state.selectedPath || !state.selectedPlan) return;
  try {
    await invoke("set_project_default_plan", {
      project: state.selectedPath,
      plan: state.selectedPlan,
    });
    const proj = state.projects.find((p) => p.path === state.selectedPath);
    if (proj) proj.default_plan = state.selectedPlan;
    toast("已设为默认计划");
  } catch (e) {
    toast(String(e));
  }
}

/** Mode B: analyze plan → plan job (does NOT start workers). */
/** 开始拆分：AI 拆分后进入确认（可编辑）；入口文案统一为「执行此计划 / 开始拆分」 */
async function analyzePlanFromPicker() {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (state.assigning) return;
  if (hasActiveRun()) {
    toastRunLocked("执行此计划");
    return;
  }
  if (!state.selectedPlan) {
    openPlanChooser(true);
    toast("请先选择计划");
    return;
  }
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }

  const planMode = $("#pp-plan-mode")?.value || "ai";
  const provider = $("#pp-provider")?.value || "claude";
  const mode = $("#pp-mode")?.value || "print";
  // Commit any in-progress concurrency edit before reading.
  const maxParallel = commitSplitMaxParallel($("#chooser-max-parallel") || $("#pp-max-parallel"));

  const doc = await ensureDoctor(true);
  if (doc && !doc.ok && provider !== "fake" && planMode !== "fake") {
    // 不硬拦死：提示 + 允许用户忽略后重试；首次仍阻止避免必败
    if (err) {
      err.textContent = "环境未就绪。可点上方「忽略」后重试，或到环境检查配置 Claude 路径";
      err.hidden = false;
    }
    renderDoctorWarn();
    // 若用户已忽略同类警告，允许继续
    const fails = (doc.lines || []).filter((l) => !l.ok);
    const key = fails.map((l) => l.name + ":" + l.detail).join("|");
    if (!(state.doctorDismissedKey && state.doctorDismissedKey === key)) {
      return;
    }
  }

  setAssignBusy(true);
  state.phase = "planning";
  state.planJob = null;
  state.planJobId = null;
  state.confirmEditing = false;
  clearPlanSession(state.selectedPath);
  stopPlanJobPoll();
  openPlanChooser(false);
  // 规划 UI 在 workspace；从聊天/其它页分配时先切回
  if (state.page !== "workspace") showPage("workspace");
  renderPhasePanels();
  renderPlanPicker();
  renderWorkspaceShell();
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
    const view = await invoke("start_plan_job_cmd", {
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
    stashPlanSession(state.selectedPath);
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
      renderPhasePanels();
      renderPlanPicker();
      setAssignBusy(false);
    } else {
      // async AI planning — keep busy + poll until planned/failed
      state.phase = "planning";
      renderPhasePanels();
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
    renderPhasePanels();
    renderPlanPicker();
    setAssignBusy(false);
  }
}

function stopPlanJobPoll() {
  if (state.planJobPollTimer) {
    clearInterval(state.planJobPollTimer);
    state.planJobPollTimer = null;
  }
}

function startPlanJobPoll() {
  stopPlanJobPoll();
  state.planJobPollTimer = setInterval(() => {
    refreshPlanJob().catch((e) => console.warn("plan poll", e));
  }, 600);
}

function planHasOptionalTasks(view) {
  const tasks = view?.tasks || [];
  return tasks.some((t) => !!t.optional);
}

function isSystemPostTask(t) {
  if (!t) return false;
  const id = String(t.id || "");
  if (id === "sys-post-inspect" || id === "sys-post-git-push") return true;
  if (id.startsWith("sys-post-")) return true;
  return String(t.group || "") === "系统收尾";
}

function countOptionalIncluded(view) {
  const tasks = view?.tasks || [];
  return tasks.filter((t) => t.optional && t.include !== false).length;
}

/**
 * Whether confirm screen must wait for human before auto-start.
 * - Business optionals (非系统): always block（默认不跑，须人勾选）
 * - System post only（设置开启、默认勾选）: 全部 include 则可 auto-start
 */
function planNeedsOptionalConfirm(view) {
  const tasks = view?.tasks || [];
  const businessOpt = tasks.filter((t) => !!t.optional && !isSystemPostTask(t));
  if (businessOpt.length > 0) return true;
  const sysOpt = tasks.filter((t) => !!t.optional && isSystemPostTask(t));
  if (!sysOpt.length) return false;
  // 系统收尾有未勾选 → 仍停一下让用户看到；全勾选则不挡 auto-start
  return sysOpt.some((t) => t.include === false);
}

async function advancePlannedJob(view) {
  stopPlanJobPoll();
  state.planJob = view;
  if (!state.confirmTaskId && view.tasks?.length) {
    state.confirmTaskId = view.tasks[0].id;
  }
  stashPlanSession(state.selectedPath);
  updateBgPlanBanner();
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
    renderPhasePanels();
    renderPlanPicker();
    setAssignBusy(false);
    await confirmAndStart();
  } else {
    const optHint = needsOpt
      ? "；含可选项，请确认勾选后再开始"
      : hasOptional
        ? "；含系统收尾（默认已勾选）"
        : "，请确认后开始";
    toast(`${how}：${n} 个任务${optHint}`);
    state.phase = "confirm";
    renderPhasePanels();
    renderPlanPicker();
    setAssignBusy(false);
  }
}

async function refreshPlanJob() {
  if (!state.planJobId) return;
  try {
    const view = await invoke("get_plan_job_cmd", {
      jobId: state.planJobId,
    });
    state.planPollFails = 0;
    state.planJob = view;
    const status = String(view.status || "").toLowerCase();
    fillPlannerLog(view);

    if (status === "planned") {
      await advancePlannedJob(view);
    } else if (status === "plan_failed") {
      stopPlanJobPoll();
      setAssignBusy(false);
      state.phase = "pick";
      const err = $("#pp-error");
      if (err) {
        err.textContent = view.error || "规划失败";
        err.hidden = false;
      }
      toast(view.error || "规划失败");
      renderPhasePanels();
      renderPlanPicker();
    } else if (status === "planning") {
      state.phase = "planning";
      // 超时保护：超过 12 分钟仍 planning
      if (state.planStartedAt && Date.now() - state.planStartedAt > 12 * 60 * 1000) {
        stopPlanJobPoll();
        setAssignBusy(false);
        state.phase = "pick";
        toast("拆分超时：智能拆分可能无响应。请检查环境，或在更多选项里改用「模拟拆分」。");
        renderPhasePanels();
        renderPlanPicker();
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
      renderPhasePanels();
    } else if (status === "confirmed" && (view.run_id || view.runId)) {
      stopPlanJobPoll();
      setAssignBusy(false);
      state.phase = "running";
      renderPhasePanels();
    } else {
      renderPhasePanels();
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
      setAssignBusy(false);
      state.phase = "pick";
      toast("无法轮询规划任务。请点刷新重试，或用 CLI：cco plan --project ...");
      renderPhasePanels();
      renderPlanPicker();
    }
  }
}

function renderConfirmPanel() {
  const job = state.planJob;
  if (!job) return;
  const layers = job.layers || [];
  const tasks = job.tasks || [];
  const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));
  const runLocked = hasActiveRun();
  const paused = isRunPaused();

  const st = String(job.status || "").toLowerCase();
  const reused = st === "confirmed";
  $("#confirm-title").textContent = job.plan_name
    ? `${reused ? "历史拆分" : "待确认"}：${job.plan_name}`
    : reused
      ? "历史拆分（可再次运行）"
      : "待确认的执行计划";
  const mpCap = job.max_parallel ?? job.maxParallel ?? "—";
  const widestWave = layers.reduce((m, l) => Math.max(m, (l || []).length), 0);
  // 依赖决定实际可并行；上限只是天花板。依赖串行时最宽波=1，看起来像「并发=1」。
  const parallelHint =
    typeof mpCap === "number" && widestWave > 0 && widestWave < mpCap
      ? ` · 依赖限制下最宽波 ${widestWave}（上限 ${mpCap} 未吃满）`
      : "";
  const optTasks = tasks.filter((t) => !!t.optional);
  const sysOpt = optTasks.filter((t) => isSystemPostTask(t));
  const bizOpt = optTasks.filter((t) => !isSystemPostTask(t));
  const optOn = optTasks.filter((t) => t.include !== false).length;
  const sysOn = sysOpt.filter((t) => t.include !== false).length;
  let optHint = "";
  if (optTasks.length > 0) {
    const bits = [`可选 ${optOn}/${optTasks.length} 已勾选`];
    if (sysOpt.length) bits.push(`系统收尾 ${sysOn}/${sysOpt.length}`);
    if (bizOpt.length) bits.push(`业务可选 ${bizOpt.filter((t) => t.include !== false).length}/${bizOpt.length}`);
    optHint = ` · ${bits.join(" · ")}`;
  }
  const confirmHint = runLocked
    ? "运行中（只读）"
    : paused
      ? "已暂停 · 仅未执行任务可编辑"
      : bizOpt.length > 0
        ? "业务可选项默认不跑 · 请勾选后再开始"
        : sysOpt.length > 0
          ? "系统收尾默认已勾选 · 可取消后开始"
          : reused
            ? "可编辑未执行任务后再次运行"
            : "可编辑 · 确认后开始";
  const modeRaw = job.digest_mode || job.digestMode || "";
  const modeLabel =
    typeof flowModeLabel === "function" ? flowModeLabel(modeRaw) : "";
  const modeBit = modeLabel ? ` · ${modeLabel}` : "";
  $("#confirm-meta").textContent = `${job.task_count || tasks.length} 个步骤 · 同时最多 ${
    mpCap
  } · ${layers.length} 波${parallelHint}${optHint}${modeBit} · ${confirmHint}`;
  applyFlowModeBadge(
    "#confirm-mode-row",
    "#confirm-mode-badge",
    "#confirm-mode-hint",
    modeRaw
  );
  // Critic hygiene strip (from finish_plan_job / manual sanitize)
  const criticEl = $("#confirm-critic-note");
  if (criticEl) {
    let critic =
      job.critic_summary ||
      job.criticSummary ||
      "";
    if (critic && typeof humanizePlannerLogLine === "function") {
      critic = humanizePlannerLogLine(critic);
    }
    if (critic && String(critic).trim()) {
      criticEl.hidden = false;
      criticEl.textContent = String(critic).trim();
      const clean =
        /无需改动|未发现可疑|无需/.test(critic) &&
        !/去掉|改写|钉入|手动清理 · 去掉/.test(critic);
      criticEl.classList.toggle("is-clean", clean);
    } else {
      criticEl.hidden = true;
      criticEl.textContent = "";
      criticEl.classList.remove("is-clean");
    }
  }
  // Structured critic chips
  const chips = $("#confirm-critic-chips");
  const nEdges = job.critic_edges_removed ?? job.criticEdgesRemoved;
  const nTitles = job.critic_titles_rewritten ?? job.criticTitlesRewritten;
  const nPrompts = job.critic_prompts_tagged ?? job.criticPromptsTagged;
  const llmUsed = job.critic_llm_used ?? job.criticLlmUsed;
  const hasStats =
    nEdges != null || nTitles != null || nPrompts != null || llmUsed != null;
  if (chips) {
    if (!hasStats) {
      chips.hidden = true;
    } else {
      chips.hidden = false;
      const modeChip = $("#chip-critic-mode");
      if (modeChip) {
        modeChip.hidden = false;
        if (llmUsed === true) {
          modeChip.textContent = "智能第二跳 ✓";
          modeChip.classList.add("is-llm");
          modeChip.classList.remove("is-rules", "is-zero");
          modeChip.title = "本次拆分启用了规则校对 + 智能第二跳";
        } else {
          modeChip.textContent = "规则校对";
          modeChip.classList.add("is-rules");
          modeChip.classList.remove("is-llm", "is-zero");
          modeChip.title =
            "仅规则校对（可在设置开启「智能第二跳校对」）";
        }
      }
      const setChip = (id, label, n) => {
        const el = $(id);
        if (!el) return;
        if (n == null) {
          el.hidden = true;
          return;
        }
        el.hidden = false;
        el.textContent = `${label} ${n}`;
        el.classList.toggle("is-zero", Number(n) === 0);
      };
      setChip("#chip-critic-edges", "清依赖", nEdges);
      setChip("#chip-critic-titles", "改标题", nTitles);
      setChip("#chip-critic-prompts", "钉提示", nPrompts);
      // Cost / duration chips (only when LLM second pass ran)
      const cost = job.critic_llm_cost_usd ?? job.criticLlmCostUsd;
      const ms = job.critic_llm_ms ?? job.criticLlmMs;
      const costChip = $("#chip-critic-cost");
      const msChip = $("#chip-critic-ms");
      if (costChip) {
        if (llmUsed === true && cost != null && Number.isFinite(Number(cost))) {
          costChip.hidden = false;
          costChip.textContent = `$${Number(cost).toFixed(3)}`;
          costChip.classList.add("is-llm");
          costChip.title = "智能第二跳费用（USD）";
        } else {
          costChip.hidden = true;
        }
      }
      if (msChip) {
        if (llmUsed === true && ms != null && Number(ms) >= 0) {
          msChip.hidden = false;
          const n = Number(ms);
          msChip.textContent =
            n >= 1000 ? `${(n / 1000).toFixed(1)}s` : `${Math.round(n)}ms`;
          msChip.classList.add("is-llm");
          msChip.title = "智能第二跳耗时";
        } else {
          msChip.hidden = true;
        }
      }
    }
  }
  // Critic free-form notes (e.g. missing inspect tail)
  const notesEl = $("#confirm-critic-notes");
  const criticActions = $("#confirm-critic-actions");
  let showInspectCta = false;
  if (notesEl) {
    const notes = job.critic_notes || job.criticNotes || [];
    const list = Array.isArray(notes) ? notes.filter((n) => String(n || "").trim()) : [];
    if (!list.length) {
      notesEl.hidden = true;
      notesEl.innerHTML = "";
    } else {
      notesEl.hidden = false;
      notesEl.innerHTML = list
        .map((n) => {
          let t = String(n);
          if (typeof humanizePlannerLogLine === "function") t = humanizePlannerLogLine(t);
          if (/检验|巡检|inspect/i.test(t)) showInspectCta = true;
          // Escape via textContent path
          const li = document.createElement("li");
          li.textContent = t;
          return li.outerHTML;
        })
        .join("");
    }
  }
  // Show "enable smart critic" when this split was rules-only.
  // Use state.confirmEditing here — `const editing` is declared later (after waves);
  // reading it early is a TDZ ReferenceError that blanked the whole task list.
  const showCriticCta = llmUsed === false || llmUsed == null;
  const editingNow = !!state.confirmEditing;
  if (criticActions) {
    const inspectBtn = $("#btn-enable-post-inspect");
    const criticBtn = $("#btn-enable-planner-critic");
    const anyCta = (showInspectCta || showCriticCta) && !runLocked;
    criticActions.hidden = !anyCta;
    if (inspectBtn) {
      inspectBtn.hidden = !showInspectCta;
      inspectBtn.disabled = !!runLocked || editingNow || !state.planJobId;
    }
    if (criticBtn) {
      criticBtn.hidden = !showCriticCta || !!runLocked;
      criticBtn.disabled = !!runLocked || editingNow || !state.planJobId;
    }
  }

  const waves = $("#confirm-waves");
  waves.innerHTML = layers
    .map((layer, i) => {
      const rows = layer
        .map((id) => {
          const t = byId[id] || {
            id,
            title: id,
            depends_on: [],
            optional: false,
            include: true,
            provider: job.provider || "claude",
          };
          const sel = state.confirmTaskId === id ? " selected" : "";
          const live = liveTaskById(id);
          const liveSt = live?.status || "";
          const pending = !live || isTaskPendingStatus(liveSt);
          const deps =
            t.depends_on && t.depends_on.length
              ? `等待 ${t.depends_on.join(", ")}`
              : "可立即开始";
          const statusHint = liveSt
            ? ` · ${statusLabel(liveSt)}`
            : pending
              ? " · 未执行"
              : "";
          const isOpt = !!t.optional;
          const included = isOpt ? t.include !== false : true;
          const optClass = isOpt
            ? included
              ? " optional-on"
              : " optional-off"
            : "";
          const isSys =
            id === "sys-post-inspect" ||
            id === "sys-post-git-push" ||
            String(t.group || "") === "系统收尾";
          const optBadge = isSys
            ? `<span class="opt-badge opt-badge-sys" title="系统收尾：不参与拆解">系统</span>`
            : `<span class="opt-badge">可选</span>`;
          const checkHtml = isOpt
            ? `<label class="wave-task-check" title="${
                isSys
                  ? "系统收尾步骤：默认勾选；取消则本次不跑"
                  : "可选：勾选后才会执行"
              }" data-check-for="${esc(id)}">
                <input type="checkbox" class="wave-opt-check" data-id="${esc(id)}" ${
                  included ? "checked" : ""
                } ${runLocked || !pending ? "disabled" : ""} />
                ${optBadge}
              </label>`
            : `<span class="wave-task-req muted" title="必选步骤">必选</span>`;
          // Main path: no engine badge (provider stays in advanced edit only).
          return `<div class="wave-task-row${sel}${pending ? "" : " done-ish"}${optClass}" data-id="${esc(id)}">
            ${checkHtml}
            <button type="button" class="wave-task" data-id="${esc(id)}">
              <div class="wave-task-title">${esc(t.title || id)}</div>
              <div class="wave-task-meta muted">${esc(id)} · ${esc(deps)}${esc(statusHint)}</div>
            </button>
          </div>`;
        })
        .join("");
      return `<div class="wave-block">
        <div class="wave-label">第 ${i + 1} 波${layer.length > 1 ? "（可并行）" : ""}</div>
        ${rows}
      </div>`;
    })
    .join("");

  $$(".wave-task", waves).forEach((b) => {
    b.onclick = () => {
      if (state.confirmEditing) {
        toast("请先保存或取消当前编辑");
        return;
      }
      state.confirmTaskId = b.dataset.id;
      renderConfirmPanel();
    };
  });

  $$(".wave-opt-check", waves).forEach((cb) => {
    cb.onchange = async (ev) => {
      ev.stopPropagation();
      if (state.confirmEditing) {
        cb.checked = !cb.checked;
        toast("请先保存或取消当前编辑");
        return;
      }
      if (hasActiveRun()) {
        cb.checked = !cb.checked;
        toast("运行中不可改勾选");
        return;
      }
      const taskId = cb.dataset.id;
      const include = !!cb.checked;
      try {
        const view = await invoke("update_plan_task_cmd", {
          jobId: state.planJobId,
          taskId,
          include,
        });
        state.planJob = view;
        state.planJobId = view.job_id || view.jobId || state.planJobId;
        stashPlanSession(state.selectedPath);
        toast(include ? `已勾选：将执行「${byId[taskId]?.title || taskId}」` : `已取消：不跑「${byId[taskId]?.title || taskId}」`);
        renderConfirmPanel();
        renderPlanPicker();
      } catch (e) {
        cb.checked = !include;
        toast(String(e));
      }
    };
    // Clicking the label should not also select the task row via bubbling only —
    // still allow selecting when interacting with checkbox area.
    cb.onclick = (ev) => ev.stopPropagation();
  });

  const cur = byId[state.confirmTaskId] || tasks[0];
  const metaEl = $("#confirm-task-meta");
  const promptEl = $("#confirm-task-prompt");
  const editForm = $("#confirm-edit-form");
  const editBtn = $("#btn-confirm-edit");
  const deleteBtn = $("#btn-confirm-delete");
  const cancelBtn = $("#btn-confirm-edit-cancel");
  const saveBtn = $("#btn-confirm-edit-save");
  const promptLabel = $("#confirm-prompt-label");
  const providerSel = $("#confirm-task-provider");
  const taskEditable = !!cur && canEditSelectedTask(cur.id);
  const editing = !!state.confirmEditing && taskEditable;
  const curProvider = (
    cur?.provider ||
    job.provider ||
    $("#pp-provider")?.value ||
    "claude"
  ).toLowerCase();

  if (cur) {
    state.confirmTaskId = cur.id;
    $("#confirm-task-title").textContent = `${cur.title}（${cur.id}）`;
    $("#confirm-task-title").classList.remove("muted");
    const kind = cur.optional
      ? cur.include !== false
        ? "可选项 · 已勾选（会执行）"
        : "可选项 · 未勾选（默认不跑）"
      : "必选步骤";
    let depTitles = [];
    if (cur.depends_on?.length > 0) {
      depTitles = cur.depends_on.map((id) => {
        const d = byId[id];
        return d ? `${d.title}` : id;
      });
    }
    $("#confirm-task-deps").textContent =
      typeof flowConfirmDepsLine === "function"
        ? flowConfirmDepsLine(kind, depTitles)
        : depTitles.length
          ? `${kind} · 等待：${depTitles.join(" · ")}`
          : `${kind} · 无依赖，可进首波`;
    const full =
      cur.prompt ||
      cur.prompt_preview ||
      cur.promptPreview ||
      "";
    if (editing) {
      if (promptEl) promptEl.hidden = true;
      if (editForm) editForm.hidden = false;
      if (promptLabel) {
        promptLabel.textContent =
          typeof flowPromptLabel === "function"
            ? flowPromptLabel(true)
            : "编辑步骤说明";
      }
      const titleInput = $("#confirm-edit-title");
      const promptInput = $("#confirm-edit-prompt");
      const editProv = $("#confirm-edit-provider");
      const depsBox = $("#confirm-edit-deps");
      if (titleInput && document.activeElement !== titleInput) {
        titleInput.value = cur.title || "";
      }
      if (promptInput && document.activeElement !== promptInput) {
        promptInput.value = full;
      }
      if (editProv && document.activeElement !== editProv) {
        editProv.value = curProvider;
      }
      // P2-1: multi-select deps (other tasks only). Rebuild when task id changes.
      if (depsBox && depsBox.dataset.forTask !== cur.id) {
        depsBox.dataset.forTask = cur.id;
        const others = tasks.filter((t) => t.id !== cur.id);
        if (!others.length) {
          depsBox.innerHTML =
            '<span class="confirm-edit-deps-empty">没有其它步骤可依赖</span>';
        } else {
          const selected = new Set(cur.depends_on || []);
          depsBox.innerHTML = others
            .map((t) => {
              const checked = selected.has(t.id) ? "checked" : "";
              return (
                `<label>` +
                `<input type="checkbox" class="confirm-dep-check" value="${esc(t.id)}" ${checked} />` +
                `<span>${esc(t.title || t.id)} <span class="muted">(${esc(t.id)})</span></span>` +
                `</label>`
              );
            })
            .join("");
        }
      }
    } else {
      if (promptEl) {
        promptEl.hidden = false;
        promptEl.classList.add("md-body");
        promptEl.innerHTML = renderMarkdown(full);
        promptEl.scrollTop = 0;
      }
      if (editForm) editForm.hidden = true;
      if (promptLabel) {
        promptLabel.textContent =
          typeof flowPromptLabel === "function"
            ? flowPromptLabel(false)
            : "完整步骤说明（执行时按此自动进行）";
      }
    }
    if (metaEl) {
      const chars = [...full].length;
      metaEl.hidden = false;
      metaEl.textContent =
        typeof flowConfirmMetaLine === "function"
          ? flowConfirmMetaLine(chars, editing)
          : editing
            ? `编辑中 · 说明 ${chars} 字`
            : `说明 ${chars} 字 · 点左侧可切换步骤`;
    }
  } else {
    $("#confirm-task-title").textContent = "选择左侧任务查看说明";
    $("#confirm-task-title").classList.add("muted");
    $("#confirm-task-deps").textContent = "";
    if (promptEl) {
      promptEl.hidden = false;
      promptEl.innerHTML = "";
    }
    if (editForm) editForm.hidden = true;
    if (metaEl) {
      metaEl.hidden = true;
      metaEl.textContent = "";
    }
  }

  // multi-cli P2-6 / H4: show per-task engine on confirm detail (not only hidden advanced).
  const providerField = $("#confirm-provider-field");
  if (providerField) {
    // Visible whenever a task is selected and editable; keep hidden while editing form open
    // (form has its own provider select).
    providerField.hidden = !cur || editing;
  }
  if (providerSel) {
    if (document.activeElement !== providerSel) {
      providerSel.value = cur ? curProvider : "claude";
    }
    providerSel.disabled = !cur || !taskEditable || editing || !!runLocked;
    providerSel.title = !cur
      ? "选择左侧步骤"
      : runLocked
        ? "运行中不可改执行通道"
        : !taskEditable
          ? "仅未执行步骤可改执行通道"
          : editing
            ? "请在编辑表单中改执行通道"
            : "本步骤执行通道（混跑可改）";
    providerSel.onchange = async () => {
      if (!cur || !taskEditable || hasActiveRun() || state.confirmEditing) {
        providerSel.value = curProvider;
        return;
      }
      const next = (providerSel.value || "claude").toLowerCase();
      if (next === curProvider) return;
      try {
        const view = await invoke("update_plan_task_cmd", {
          jobId: state.planJobId,
          taskId: cur.id,
          provider: next,
        });
        state.planJob = view;
        state.planJobId = view.job_id || view.jobId || state.planJobId;
        stashPlanSession(state.selectedPath);
        const label =
          typeof flowEngineLabel === "function"
            ? flowEngineLabel(next)
            : next === "codex"
              ? "备用通道"
              : next === "fake"
                ? "演练"
                : "默认通道";
        toast(`已设「${cur.title || cur.id}」→ ${label}`);
        renderConfirmPanel();
        renderPlanPicker();
      } catch (e) {
        providerSel.value = curProvider;
        toast(String(e));
      }
    };
  }

  if (editBtn) {
    editBtn.hidden = !cur || editing || !taskEditable;
    editBtn.disabled = !taskEditable;
    if (!taskEditable && cur) {
      editBtn.title = runLocked
        ? "运行中不可编辑"
        : paused
          ? "该任务已执行，不可编辑"
          : "当前状态不可编辑";
    } else {
      editBtn.title = "编辑标题 / 说明 / 依赖";
    }
  }
  if (deleteBtn) {
    const canDelete =
      !!cur && taskEditable && !editing && tasks.length > 1 && !runLocked;
    deleteBtn.hidden = !canDelete;
    deleteBtn.disabled = !canDelete;
    deleteBtn.title = !cur
      ? "选择左侧步骤"
      : runLocked
        ? "运行中不可删除"
        : tasks.length <= 1
          ? "至少保留一个步骤"
          : "从本轮拆分中删除此步骤";
  }
  if (cancelBtn) cancelBtn.hidden = !editing;
  if (saveBtn) saveBtn.hidden = !editing;

  $("#confirm-error").hidden = true;
  const startBtn = $("#btn-confirm-start");
  if (startBtn) {
    startBtn.disabled = !!runLocked || editing;
    startBtn.textContent = runLocked
      ? "运行中…"
      : paused
        ? "继续运行"
        : st === "confirmed"
          ? "再次运行"
          : "开始运行";
  }
  const replanBtn = $("#btn-replan");
  if (replanBtn) {
    replanBtn.disabled = !!runLocked || editing;
    replanBtn.title = runLocked
      ? "运行中，请先停止后再重新拆分"
      : "保留当前计划，立刻按最新规则再拆一轮";
    if (!runLocked) replanBtn.textContent = "重新拆分";
  }
  const sanitizeBtn = $("#btn-sanitize-deps");
  if (sanitizeBtn) {
    sanitizeBtn.disabled = !!runLocked || editing || !state.planJobId;
    sanitizeBtn.hidden = !!runLocked;
    sanitizeBtn.title = runLocked
      ? "运行中不可改依赖"
      : "去掉说明里未写明原因的依赖，让正交步骤可并行";
  }
  const backBtn = $("#btn-confirm-back");
  if (backBtn) {
    // Show when viewing split during/after a run (chip open), not on first confirm.
    const showBack =
      !!runLocked ||
      !!paused ||
      state.returnPhaseAfterConfirm != null ||
      state.phase === "running" ||
      (st === "confirmed" && state.live?.run_id);
    backBtn.hidden = !showBack;
  }
  updateSplitPlanChip();
}

function beginConfirmEdit() {
  if (hasActiveRun()) {
    toast("运行中不可编辑，请先停止或待计划暂停");
    return;
  }
  if (!state.planJobId || !state.confirmTaskId) {
    toast("请先选择任务");
    return;
  }
  if (!canEditSelectedTask(state.confirmTaskId)) {
    toast("仅未执行的任务可编辑（暂停后可选左侧 pending 任务）");
    return;
  }
  state.confirmEditing = true;
  renderConfirmPanel();
  setTimeout(() => $("#confirm-edit-title")?.focus(), 0);
}

function cancelConfirmEdit() {
  state.confirmEditing = false;
  renderConfirmPanel();
}

async function saveConfirmEdit() {
  const err = $("#confirm-error");
  if (err) err.hidden = true;
  if (hasActiveRun()) {
    toast("运行中不可保存编辑");
    return;
  }
  if (!canEditSelectedTask(state.confirmTaskId)) {
    toast("仅未执行的任务可保存修改");
    return;
  }
  if (!state.planJobId || !state.confirmTaskId) {
    toast("没有可保存的任务");
    return;
  }
  const title = ($("#confirm-edit-title")?.value || "").trim();
  const prompt = ($("#confirm-edit-prompt")?.value || "").trimEnd();
  const provider = (
    $("#confirm-edit-provider")?.value ||
    $("#confirm-task-provider")?.value ||
    state.planJob?.provider ||
    "claude"
  ).toLowerCase();
  // P2-1: collect depends_on from checkbox group
  const dependsOn = [
    ...document.querySelectorAll("#confirm-edit-deps .confirm-dep-check:checked"),
  ].map((el) => el.value);
  if (!title) {
    if (err) {
      err.textContent = "标题不能为空";
      err.hidden = false;
    }
    return;
  }
  if (!prompt.trim()) {
    if (err) {
      err.textContent = "任务说明不能为空";
      err.hidden = false;
    }
    return;
  }
  try {
    const view = await invoke("update_plan_task_cmd", {
      jobId: state.planJobId,
      taskId: state.confirmTaskId,
      title,
      prompt,
      provider,
      dependsOn,
    });
    state.planJob = view;
    state.planJobId = view.job_id || view.jobId || state.planJobId;
    state.confirmEditing = false;
    // Force deps box rebuild next open
    const depsBox = $("#confirm-edit-deps");
    if (depsBox) delete depsBox.dataset.forTask;
    stashPlanSession(state.selectedPath);
    toast("已保存任务修改（含依赖）");
    renderConfirmPanel();
    renderPlanPicker();
  } catch (e) {
    if (err) {
      err.textContent = String(e);
      err.hidden = false;
    }
    toast(String(e));
  }
}

/** P2-1: delete selected task from proposed plan. */
async function deleteConfirmTask() {
  if (hasActiveRun()) {
    toast("运行中不可删除");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  if (!state.planJobId || !state.confirmTaskId) {
    toast("请先选择任务");
    return;
  }
  if (!canEditSelectedTask(state.confirmTaskId)) {
    toast("仅未执行的任务可删除");
    return;
  }
  const tasks = state.planJob?.tasks || [];
  if (tasks.length <= 1) {
    toast("至少保留一个步骤");
    return;
  }
  const cur = tasks.find((t) => t.id === state.confirmTaskId);
  const label = cur?.title || state.confirmTaskId;
  if (!window.confirm(`从本轮拆分中删除「${label}」？\n依赖它的步骤会自动去掉这条边。`)) {
    return;
  }
  try {
    const view = await invoke("remove_plan_task_cmd", {
      jobId: state.planJobId,
      taskId: state.confirmTaskId,
    });
    state.planJob = view;
    state.planJobId = view.job_id || view.jobId || state.planJobId;
    state.confirmTaskId = view.tasks?.[0]?.id || null;
    state.confirmEditing = false;
    const depsBox = $("#confirm-edit-deps");
    if (depsBox) delete depsBox.dataset.forTask;
    stashPlanSession(state.selectedPath);
    toast(`已删除「${label}」`);
    renderConfirmPanel();
    renderPlanPicker();
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** Only from confirm phase — starts workers. */
async function confirmAndStart() {
  const err = $("#confirm-error");
  err.hidden = true;
  if (hasActiveRun()) {
    toastRunLocked("再次启动");
    return;
  }
  if (state.confirmEditing) {
    err.textContent = "请先保存或取消编辑";
    err.hidden = false;
    return;
  }
  // Paused run: resume instead of spawning a new run.
  if (isRunPaused() && state.live?.run_id) {
    try {
      await invoke("resume_run_cmd", { runId: state.live.run_id });
      toast("正在继续…");
      state.phase = "running";
      state.confirmEditing = false;
      state.returnPhaseAfterConfirm = null;
      renderPhasePanels();
      renderPlanPicker();
      updateSplitPlanChip();
      setTimeout(() => {
        loadLive().catch(() => {});
        loadProjects().catch(() => {});
      }, 600);
    } catch (e) {
      err.textContent = String(e);
      err.hidden = false;
      toast(String(e));
    }
    return;
  }
  if (!state.planJobId) {
    err.textContent = "没有待确认的规划";
    err.hidden = false;
    return;
  }
  const provider = state.planJob?.provider || $("#pp-provider")?.value || "claude";
  const doc = await ensureDoctor(true);
  if (doc && !doc.ok && provider !== "fake") {
    err.textContent = "环境未就绪，请先处理警告或改用模拟运行后重新规划";
    err.hidden = false;
    renderDoctorWarn();
    return;
  }
  try {
    const res = await invoke("confirm_start_cmd", { jobId: state.planJobId });
    toast("已开始运行");
    // Keep split job for top-bar chip / read-only re-open while running.
    if (state.planJob) {
      state.planJob = {
        ...state.planJob,
        status: "confirmed",
        run_id: res?.run_id || res?.runId || state.planJob.run_id || null,
      };
    }
    state.phase = "running";
    state.confirmEditing = false;
    state.returnPhaseAfterConfirm = null;
    state.selectedTaskId = null;
    state.planCollapsed = true;
    state.closedPanels = {};
    setAssignBusy(false);
    // Session is no longer "planning/confirm", but keep planJob in memory for chip.
    try {
      if (state.selectedPath) delete state.planSessions[state.selectedPath];
    } catch (_) {}
    renderPhasePanels();
    renderPlanPicker();
    updateSplitPlanChip();
    updateBgPlanBanner();
    await loadLive();
    await loadProjects();
    renderProjectList();
  } catch (e) {
    err.textContent = String(e);
    err.hidden = false;
    toast(String(e));
  }
}

function cancelPlanning() {
  stopPlanJobPoll();
  setAssignBusy(false);
  clearPlanSession(state.selectedPath);
  state.phase = "pick";
  state.planJobId = null;
  state.planJob = null;
  renderPhasePanels();
  renderPlanPicker();
  updateBgPlanBanner();
}

/**
 * Confirm-screen re-split: keep current plan path and start a fresh plan job
 * (one click — no need to re-pick the file). Falls back to chooser if no plan.
 * P2-2: pass preserve_from_job_id so human title/prompt/deps/deletes re-apply.
 */
async function replanFromConfirm() {
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

  stopPlanJobPoll();
  setAssignBusy(false);
  clearPlanSession(state.selectedPath);
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.confirmEditing = false;
  state.returnPhaseAfterConfirm = null;
  state.phase = "pick";
  renderPhasePanels();
  renderPlanPicker();
  updateSplitPlanChip();
  updateBgPlanBanner();

  if (!state.selectedPlan || !state.selectedPath) {
    openPlanChooser(true);
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
  if (typeof analyzePlanFromPicker === "function") {
    await analyzePlanFromPicker();
  } else {
    openPlanChooser(true);
  }
}

/**
 * Confirm-screen CTA when critic notes missing inspect tail:
 * enable settings.post_inspect_enabled, then re-split current plan.
 */
async function enablePostInspectAndResplit() {
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
    await invoke("set_settings_cmd", {
      update: { post_inspect_enabled: true },
    });
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
async function enablePlannerCriticAndResplit() {
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
    await invoke("set_settings_cmd", {
      update: { planner_critic_enabled: true },
    });
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
async function sanitizeDepsFromConfirm() {
  if (hasActiveRun()) {
    toastRunLocked("清理依赖");
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
  if (btn) {
    btn.disabled = true;
    btn.textContent = "清理中…";
  }
  try {
    const resp = await invoke("sanitize_plan_deps_cmd", {
      jobId: state.planJobId,
    });
    const removed = resp?.removed ?? resp?.Removed ?? 0;
    const view = resp?.view || resp;
    if (view) {
      state.planJob = view;
      state.planJobId = view.job_id || view.jobId || state.planJobId;
      stashPlanSession(state.selectedPath);
    }
    if (removed > 0) {
      toast(`已去掉 ${removed} 条可疑依赖 · 步骤可更多并行`);
    } else {
      toast("没有发现可疑依赖 · 当前依赖均有说明支撑");
    }
    renderConfirmPanel();
    renderPlanPicker();
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "清理可疑依赖";
    }
  }
}

/* ── Workspace live ── */
async function loadLive() {
  if (!state.selectedPath) {
    state.live = null;
    return;
  }
  state.now = Date.now();
  // 规划中时顺带刷新 plan job，防止 setInterval 被卡住时永远转圈
  if (state.phase === "planning" && state.planJobId) {
    await refreshPlanJob().catch(() => {});
  }
  const prevLive = hasActiveRun();
  state.live = await invoke("get_project_live", {
    project: state.selectedPath,
    log_max_bytes: 96000,
  });
  // Run ended while on running phase → unlock switch / plan choose.
  const nowLive = hasActiveRun();
  if (prevLive && !nowLive && state.phase === "running") {
    state.phase = "done";
  }
  // auto-select task
  ensureSelectedTask();
  renderWorkspace();
  if (prevLive !== nowLive) {
    renderProjectList();
    renderPlanPicker();
    updateSplitPlanChip();
  } else if (state.page !== "workspace") {
    // 聊天/设置等页：轮询时刷新「监控计划」与底部悬浮条
    try { renderPlanPicker(); } catch (_) {}
    try { updateBgPlanBanner(); } catch (_) {}
  } else {
    try { updateBgPlanBanner(); } catch (_) {}
  }
}

function ensureSelectedTask() {
  const tasks = state.live?.tasks || [];
  if (!tasks.length) {
    state.selectedTaskId = null;
    return;
  }
  const ids = new Set(tasks.map((t) => t.task_id));
  if (state.selectedTaskId && ids.has(state.selectedTaskId)) {
    // keep, unless we should auto-focus a new failure
  } else {
    state.selectedTaskId = null;
  }

  // Prefer failed, then running, then first
  const failed = tasks.find((t) => isFailedStatus(t.status));
  const running = tasks.find((t) => isLiveStatus(t.status));
  if (!state.selectedTaskId) {
    state.selectedTaskId = (failed || running || tasks[0]).task_id;
  } else if (failed && isFailedStatus(failed.status)) {
    // if current is done and there's a failure, focus failure once
    const cur = tasks.find((t) => t.task_id === state.selectedTaskId);
    if (cur && !isFailedStatus(cur.status) && !isLiveStatus(cur.status)) {
      state.selectedTaskId = failed.task_id;
    }
  }
}

