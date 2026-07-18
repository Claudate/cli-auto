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
  dragSession: {}, // taskId -> true 本会话拖过才保留 free
  taskStripExpanded: localStorage.getItem("cco.taskStripExpanded") === "1",
  taskDashCollapsed: localStorage.getItem("cco.taskDashCollapsed") === "1",
  cliBodyHeight: Number(localStorage.getItem("cco.cliBodyHeight") || 300) || 300,
  assigning: false, // 分配计划进行中（防连点 + 按钮转圈）
  plansLoading: false,
  planPollFails: 0,
  planStartedAt: 0,
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
  if (!t) {
    console.log("[toast]", msg);
    return;
  }
  t.hidden = false;
  t.textContent = msg;
  clearTimeout(toast._t);
  toast._t = setTimeout(() => {
    t.hidden = true;
  }, 3200);
}

function getInvoke() {
  const w = window;
  // Tauri 2 多种全局形态，全部兜底
  const candidates = [
    w.__TAURI__?.core?.invoke && w.__TAURI__.core.invoke.bind(w.__TAURI__.core),
    w.__TAURI__?.tauri?.invoke && w.__TAURI__.tauri.invoke.bind(w.__TAURI__.tauri),
    w.__TAURI_INTERNALS__?.invoke && w.__TAURI_INTERNALS__.invoke.bind(w.__TAURI_INTERNALS__),
    typeof w.__TAURI_INVOKE__ === "function" && w.__TAURI_INVOKE__,
  ];
  for (const c of candidates) {
    if (typeof c === "function") return c;
  }
  return null;
}

function isTauriReady() {
  return !!getInvoke();
}

async function invoke(cmd, args = {}) {
  const inv = getInvoke();
  if (!inv) throw new Error("请通过 CCO.app 启动（invoke 不可用）");
  try {
    return await inv(cmd, args);
  } catch (e) {
    const msg = e?.message || e?.toString?.() || String(e);
    throw new Error(msg);
  }
}

async function openNativeDialog(opts) {
  // tauri-plugin-dialog 全局形态兜底
  const d =
    window.__TAURI__?.dialog ||
    window.__TAURI__?.plugins?.dialog ||
    null;
  if (d?.open) return d.open(opts);
  // 动态 import（若打包进了 webview）
  try {
    if (window.__TAURI__?.core?.invoke) {
      // plugin command path used by some builds
      return await invoke("plugin:dialog|open", opts);
    }
  } catch (_) {}
  throw new Error("对话框不可用");
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


/** 把绝对路径收成项目相对路径，便于列表匹配与预览 */
function normalizePlanPath(planPath, projectRoot = state.selectedPath) {
  if (!planPath) return null;
  let p = String(planPath).trim();
  if (!p) return null;
  if (projectRoot) {
    const root = String(projectRoot).replace(/\/+$/, "");
    if (p === root) return null;
    if (p.startsWith(root + "/")) p = p.slice(root.length + 1);
  }
  // 兼容 file:// 与重复项目前缀
  p = p.replace(/^file:\/\//, "");
  return p;
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
  try { updateTopPlanInfo(); } catch (_) {}
  $$(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${name}`));
  const sub = $("#page-sub");
  if (name === "welcome") {
    $("#page-title").textContent = "欢迎";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "添加项目 → 选计划 → 开始运行";
    }
  } else if (name === "workspace") {
    updateWorkspaceTitle();
  } else if (name === "doctor") {
    $("#page-title").textContent = "环境检查";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "确认本机 CLI 与依赖就绪";
    }
  } else if (name === "help") {
    $("#page-title").textContent = "帮助";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "";
    }
  } else if (name === "settings") {
    $("#page-title").textContent = "设置";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "";
    }
  }
}

function updateWorkspaceTitle() {
  // 工作区标题只展示计划，交给 updateTopPlanInfo
  updateTopPlanInfo();
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
    // 点击由全局委托处理 btn-welcome-add2
  }
}

/* ── Modal（仅添加项目） ── */
function openModal() {
  const m = $("#modal");
  if (m) m.hidden = false;
  const p = $("#m-project-path");
  const n = $("#m-project-name");
  if (p) p.value = "";
  if (n) n.value = "";
}

function closeModal() {
  const m = $("#modal");
  if (m) m.hidden = true;
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
  const rawCandidate =
    state.live?.plan_path || proj?.default_plan || proj?.last_plan || state.plans[0] || null;
  const candidate = normalizePlanPath(rawCandidate, path) || rawCandidate;
  // 完成/运行中也要回填「当前计划」，避免顶栏空白
  if (candidate) {
    try {
      await selectPlan(candidate);
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
    const tail =
      state.planJob?.planner_log_tail ||
      state.planJob?.plannerLogTail ||
      "";
    if (log && tail) {
      log.textContent = tail;
      log.scrollTop = log.scrollHeight;
    } else if (log && !log.textContent) {
      log.textContent = "正在分析…";
    }
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
    const selected = await openNativeDialog({ directory: true, multiple: false });
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
    const list = Array.isArray(plans) ? plans.slice() : [];
    // 用户手动选的计划若不在扫描结果中，置顶保留
    if (state.selectedPlan && !list.includes(state.selectedPlan)) {
      list.unshift(state.selectedPlan);
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
  const btnAdv = $("#btn-advanced-toggle");

  const inWorkspace = !!state.selectedPath && state.page === "workspace";
  const hideForPhase = state.phase === "planning" || state.phase === "confirm";

  // 顶栏按钮：刷新左侧
  if (btnChoose) btnChoose.hidden = !inWorkspace || hideForPhase;
  if (btnAssign) btnAssign.hidden = !inWorkspace || hideForPhase;
  if (btnAdv) btnAdv.hidden = !inWorkspace || hideForPhase;

  // 高级面板容器：仅在展开时显示
  if (pp) {
    if (!inWorkspace || hideForPhase) {
      pp.hidden = true;
    } else {
      // 无「当前计划」条；仅 advanced / error 需要时露头
      const showAdv = !!state.advancedOpen;
      const err = $("#pp-error");
      const hasErr = err && !err.hidden && err.textContent;
      pp.hidden = !(showAdv || hasErr);
      pp.classList.add("headless", "compact");
    }
  }

  if (!inWorkspace) {
    openPlanChooser(false);
    updateTopPlanInfo();
    return;
  }

  const active = isLiveStatus(state.live?.run_status);
  if (btnAssign) {
    // 弹窗化后无计划也可点开选计划；仅运行中禁用
    btnAssign.disabled = !!active;
    btnAssign.textContent = active ? "运行中…" : "分配计划";
  }

  const adv = $("#advanced-body");
  if (adv) adv.hidden = !state.advancedOpen;
  if (btnAdv) btnAdv.textContent = state.advancedOpen ? "收起高级" : "高级";

  try {
    const defP = $("#s-default-provider")?.value;
    if (defP && $("#pp-provider") && !$("#pp-provider").dataset.touched) {
      $("#pp-provider").value = defP;
    }
  } catch (_) {}

  if (state.planChooserOpen) renderPlanChooser();
  updateTopPlanInfo();
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

async function selectPlan(planPath) {
  state.selectedPlan = normalizePlanPath(planPath) || planPath || null;
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
    const selected = await openNativeDialog({
      multiple: false,
      filters: [{ name: "Plan", extensions: ["md", "yaml", "yml", "json"] }],
    });
    if (!selected) return;
    const proj = state.selectedPath;
    const rel = selected.startsWith(proj + "/") ? selected.slice(proj.length + 1) : selected;
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
/** 分配计划：AI 拆分后自动开始执行 */
async function analyzePlanFromPicker() {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (state.assigning) return;
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
  stopPlanJobPoll();
  openPlanChooser(false);
  renderPhasePanels();
  renderPlanPicker();
  renderWorkspaceShell();
  const logEl0 = $("#planner-log");
  if (logEl0) logEl0.textContent = "正在启动规划…";
  const sub0 = $("#planning-sub");
  if (sub0) sub0.textContent = `正在分析 ${planDisplayName(state.selectedPlan)}…`;

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
    // Tauri/serde 字段兼容
    state.planJobId = view.job_id || view.jobId || null;
    state.planStartedAt = Date.now();
    state.planPollFails = 0;
    const logEl = $("#planner-log");
    if (logEl) logEl.textContent = view.planner_log_tail || view.plannerLogTail || "规划中…";

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

async function advancePlannedJob(view) {
  stopPlanJobPoll();
  state.planJob = view;
  if (!state.confirmTaskId && view.tasks?.length) {
    state.confirmTaskId = view.tasks[0].id;
  }
  const n = view.task_count || view.tasks?.length || 0;
  const adapter = view.adapter || "";
  const how =
    adapter.includes("heuristic")
      ? "本地启发式拆分"
      : adapter.includes("llm")
        ? "Claude CLI 规划"
        : "规划完成";
  if (state.autoStartAfterPlan) {
    toast(`${how}：${n} 个任务，正在启动…`);
    state.phase = "confirm";
    renderPhasePanels();
    setAssignBusy(false);
    await confirmAndStart();
  } else {
    toast(`${how}：${n} 个任务，请确认后开始`);
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
      job_id: state.planJobId,
      jobId: state.planJobId,
    });
    state.planPollFails = 0;
    state.planJob = view;
    const status = String(view.status || "").toLowerCase();
    const logTail = view.planner_log_tail || view.plannerLogTail || "";
    const logEl = $("#planner-log");
    if (logEl && logTail) {
      logEl.textContent = logTail;
      logEl.scrollTop = logEl.scrollHeight;
    }

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
  setAssignBusy(false);
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
  // 规划中时顺带刷新 plan job，防止 setInterval 被卡住时永远转圈
  if (state.phase === "planning" && state.planJobId) {
    await refreshPlanJob().catch(() => {});
  }
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

  // 规划/确认相位不可被历史 run 的 finished 状态冲掉（否则转圈面板闪一下就没）
  if (state.phase === "planning" || state.phase === "confirm") {
    // keep planning UI
  } else if (active) {
    state.phase = "running";
  } else if (finished) {
    state.phase = "done";
  }

  const body = $("#workspace-body");
  if (body) {
    body.classList.remove("mode-idle", "mode-running", "mode-done", "mode-plan");
    if (state.phase === "planning" || state.phase === "confirm") body.classList.add("mode-plan");
    else if (active) body.classList.add("mode-running");
    else if (finished) body.classList.add("mode-done");
    else body.classList.add("mode-idle");
  }

  renderDoctorWarn();
  renderPhasePanels();
  if (state.phase === "pick" || state.phase === "done" || state.phase === "running") {
    // 运行态若计划空，从 live 回填
    if (!state.selectedPlan && state.live?.plan_path) {
      state.selectedPlan = normalizePlanPath(state.live.plan_path) || state.live.plan_path;
    }
    renderPlanPicker();
  }
  updateTopPlanInfo();


  // legacy hide
  const runBanner = $("#run-banner");
  if (runBanner) runBanner.hidden = true;
  const errBar = $("#error-summary");
  if (errBar) errBar.hidden = true;
  const comp = $("#completion-panel");
  if (comp) comp.hidden = true;

  renderTaskStrip(live, tasks, {
    hasRun,
    active,
    finished,
    runStatus,
  });

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
  renderCliBoard(tasks);
}

function savePanelPos() {
  try {
    localStorage.setItem("cco.panelPos", JSON.stringify(state.panelPos || {}));
  } catch (_) {}
}


function taskBucket(st) {
  const s = String(st || "").toLowerCase();
  if (isFailedStatus(s)) return "fail";
  if (isDoneStatus(s)) return "done";
  if (isLiveStatus(s) || ["starting", "queued", "running"].includes(s)) return "run";
  return "wait"; // pending / unknown
}

function renderTaskStrip(live, tasks, ctx) {
  const card = $("#result-card");
  if (!card) return;
  const { hasRun, active, finished, runStatus } = ctx;
  card.hidden = !(hasRun && state.phase !== "planning" && state.phase !== "confirm");
  if (card.hidden) return;

  let done = 0, run = 0, wait = 0, fail = 0;
  tasks.forEach((t) => {
    const b = taskBucket(t.status);
    if (b === "done") done++;
    else if (b === "run") run++;
    else if (b === "fail") fail++;
    else wait++;
  });

  card.classList.toggle("ok", finished && fail === 0 && done > 0);
  card.classList.toggle("bad", fail > 0);

  const setN = (id, n) => {
    const el = $(id);
    if (el) el.textContent = String(n);
  };
  setN("#stat-done-n", done);
  setN("#stat-run-n", run);
  setN("#stat-wait-n", wait);
  setN("#stat-fail-n", fail);
  const kpiFail = $("#kpi-fail");
  if (kpiFail) kpiFail.hidden = fail === 0;

  const setStat = (id, label, n) => {
    const el = $(id);
    if (el) el.textContent = `${label} ${n}`;
  };
  setStat("#stat-done", "完成", done);
  setStat("#stat-run", "进行中", run);
  setStat("#stat-wait", "未启动", wait);
  setStat("#stat-fail", "失败", fail);

  const runEnd = finished
    ? tasks.map((t) => t.finished_at).filter(Boolean).sort().slice(-1)[0] || null
    : null;
  const meta = $("#result-meta-text");
  if (meta) {
    const bits = [];
    if (tasks.length) bits.push(`共 ${tasks.length} 项`);
    if (live?.started_at) bits.push(formatElapsed(live.started_at, runEnd));
    meta.textContent = bits.join(" · ");
  }

  const errText = $("#error-summary-text");
  if (errText) {
    if (fail > 0 && !state.taskDashCollapsed) {
      const first = tasks.find((t) => isFailedStatus(t.status));
      const sum = first ? taskErrorSummary(first) : "";
      errText.hidden = false;
      errText.textContent = sum ? `${first.task_id}：${sum}` : `${fail} 个任务失败`;
    } else {
      errText.hidden = true;
      errText.textContent = "";
    }
  }

  const stop = $("#btn-ws-stop-all");
  if (stop) stop.hidden = !active;
  const resume = $("#btn-ws-resume");
  if (resume) {
    resume.hidden = !["paused", "failed", "aborted"].includes(
      String(runStatus || "").toLowerCase()
    );
  }
  // 再跑一次改到 CLI 卡片标题栏；换计划删除；收起改为看板伸缩
  const rerun = $("#btn-rerun");
  if (rerun) rerun.hidden = true;
  const change = $("#btn-change-plan");
  if (change) change.hidden = true;
  const dismiss = $("#btn-ws-dismiss-run");
  if (dismiss) dismiss.hidden = true;

  const toggle = $("#btn-task-dash-toggle");
  if (toggle) {
    toggle.hidden = !hasRun;
    toggle.textContent = state.taskDashCollapsed ? "▸" : "▾";
    toggle.title = state.taskDashCollapsed ? "展开任务看板" : "折叠任务看板";
    toggle.setAttribute("aria-label", toggle.title);
    toggle.setAttribute("aria-expanded", state.taskDashCollapsed ? "false" : "true");
  }
  card.classList.toggle("collapsed", !!state.taskDashCollapsed);

  const body = $("#task-strip-body");
  if (body) body.hidden = !!state.taskDashCollapsed;
  const list = $("#task-strip-list");
  if (!list) return;

  if (!tasks.length) {
    list.innerHTML = `<div class="task-dash-empty muted">暂无拆分任务</div>`;
    return;
  }

  list.innerHTML = tasks
    .map((t) => {
      const b = taskBucket(t.status);
      const label =
        b === "done" ? "已完成" : b === "run" ? "进行中" : b === "fail" ? "失败" : "未启动";
      const title = t.title || t.task_id;
      const sel = t.task_id === state.selectedTaskId ? " selected" : "";
      const elapsed = formatElapsed(t.started_at, t.finished_at);
      const cost = t.cost_usd != null ? `$${Number(t.cost_usd).toFixed(2)}` : "";
      return `<button type="button" class="task-tile ${b}${sel}" data-task="${esc(t.task_id)}">
        <div class="task-tile-top">
          <span class="dot ${statusDot(t.status)}"></span>
          <span class="task-tile-st">${esc(label)}</span>
        </div>
        <div class="task-tile-name" title="${esc(title)}">${esc(title)}</div>
        <div class="task-tile-foot muted">
          <span>${esc(t.task_id)}</span>
          <span>${esc(elapsed)}${cost ? " · " + cost : ""}</span>
        </div>
      </button>`;
    })
    .join("");
}

function applyCliBodyHeight(h) {
  const n = Math.max(160, Math.min(800, Number(h) || 300));
  state.cliBodyHeight = n;
  localStorage.setItem("cco.cliBodyHeight", String(n));
  document.documentElement.style.setProperty("--cli-body-h", n + "px");
  const sel = $("#cli-height-select");
  if (sel && String(sel.value) !== String(n)) {
    // pick closest option or set custom
    const opts = [...sel.options].map((o) => Number(o.value));
    if (opts.includes(n)) sel.value = String(n);
  }
  // update existing bodies
  $$(".cli-window-body").forEach((el) => {
    el.style.height = n + "px";
    el.style.maxHeight = n + "px";
    el.style.minHeight = n + "px";
  });
}

function isAiInteractionEvent(e) {
  if (!e) return false;
  const k = String(e.kind || "").toLowerCase();
  // 红框3：CLI 只保留 AI 对话 / 工具调用 / 结果
  if (k === "message" || k === "tool_use" || k === "tool_result" || k === "result") return true;
  // 业务级 error 可保留；stderr / meta / system 噪音一律丢弃
  if (k === "error") {
    const lvl = String(e.level || "").toLowerCase();
    if (lvl === "debug" || lvl === "trace") return false;
    const blob = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
    if (isNoiseText(blob)) return false;
    return true;
  }
  return false;
}

function isNoiseText(s) {
  const t = String(s || "");
  if (!t.trim()) return true;
  if (/Ignoring\s+--allowedTools/i.test(t)) return true;
  if (/Ignoring\s+--[\w-]+/i.test(t)) return true;
  if (/^\s*stderr\b/i.test(t)) return true;
  if (/^\s*\[?(system|meta|debug|trace)\]?/i.test(t)) return true;
  if (/permission\s*prompt/i.test(t) && /allowedTools/i.test(t)) return true;
  if (/CLI\s*warning/i.test(t)) return true;
  if (/deprecated\s+flag/i.test(t)) return true;
  if (/node:?\s*warn/i.test(t)) return true;
  if (/experimental\s+feature/i.test(t)) return true;
  if (/^\s*warn(ing)?:/i.test(t)) return true;
  return false;
}

function aiLogPlainText(t) {
  const events = (Array.isArray(t?.log_events) ? t.log_events : [])
    .filter(isAiInteractionEvent)
    .filter((ev) => String(ev.kind || "").toLowerCase() !== "result");
  if (events.length) {
    return events
      .map((ev) => {
        const kind = ev.kind || "";
        const title = ev.title || "";
        const summary = ev.summary || "";
        return [kind, title, summary].filter(Boolean).join("\t");
      })
      .join("\n");
  }
  // 无结构化事件时：不回落整段 log_tail，避免系统噪音污染
  if (isLiveStatus(t?.status)) return "AI 运行中，等待交互输出…";
  if (isFailedStatus(t?.status)) return t?.error ? String(t.error) : "任务失败，无 AI 交互日志。";
  return "";
}

function panelLogHtml(t) {
  const st = String(t.status || "").toLowerCase();
  const events = (Array.isArray(t.log_events) ? t.log_events : []).filter(isAiInteractionEvent);
  const mode = state.logViewMode || "term";

  // 默认 term / pretty：只渲染 AI 事件，绝不 dump 原始 log_tail
  // result 摘要不进黑区（成功态窗外徽章已表达）
  const viewEvents = events.filter((e) => String(e.kind || "").toLowerCase() !== "result");
  if (mode !== "raw") {
    if (!viewEvents.length) {
      if (isLiveStatus(st)) {
        return '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>';
      }
      if (isFailedStatus(st)) {
        const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
        return err
          ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
          : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>';
      }
      // 完成且仅有 result 摘要：黑区留空，成功由窗外徽章表达
      return "";
    }
    if (mode === "pretty") {
      return viewEvents.slice(-40).map((e) => renderLogEvent(e)).join("") || "";
    }
    return (
      viewEvents
        .slice(-50)
        .map((e) => renderTranscriptLine(e))
        .filter(Boolean)
        .join("") || ""
    );
  }

  // raw 模式：执行交互文本；result 摘要已在 aiLogPlainText 过滤
  const plain = aiLogPlainText(t);
  if (!plain) {
    if (isLiveStatus(st)) return '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>';
    if (isFailedStatus(st)) {
      const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
      return err
        ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
        : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>';
    }
    return "";
  }
  return '<pre class="panel-log-pre">' + esc(plain) + "</pre>";
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
  if (filt) {
    filt.classList.toggle("active", state.filterFailedOnly);
    // 单任务完成态：过滤器噪音低，仍保留
    filt.hidden = tasks.length <= 1;
  }
  // 单任务时工具条更安静
  const toolbar = document.querySelector(".board-toolbar");
  if (toolbar) toolbar.classList.toggle("quiet", tasks.length <= 1);

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
  board.classList.add("cols-2");
  board.dataset.cols = "2";
  // 强制布局属性，防止旧 inline / 缓存样式
  board.style.display = "grid";
  board.style.gridTemplateColumns = "calc((100% - 0.75rem) / 2) calc((100% - 0.75rem) / 2)";
  board.style.gap = "0.75rem";
  board.style.overflowX = "hidden";
  document.documentElement.style.setProperty("--cli-body-h", (state.cliBodyHeight || 300) + "px");
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
    // 默认一律走 2 列网格，不用 free 绝对定位（避免记忆宽度把窗拉满）
    // 仅当用户本会话拖过且宽度明显是半列时才恢复 free
    const half = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
    const usableFree =
      pos &&
      typeof pos.x === "number" &&
      typeof pos.y === "number" &&
      state.dragSession &&
      state.dragSession[t.task_id];
    if (usableFree) {
      card.classList.add("free");
      card.style.left = pos.x + "px";
      card.style.top = pos.y + "px";
      card.style.width = half + "px";
      card.style.maxWidth = half + "px";
    } else {
      card.classList.remove("free");
      card.dataset.slot = String(idx);
      card.style.left = "";
      card.style.top = "";
      card.style.width = "";
      card.style.maxWidth = "";
      // 双保险：非 free 时清掉可能的全宽 inline
      card.style.gridColumn = "";
    }
    card.innerHTML = `
      <div class="cli-window-head" data-drag="${esc(t.task_id)}">
        <div class="cli-window-title">
          <span class="dot ${statusDot(st)}"></span>
          <strong title="${esc(title)}">${esc(title)}</strong>
          ${badge(t.status)}
        </div>
        <div class="cli-window-actions">
          ${
            !isLiveStatus(state.live?.run_status) && state.live?.run_id
              ? `<button type="button" class="btn primary sm cli-rerun-btn" data-rerun="${esc(t.task_id)}" title="再跑一次">再跑一次</button>`
              : ""
          }
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
      const h = state.cliBodyHeight || 300;
      body.style.height = h + "px";
      body.style.maxHeight = h + "px";
      body.style.minHeight = h + "px";
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
      const text = aiLogPlainText(t);
      try {
        await navigator.clipboard.writeText(text || "");
        toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
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
      const half = Math.max(260, Math.floor((board.clientWidth - 12) / 2));
      card.style.left = x + "px";
      card.style.top = y + "px";
      // 拖出后保持半列宽，避免变成全宽条
      card.style.width = Math.min(rect.width || half, half * 1.15) + "px";
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
      const halfW = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
      state.panelPos[id] = {
        x: parseFloat(card.style.left) || 0,
        y: parseFloat(card.style.top) || 0,
        w: halfW,
      };
      state.dragSession = state.dragSession || {};
      state.dragSession[id] = true;
      card.style.width = halfW + "px";
      card.style.maxWidth = halfW + "px";
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
  if (!isAiInteractionEvent(e)) return "";
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
  const noiseProbe = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
  if (isNoiseText(noiseProbe)) return "";
  const title = e.title && e.title !== label ? esc(e.title) : "";
  const summary = esc(e.summary || "");
  // tool_result / result 默认不把超长 detail 塞进 CLI 主视图
  const detail =
    e.detail && e.kind !== "result" && e.kind !== "tool_result" ? esc(e.detail) : "";
  let body = "";
  if (title && summary) body = `<span style="opacity:.85">${title}</span>  ${summary}`;
  else body = summary || title || "…";
  // 黑区只留执行交互；result success/$cost 由窗外徽章表达
  if (e.kind === "result") return "";
  if (e.kind === "tool_result") {
    const short = (summary || title || "完成").slice(0, 280);
    return `<div class="tx-line role-result">
      <div class="tx-role">tool✓</div>
      <div class="tx-body">${short}</div>
    </div>`;
  }
  if (detail && e.kind === "tool_use" && detail.length > 160) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  if (detail && e.kind === "message" && detail.length > 220) {
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

/* ═══════════════════════════════════════════════
 * 全局事件委托：按钮失效的根治方案
 * - 不依赖 wire 时序
 * - 不依赖 Tauri 是否已就绪（先响应 UI）
 * - 动态生成的按钮也能点（按 id / data-action）
 * - 每次点击 try/catch，失败 toast，绝不静默
 * ═══════════════════════════════════════════════ */
const UI_ACTIONS = {
  "btn-add-plus": () => openModal(),
  "btn-welcome-add": () => openModal(),
  "btn-welcome-add2": () => openModal(),
  "btn-welcome-help": () => showPage("help"),
  "btn-refresh": async () => {
    if (state.page === "workspace" && state.selectedPath) {
      await loadProjects();
      await loadLive();
      await loadPlansForPicker().catch(() => {});
      const proj = state.projects.find((p) => p.path === state.selectedPath);
      const raw =
        state.live?.plan_path || proj?.default_plan || proj?.last_plan || state.selectedPlan;
      const cand = normalizePlanPath(raw) || raw;
      if (cand) await selectPlan(cand).catch(() => {});
      else updateTopPlanInfo();
    } else {
      await loadProjects();
    }
    toast("已刷新");
  },
  "modal-close": () => closeModal(),
  "modal-backdrop": () => closeModal(),
  "m-pick-folder": () => pickFolderToModal(),
  "m-confirm-project": () => addProjectFromModal(),
  "m-cancel-project": () => closeModal(),
  "btn-ws-stop-all": () => stopAll(),
  "btn-ws-resume": () => resumeRun(),
  "btn-remove-project": () => removeSelectedProject(),
  "btn-ws-dismiss-run": () => dismissRun(),
  "btn-task-dash-toggle": () => {
    state.taskDashCollapsed = !state.taskDashCollapsed;
    localStorage.setItem("cco.taskDashCollapsed", state.taskDashCollapsed ? "1" : "0");
    const tasks = state.live?.tasks || [];
    renderTaskStrip(state.live, tasks, {
      hasRun: !!state.live?.run_id,
      active: isLiveStatus(state.live?.run_status),
      finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
      runStatus: state.live?.run_status,
    });
  },
  "btn-chooser-assign": () => analyzePlanFromPicker(),
  "btn-stop-task": () => cancelTask(),
  "btn-pp-scan": async () => {
    await loadPlansForPicker();
    renderPlanChooser();
  },
  "btn-pp-pick": () => pickPlanFileForPicker(),
  "btn-pp-pick-empty": () => pickPlanFileForPicker(),
  "btn-chooser-scan": async () => {
    await loadPlansForPicker();
    renderPlanChooser();
  },
  "btn-chooser-pick": () => pickPlanFileForPicker(),
  "btn-chooser-close": () => openPlanChooser(false),
  "btn-plan-choose": async () => {
    // 先打开面板，再扫计划——避免 invoke 失败导致「按钮像死了」
    openPlanChooser(true);
    try {
      await loadPlansForPicker();
      renderPlanChooser();
      updateChooserAssignState();
    } catch (e) {
      toast(String(e));
      renderPlanChooser();
      updateChooserAssignState();
    }
  },
  "btn-pp-analyze": async () => {
    // 弹窗化：顶栏「分配计划」打开合并弹窗，底部确认才执行
    openPlanChooser(true);
    try {
      await loadPlansForPicker();
      renderPlanChooser();
      updateChooserAssignState();
    } catch (e) {
      toast(String(e));
      renderPlanChooser();
      updateChooserAssignState();
    }
  },
  "btn-pp-set-default": () => setDefaultPlan(),
  "btn-confirm-start": () => confirmAndStart(),
  "btn-replan": () => replanFromConfirm(),
  "btn-cancel-planning": () => cancelPlanning(),
  "btn-plan-expand": () => openPlanChooser(true),
  "btn-restore-panels": () => {
    state.closedPanels = {};
    renderCliBoard(state.live?.tasks || []);
  },
  "btn-doctor-dismiss": () => {
    const d = state.doctorCache;
    const fails = (d?.lines || []).filter((l) => !l.ok);
    state.doctorDismissedKey =
      fails.map((l) => l.name + ":" + l.detail).join("|") || "dismissed";
    renderDoctorWarn();
    toast("已暂时忽略环境提示");
  },
  "btn-advanced-toggle": () => {
    state.advancedOpen = !state.advancedOpen;
    localStorage.setItem(ADVANCED_KEY, state.advancedOpen ? "1" : "0");
    renderPlanPicker();
  },
  "btn-task-expand": () => {
    state.taskStripExpanded = !state.taskStripExpanded;
    localStorage.setItem("cco.taskStripExpanded", state.taskStripExpanded ? "1" : "0");
    const tasks = state.live?.tasks || [];
    renderTaskStrip(state.live, tasks, {
      hasRun: !!state.live?.run_id,
      active: isLiveStatus(state.live?.run_status),
      finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
      runStatus: state.live?.run_status,
    });
  },
  "btn-filter-failed": () => {
    state.filterFailedOnly = !state.filterFailedOnly;
    renderCliBoard(state.live?.tasks || []);
  },
  "btn-copy-log": async () => {
    const t =
      (state.live?.tasks || []).find((x) => x.task_id === state.selectedTaskId) ||
      (state.live?.tasks || [])[0];
    const text = aiLogPlainText(t);
    await navigator.clipboard.writeText(text || "");
    toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
  },
  "btn-rerun": () => {
    state.phase = "pick";
    state.planJobId = null;
    state.planJob = null;
    state.closedPanels = {};
    state.taskDashCollapsed = false;
    localStorage.setItem("cco.taskDashCollapsed", "0");
    renderPhasePanels();
    renderPlanPicker();
    if (state.selectedPlan) return analyzePlanFromPicker();
    openPlanChooser(true);
    toast("请先选择计划");
  },
  "btn-change-plan": () => {
    // 已移除「换计划」入口；保留 id 防旧调用
  },
  "btn-doctor-recheck": async () => {
    await ensureDoctor(true);
    toast(state.doctorCache?.ok ? "环境正常" : "仍有问题，请查看详情");
  },
  "btn-doctor-open": async () => {
    showPage("doctor");
    await loadDoctor();
  },
  "btn-open-doctor": async () => {
    showPage("doctor");
    await loadDoctor();
  },
  "btn-doctor": () => loadDoctor(),
  "btn-doctor-back": () => backFromSubpage(),
  "btn-open-settings": async () => {
    showPage("settings");
    await loadSettings();
  },
  "btn-settings-save": () => saveSettings(),
  "btn-settings-back": () => backFromSubpage(),
  "btn-open-help": () => showPage("help"),
  "btn-help-back": () => backFromSubpage(),
  "brand-home": () => goHome(),
};

function bindGlobalUI() {
  if (window.__ccoUiBound) return;
  window.__ccoUiBound = true;

  document.addEventListener(
    "click",
    (e) => {
      // plan chooser backdrop
      if (e.target?.id === "plan-chooser") {
        openPlanChooser(false);
        return;
      }

      // 动态列表：项目 / 计划 / 任务条
      const proj = e.target?.closest?.(".project-item[data-path]");
      if (proj) {
        e.preventDefault();
        Promise.resolve(selectProject(proj.dataset.path)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const planItem = e.target?.closest?.(".plan-item[data-plan]");
      if (planItem) {
        e.preventDefault();
        Promise.resolve(selectPlan(planItem.dataset.plan))
          .then(() => {
            if (state.planChooserOpen) {
              renderPlanChooser();
              updateChooserAssignState();
            }
          })
          .catch((err) => toast(String(err?.message || err)));
        return;
      }
      const rerunBtn = e.target?.closest?.("[data-rerun]");
      if (rerunBtn?.dataset?.rerun) {
        e.preventDefault();
        e.stopPropagation();
        state.selectedTaskId = rerunBtn.dataset.rerun;
        const fn = UI_ACTIONS["btn-rerun"];
        if (fn) Promise.resolve(fn()).catch((err) => toast(String(err?.message || err)));
        return;
      }
      const taskChip = e.target?.closest?.(".task-tile[data-task], .task-chip[data-task]");
      if (taskChip) {
        e.preventDefault();
        state.selectedTaskId = taskChip.dataset.task;
        if (state.closedPanels[taskChip.dataset.task]) {
          delete state.closedPanels[taskChip.dataset.task];
        }
        const tasks = state.live?.tasks || [];
        renderCliBoard(tasks);
        renderTaskStrip(state.live, tasks, {
          hasRun: !!state.live?.run_id,
          active: isLiveStatus(state.live?.run_status),
          finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
          runStatus: state.live?.run_status,
        });
        return;
      }
      // CLI 窗口内动态按钮
      const closeBtn = e.target?.closest?.("[data-close]");
      if (closeBtn?.dataset?.close) {
        e.preventDefault();
        e.stopPropagation();
        state.closedPanels[closeBtn.dataset.close] = true;
        renderCliBoard(state.live?.tasks || []);
        return;
      }
      const copyBtn = e.target?.closest?.("[data-copy]");
      if (copyBtn?.dataset?.copy) {
        e.preventDefault();
        e.stopPropagation();
        const t = (state.live?.tasks || []).find((x) => x.task_id === copyBtn.dataset.copy);
        {
          const text = aiLogPlainText(t);
          Promise.resolve(navigator.clipboard.writeText(text || ""))
            .then(() => toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制"))
            .catch(() => toast("复制失败"));
        }
        return;
      }
      const stopBtn = e.target?.closest?.("[data-stop]");
      if (stopBtn?.dataset?.stop) {
        e.preventDefault();
        e.stopPropagation();
        state.selectedTaskId = stopBtn.dataset.stop;
        Promise.resolve(cancelTask()).catch((err) => toast(String(err?.message || err)));
        return;
      }

      const el = e.target?.closest?.(
        "button[id], [id].linkish, [id].icon-btn, [id].filter-chip, #brand-home, [data-action]"
      );
      if (!el) return;

      // log mode / font size segments (no stable single action id on parent)
      if (el.closest?.("#log-view-mode") && el.dataset?.mode) {
        state.logViewMode = el.dataset.mode || "term";
        localStorage.setItem("cco.logViewMode", state.logViewMode);
        $$("#log-view-mode button").forEach((b) =>
          b.classList.toggle("active", b.dataset.mode === state.logViewMode)
        );
        const tasks = state.live?.tasks || [];
        if (tasks.length) renderCliBoard(tasks);
        return;
      }
      if (el.closest?.("#log-font-group") && el.dataset?.size) {
        applyLogFontSize(Number(el.dataset.size));
        return;
      }

      const action = el.dataset?.action || el.id;
      if (!action) return;
      const fn = UI_ACTIONS[action];
      if (!fn) return;

      // disabled / aria-disabled
      if (el.disabled || el.getAttribute("aria-disabled") === "true") return;

      e.preventDefault();
      Promise.resolve()
        .then(() => fn(e))
        .catch((err) => {
          console.error("UI action failed", action, err);
          toast(`${action}: ${err?.message || err}`);
        });
    },
    true // capture：不被子层 stopPropagation 吃掉
  );

  document.addEventListener("change", (e) => {
    const t = e.target;
    if (!t) return;
    if (t.id === "cli-height-select") {
      applyCliBodyHeight(t.value);
      const tasks = state.live?.tasks || [];
      if (tasks.length) renderCliBoard(tasks);
    }
    if (t.id === "pp-provider") {
      t.dataset.touched = "1";
    }
  });

  // 初始高度
  try {
    applyCliBodyHeight(state.cliBodyHeight || 300);
    const hSel = $("#cli-height-select");
    if (hSel) hSel.value = String(state.cliBodyHeight || 300);
  } catch (_) {}
}

/** 兼容旧名：wire 只做委托注册，永不抛致命错 */
function wire() {
  try {
    applyLogFontSize(state.logFontSize);
  } catch (_) {}
  bindGlobalUI();
}

async function boot() {
  bindGlobalUI();
  // 等 invoke 就绪（最多 ~5s），期间 UI 按钮已可点
  let ready = isTauriReady();
  for (let i = 0; !ready && i < 100; i++) {
    await new Promise((r) => setTimeout(r, 50));
    ready = isTauriReady();
  }
  if (!ready) {
    const cs = $("#conn-status");
    if (cs) cs.textContent = "需要通过 CCO.app 启动";
    // 仍不阻断本地 UI
    return;
  }
  try {
    const meta = await invoke("meta");
    const cs = $("#conn-status");
    if (cs) cs.textContent = `桌面应用 · v${meta.version}`;
    await loadProjects();
    const active = state.projects.find(
      (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
    );
    if (active) await selectProject(active.path);
    else if (state.projects.length === 1) await selectProject(state.projects[0].path);
    else if (state.projects.length > 0) goHome();
    else showPage("welcome");
    startPolling();
  } catch (e) {
    console.error(e);
    const cs = $("#conn-status");
    if (cs) cs.textContent = "后端连接异常";
    toast(String(e?.message || e));
  }
}

function waitTauri() {
  bindGlobalUI();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => boot().catch(console.error));
  } else {
    boot().catch(console.error);
  }
}

// 立即绑定（脚本在 body 末尾，DOM 已有按钮）
bindGlobalUI();
waitTauri();
