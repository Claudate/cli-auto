/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: plan UI 片段 · 顶栏选择/分配可见性 · 全局 plan-chooser
 * [POS]: web/js D4 自 app.js 纵切；无构建器，顺序 script 共享全局
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 * 注：#plan-chooser 在 main 级（非 page-workspace）；聊天页只显示「选择计划」，分配走就绪条
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
    } else if (status === "planned") {
      toast(`已恢复上次拆分：${n} 个任务，可直接开始运行`);
    } else if (status === "confirmed") {
      toast(`已恢复历史拆分：${n} 个任务（可再次运行，无需重拆）`);
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
  // 同项目再点：只回工作区，保留规划
  if (path && path === state.selectedPath) {
    showPage("workspace");
    renderProjectList();
    renderPhasePanels();
    renderPlanPicker();
    renderWorkspace();
    updateTopPlanInfo();
    updateBgPlanBanner();
    if (state.phase === "planning" && state.planJobId) {
      startPlanJobPoll();
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

  showPage("workspace");
  renderProjectList();
  await Promise.all([loadLive(), loadPlansForPicker(), ensureDoctor()]);

  if (restoredMem) {
    renderPhasePanels();
    renderPlanPicker();
    renderWorkspace();
    updateTopPlanInfo();
    updateBgPlanBanner();
    if (state.phase === "planning" && state.planJobId) {
      await refreshPlanJob().catch(() => {});
    }
    toast(state.phase === "planning" ? "已回到后台规划" : "已恢复待确认计划");
    return;
  }

  // 内存无会话 → 从磁盘接上该项目最近一次拆分（planned/confirmed/planning）
  // 若当前有活动 run，优先显示运行；否则恢复拆分结果，避免每次重拆
  const activeRun = !!(state.live?.run_id && isLiveStatus(state.live?.run_status));
  if (!activeRun) {
    const restoredDisk = await tryRestorePersistedPlanJob(path);
    if (restoredDisk) {
      renderPhasePanels();
      renderPlanPicker();
      renderWorkspace();
      updateTopPlanInfo();
      updateBgPlanBanner();
      if (state.phase === "planning" && state.planJobId) {
        await refreshPlanJob().catch(() => {});
      }
      return;
    }
  }

  if (state.live?.run_id && isLiveStatus(state.live?.run_status)) {
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
      renderPlanPicker();
    }
  } else {
    renderPlanPicker();
  }
  updateTopPlanInfo();
  renderPhasePanels();
  renderWorkspace();
  updateBgPlanBanner();
}

function renderPhasePanels() {
  const planning = $("#plan-phase-planning");
  const confirm = $("#plan-phase-confirm");
  if (!planning || !confirm) return;

  const ph = state.phase;
  planning.hidden = ph !== "planning";
  confirm.hidden = ph !== "confirm";

  if (ph === "planning") {
    if (state.planJob) {
      fillPlannerLog(state.planJob);
    } else {
      const log = $("#planner-log");
      if (log && !log.dataset.sig) {
        log.innerHTML = '<div class="cli-empty-ai muted">正在分析…</div>';
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

async function loadPlansForPicker() {
  if (!state.selectedPath) {
    state.plans = [];
    state.plansLoading = false;
    if (state.planChooserOpen) renderPlanChooser();
    updateChooserAssignState();
    return [];
  }
  state.plansLoading = true;
  if (state.planChooserOpen) renderPlanChooser();
  try {
    const plans = (await invoke("get_plans", { project: state.selectedPath })) || [];
    const root = state.selectedPath;
    // 只保留当前项目内路径；绝对路径收成相对
    const list = (Array.isArray(plans) ? plans : [])
      .map((p) => normalizePlanPath(p, root) || p)
      .filter((p) => isPlanUnderProject(p, root));
    // 用户手动选的计划若不在扫描结果中，且仍属本项目，置顶保留
    const selected = normalizePlanPath(state.selectedPlan, root) || state.selectedPlan;
    if (selected && isPlanUnderProject(selected, root) && !list.includes(selected)) {
      list.unshift(selected);
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
  return state.plans;
}

function setAssignBusy(busy) {
  state.assigning = !!busy;
  const ids = ["btn-chooser-assign", "btn-pp-analyze"];
  for (const id of ids) {
    const btn = document.getElementById(id);
    if (!btn) continue;
    if (busy) {
      btn.disabled = true;
      btn.classList.add("is-busy");
      if (!btn.dataset.label) btn.dataset.label = btn.textContent || "分配计划";
      btn.innerHTML = '<span class="spinner sm" aria-hidden="true"></span><span>分配中…</span>';
    } else {
      btn.classList.remove("is-busy");
      const active = isLiveStatus(state.live?.run_status);
      const label = btn.dataset.label || "分配计划";
      btn.textContent = active ? "运行中…" : label;
      delete btn.dataset.label;
      if (btn.id === "btn-chooser-assign") {
        btn.disabled = !state.selectedPlan || !!active;
      } else {
        btn.disabled = !!active;
      }
    }
  }
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

function openPlanChooser(open = true) {
  if (open && hasActiveRun()) {
    toastRunLocked("切换/选择计划");
    return;
  }
  state.planChooserOpen = open;
  const sheet = $("#plan-chooser");
  if (!sheet) return;
  sheet.hidden = !open;
  if (open) {
    renderPlanChooser();
    updateChooserAssignState();
  }
}

function updateChooserAssignState() {
  const btn = $("#btn-chooser-assign");
  const label = $("#chooser-selected-label");
  const active = isLiveStatus(state.live?.run_status);
  const plan = state.selectedPlan;
  if (label) {
    label.textContent = plan ? `已选：${planDisplayName(plan)}` : "未选择计划";
    label.title = plan || "";
  }
  if (btn && !state.assigning) {
    btn.disabled = !plan || !!active;
    btn.textContent = active ? "运行中…" : "分配计划";
  }
}

function renderPlanChooser() {
  const list = $("#chooser-list");
  const empty = $("#chooser-empty");
  if (!list) return;
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
  // 仅渲染列表；点选走全局委托，避免 onclick + capture 双触发
  list.innerHTML = state.plans
    .map((p) => {
      const selected = p === state.selectedPlan;
      const title = planDisplayName(p);
      return `<button type="button" class="plan-item${selected ? " selected" : ""}" data-plan="${esc(p)}">
        <div class="plan-item-title">${esc(title)}</div>
        <div class="plan-item-path">${esc(p)}</div>
      </button>`;
    })
    .join("");
  updateChooserAssignState();
}

function renderPlanPicker() {
  const pp = $("#plan-picker");
  const btnChoose = $("#btn-plan-choose");
  const btnAssign = $("#btn-pp-analyze");
  const btnEdit = $("#btn-edit-plan");
  const btnChat = $("#btn-open-chat");
  const btnMonitor = $("#btn-monitor-plan");

  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const inChat = !!state.selectedPath && state.page === "chat";
  // 选择计划：workspace + chat 都可；分配计划：仅 workspace（聊天页用就绪条 #btn-chat-assign）
  const hideForPhase = state.phase === "planning" || state.phase === "confirm";
  const runActive = hasActiveRun();
  const hasSplit =
    !!state.planJob &&
    ["planned", "confirmed"].includes(String(state.planJob.status || "").toLowerCase());

  // 顶栏「聊天」：已选项目常驻；chat 页隐藏自指（已在聊天）
  if (btnChat) {
    btnChat.hidden =
      !state.selectedPath || state.page === "welcome" || state.page === "chat";
    btnChat.disabled = false;
    btnChat.title = "与 AI 共建计划文档";
  }

  // 顶栏「监控计划」：离开 workspace 且有规划/运行可看时显示（聊天/设置/帮助等）
  if (btnMonitor) {
    const showMon =
      !!state.selectedPath &&
      state.page !== "workspace" &&
      state.page !== "welcome" &&
      hasMonitorableActivity();
    btnMonitor.hidden = !showMon;
    if (showMon) {
      if (runActive) {
        btnMonitor.textContent = "监控计划";
        btnMonitor.title = "返回工作区查看运行中的 CLI";
      } else if (isRunPaused()) {
        btnMonitor.textContent = "监控计划";
        btnMonitor.title = "返回工作区查看已暂停的计划";
      } else if (isPlanSessionActive()) {
        btnMonitor.textContent = state.phase === "planning" ? "查看规划" : "返回确认";
        btnMonitor.title =
          state.phase === "planning" ? "返回工作区查看拆分进度" : "返回工作区确认拆分结果";
      } else {
        btnMonitor.textContent = "查看结果";
        btnMonitor.title = "返回工作区查看运行结果";
      }
    }
  }

  // 顶栏「选择计划」：workspace / chat；规划/确认相位隐藏
  if (btnChoose) {
    btnChoose.hidden = !(inWorkspace || inChat) || hideForPhase;
    btnChoose.disabled = !!runActive;
    btnChoose.title = runActive ? "运行中，请先停止后再切换计划" : "选择计划";
  }
  // 顶栏「分配计划」：仅 workspace 显示；聊天页只在保存后用就绪条分配
  if (btnAssign) {
    btnAssign.hidden = !inWorkspace || hideForPhase;
  }

  // 「编辑计划」：仅在有拆分结果时显示；运行中禁用，暂停后可进确认页改未执行任务
  if (btnEdit) {
    const showEdit = inWorkspace && hasSplit;
    btnEdit.hidden = !showEdit;
    if (showEdit) {
      const editableNow = canEditSelectedTask(state.confirmTaskId || state.planJob?.tasks?.[0]?.id);
      btnEdit.disabled = !!runActive && !isRunPaused();
      if (runActive && !isRunPaused()) {
        btnEdit.title = "运行中不可编辑，请先停止或待计划暂停";
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

  // 非 workspace：聊天页保留已开的 plan-chooser（全局浮层）；其它页关掉
  if (!inWorkspace) {
    if (!inChat && state.planChooserOpen && !runActive) {
      openPlanChooser(false);
    } else if (inChat && state.planChooserOpen) {
      renderPlanChooser();
      updateChooserAssignState();
    }
    updateSplitPlanChip();
    updateTopPlanInfo();
    return;
  }

  const active = runActive || isLiveStatus(state.live?.run_status);
  if (btnAssign) {
    // 弹窗化后无计划也可点开选计划；仅运行中禁用
    btnAssign.disabled = !!active;
    btnAssign.textContent = active ? "运行中…" : "分配计划";
    btnAssign.title = active ? "运行中，请先停止后再分配新计划" : "分配计划";
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

/** Top-bar summary of the latest split plan (right of 分配计划). */
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
    toast("还没有拆分结果，请先分配计划");
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
    // 留在弹窗内，方便直接点「分配计划」
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
/** 分配计划：AI 拆分后进入确认（可编辑） */
async function analyzePlanFromPicker() {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (state.assigning) return;
  if (hasActiveRun()) {
    toastRunLocked("分配计划");
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
    logEl0.innerHTML = '<div class="cli-empty-ai muted">正在启动规划…</div>';
  }
  const sub0 = $("#planning-sub");
  if (sub0) sub0.textContent = `正在分析 ${planDisplayName(state.selectedPlan)}…（并发 ${maxParallel}）`;

  try {
    const view = await invoke("start_plan_job_cmd", {
      req: {
        project: state.selectedPath,
        plan: state.selectedPlan,
        plan_mode: planMode,
        provider,
        mode,
        max_parallel: maxParallel,
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

function countOptionalIncluded(view) {
  const tasks = view?.tasks || [];
  return tasks.filter((t) => t.optional && t.include !== false).length;
}

async function advancePlannedJob(view) {
  stopPlanJobPoll();
  state.planJob = view;
  if (!state.confirmTaskId && view.tasks?.length) {
    state.confirmTaskId = view.tasks[0].id;
  }
  stashPlanSession(state.selectedPath);
  updateBgPlanBanner();
  // 人不在工作区时不自动开跑，避免后台误启 worker；提示返回确认
  if (state.page !== "workspace") {
    state.phase = "confirm";
    setAssignBusy(false);
    stashPlanSession(state.selectedPath);
    toast(`拆分完成（${view.task_count || view.tasks?.length || 0} 任务），请返回确认`);
    updateBgPlanBanner();
    return;
  }
  const n = view.task_count || view.tasks?.length || 0;
  const adapter = view.adapter || "";
  const how =
    adapter.includes("heuristic")
      ? "本地启发式拆分"
      : adapter.includes("llm")
        ? "Claude CLI 规划"
        : "规划完成";
  // Optional tasks need an explicit user choice — never auto-start past them.
  const hasOptional = planHasOptionalTasks(view);
  if (state.autoStartAfterPlan && !hasOptional) {
    toast(`${how}：${n} 个任务，正在启动…`);
    state.phase = "confirm";
    renderPhasePanels();
    setAssignBusy(false);
    await confirmAndStart();
  } else {
    const optHint = hasOptional
      ? "；含可选项，请勾选后再开始"
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
        toast("规划超时：Claude CLI 可能卡住。请检查 claude 是否在 PATH，或高级选项改用模拟。");
        renderPhasePanels();
        renderPlanPicker();
        return;
      }
      const sub = $("#planning-sub");
      if (sub) {
        const elapsed = state.planStartedAt
          ? Math.round((Date.now() - state.planStartedAt) / 1000)
          : 0;
        sub.textContent = `正在调用 Claude CLI 拆分（已等待 ${elapsed}s）…`;
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
  const optOn = optTasks.filter((t) => t.include !== false).length;
  const optHint =
    optTasks.length > 0
      ? ` · 可选 ${optOn}/${optTasks.length} 已勾选`
      : "";
  $("#confirm-meta").textContent = `${job.task_count || tasks.length} 个任务 · 并发上限 ${
    mpCap
  } · ${layers.length} 波${parallelHint}${optHint} · 规划方式 ${job.plan_mode || "—"} · ${
    runLocked
      ? "运行中（只读）"
      : paused
        ? "已暂停 · 仅未执行任务可编辑"
        : optTasks.length > 0
          ? "勾选可选项后开始（默认不跑可选）"
          : reused
            ? "可编辑未执行任务后再次运行"
            : "可编辑 · 确认后开始"
  }`;

  const waves = $("#confirm-waves");
  waves.innerHTML = layers
    .map((layer, i) => {
      const rows = layer
        .map((id) => {
          const t = byId[id] || { id, title: id, depends_on: [], optional: false, include: true };
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
          const checkHtml = isOpt
            ? `<label class="wave-task-check" title="可选：勾选后才会执行" data-check-for="${esc(id)}">
                <input type="checkbox" class="wave-opt-check" data-id="${esc(id)}" ${
                  included ? "checked" : ""
                } ${runLocked || !pending ? "disabled" : ""} />
                <span class="opt-badge">可选</span>
              </label>`
            : `<span class="wave-task-req muted" title="必选任务">必选</span>`;
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
  const cancelBtn = $("#btn-confirm-edit-cancel");
  const saveBtn = $("#btn-confirm-edit-save");
  const promptLabel = $("#confirm-prompt-label");
  const taskEditable = !!cur && canEditSelectedTask(cur.id);
  const editing = !!state.confirmEditing && taskEditable;

  if (cur) {
    state.confirmTaskId = cur.id;
    $("#confirm-task-title").textContent = `${cur.title}（${cur.id}）`;
    $("#confirm-task-title").classList.remove("muted");
    const kind = cur.optional
      ? cur.include !== false
        ? "可选项 · 已勾选（会执行）"
        : "可选项 · 未勾选（默认不跑）"
      : "必选任务";
    if (cur.depends_on?.length > 0) {
      const depLabels = cur.depends_on.map((id) => {
        const d = byId[id];
        return d ? `${d.title}（${id}）` : id;
      });
      $("#confirm-task-deps").textContent = `${kind} · 依赖：${depLabels.join(" · ")}`;
    } else {
      $("#confirm-task-deps").textContent = `${kind} · 无依赖，属于首波`;
    }
    const full =
      cur.prompt ||
      cur.prompt_preview ||
      cur.promptPreview ||
      "";
    if (editing) {
      if (promptEl) promptEl.hidden = true;
      if (editForm) editForm.hidden = false;
      if (promptLabel) promptLabel.textContent = "编辑任务说明";
      const titleInput = $("#confirm-edit-title");
      const promptInput = $("#confirm-edit-prompt");
      if (titleInput && document.activeElement !== titleInput) {
        titleInput.value = cur.title || "";
      }
      if (promptInput && document.activeElement !== promptInput) {
        promptInput.value = full;
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
        promptLabel.textContent = "完整任务说明（Markdown 渲染 · 将发给 worker CLI）";
      }
    }
    if (metaEl) {
      const chars = [...full].length;
      metaEl.hidden = false;
      metaEl.textContent = editing
        ? `编辑中 · ${chars} 字`
        : `说明长度 ${chars} 字 · 点左侧可切换任务`;
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
      editBtn.title = "编辑此任务说明";
    }
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
    replanBtn.title = runLocked ? "运行中，请先停止后再重新规划" : "清空本次拆分并重新分配";
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
    });
    state.planJob = view;
    state.planJobId = view.job_id || view.jobId || state.planJobId;
    state.confirmEditing = false;
    stashPlanSession(state.selectedPath);
    toast("已保存任务修改");
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

function replanFromConfirm() {
  if (hasActiveRun()) {
    toastRunLocked("重新规划");
    return;
  }
  if (state.confirmEditing) {
    toast("请先保存或取消编辑");
    return;
  }
  stopPlanJobPoll();
  setAssignBusy(false);
  clearPlanSession(state.selectedPath);
  state.phase = "pick";
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.confirmEditing = false;
  state.returnPhaseAfterConfirm = null;
  renderPhasePanels();
  renderPlanPicker();
  updateSplitPlanChip();
  updateBgPlanBanner();
  openPlanChooser(true);
  toast("已清空拆分，可调整并发后再次「分配计划」");
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

