/**
 * [INPUT]: window.state · $ · statusUi helpers · classic stash/banner globals
 * [OUTPUT]: showPage · goHome · loadProjects · renderProjectList · run-lock · modal
 * note: 系统页(settings/doctor/help) 切页时 renderPlanPicker，清业务顶栏 CTA/阶段条
 * [POS]: D9 自 state.js 抽出；classic 经 installShellUi → window；features 仍读 window
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * note: 不写 Mode B / confirm / soft-fill / optional 策略；纯壳导航 + 列表渲染
 * note: 2026-08-15 P4-2 —— installSidebarChrome（rail 折叠 · 搜索 + #sidebar-count N/M · hover 路径复制卡）模块级 sidebarQuery 瞬态
 */

function g(name) {
  return typeof window !== "undefined" ? window[name] : undefined;
}

function call(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

function $el(s, el = document) {
  const $ = g("$");
  if (typeof $ === "function") return $(s, el);
  return (el || document).querySelector(s);
}

function $$el(s, el = document) {
  const $$ = g("$$");
  if (typeof $$ === "function") return $$(s, el);
  return [...(el || document).querySelectorAll(s)];
}

function st() {
  return g("state") || {};
}

/* P4-2：侧栏搜索词（模块级瞬态；renderProjectList 过滤用） */
let sidebarQuery = "";

/* ── Run-lock helpers (read state.live · display-only) ── */

/**
 * Live run was soft-ended via「结束计划」(SQLite last_dismissed on project row).
 * Double-check: project_live should already omit it; never treat as resumeable.
 */
function isDismissedCurrentLive(state) {
  const rid = state?.live?.run_id;
  if (!rid || !state?.selectedPath) return false;
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  if (!proj) return false;
  const last = proj.last_run_id || proj.lastRunId || null;
  const dismissed = !!(proj.last_dismissed || proj.lastDismissed);
  return dismissed && last && String(last) === String(rid);
}

/** True when the currently selected project has a live run (not paused). */
export function hasActiveRun() {
  const state = st();
  if (isDismissedCurrentLive(state)) return false;
  const isLive = g("isLiveStatus");
  if (typeof isLive !== "function") return !!(state.live?.run_id);
  return !!(state.live?.run_id && isLive(state.live?.run_status));
}

export function isRunPaused() {
  const state = st();
  // 已「结束计划」的 paused 不当作可续跑（stop_task 残留 desk）
  if (isDismissedCurrentLive(state)) return false;
  const isPaused = g("isPausedStatus");
  if (typeof isPaused !== "function") {
    return !!(
      state.live?.run_id &&
      String(state.live?.run_status || "").toLowerCase() === "paused"
    );
  }
  return !!(state.live?.run_id && isPaused(state.live?.run_status));
}

export function liveTaskById(taskId) {
  const state = st();
  if (!taskId) return null;
  return (
    (state.live?.tasks || []).find(
      (t) => t.task_id === taskId || t.id === taskId
    ) || null
  );
}

/**
 * Edit allowed when:
 * - confirming a split (desk open; historical project live must not lock), or
 * - run is paused, and the selected task has not started.
 */
export function canEditSelectedTask(taskId) {
  const state = st();
  const id = taskId != null ? taskId : state.confirmTaskId;
  if (!id) return false;
  // A1：拆分台可改任务图；勿「项目最近一次 completed live」当本轮已开跑。
  // confirm 分支必须优先于 hasActiveRun()，否则残留的 live 会把确认台一并锁死，
  // 高级折叠里的执行通道 / 角色 / 范围 全部不可点。
  if (state.phase === "confirm") {
    const job = state.planJob;
    const jrid = job?.run_id || job?.runId || null;
    // 本 job 尚未 spawn，或 live 不是本 job 的 run → 可编辑
    if (!jrid) return true;
    if (!state.live?.run_id || String(state.live.run_id) !== String(jrid)) {
      return true;
    }
    // 本 job 已确认且 live 就是该 run → 回退到运行中/暂停语义
    if (hasActiveRun()) return false;
    if (!isRunPaused()) return false;
    const t = liveTaskById(id);
    if (!t) return true;
    const v = String(t.status || "").toLowerCase();
    return !v || v === "pending" || v === "queued" || v === "waiting" || v === "ready";
  }
  // 无活跃运行时，历史拆分台可改 provider（用户重开前可调整通道）
  if (!hasActiveRun()) return true;
  if (!isRunPaused()) return false;
  const t = liveTaskById(id);
  if (!t) return true;
  const isPending = g("isTaskPendingStatus");
  if (typeof isPending === "function") return isPending(t.status);
  const v = String(t.status || "").toLowerCase();
  return !v || v === "pending" || v === "queued" || v === "waiting" || v === "ready";
}

export function toastRunLocked(action = "此操作") {
  call(
    "toast",
    `本轮还在执行，请先在运行页点「全部停止」后再${action}`
  );
}

/* ── Pages ── */

export function showPage(name) {
  const state = st();
  // 离开聊天前先缓存会话，避免切页把内存历史冲掉
  if (state.page === "chat" && name !== "chat") {
    try {
      call("stashChatSession", state.chatProjectPath || state.selectedPath);
    } catch (_) {}
  }
  state.page = name;
  try {
    call("updateTopPlanInfo");
  } catch (_) {}
  try {
    call("updateBgPlanBanner");
  } catch (_) {}
  // 切走工作区时先缓存当前规划，保证后台可续
  if (name !== "workspace" && state.selectedPath) {
    try {
      if (call("isPlanSessionActive")) {
        call("stashPlanSession", state.selectedPath);
      }
    } catch (_) {}
  }
  $$el(".page").forEach((p) =>
    p.classList.toggle("active", p.id === `page-${name}`)
  );
  const sub = $el("#page-sub");
  // F3：body 上标记主区角色，便于 CSS 互斥噪音
  try {
    document.body.dataset.ccoPage = name;
    document.body.dataset.ccoPhase = state.phase || "pick";
    document.body.classList.toggle("cco-run-active", hasActiveRun());
  } catch (_) {}
  try {
    call("refreshFlowStrips");
  } catch (_) {}
  if (name === "welcome") {
    const title = $el("#page-title");
    if (title) title.textContent = "欢迎";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "添加项目 → 写计划 → 拆成步骤 → 执行规划";
    }
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "workspace") {
    updateWorkspaceTitle();
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "chat") {
    const title = $el("#page-title");
    if (title) title.textContent = "写计划";
    if (sub) {
      sub.hidden = false;
      const proj = (state.projects || []).find(
        (p) => p.path === state.selectedPath
      );
      const label =
        proj?.name ||
        (state.selectedPath
          ? String(state.selectedPath).split(/[/\\]/).filter(Boolean).pop()
          : "");
      // 后台 Mode B 态只走顶栏监控 ghost / 可关 banner，副标题不再夹「待确认/返回确认」
      sub.textContent = label ? `和 AI 一起写计划 · ${label}` : "和 AI 一起写计划";
    }
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "plans") {
    const title = $el("#page-title");
    if (title) title.textContent = "计划管理";
    if (sub) {
      sub.hidden = false;
      const proj = (state.projects || []).find(
        (p) => p.path === state.selectedPath
      );
      const label =
        proj?.name ||
        (state.selectedPath
          ? String(state.selectedPath).split(/[/\\]/).filter(Boolean).pop()
          : "");
      sub.textContent = label
        ? `选中 · 预览 · 编辑 · 拆成步骤 · ${label}`
        : "选中计划后预览、编辑或拆成步骤";
    }
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "doctor") {
    const title = $el("#page-title");
    if (title) title.textContent = "环境检查";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "确认本机 CLI 与依赖就绪";
    }
    // 系统页：重算顶栏（隐藏业务 CTA / 阶段条）
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "help") {
    const title = $el("#page-title");
    if (title) title.textContent = "帮助";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "";
    }
    try {
      call("renderPlanPicker");
    } catch (_) {}
  } else if (name === "settings") {
    const title = $el("#page-title");
    if (title) title.textContent = "设置";
    if (sub) {
      sub.hidden = false;
      sub.textContent = "常用优先 · 高级默认折叠";
    }
    try {
      call("renderPlanPicker");
    } catch (_) {}
  }
}

export function updateWorkspaceTitle() {
  // 工作区标题只展示计划，交给 updateTopPlanInfo
  try {
    call("updateTopPlanInfo");
  } catch (_) {}
}

export function goHome() {
  const state = st();
  // 多项目可并行：离开工作区不停止运行；规划/确认先缓存，回项目可接上
  if (state.selectedPath) {
    state.lastWorkspacePath = state.selectedPath;
    try {
      if (call("isPlanSessionActive")) {
        call("stashPlanSession", state.selectedPath);
      }
    } catch (_) {}
  }
  state.selectedPath = null;
  state.live = null;
  state.selectedTaskId = null;
  // 不清 planJobId/phase：全局 poll 继续；悬浮条可点回
  renderProjectList();
  try {
    call("updateBgPlanBanner");
  } catch (_) {}
  if (!state.projects || state.projects.length === 0) {
    showPage("welcome");
  } else {
    // 有项目但未选中：欢迎页提示选项目
    showPage("welcome");
    const title = $el("#page-title");
    const sub = $el("#page-sub");
    if (title) title.textContent = "我的项目";
    if (sub) sub.textContent = "从左侧选择一个项目（可多项目同时运行）";
    const we = $el("#welcome-empty");
    if (!we) return;
    const tplHtml =
      typeof g("planTemplateWelcomeHtml") === "function"
        ? call("planTemplateWelcomeHtml")
        : "";
    we.innerHTML = `
      <p class="welcome-kicker muted">本机任务控制台</p>
      <h2>选择左侧项目，或添加新项目</h2>
      <p class="muted">进入项目后写计划、从模板开始或选已有 →「拆成步骤」→ 拆分台确认后开跑。多项目可并行。</p>
      <div class="welcome-actions">
        <button class="btn primary" id="btn-welcome-add2" type="button">添加项目文件夹</button>
      </div>
      ${tplHtml || ""}`;
    // 点击由全局委托处理 btn-welcome-add2 / data-plan-template
  }
}

/* ── Modal（仅添加项目） ── */

export function openModal() {
  const m = $el("#modal");
  if (m) m.hidden = false;
  const p = $el("#m-project-path");
  const n = $el("#m-project-name");
  if (p) p.value = "";
  if (n) n.value = "";
}

export function closeModal() {
  const m = $el("#modal");
  if (m) m.hidden = true;
  // C4: any close (add-project success or cancel) drops the stashed welcome
  // template so it can't surprisingly auto-apply on a later add.
  try {
    sessionStorage.removeItem("cco.pendingPlanTemplate");
  } catch (_) {}
}

/* ── Projects ── */

/**
 * Load project list. Prefer gateway (post-main); invoke bridge only pre-main.
 */
export async function loadProjects() {
  const state = st();
  const gw = typeof window !== "undefined" ? window.ccoGateway : null;
  if (gw?.getProjects) {
    state.projects = (await gw.getProjects()) || [];
  } else {
    // pre-main classic bridge only
    const inv = g("invoke");
    if (typeof inv !== "function") {
      throw new Error("请通过 CCO.app 启动（gateway 未就绪）");
    }
    state.projects = (await inv("get_projects")) || [];
  }
  renderProjectList();
  if (
    state.selectedPath &&
    !state.projects.some((p) => p.path === state.selectedPath)
  ) {
    state.selectedPath = null;
    state.live = null;
  }
}

export function renderProjectList() {
  const state = st();
  const el = $el("#project-list");
  if (!el) return;
  const countEl = $el("#sidebar-count");
  if (!state.projects || !state.projects.length) {
    if (countEl) countEl.textContent = "0";
    const plus =
      typeof window.ccoIcon === "function"
        ? window.ccoIcon("plus", { size: 14 })
        : "+";
    el.innerHTML = `<p class="muted empty-hint">尚未添加项目<br/>点侧栏 ${plus} 添加</p>`;
    return;
  }
  // 各项目状态独立展示；允许多项目并行运行，不因当前项目在跑而锁其它项
  // statusLabel / statusDot / esc / shortPath / isLiveStatus ← shared/statusUi (window)
  const statusLabel = g("statusLabel") || ((s) => s || "—");
  const statusDot = g("statusDot") || (() => "");
  const esc =
    g("esc") ||
    ((s) =>
      String(s ?? "")
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;"));
  const shortPath = g("shortPath") || ((p) => p || "—");
  const isLiveStatus =
    g("isLiveStatus") ||
    ((s) =>
      ["running", "starting", "queued", "validated", "init", "resuming"].includes(
        String(s || "").toLowerCase()
      ));
  const isPausedStatus =
    g("isPausedStatus") ||
    ((s) => String(s || "").toLowerCase() === "paused");
  const xIco =
    typeof window.ccoIcon === "function"
      ? window.ccoIcon("x", { size: 14, className: "ico-btn" })
      : "×";
  const copyIco =
    typeof window.ccoIcon === "function"
      ? window.ccoIcon("copy", { size: 13 })
      : "";

  // P4-2：侧栏搜索过滤（名称/路径包含 · 忽略大小写）
  const total = state.projects.length;
  const q = sidebarQuery.trim().toLowerCase();
  const projects = q
    ? state.projects.filter(
        (p) =>
          (p.name || "").toLowerCase().includes(q) ||
          (p.path || "").toLowerCase().includes(q)
      )
    : state.projects;
  if (countEl) countEl.textContent = q ? `${projects.length}/${total}` : String(total);

  if (!projects.length) {
    el.innerHTML = `<p class="muted empty-hint">没有匹配的项目</p>`;
    return;
  }

  el.innerHTML = projects
    .map((p) => {
      const stt = p.active_status || p.last_status || "";
      const live = p.running_tasks > 0 || isLiveStatus(p.active_status);
      // 已「结束计划」的 last run：不当作可续跑（SQLite last_dismissed）
      const dismissed = !!p.last_dismissed;
      const paused =
        !dismissed && isPausedStatus(p.last_status) && !live;
      const isCurrent = p.path === state.selectedPath;
      let meta;
      if (live) {
        meta = `${p.running_tasks || 0}/${p.total_tasks || "?"} 任务 · 运行中`;
      } else if (paused) {
        meta = "已暂停 · 可续跑";
      } else if (dismissed && p.last_status) {
        meta = `最近: ${statusLabel(p.last_status)} · 已结束本轮`;
      } else if (p.last_status) {
        meta = `最近: ${statusLabel(p.last_status)}`;
      } else if (p.exists) {
        meta = "无活动运行";
      } else {
        meta = "路径不存在";
      }
      // shell-chrome B1：悬停 × 移除（不删磁盘）；运行中仍显示但 handler 会 toast 锁
      // P4-2：name-text 供 rail 隐藏文案 · hover 复制卡（路径一键复制）
      return `<div class="project-item-row ${
        isCurrent ? "active" : ""
      }" data-path="${esc(p.path)}">
        <button type="button" class="project-item ${
          isCurrent ? "active" : ""
        }" data-path="${esc(p.path)}" title="${esc(p.name)}">
          <div class="name"><span class="dot ${statusDot(stt) || (live ? "live" : "")}"></span><span class="name-text">${esc(
            p.name
          )}</span></div>
          <div class="path" title="${esc(p.path)}">${esc(shortPath(p.path))}</div>
          <div class="meta">${esc(meta)}</div>
        </button>
        <button type="button" class="icon-btn sm project-item-remove" data-remove-path="${esc(
          p.path
        )}" title="从列表移除（不删文件夹）" aria-label="从列表移除 ${esc(
        p.name
      )}"${live ? ' data-run-locked="1"' : ""}>${xIco}</button>
        <div class="project-hover-card">
          <div class="project-hover-label muted">项目路径</div>
          <div class="project-hover-path" title="${esc(p.path)}">${esc(
        p.path
      )}</div>
          <button type="button" class="btn ghost sm project-copy-path" data-copy-path="${esc(
        p.path
      )}">${copyIco}复制路径</button>
        </div>
      </div>`;
    })
    .join("");
  $$el(".project-item", el).forEach((b) => {
    b.onclick = () => {
      const fn = g("selectProject");
      if (typeof fn === "function") fn(b.dataset.path);
    };
  });
  $$el(".project-item-remove", el).forEach((b) => {
    b.onclick = (ev) => {
      try {
        ev.preventDefault();
        ev.stopPropagation();
      } catch (_) {}
      const path = b.getAttribute("data-remove-path");
      if (!path) return;
      const fn = g("removeSelectedProject");
      if (typeof fn === "function") {
        fn(path);
      } else {
        call("toast", "移除不可用");
      }
    };
  });
}

/* ── P4-2 侧栏 chrome：折叠 rail · 搜索 · hover 复制（几何瞬态，不入 localStorage）── */

function installSidebarChrome() {
  if (typeof document === "undefined") return;
  const collapse = document.getElementById("btn-sidebar-collapse");
  if (collapse && !collapse.dataset.ccoA2Wired) {
    collapse.dataset.ccoA2Wired = "1";
    collapse.addEventListener("click", () => {
      const collapsed = document.body.classList.toggle("cco-sidebar-collapsed");
      const label = collapsed ? "展开侧栏" : "收起侧栏";
      collapse.title = label;
      collapse.setAttribute("aria-label", label);
    });
  }

  const searchWrap = document.getElementById("sidebar-search");
  const searchToggle = document.getElementById("btn-sidebar-search-toggle");
  const searchInput = document.getElementById("sidebar-search-input");
  const searchClear = document.getElementById("sidebar-search-clear");

  if (searchToggle && searchWrap && !searchToggle.dataset.ccoA2Wired) {
    searchToggle.dataset.ccoA2Wired = "1";
    searchToggle.addEventListener("click", () => {
      // rail 模式点搜索：先展开侧栏再聚焦输入
      if (document.body.classList.contains("cco-sidebar-collapsed")) {
        document.body.classList.remove("cco-sidebar-collapsed");
        collapse.title = "收起侧栏";
        collapse.setAttribute("aria-label", "收起侧栏");
      }
      const open = searchWrap.hidden;
      searchWrap.hidden = !open;
      searchToggle.setAttribute("aria-expanded", String(!open));
      if (open) setTimeout(() => searchInput?.focus(), 0);
    });
  }
  if (searchInput && !searchInput.dataset.ccoA2Wired) {
    searchInput.dataset.ccoA2Wired = "1";
    let timer = null;
    searchInput.addEventListener("input", () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        sidebarQuery = searchInput.value || "";
        renderProjectList();
      }, 250);
    });
    searchInput.addEventListener("keydown", (ev) => {
      if (ev.key !== "Escape") return;
      ev.preventDefault();
      sidebarQuery = "";
      searchInput.value = "";
      renderProjectList();
      if (searchWrap) searchWrap.hidden = true;
      searchToggle?.setAttribute("aria-expanded", "false");
    });
  }
  if (searchClear && !searchClear.dataset.ccoA2Wired) {
    searchClear.dataset.ccoA2Wired = "1";
    searchClear.addEventListener("click", () => {
      sidebarQuery = "";
      if (searchInput) {
        searchInput.value = "";
        searchInput.focus();
      }
      renderProjectList();
    });
  }

  const list = document.getElementById("project-list");
  if (list && !list.dataset.ccoCopyWired) {
    list.dataset.ccoCopyWired = "1";
    list.addEventListener("click", (ev) => {
      const copyBtn = ev.target?.closest?.("[data-copy-path]");
      if (!copyBtn) return;
      ev.preventDefault();
      ev.stopPropagation();
      const path = copyBtn.getAttribute("data-copy-path");
      if (!path) return;
      const done = () => call("toast", "项目路径已复制");
      const fallback = () => {
        try {
          const ta = document.createElement("textarea");
          ta.value = path;
          document.body.appendChild(ta);
          ta.select();
          document.execCommand("copy");
          document.body.removeChild(ta);
          done();
        } catch (_) {
          call("toast", "复制失败");
        }
      };
      const clip = typeof navigator !== "undefined" ? navigator.clipboard : null;
      if (clip?.writeText) {
        clip.writeText(path).then(done).catch(fallback);
      } else {
        fallback();
      }
    });
  }
}

/**
 * Bridge shell helpers onto window for classic scripts + legacy.js hosts.
 * @param {typeof globalThis} [target]
 */
export function installShellUi(
  target = typeof window !== "undefined" ? window : globalThis
) {
  installSidebarChrome();
  if (!target) return;
  Object.assign(target, {
    hasActiveRun,
    isRunPaused,
    liveTaskById,
    canEditSelectedTask,
    toastRunLocked,
    showPage,
    updateWorkspaceTitle,
    goHome,
    openModal,
    closeModal,
    loadProjects,
    renderProjectList,
  });
}

const shellUi = {
  hasActiveRun,
  isRunPaused,
  liveTaskById,
  canEditSelectedTask,
  toastRunLocked,
  showPage,
  updateWorkspaceTitle,
  goHome,
  openModal,
  closeModal,
  loadProjects,
  renderProjectList,
  installShellUi,
};

export default shellUi;
