/**
 * [INPUT]: 依赖 Tauri invoke / dialog，消费 services 暴露的桌面命令
 * [OUTPUT]: 项目任务控制台 UI 状态机（选计划→分配→监视）
 * [POS]: web/ 前端核心，与 index.html、app.css 协作
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */

/* cco desktop — 项目任务控制台（浅色 · 主从监视） */

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];

const LOG_FONT_KEY = "cco.logFontSize";
const ADVANCED_KEY = "cco.advancedOpen";

const state = {
  page: "welcome", // welcome | workspace | doctor | help | settings
  /** Mode B phase: pick | planning | confirm | running | done */
  phase: "pick",
  projects: [],
  selectedPath: null,
  live: null,
  pollTimer: null,
  logStick: true,
  now: Date.now(),
  plans: [],
  selectedPlan: null,
  planPreview: null,
  selectedTaskId: null,
  filterFailedOnly: false,
  planCollapsed: true, // 默认只显示当前计划条
  planChooserOpen: false,
  advancedOpen: localStorage.getItem(ADVANCED_KEY) === "1",
  doctorCache: null, // { ok, at, lines }
  doctorDismissedKey: null, // 用户点忽略后隐藏同类警告
  autoStartAfterPlan: true, // 分配计划后自动开跑
  closedPanels: {}, // taskId -> true
  panelPos: JSON.parse(localStorage.getItem("cco.panelPos") || "{}"),
  logFontSize: Number(localStorage.getItem(LOG_FONT_KEY) || 14) || 14,
  logViewMode: (() => {
    if (localStorage.getItem("cco.logViewMigrated") !== "term-v1") {
      localStorage.setItem("cco.logViewMode", "term");
      localStorage.setItem("cco.logViewMigrated", "term-v1");
      return "term";
    }
    return localStorage.getItem("cco.logViewMode") || "term";
  })(),
  planJobId: null,
  planJob: null,
  confirmTaskId: null,
  planJobPollTimer: null,
  drag: null, // { id, ox, oy }
};

/* ── Status labels (人话) ── */
const STATUS_LABEL = {
  completed: "已完成",
  done: "已完成",
  ok: "正常",
  running: "运行中",
  starting: "启动中",
  queued: "排队中",
  validated: "校验中",
  init: "初始化",
  paused: "已暂停",
  resuming: "恢复中",
  failed: "失败",
  aborted: "已中止",
  timeout: "超时",
  stopped: "已停止",
  pending: "等待中",
  skipped: "已跳过",
  idle: "空闲",
  err: "错误",
};

function statusLabel(status) {
  const s = String(status || "").toLowerCase();
  return STATUS_LABEL[s] || status || "—";
}

function toast(msg) {
  const t = $("#toast");
  t.hidden = false;
  t.textContent = msg;
  clearTimeout(toast._t);
  toast._t = setTimeout(() => {
    t.hidden = true;
  }, 3200);
}

function getInvoke() {
  if (window.__TAURI_INTERNALS__ || window.__TAURI__) {
    const core = window.__TAURI__?.core;
    if (core?.invoke) return core.invoke.bind(core);
  }
  return null;
}

async function invoke(cmd, args = {}) {
  const inv = getInvoke();
  if (!inv) throw new Error("请通过 CCO.app 启动");
  return inv(cmd, args);
}

function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function shortPath(p) {
  if (!p) return "—";
  const parts = String(p).split("/").filter(Boolean);
  return parts.length > 3 ? "…/" + parts.slice(-3).join("/") : p;
}

function planDisplayName(path) {
  if (!path) return "—";
  const parts = String(path).split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

function badge(status) {
  const s = String(status || "").toLowerCase();
  let cls = "muted";
  if (["completed", "done", "ok", "skipped"].includes(s)) cls = "ok";
  else if (["running", "starting", "queued", "validated", "init", "paused", "resuming", "pending"].includes(s))
    cls = "warn";
  else if (["failed", "aborted", "timeout", "stopped", "err"].includes(s)) cls = "err";
  return `<span class="badge ${cls}">${esc(statusLabel(status))}</span>`;
}

function statusDot(status) {
  const s = String(status || "").toLowerCase();
  if (["running", "starting", "queued", "validated", "init"].includes(s)) return "live";
  if (["paused", "resuming", "pending"].includes(s)) return "warn";
  if (["failed", "aborted", "timeout", "stopped"].includes(s)) return "err";
  if (["completed", "done", "ok", "skipped"].includes(s)) return "live";
  return "";
}

function isLiveStatus(s) {
  return ["running", "starting", "queued", "validated", "init", "paused", "resuming"].includes(
    String(s || "").toLowerCase()
  );
}

function isFailedStatus(s) {
  return ["failed", "aborted", "timeout", "stopped"].includes(String(s || "").toLowerCase());
}

function isDoneStatus(s) {
  return ["completed", "done", "ok", "skipped"].includes(String(s || "").toLowerCase());
}

function formatElapsed(startedAt, finishedAt) {
  if (!startedAt) return "—";
  const start = Date.parse(startedAt);
  if (Number.isNaN(start)) return "—";
  const end = finishedAt ? Date.parse(finishedAt) : state.now;
  if (Number.isNaN(end)) return "—";
  let sec = Math.max(0, Math.floor((end - start) / 1000));
  const h = Math.floor(sec / 3600);
  sec %= 3600;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function taskErrorSummary(t) {
  if (!t) return "";
  if (t.error) return String(t.error).split("\n")[0].slice(0, 160);
  if (isFailedStatus(t.status) && t.log_tail) {
    const lines = String(t.log_tail).trim().split("\n").filter(Boolean);
    return (lines[lines.length - 1] || "").slice(0, 160);
  }
  return "";
}

function applyLogFontSize(px) {
  state.logFontSize = px;
  document.documentElement.style.setProperty("--log-font-size", `${px / 16}rem`);
  localStorage.setItem(LOG_FONT_KEY, String(px));
  $$("#log-font-group button").forEach((b) => {
    b.classList.toggle("active", Number(b.dataset.size) === px);
  });
  const sel = $("#s-log-font");
  if (sel) sel.value = String(px);
}

/* ── Pages ── */
function showPage(name) {
  state.page = name;
  $$(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${name}`));
  if (name === "welcome") {
    $("#page-title").textContent = "欢迎";
    $("#page-sub").textContent = "添加项目 → 选计划 → 开始运行";
  } else if (name === "workspace") {
    updateWorkspaceTitle();
  } else if (name === "doctor") {
    $("#page-title").textContent = "环境检查";
    $("#page-sub").textContent = "确认本机 CLI 与依赖就绪";
  } else if (name === "help") {
    $("#page-title").textContent = "帮助";
    $("#page-sub").textContent = "";
  } else if (name === "settings") {
    $("#page-title").textContent = "设置";
    $("#page-sub").textContent = "";
  }
}

function updateWorkspaceTitle() {
  const p = state.projects.find((x) => x.path === state.selectedPath);
  $("#page-title").textContent = p?.name || shortPath(state.selectedPath) || "项目";
  $("#page-sub").textContent = state.selectedPath || "";
}

function goHome() {
  state.selectedPath = null;
  state.live = null;
  state.selectedTaskId = null;
  renderProjectList();
  if (state.projects.length === 0) {
    showPage("welcome");
  } else {
    // 有项目但未选中：欢迎页提示选项目
    showPage("welcome");
    $("#page-title").textContent = "我的项目";
    $("#page-sub").textContent = "从左侧选择一个项目";
    const we = $("#welcome-empty");
    we.innerHTML = `
      <h2>选择左侧项目，或添加新项目</h2>
      <p class="muted">项目内选计划后即可开始运行，右侧会显示任务进度与完整日志。</p>
      <div class="welcome-actions">
        <button class="btn primary" id="btn-welcome-add2" type="button">添加项目文件夹</button>
      </div>`;
    $("#btn-welcome-add2").onclick = () => openModal();
  }
}

/* ── Modal（仅添加项目） ── */
function openModal() {
  $("#modal").hidden = false;
  $("#m-project-path").value = "";
  $("#m-project-name").value = "";
}

function closeModal() {
  $("#modal").hidden = true;
}

/* ── Projects ── */
async function loadProjects() {
  state.projects = (await invoke("get_projects")) || [];
  renderProjectList();
  if (state.selectedPath && !state.projects.some((p) => p.path === state.selectedPath)) {
    state.selectedPath = null;
    state.live = null;
  }
}

function renderProjectList() {
  const el = $("#project-list");
  if (!state.projects.length) {
    el.innerHTML = `<p class="muted empty-hint">尚未添加项目<br/>点 ＋ 添加</p>`;
    return;
  }
  el.innerHTML = state.projects
    .map((p) => {
      const st = p.active_status || p.last_status || "";
      const live = p.running_tasks > 0 || isLiveStatus(p.active_status);
      let meta;
      if (live) {
        meta = `${p.running_tasks || 0}/${p.total_tasks || "?"} 任务 · 运行中`;
      } else if (p.last_status) {
        meta = `最近: ${statusLabel(p.last_status)}`;
      } else if (p.exists) {
        meta = "无活动运行";
      } else {
        meta = "路径不存在";
      }
      return `<button type="button" class="project-item ${
        p.path === state.selectedPath ? "active" : ""
      }" data-path="${esc(p.path)}">
        <div class="name"><span class="dot ${statusDot(st) || (live ? "live" : "")}"></span>${esc(
          p.name
        )}</div>
        <div class="path" title="${esc(p.path)}">${esc(shortPath(p.path))}</div>
        <div class="meta">${esc(meta)}</div>
      </button>`;
    })
    .join("");
  $$(".project-item", el).forEach((b) => {
    b.onclick = () => selectProject(b.dataset.path);
  });
}

async function selectProject(path) {
  state.selectedPath = path;
  state.logStick = true;
  state.selectedPlan = null;
  state.planPreview = null;
  state.selectedTaskId = null;
  state.planCollapsed = false;
  state.filterFailedOnly = false;
  state.planJobId = null;
  state.planJob = null;
  state.confirmTaskId = null;
  state.phase = "pick";
  showPage("workspace");
  renderProjectList();
  await Promise.all([loadLive(), loadPlansForPicker(), ensureDoctor()]);
  // 进入已有活动运行时默认折叠计划区，并同步 phase
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
  const candidate =
    state.live?.plan_path || proj?.default_plan || proj?.last_plan || state.plans[0] || null;
  if (candidate && state.phase === "pick") {
    await selectPlan(candidate);
  } else {
    renderPlanPicker();
  }
  renderPhasePanels();
}

function renderPhasePanels() {
  const planning = $("#plan-phase-planning");
  const confirm = $("#plan-phase-confirm");
  if (!planning || !confirm) return;

  const ph = state.phase;
  planning.hidden = ph !== "planning";
  confirm.hidden = ph !== "confirm";

  if (ph === "planning") {
    const log = $("#planner-log");
    if (log) log.textContent = state.planJob?.planner_log_tail || "正在分析…";
  }
  if (ph === "confirm") {
    renderConfirmPanel();
  }
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
    const dialog = window.__TAURI__?.dialog;
    if (!dialog?.open) return toast("对话框不可用");
    const selected = await dialog.open({ directory: true, multiple: false });
    if (selected) $("#m-project-path").value = selected;
  } catch (e) {
    toast(String(e));
  }
}

async function removeSelectedProject() {
  if (!state.selectedPath) return;
  try {
    await invoke("remove_project_cmd", { path: state.selectedPath });
    toast("已移除项目");
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
  state.live = null;
  state.selectedTaskId = null;
  state.phase = "pick";
  state.planCollapsed = false;
  renderWorkspace();
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
  // 已成功跑过时只软提示，不恐吓
  const live = state.live;
  const soft =
    live &&
    ["completed", "done"].includes(String(live.run_status || "").toLowerCase());
  bar.classList.toggle("soft", !!soft);
  const detail = fails
    .map((l) => `${l.name}: ${l.detail}`)
    .slice(0, 2)
    .join(" · ");
  $("#doctor-warn-text").textContent = soft
    ? `环境提示（不影响查看历史）：${detail || "部分检查未通过"}`
    : detail || "环境检查未通过。若 Claude 已安装，请点「重新检查」或到设置确认路径。";
  bar.hidden = false;
}

function setPlanCollapsed(collapsed) {
  // 新 UX：计划区永远紧凑；collapsed 语义保留给兼容
  state.planCollapsed = true;
  const pp = $("#plan-picker");
  if (pp) pp.classList.add("compact", "collapsed");
}

function openPlanChooser(open = true) {
  state.planChooserOpen = open;
  const sheet = $("#plan-chooser");
  if (!sheet) return;
  sheet.hidden = !open;
  if (open) renderPlanChooser();
}

function renderPlanChooser() {
  const list = $("#chooser-list");
  const empty = $("#chooser-empty");
  if (!list) return;
  if (!state.plans.length) {
    if (empty) empty.hidden = false;
    list.innerHTML = "";
    return;
  }
  if (empty) empty.hidden = true;
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
  $$(".plan-item", list).forEach((b) => {
    b.onclick = async () => {
      await selectPlan(b.dataset.plan);
      openPlanChooser(false);
    };
  });
}

function renderPlanPicker() {
  const pp = $("#plan-picker");
  if (!pp) return;
  if (!state.selectedPath) {
    pp.hidden = true;
    openPlanChooser(false);
    return;
  }
  // planning 时隐藏分配条，避免抢戏
  if (state.phase === "planning") {
    pp.hidden = true;
    return;
  }
  // confirm 且自动开跑时不展示；手动确认时也隐藏分配条
  if (state.phase === "confirm") {
    pp.hidden = true;
    return;
  }
  pp.hidden = false;
  pp.classList.add("compact", "collapsed");

  const nameEl = $("#plan-active-name");
  const pathEl = $("#plan-active-path");
  const btnAssign = $("#btn-pp-analyze");
  const live = state.live;
  const active = isLiveStatus(live?.run_status);

  if (state.selectedPlan) {
    const title =
      state.planPreview?.name ||
      planDisplayName(state.selectedPlan) ||
      "已选计划";
    if (nameEl) nameEl.textContent = title;
    if (pathEl) pathEl.textContent = state.selectedPlan;
    if (btnAssign) {
      btnAssign.disabled = !!active;
      btnAssign.textContent = active ? "运行中…" : "分配计划";
    }
  } else {
    if (nameEl) nameEl.textContent = "尚未选择";
    if (pathEl) pathEl.textContent = "点「选择计划」挑一份 .md";
    if (btnAssign) {
      btnAssign.disabled = true;
      btnAssign.textContent = "分配计划";
    }
  }

  // advanced
  const adv = $("#advanced-body");
  if (adv) adv.hidden = !state.advancedOpen;
  const tog = $("#btn-advanced-toggle");
  if (tog) tog.textContent = state.advancedOpen ? "▾ 高级" : "▸ 高级";
  try {
    const defP = $("#s-default-provider")?.value;
    if (defP && !$("#pp-provider").dataset.touched) $("#pp-provider").value = defP;
  } catch (_) {}

  if (state.planChooserOpen) renderPlanChooser();
}

function renderPlanPreview() {
  // 紧凑模式不再展示大预览；保留函数避免旧调用报错
  return;
}

async function selectPlan(planPath) {
  state.selectedPlan = planPath;
  state.planPreview = null;
  if (state.phase === "confirm" || state.phase === "planning") {
    state.phase = "pick";
    state.planJobId = null;
    state.planJob = null;
  }
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
    state.planPreview = { name: planDisplayName(planPath), task_count: "?", max_parallel: "?" };
  }
  renderPlanPicker();
}

async function pickPlanFileForPicker() {
  try {
    const dialog = window.__TAURI__?.dialog;
    if (!dialog?.open) return toast("对话框不可用");
    const selected = await dialog.open({
      multiple: false,
      filters: [{ name: "Plan", extensions: ["md", "yaml", "yml", "json"] }],
    });
    if (!selected) return;
    const proj = state.selectedPath;
    const rel = selected.startsWith(proj + "/") ? selected.slice(proj.length + 1) : selected;
    if (!state.plans.includes(rel)) state.plans = [rel, ...state.plans];
    await selectPlan(rel);
    openPlanChooser(false);
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
/** 分配计划：AI 拆分后自动开始执行 */
async function analyzePlanFromPicker() {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (!state.selectedPlan) {
    openPlanChooser(true);
    toast("请先选择计划");
    return;
  }

  const planMode = $("#pp-plan-mode")?.value || "ai";
  const provider = $("#pp-provider")?.value || "claude";
  const mode = $("#pp-mode")?.value || "print";

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

  state.phase = "planning";
  state.planJob = null;
  state.planJobId = null;
  stopPlanJobPoll();
  renderPhasePanels();
  renderPlanPicker();

  try {
    const view = await invoke("start_plan_job_cmd", {
      req: {
        project: state.selectedPath,
        plan: state.selectedPlan,
        plan_mode: planMode,
        provider,
        mode,
      },
    });
    state.planJob = view;
    state.planJobId = view.job_id;
    const logEl = $("#planner-log");
    if (logEl) logEl.textContent = view.planner_log_tail || "规划中…";

    if (view.status === "planned") {
      if (state.autoStartAfterPlan) {
        toast(`已拆分 ${view.task_count || 0} 个任务，正在启动…`);
        state.phase = "confirm";
        state.confirmTaskId = view.tasks?.[0]?.id || null;
        renderPhasePanels();
        await confirmAndStart();
      } else {
        state.phase = "confirm";
        state.confirmTaskId = view.tasks?.[0]?.id || null;
        toast(`已拆分 ${view.task_count || 0} 个任务，请确认后开始`);
        renderPhasePanels();
        renderPlanPicker();
      }
    } else if (view.status === "plan_failed") {
      state.phase = "pick";
      if (err) {
        err.textContent = view.error || "规划失败";
        err.hidden = false;
      }
      toast(view.error || "规划失败");
      renderPhasePanels();
      renderPlanPicker();
    } else {
      state.phase = "planning";
      renderPhasePanels();
      startPlanJobPoll();
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
    refreshPlanJob().catch(() => {});
  }, 800);
}

async function refreshPlanJob() {
  if (!state.planJobId) return;
  try {
    const view = await invoke("get_plan_job_cmd", { job_id: state.planJobId });
    state.planJob = view;
    if (view.status === "planned") {
      stopPlanJobPoll();
      if (!state.confirmTaskId && view.tasks?.length) {
        state.confirmTaskId = view.tasks[0].id;
      }
      if (state.autoStartAfterPlan) {
        toast(`已拆分 ${view.task_count || 0} 个任务，正在启动…`);
        state.phase = "confirm";
        renderPhasePanels();
        await confirmAndStart();
      } else {
        state.phase = "confirm";
        toast(`已拆分 ${view.task_count || 0} 个任务，请确认后开始`);
        renderPhasePanels();
      }
    } else if (view.status === "plan_failed") {
      stopPlanJobPoll();
      state.phase = "pick";
      const err = $("#pp-error");
      if (err) {
        err.textContent = view.error || "规划失败";
        err.hidden = false;
      }
      toast(view.error || "规划失败");
      renderPhasePanels();
      renderPlanPicker();
    } else if (view.status === "planning") {
      state.phase = "planning";
      const logEl = $("#planner-log");
      if (logEl) {
        logEl.textContent = view.planner_log_tail || "规划中…";
        logEl.scrollTop = logEl.scrollHeight;
      }
      renderPhasePanels();
    } else if (view.status === "confirmed" && view.run_id) {
      stopPlanJobPoll();
      state.phase = "running";
      renderPhasePanels();
    } else {
      renderPhasePanels();
    }
  } catch (e) {
    console.warn(e);
  }
}

function renderConfirmPanel() {
  const job = state.planJob;
  if (!job) return;
  const layers = job.layers || [];
  const tasks = job.tasks || [];
  const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));

  $("#confirm-title").textContent = job.plan_name
    ? `待确认：${job.plan_name}`
    : "待确认的执行计划";
  $("#confirm-meta").textContent = `${job.task_count || tasks.length} 个任务 · 最多同时 ${
    job.max_parallel ?? "—"
  } 个 · ${layers.length} 波 · 规划方式 ${job.plan_mode || "—"}`;

  const waves = $("#confirm-waves");
  waves.innerHTML = layers
    .map((layer, i) => {
      const rows = layer
        .map((id) => {
          const t = byId[id] || { id, title: id, depends_on: [] };
          const sel = state.confirmTaskId === id ? " selected" : "";
          const deps =
            t.depends_on && t.depends_on.length
              ? `等待 ${t.depends_on.join(", ")}`
              : "可立即开始";
          return `<button type="button" class="wave-task${sel}" data-id="${esc(id)}">
            <div class="wave-task-title">${esc(t.title || id)}</div>
            <div class="wave-task-meta muted">${esc(id)} · ${esc(deps)}</div>
          </button>`;
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
      state.confirmTaskId = b.dataset.id;
      renderConfirmPanel();
    };
  });

  const cur = byId[state.confirmTaskId] || tasks[0];
  if (cur) {
    state.confirmTaskId = cur.id;
    $("#confirm-task-title").textContent = `${cur.title}（${cur.id}）`;
    $("#confirm-task-title").classList.remove("muted");
    $("#confirm-task-deps").textContent =
      cur.depends_on?.length > 0
        ? `依赖：${cur.depends_on.join(", ")}`
        : "无依赖，属于首波";
    $("#confirm-task-prompt").textContent = cur.prompt_preview || "（无预览）";
  } else {
    $("#confirm-task-title").textContent = "选择左侧任务查看说明";
    $("#confirm-task-title").classList.add("muted");
    $("#confirm-task-deps").textContent = "";
    $("#confirm-task-prompt").textContent = "";
  }
  $("#confirm-error").hidden = true;
}

/** Only from confirm phase — starts workers. */
async function confirmAndStart() {
  const err = $("#confirm-error");
  err.hidden = true;
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
    const res = await invoke("confirm_start_cmd", { job_id: state.planJobId });
    toast("已开始运行");
    state.phase = "running";
    state.selectedTaskId = null;
    state.planCollapsed = true;
    state.closedPanels = {};
    renderPhasePanels();
    renderPlanPicker();
    await loadLive();
    await loadProjects();
  } catch (e) {
    err.textContent = String(e);
    err.hidden = false;
    toast(String(e));
  }
}

function cancelPlanning() {
  stopPlanJobPoll();
  state.phase = "pick";
  state.planJobId = null;
  state.planJob = null;
  renderPhasePanels();
  renderPlanPicker();
}

function replanFromConfirm() {
  state.phase = "pick";
  // keep selected plan; clear job
  state.planJobId = null;
  state.planJob = null;
  renderPhasePanels();
  renderPlanPicker();
  toast("可调整高级选项后再次「分析并拆分任务」");
}

/* ── Workspace live ── */
async function loadLive() {
  if (!state.selectedPath) {
    state.live = null;
    return;
  }
  state.now = Date.now();
  state.live = await invoke("get_project_live", {
    project: state.selectedPath,
    log_max_bytes: 96000,
  });
  // auto-select task
  ensureSelectedTask();
  renderWorkspace();
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

function renderWorkspace() {
  updateWorkspaceTitle();
  const live = state.live;
  const runStatus = live?.run_status;
  const hasRun = !!live?.run_id;
  const active = isLiveStatus(runStatus);
  const tasks = live?.tasks || [];
  const finished =
    hasRun &&
    !active &&
    ["completed", "done", "failed", "aborted", "stopped", "paused"].includes(
      String(runStatus || "").toLowerCase()
    );

  // Don't clobber planning/confirm with live sync unless exec is active
  if (active) {
    state.phase = "running";
  } else if (finished && state.phase === "running") {
    state.phase = "done";
  }

  renderDoctorWarn();
  renderPhasePanels();
  if (state.phase === "pick" || state.phase === "done") {
    renderPlanPicker();
  }

  $("#run-banner").hidden = !hasRun || state.phase === "planning" || state.phase === "confirm";
  if (hasRun) {
    $("#run-status-badge").innerHTML = badge(runStatus || "idle");
    const done = tasks.filter((t) => isDoneStatus(t.status)).length;
    const total = tasks.length;
    $("#run-progress-label").textContent = total ? `已完成 ${done}/${total}` : "";
    const layers = live?.layers || [];
    const waveEl = $("#run-wave-label");
    if (waveEl) {
      if (live?.current_wave && layers.length) {
        waveEl.textContent = `· 第 ${live.current_wave}/${layers.length} 波`;
      } else if (layers.length) {
        waveEl.textContent = `· 共 ${layers.length} 波`;
      } else {
        waveEl.textContent = "";
      }
    }
    const runEnd = finished
      ? (tasks.map((t) => t.finished_at).filter(Boolean).sort().slice(-1)[0] || null)
      : null;
    $("#run-elapsed-label").textContent = live?.started_at
      ? `用时 ${formatElapsed(live.started_at, runEnd)}`
      : "";
    $("#run-plan-label").textContent = live?.plan_path
      ? `· ${planDisplayName(live.plan_path)}`
      : "";
    $("#btn-ws-stop-all").hidden = !active;
    $("#btn-ws-resume").hidden = !["paused", "failed", "aborted"].includes(
      String(runStatus || "").toLowerCase()
    );
    // 隐藏运行按钮：仅在非 planning/confirm 时显示
    $("#btn-ws-dismiss-run").hidden = state.phase === "planning" || state.phase === "confirm";
  }

  // Error summary bar
  const errBar = $("#error-summary");
  const failedTasks = tasks.filter((t) => isFailedStatus(t.status));
  if (failedTasks.length) {
    const first = failedTasks[0];
    const sum = taskErrorSummary(first);
    $("#error-summary-text").textContent = sum
      ? `${first.task_id}：${sum}`
      : `${failedTasks.length} 个任务失败`;
    errBar.hidden = false;
  } else {
    errBar.hidden = true;
  }

  // Completion panel
  const comp = $("#completion-panel");
  if (finished && tasks.length && state.phase !== "planning" && state.phase !== "confirm") {
    comp.hidden = false;
    comp.classList.add("compact");
    const failN = failedTasks.length;
    const okN = tasks.filter((t) => isDoneStatus(t.status)).length;
    const st = String(runStatus || "").toLowerCase();
    if (failN === 0 && (st === "completed" || st === "done")) {
      $("#completion-title").textContent = "全部完成";
    } else if (st === "paused") {
      $("#completion-title").textContent = "已暂停";
    } else {
      $("#completion-title").textContent = "运行结束（有失败）";
    }
    const endAt = tasks.map((t) => t.finished_at).filter(Boolean).sort().slice(-1)[0] || null;
    $("#completion-stats").textContent = `成功 ${okN} · 失败 ${failN} · 共 ${tasks.length} · 用时 ${formatElapsed(
      live.started_at,
      endAt
    )}`;
    $("#completion-list").innerHTML = tasks
      .map((t) => {
        const cls = isFailedStatus(t.status)
          ? "fail-item"
          : isDoneStatus(t.status)
            ? "ok-item"
            : "";
        const sum = taskErrorSummary(t);
        return `<div class="${cls}">${esc(statusLabel(t.status))} · ${esc(t.task_id)}${
          sum ? ` — ${esc(sum)}` : ""
        }</div>`;
      })
      .join("");
  } else {
    comp.hidden = true;
  }

  // Multi-window CLI board
  const monitor = $("#monitor");
  const cliEmpty = $("#cli-empty");
  if (state.phase === "planning" || state.phase === "confirm") {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = true;
    return;
  }
  if (!tasks.length) {
    if (monitor) monitor.hidden = true;
    if (cliEmpty) cliEmpty.hidden = hasRun;
    return;
  }
  if (monitor) monitor.hidden = false;
  if (cliEmpty) cliEmpty.hidden = true;
  if ($("#task-list-pane")) $("#task-list-pane").hidden = true;
  if ($("#detail-pane")) $("#detail-pane").hidden = true;

  renderCliBoard(tasks);
}

function savePanelPos() {
  try {
    localStorage.setItem("cco.panelPos", JSON.stringify(state.panelPos || {}));
  } catch (_) {}
}

function panelLogHtml(t) {
  const st = String(t.status || "").toLowerCase();
  const events = Array.isArray(t.log_events) ? t.log_events : [];
  let raw = t.log_tail || "";
  if (!raw && t.error) raw = `error: ${t.error}`;
  if (!raw && isLiveStatus(st)) {
    raw = `任务已启动，等待 CLI 输出…\n状态: ${statusLabel(t.status)}`;
  } else if (!raw && isFailedStatus(st)) {
    raw = `任务失败，无日志输出。\n状态: ${statusLabel(t.status)}`;
  }
  const mode = state.logViewMode || "term";
  if (mode === "raw" || !events.length) {
    // 短终端：最多末尾 40 行
    const lines = String(raw || "").split("\n");
    const tail = lines.slice(-40).join("\n");
    return `<pre class="panel-log-pre">${esc(tail)}</pre>`;
  }
  if (mode === "pretty") {
    return events.slice(-30).map((e) => renderLogEvent(e)).join("");
  }
  return events.slice(-40).map((e) => renderTranscriptLine(e)).join("");
}

function renderCliBoard(tasks) {
  const board = $("#cli-board");
  if (!board) return;

  let shown = tasks;
  if (state.filterFailedOnly) {
    shown = tasks.filter((t) => isFailedStatus(t.status));
    if (!shown.length) shown = tasks;
  }
  const filt = $("#btn-filter-failed");
  if (filt) filt.classList.toggle("active", state.filterFailedOnly);

  const closedCount = Object.keys(state.closedPanels || {}).filter((id) =>
    tasks.some((t) => t.task_id === id)
  ).length;
  const restoreBtn = $("#btn-restore-panels");
  if (restoreBtn) {
    restoreBtn.hidden = closedCount === 0;
    restoreBtn.textContent = `恢复已关闭 (${closedCount})`;
  }

  // 可见面板
  const visible = shown.filter((t) => !state.closedPanels[t.task_id]);
  // 自动布局：网格，若用户拖过则用绝对坐标
  const cols = Math.max(1, Math.min(2, visible.length));
  board.classList.toggle("single", visible.length === 1);
  board.innerHTML = "";

  visible.forEach((t, idx) => {
    const st = String(t.status || "").toLowerCase();
    const failed = isFailedStatus(st);
    const title = t.title || t.task_id;
    const elapsed = formatElapsed(t.started_at, t.finished_at);
    const sum = taskErrorSummary(t);
    const pos = state.panelPos[t.task_id];
    const card = document.createElement("div");
    card.className = `cli-window${failed ? " failed" : ""}${
      t.task_id === state.selectedTaskId ? " selected" : ""
    }`;
    card.dataset.task = t.task_id;
    if (pos && typeof pos.x === "number" && typeof pos.y === "number") {
      card.classList.add("free");
      card.style.left = pos.x + "px";
      card.style.top = pos.y + "px";
      if (pos.w) card.style.width = pos.w + "px";
    } else {
      // flow grid index hint
      card.dataset.slot = String(idx);
    }
    card.innerHTML = `
      <div class="cli-window-head" data-drag="${esc(t.task_id)}">
        <div class="cli-window-title">
          <span class="dot ${statusDot(st)}"></span>
          <strong title="${esc(title)}">${esc(title)}</strong>
          ${badge(t.status)}
        </div>
        <div class="cli-window-actions">
          <button type="button" class="icon-btn sm" data-focus="${esc(t.task_id)}" title="聚焦">◉</button>
          <button type="button" class="icon-btn sm" data-close="${esc(t.task_id)}" title="关闭窗口">×</button>
        </div>
      </div>
      <div class="cli-window-meta muted">
        ${esc(t.task_id)} · ${esc(elapsed)}${
          t.cost_usd != null ? ` · $${Number(t.cost_usd).toFixed(4)}` : ""
        }${t.provider ? ` · ${esc(t.provider)}` : ""}
      </div>
      ${
        sum && failed
          ? `<div class="cli-window-err" title="${esc(sum)}">${esc(sum)}</div>`
          : ""
      }
      <div class="cli-window-body log-console term-mode" data-log="${esc(t.task_id)}"></div>
      <div class="cli-window-foot">
        <button type="button" class="btn ghost sm" data-copy="${esc(t.task_id)}">复制</button>
        <button type="button" class="btn danger sm" data-stop="${esc(t.task_id)}" ${
          isLiveStatus(st) ? "" : "hidden"
        }>停止</button>
      </div>`;
    board.appendChild(card);
    const body = card.querySelector(".cli-window-body");
    if (body) {
      body.innerHTML = panelLogHtml(t);
      // 默认贴底
      body.scrollTop = body.scrollHeight;
    }
  });

  // events
  $$("[data-close]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      state.closedPanels[b.dataset.close] = true;
      renderCliBoard(tasks);
    };
  });
  $$("[data-focus]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      state.selectedTaskId = b.dataset.focus;
      // bring to front
      const card = board.querySelector(`.cli-window[data-task="${CSS.escape(b.dataset.focus)}"]`);
      if (card) {
        card.style.zIndex = String(Date.now() % 100000);
        card.classList.add("selected");
      }
    };
  });
  $$("[data-copy]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      const t = tasks.find((x) => x.task_id === b.dataset.copy);
      const text =
        (t && Array.isArray(t.log_events) && t.log_events.length
          ? t.log_events.map((ev) => `${ev.kind}\t${ev.title || ""}\t${ev.summary || ""}`).join("\n")
          : t?.log_tail) || "";
      try {
        await navigator.clipboard.writeText(text);
        toast("日志已复制");
      } catch (_) {
        toast("复制失败");
      }
    };
  });
  $$("[data-stop]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      state.selectedTaskId = b.dataset.stop;
      await cancelTask();
    };
  });

  // drag move
  $$("[data-drag]", board).forEach((head) => {
    head.onpointerdown = (ev) => {
      if (ev.button !== 0) return;
      if (ev.target.closest("button")) return;
      const id = head.dataset.drag;
      const card = head.closest(".cli-window");
      if (!card) return;
      const rect = card.getBoundingClientRect();
      const boardRect = board.getBoundingClientRect();
      // switch to free layout
      card.classList.add("free");
      const x = rect.left - boardRect.left + board.scrollLeft;
      const y = rect.top - boardRect.top + board.scrollTop;
      card.style.left = x + "px";
      card.style.top = y + "px";
      card.style.width = rect.width + "px";
      card.style.zIndex = String(Date.now() % 100000);
      state.drag = {
        id,
        ox: ev.clientX - rect.left,
        oy: ev.clientY - rect.top,
      };
      head.setPointerCapture(ev.pointerId);
    };
    head.onpointermove = (ev) => {
      if (!state.drag || state.drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const boardRect = board.getBoundingClientRect();
      let x = ev.clientX - boardRect.left - state.drag.ox + board.scrollLeft;
      let y = ev.clientY - boardRect.top - state.drag.oy + board.scrollTop;
      x = Math.max(0, x);
      y = Math.max(0, y);
      card.style.left = x + "px";
      card.style.top = y + "px";
    };
    head.onpointerup = (ev) => {
      if (!state.drag || state.drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const id = state.drag.id;
      state.drag = null;
      state.panelPos[id] = {
        x: parseFloat(card.style.left) || 0,
        y: parseFloat(card.style.top) || 0,
        w: card.offsetWidth,
      };
      savePanelPos();
      try {
        head.releasePointerCapture(ev.pointerId);
      } catch (_) {}
    };
  });
}

function renderTaskList(tasks) {
  // 兼容旧调用：转交看板
  renderCliBoard(tasks);
}

function renderDetailLog(tasks) {
  // 紧凑多窗口模式下，日志已在各窗口内；保留隐藏 detail 同步以便复制按钮
  const t = tasks.find((x) => x.task_id === state.selectedTaskId) || tasks[0];
  if (!t) return;
  const logEl = $("#cli-detail-log");
  if (logEl) {
    logEl.textContent = t.log_tail || "";
  }
  const stop = $("#btn-stop-task");
  if (stop) stop.hidden = !isLiveStatus(t.status);
}

function transcriptRole(e) {
  const k = String(e.kind || "");
  if (k === "tool_use" || k === "tool_result") return "tool";
  if (k === "message") return "assistant";
  if (k === "result") return "result";
  if (k === "error") return "error";
  if (k === "stderr") return "stderr";
  if (k === "meta") return "meta";
  if (k === "raw_line") return "out";
  return k || "out";
}

function renderTranscriptLine(e) {
  const role = transcriptRole(e);
  const label =
    role === "tool"
      ? e.kind === "tool_result"
        ? "result"
        : "tool"
      : role === "assistant"
        ? "asst"
        : role === "out"
          ? "out"
          : role;
  // stderr: single folded block
  if (e.kind === "stderr") {
    const body = esc(e.detail || e.summary || "");
    const title = esc(e.title || "stderr");
    const sum = esc(e.summary || "");
    return `<details class="tx-fold">
      <summary>⚠ ${title}${sum ? " — " + sum : ""}</summary>
      <pre>${body}</pre>
    </details>`;
  }
  const title = e.title && e.title !== label ? esc(e.title) : "";
  const summary = esc(e.summary || "");
  const detail = e.detail ? esc(e.detail) : "";
  let body = "";
  if (title && summary) body = `<span style="opacity:.85">${title}</span>  ${summary}`;
  else body = summary || title || "…";
  if (detail && e.kind === "result") {
    // keep result detail folded to avoid huge blocks
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.2rem"><summary>展开详情</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  if (detail && (e.kind === "tool_use" || e.kind === "message") && detail.length > 160) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  return `<div class="tx-line role-${esc(role)}">
    <div class="tx-role">${esc(label)}</div>
    <div class="tx-body">${body}</div>
  </div>`;
}

function renderLogEvent(e) {
  const kind = esc(e.kind || "raw_line");
  const level = esc(e.level || "info");
  const title = esc(e.title || kind);
  const summary = esc(e.summary || "");
  const detail = e.detail ? esc(e.detail) : "";
  const detailBlock = detail
    ? `<details><summary>展开详情</summary><div class="detail">${detail}</div></details>`
    : "";
  return `<div class="log-event kind-${kind} level-${level}">
    <div class="kind">${kind}</div>
    <div class="body">
      <div class="title">${title}</div>
      <div class="summary">${summary}</div>
      ${detailBlock}
    </div>
  </div>`;
}


async function cancelTask() {
  const runId = state.live?.run_id;
  const taskId = state.selectedTaskId;
  if (!runId || typeof runId !== 'string' || !runId.trim()) {
    return toast("无活动任务");
  }
  if (!taskId) return toast("请先选择任务");
  try {
    await invoke("stop_task_cmd", { runId, taskId });
    toast(`已停止 ${taskId}`);
    await loadLive();
    await loadProjects();
  } catch (e) {
    toast(String(e));
  }
}

async function stopAll() {
  const runId = state.live?.run_id;
  // 防御性检查：确保 run_id 有意义
  if (!runId || typeof runId !== 'string' || runId.trim() === '') {
    console.warn("stopAll: no valid run_id, skipping");
    return;
  }
  try {
    await invoke("stop_run_cmd", { runId });
    toast("已请求全部停止");
    await loadLive();
    await loadProjects();
  } catch (e) {
    toast(String(e));
  }
}

async function resumeRun() {
  const runId = state.live?.run_id;
  if (!runId || typeof runId !== 'string' || !runId.trim()) {
    return toast("无运行记录可继续");
  }
  try {
    await invoke("resume_run_cmd", { runId });
    toast("正在继续…");
    setTimeout(() => {
      loadLive().catch(() => {});
      loadProjects().catch(() => {});
    }, 800);
  } catch (e) {
    toast(String(e));
  }
}

/* ── Doctor page ── */
async function loadDoctor() {
  try {
    const d = await invoke("doctor_cmd", { project: state.selectedPath || null });
    state.doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    $("#doctor-list").innerHTML = `<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th></tr></thead>
      <tbody>
        ${lines
          .map(
            (l) => `<tr>
          <td>${esc(l.name)}</td>
          <td>${l.ok ? badge("ok") : badge("failed")}</td>
          <td class="muted">${esc(l.detail)}</td>
        </tr>`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">${d.ok ? "关键检查通过" : "存在失败项，请按详情处理"}</p>`;
    renderDoctorWarn();
  } catch (e) {
    toast(String(e));
  }
}

/* ── Poll ── */
function startPolling(intervalMs = 2000) {
  clearInterval(state.pollTimer);
  state.pollTimer = setInterval(() => {
    state.now = Date.now();
    if (state.page === "workspace" && state.selectedPath) {
      loadProjects().catch(() => {});
      loadLive().catch(() => {});
    } else if (state.page === "welcome") {
      loadProjects().catch(() => {});
    }
  }, intervalMs);
}

/* ── Settings ── */
async function loadSettings() {
  try {
    const s = await invoke("get_settings_cmd");
    $("#s-poll-interval").value = s.poll_interval_secs;
    const modeIdx = { print: 0, bg: 1, auto: 2 };
    $("#s-default-mode").value = modeIdx[s.default_mode] ?? 0;
    $("#s-default-provider").value = s.default_provider;
    $("#s-max-parallel").value = s.max_parallel;
    $("#s-log-font").value = String(state.logFontSize);
  } catch (_) {
    /* ignore */
  }
}

async function saveSettings() {
  const pollVal = parseInt($("#s-poll-interval").value, 10);
  const modeVal = parseInt($("#s-default-mode").value, 10);
  const providerVal = $("#s-default-provider").value.trim();
  const maxParallelVal = parseInt($("#s-max-parallel").value, 10);
  const fontVal = parseInt($("#s-log-font").value, 10) || 14;
  const status = $("#s-save-status");
  if (!pollVal || pollVal < 1 || pollVal > 60) {
    status.className = "save-status err";
    status.textContent = "刷新间隔需在 1–60 秒之间";
    status.hidden = false;
    return;
  }
  try {
    const updated = await invoke("set_settings_cmd", {
      update: {
        poll_interval_secs: pollVal,
        default_mode: modeVal,
        default_provider: providerVal,
        max_parallel: maxParallelVal || 2,
      },
    });
    applyLogFontSize(fontVal);
    // sync picker defaults
    if ($("#pp-provider")) $("#pp-provider").value = providerVal;
    status.className = "save-status ok";
    status.textContent = "已保存";
    status.hidden = false;
    setTimeout(() => {
      status.hidden = true;
    }, 2500);
    startPolling(Math.min(updated.poll_interval_secs * 1000, 5000));
  } catch (e) {
    status.className = "save-status err";
    status.textContent = "保存失败: " + e;
    status.hidden = false;
  }
}

function backFromSubpage() {
  if (state.selectedPath) {
    showPage("workspace");
    renderWorkspace();
  } else {
    goHome();
  }
}

/* ── Wire ── */
function wire() {
  applyLogFontSize(state.logFontSize);

  $("#btn-add-plus").onclick = () => openModal();
  $("#btn-welcome-add").onclick = () => openModal();
  $("#btn-welcome-help").onclick = () => showPage("help");

  $("#btn-refresh").onclick = async () => {
    try {
      if (state.page === "workspace" && state.selectedPath) {
        await loadProjects();
        await loadLive();
        await loadPlansForPicker();
      } else {
        await loadProjects();
      }
      toast("已刷新");
    } catch (e) {
      toast(String(e));
    }
  };

  $("#modal-close").onclick = closeModal;
  $("#modal-backdrop").onclick = closeModal;
  $("#m-pick-folder").onclick = pickFolderToModal;
  $("#m-confirm-project").onclick = addProjectFromModal;
  $("#m-cancel-project").onclick = closeModal;

  $("#btn-ws-stop-all").onclick = stopAll;
  $("#btn-ws-resume").onclick = resumeRun;
  $("#btn-remove-project").onclick = removeSelectedProject;
  $("#btn-ws-dismiss-run").onclick = dismissRun;
  $("#btn-stop-task").onclick = cancelTask;

  // 计划：紧凑条 + 选择器
  const bind = (id, fn) => {
    const el = $(id);
    if (el) el.onclick = fn;
  };
  bind("#btn-pp-scan", () => loadPlansForPicker().then(() => renderPlanChooser()).catch(() => {}));
  bind("#btn-pp-pick", pickPlanFileForPicker);
  bind("#btn-pp-pick-empty", pickPlanFileForPicker);
  bind("#btn-chooser-scan", () => loadPlansForPicker().then(() => renderPlanChooser()).catch(() => {}));
  bind("#btn-chooser-pick", pickPlanFileForPicker);
  bind("#btn-chooser-close", () => openPlanChooser(false));
  bind("#btn-plan-choose", async () => {
    await loadPlansForPicker();
    openPlanChooser(true);
  });
  bind("#btn-pp-analyze", analyzePlanFromPicker);
  bind("#btn-pp-set-default", setDefaultPlan);
  bind("#btn-confirm-start", confirmAndStart);
  bind("#btn-replan", replanFromConfirm);
  bind("#btn-cancel-planning", cancelPlanning);
  bind("#btn-plan-expand", () => openPlanChooser(true));
  bind("#btn-restore-panels", () => {
    state.closedPanels = {};
    const tasks = state.live?.tasks || [];
    renderCliBoard(tasks);
  });
  bind("#btn-doctor-dismiss", () => {
    const d = state.doctorCache;
    const fails = (d?.lines || []).filter((l) => !l.ok);
    state.doctorDismissedKey = fails.map((l) => l.name + ":" + l.detail).join("|") || "dismissed";
    renderDoctorWarn();
    toast("已暂时忽略环境提示");
  });
  // 点遮罩关闭选择器
  const chooser = $("#plan-chooser");
  if (chooser) {
    chooser.addEventListener("click", (e) => {
      if (e.target === chooser) openPlanChooser(false);
    });
  }

  bind("#btn-advanced-toggle", () => {
    state.advancedOpen = !state.advancedOpen;
    localStorage.setItem(ADVANCED_KEY, state.advancedOpen ? "1" : "0");
    const adv = $("#advanced-body");
    if (adv) adv.hidden = !state.advancedOpen;
    const tog = $("#btn-advanced-toggle");
    if (tog) tog.textContent = state.advancedOpen ? "▾ 高级" : "▸ 高级";
  });
  $("#pp-provider")?.addEventListener("change", () => {
    $("#pp-provider").dataset.touched = "1";
  });

  bind("#btn-filter-failed", () => {
    state.filterFailedOnly = !state.filterFailedOnly;
    const tasks = state.live?.tasks || [];
    renderCliBoard(tasks);
  });

  $("#btn-copy-log").onclick = async () => {
    const t = (state.live?.tasks || []).find((x) => x.task_id === state.selectedTaskId)
      || (state.live?.tasks || [])[0];
    let text = "";
    if (t && state.logViewMode !== "raw" && Array.isArray(t.log_events) && t.log_events.length) {
      text = t.log_events
        .map((e) => {
          const head = `${e.kind}\t${e.title || ""}\t${e.summary || ""}`;
          if (e.detail && e.kind === "stderr") {
            return `${head}\n${String(e.detail).split("\n").slice(0, 40).join("\n")}`;
          }
          return head;
        })
        .join("\n");
    } else {
      text = $("#cli-detail-log")?.textContent || t?.log_tail || "";
    }
    try {
      await navigator.clipboard.writeText(text);
      toast("日志已复制");
    } catch (_) {
      toast("复制失败");
    }
  };

  $$("#log-view-mode button").forEach((b) => {
    b.onclick = () => {
      state.logViewMode = b.dataset.mode || "term";
      localStorage.setItem("cco.logViewMode", state.logViewMode);
      const tasks = state.live?.tasks || [];
      if (tasks.length) renderCliBoard(tasks);
      else renderDetailLog(tasks);
    };
  });

  $$("#log-font-group button").forEach((b) => {
    b.onclick = () => applyLogFontSize(Number(b.dataset.size));
  });

  const logEl = $("#cli-detail-log");
  logEl?.addEventListener("scroll", () => {
    state.logStick = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 40;
  });

  $("#btn-rerun").onclick = () => {
    state.phase = "pick";
    state.planJobId = null;
    state.planJob = null;
    state.planCollapsed = true;
    setPlanCollapsed(true);
    $("#completion-panel").hidden = true;
    renderPhasePanels();
    renderPlanPicker();
    if (state.selectedPlan) analyzePlanFromPicker();
    else {
      openPlanChooser(true);
      toast("请先选择计划");
    }
  };
  $("#btn-change-plan").onclick = async () => {
    state.phase = "pick";
    state.planJobId = null;
    state.planJob = null;
    state.planCollapsed = true;
    setPlanCollapsed(true);
    $("#completion-panel").hidden = true;
    renderPhasePanels();
    renderPlanPicker();
    await loadPlansForPicker();
    openPlanChooser(true);
  };

  $("#btn-doctor-recheck").onclick = async () => {
    await ensureDoctor(true);
    toast(state.doctorCache?.ok ? "环境正常" : "仍有问题，请查看详情");
  };
  $("#btn-doctor-open").onclick = async () => {
    showPage("doctor");
    await loadDoctor();
  };

  $("#btn-open-doctor").onclick = async () => {
    showPage("doctor");
    await loadDoctor();
  };
  $("#btn-doctor").onclick = loadDoctor;
  $("#btn-doctor-back").onclick = backFromSubpage;

  $("#btn-open-settings").onclick = async () => {
    showPage("settings");
    await loadSettings();
  };
  $("#btn-settings-save").onclick = saveSettings;
  $("#btn-settings-back").onclick = backFromSubpage;

  $("#btn-open-help").onclick = () => showPage("help");
  $("#btn-help-back").onclick = backFromSubpage;

  $("#brand-home")?.addEventListener("click", goHome);
}

async function boot() {
  wire();
  try {
    const meta = await invoke("meta");
    $("#conn-status").textContent = `桌面应用 · v${meta.version}`;
    await loadProjects();
    // 恢复上次项目：有活动优先，否则第一个
    const active = state.projects.find(
      (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
    );
    if (active) {
      await selectProject(active.path);
    } else if (state.projects.length === 1) {
      await selectProject(state.projects[0].path);
    } else if (state.projects.length > 0) {
      goHome();
    } else {
      showPage("welcome");
    }
    startPolling();
  } catch (e) {
    $("#conn-status").textContent = "需要通过 CCO.app 启动";
  }
}

function waitTauri() {
  if (getInvoke()) boot();
  else setTimeout(waitTauri, 50);
}
waitTauri();
