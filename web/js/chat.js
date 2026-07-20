/**
 * [INPUT]: state · invoke · selectPlan / openPlanChooser / showPage / toast
 * [OUTPUT]: 聊天建计划 UI · chat_send/save · 多会话切换（C3）· 计划 diff · 流式 partial · 右轨 plan-rail · 全文 modal · 分配跳转（方案 A）
 * [POS]: web/js 聊天页；不 spawn worker，分配同源 analyzePlanFromPicker
 * note: chatBusy 显示「思考中」气泡；发送禁用防双发；后端 spawn_blocking 不堵 UI
 * note: chatSessions 缓存键 = project::session_id；切页/切会话不丢
 * note: C3 chat_list/new/delete_session + #chat-session-select
 * note: H1 plan-rail 点开全文用 read_plan_md（不默认 open_path）；未执行可 App 内改
 * note: H2 右轨与 chooser 共用 planExecBadgeInfo + showExecutedPlans 过滤
 * note: G0 标题截断/line-clamp · 右栏默认隐藏 icon 展开；G1 计划管理=独立 page-plans
 * note: E0–E2 计划管理只进管理页；执行走 startExecuteFromSelection
 * note: 保存/执行 CTA 只在聊天答复计划卡底部；#chat-ready-bar 常隐（不再贴输入框上方）
 * note: chatFormatBody 对 ```plan 嵌套 fence 做行首 depth 计数（与 services/chat extract_plan_fence 对齐）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — chat plan builder */

function chatProjectName() {
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  if (proj?.name) return proj.name;
  if (!state.selectedPath) return "";
  const parts = String(state.selectedPath).split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || state.selectedPath;
}

function ensureChatState() {
  if (!state.chatSession) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null };
  }
  if (state.chatBusy == null) state.chatBusy = false;
  if (state.chatWaitStartedAt == null) state.chatWaitStartedAt = 0;
  if (state.chatDraftPlan === undefined) state.chatDraftPlan = null;
  if (state.chatFake == null) state.chatFake = false;
  if (state.chatEnvNote === undefined) state.chatEnvNote = null;
  if (!state.chatSessions) state.chatSessions = {};
  if (state.chatProjectPath === undefined) state.chatProjectPath = null;
  if (state._chatLoadSeq == null) state._chatLoadSeq = 0;
  // C3 multi-session list (ChatSessionSummary[])
  if (!Array.isArray(state.chatSessionList)) state.chatSessionList = [];
  if (state.chatSessionListLoading == null) state.chatSessionListLoading = false;
  // H1 plan-rail + full-view modal
  if (!Array.isArray(state.planRailItems)) state.planRailItems = [];
  if (state.planRailLoading == null) state.planRailLoading = false;
  if (!Array.isArray(state.planMetaItems)) state.planMetaItems = [];
  if (!state.planMetaByPath) state.planMetaByPath = {};
  // 聊天右栏：仅 icon 展开，默认关（per-project）
  if (state.planRailOpen == null) {
    const key = state.selectedPath
      ? `cco.planRailOpen:${state.selectedPath}`
      : "cco.planRailOpen";
    state.planRailOpen = localStorage.getItem(key) === "1";
  }
  if (state.planRailSelected == null) state.planRailSelected = null;
  // G1: default plans dir (project-relative), persisted per project
  if (state.plansDir == null) {
    const k = state.selectedPath
      ? `cco.plansDir:${state.selectedPath}`
      : "cco.plansDir";
    state.plansDir = localStorage.getItem(k) || "plans";
  }
  // G4: pending image files before send [{name,mime,dataUrl,size}]
  if (!Array.isArray(state.chatPendingAttachments)) state.chatPendingAttachments = [];
  if (state.showExecutedPlans == null) {
    state.showExecutedPlans = localStorage.getItem("cco.showExecutedPlans") === "1";
  }
  if (state.planFull == null) {
    state.planFull = {
      open: false,
      path: null,
      title: null,
      markdown: "",
      original: "",
      editing: false,
      dirty: false,
      everCompleted: false,
      lastRunStatus: null,
      saving: false,
      // C3/P2-9: disk (left) vs current draft (right)
      diffing: false,
      diffLeft: "",
      diffRight: "",
    };
  }
  // C3 streaming partial text while chat_send runs (poll only; falls back to wait label)
  if (state.chatStreamText == null) state.chatStreamText = "";
}

/** G0: short list title from markdown H1 (cut at ## / max 80 chars). */
function sanitizePlanTitle(raw) {
  if (!raw) return "";
  let s = String(raw).trim();
  const hashIdx = s.indexOf("##");
  if (hashIdx >= 0) s = s.slice(0, hashIdx).trimEnd();
  const nlHash = s.indexOf("\n# ");
  if (nlHash >= 0) s = s.slice(0, nlHash).trimEnd();
  s = s.trim();
  if (!s) return "";
  const chars = Array.from(s);
  if (chars.length <= 80) return s;
  return chars.slice(0, 80).join("") + "…";
}

/** G1: show/hide plan-rail; persist per project. */
function setPlanRailOpen(open, { persist = true } = {}) {
  ensureChatState();
  state.planRailOpen = !!open;
  if (persist && state.selectedPath) {
    localStorage.setItem(
      `cco.planRailOpen:${state.selectedPath}`,
      state.planRailOpen ? "1" : "0"
    );
  }
  applyPlanRailVisibility();
}

function applyPlanRailVisibility() {
  ensureChatState();
  const rail = $("#plan-rail");
  const layout = document.querySelector("#page-chat .chat-layout");
  const toggle = $("#btn-chat-rail-toggle");
  const open = !!state.planRailOpen;
  if (rail) {
    if (open) rail.removeAttribute("hidden");
    else rail.setAttribute("hidden", "");
  }
  if (layout) layout.classList.toggle("plan-rail-open", open);
  if (toggle) {
    toggle.setAttribute("aria-pressed", open ? "true" : "false");
    toggle.setAttribute("aria-label", open ? "收起右侧计划列表" : "展开右侧计划列表");
    toggle.title = open ? "收起右侧计划列表" : "展开右侧计划列表";
    toggle.classList.toggle("is-on", open);
    toggle.textContent = open ? "◀" : "☰";
  }
}

/** 聊天页右侧列表：仅 icon 切换（≠ 计划管理页） */
function toggleChatPlanRail() {
  ensureChatState();
  setPlanRailOpen(!state.planRailOpen);
  if (state.planRailOpen) {
    Promise.resolve(loadPlanRail()).catch(() => {});
  }
  renderPlanRail();
}

function syncPlansDirLabels() {
  const d = getPlansDir();
  const text = d.endsWith("/") ? d : `${d}/`;
  for (const id of ["plan-rail-dir-label", "plans-mgmt-dir-label"]) {
    const el = $(id);
    if (el) el.textContent = text;
  }
}

/** G1: project-relative plans directory (default plans). */
function getPlansDir() {
  ensureChatState();
  if (state.selectedPath) {
    const k = `cco.plansDir:${state.selectedPath}`;
    const v = localStorage.getItem(k);
    if (v) state.plansDir = v;
  }
  return (state.plansDir || "plans").replace(/^\/+|\/+$/g, "") || "plans";
}

function setPlansDir(dir) {
  ensureChatState();
  let d = String(dir || "plans").trim().replace(/\\/g, "/");
  d = d.replace(/^\/+|\/+$/g, "");
  if (!d || d.includes("..") || d.startsWith("/")) {
    toast("计划目录必须是项目内相对路径，例如 plans 或 docs/plans");
    return false;
  }
  state.plansDir = d;
  if (state.selectedPath) {
    localStorage.setItem(`cco.plansDir:${state.selectedPath}`, d);
  }
  syncPlansDirLabels();
  toast(`新计划将保存到 ${d}/ · 列表已按此夹刷新`);
  // E4：换夹后立刻重扫管理页 / 右栏
  Promise.resolve(loadPlanRail())
    .then(() => {
      if (state.page === "plans") {
        try {
          renderPlansMgmtPage();
        } catch (_) {}
      }
    })
    .catch(() => {});
  return true;
}

/**
 * 换夹：优先系统选目录（项目内），回退 prompt。
 * 选完立刻刷新列表。
 */
async function promptPlansDir() {
  const cur = getPlansDir();
  const root = state.selectedPath;
  if (!root) {
    toast("请先选择项目");
    return;
  }
  const rootNorm = String(root).replace(/[/\\]+$/, "");
  // 1) 系统文件夹选择
  try {
    if (typeof openNativeDialog === "function") {
      const selected = await openNativeDialog({
        directory: true,
        multiple: false,
        defaultPath: rootNorm,
        title: "选择计划文件夹（须在当前项目内）",
      });
      if (selected) {
        const abs = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
        if (abs) {
          let rel =
            typeof normalizePlanPath === "function"
              ? normalizePlanPath(abs, rootNorm) || abs
              : abs;
          rel = String(rel || "").replace(/\\/g, "/").replace(/^\.\//, "");
          // 若用户选的是项目根，提示用子目录
          if (!rel || rel === rootNorm || rel === ".") {
            toast("请选择项目内的子文件夹，例如 plans 或 docs");
            return;
          }
          // strip absolute if still abs
          if (rel.startsWith(rootNorm + "/") || rel.startsWith(rootNorm + "\\")) {
            rel = rel.slice(rootNorm.length + 1);
          }
          if (rel.includes("..") || rel.startsWith("/")) {
            toast("计划目录必须在项目内");
            return;
          }
          setPlansDir(rel);
          return;
        }
      }
      // user cancelled folder dialog — fall through only if they want typed path
      // 取消不算失败；再给 prompt 一次
    }
  } catch (e) {
    console.warn("promptPlansDir dialog", e);
  }
  // 2) 文本回退
  const next = window.prompt(
    "默认计划文件夹（相对项目根，例如 plans 或 docs）",
    cur
  );
  if (next == null) return;
  setPlansDir(next);
}

/** 在访达中打开当前 plans_dir（或项目根） */
async function openPlansDirInFinder() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const root = String(state.selectedPath).replace(/[/\\]+$/, "");
  const dir = getPlansDir();
  // 尾斜杠提示 open_path 按目录创建
  const abs = `${root}/${dir}/`.replace(/\/+/g, "/");
  try {
    await invoke("open_path", { path: abs });
    toast(`已打开 ${dir}/`);
  } catch (e1) {
    try {
      await invoke("open_path", { path: root });
      toast("已打开项目根（计划夹创建失败）");
    } catch (e) {
      toast(String(e?.message || e || e1 || "无法打开文件夹"));
    }
  }
}

/** 空态一键：勾选「显示其它位置」并刷新 */
function showOtherPlansLocations() {
  const cb = $("#plans-mgmt-show-other");
  if (cb) {
    cb.checked = true;
  }
  try {
    renderPlansMgmtPage();
  } catch (_) {}
  toast("已显示其它位置的计划");
}

/** 管理页：选一个计划文件并选中（可跨 plans_dir） */
async function pickPlanFileForMgmt() {
  try {
    if (typeof pickPlanFileForPicker === "function") {
      await pickPlanFileForPicker();
      // 手动选中后并入列表扫描；必要时打开「其它位置」
      try {
        await loadPlanRail();
      } catch (_) {}
      const path = state.selectedPlan;
      if (path) {
        if (typeof selectPlanRailItem === "function") selectPlanRailItem(path);
        const root = state.selectedPath;
        if (
          typeof isPathInPlansDir === "function" &&
          !isPathInPlansDir(path, getPlansDir(), root)
        ) {
          const cb = $("#plans-mgmt-show-other");
          if (cb) cb.checked = true;
        }
        if (state.page === "plans") renderPlansMgmtPage();
        toast("已选中计划");
      }
      return;
    }
    toast("选择文件不可用");
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/**
 * E4：路径是否在当前 plans_dir 下（相对项目路径）。
 * pinPaths 始终保留（选中/草稿/手动挑的文件）。
 */
function isPathInPlansDir(path, plansDir, root) {
  if (!path) return false;
  const dir = String(plansDir || "plans")
    .replace(/\\/g, "/")
    .replace(/^\/+|\/+$/g, "") || "plans";
  let rel =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(path, root) || path
      : path;
  rel = String(rel || "").replace(/\\/g, "/").replace(/^\.\//, "");
  // 绝对路径：尽量 strip project root
  if (root && (rel.startsWith("/") || /^[A-Za-z]:\//.test(rel))) {
    const r = String(root).replace(/\\/g, "/").replace(/\/+$/, "");
    const full = rel.replace(/\\/g, "/");
    if (full.startsWith(r + "/")) rel = full.slice(r.length + 1);
  }
  rel = rel.replace(/^\/+/, "");
  const prefix = dir + "/";
  return rel === dir || rel.startsWith(prefix);
}

/** 过滤到本夹；pin 始终保留。返回 { primary, other } */
function partitionByPlansDir(items, { plansDir, root, pinPaths = [], showOther = false } = {}) {
  const pins = new Set(
    (pinPaths || [])
      .filter(Boolean)
      .map((p) =>
        typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p
      )
  );
  const primary = [];
  const other = [];
  for (const it of items || []) {
    const path = it.path || it;
    const norm =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(path, root) || path
        : path;
    const pinned = pins.has(norm) || pins.has(path);
    if (pinned || isPathInPlansDir(path, plansDir, root)) {
      primary.push(it);
    } else {
      other.push(it);
    }
  }
  if (showOther) {
    return { primary: primary.concat(other), other: [], otherCount: other.length };
  }
  return { primary, other, otherCount: other.length };
}

/**
 * 计划管理页入口（独立 page=plans）。
 * E0：只进管理页（列表/预览/编辑）；**禁止**自动弹执行选项。
 * 有选中 → 高亮并拉详情；执行由用户点「执行此计划」。
 */
async function openPlanManagement() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.page === "chat" && state.chatProjectPath) {
    try {
      stashChatSession(state.chatProjectPath || state.selectedPath);
    } catch (_) {}
  }
  // First open: confirm default plans dir once per project
  const flagKey = `cco.plansDirPrompted:${state.selectedPath}`;
  if (localStorage.getItem(flagKey) !== "1") {
    localStorage.setItem(flagKey, "1");
    const ok = window.confirm(
      `新计划默认保存到项目下的「${getPlansDir()}/」。\n\n确定使用此目录？\n（可在计划管理页点「换夹」修改）`
    );
    if (!ok) {
      promptPlansDir();
    }
  }
  const selected =
    state.planRailSelected ||
    state.selectedPlan ||
    state.chatDraftPlan ||
    null;
  // 管理 ≠ 执行：关掉可能残留的选项层
  try {
    if (typeof openPlanChooser === "function") openPlanChooser(false);
  } catch (_) {}
  showPage("plans");
  try {
    if (typeof renderPlanPicker === "function") renderPlanPicker();
  } catch (_) {}
  await loadPlanRail();
  if (selected) {
    selectPlanRailItem(selected);
    try {
      if (typeof selectPlan === "function") selectPlan(selected);
      state.chatDraftPlan = selected;
    } catch (_) {}
  }
  renderPlansMgmtPage();
  if (!selected) {
    toast("从左侧选一份计划，再点「执行此计划」");
  }
}

function renderPlansMgmtPage() {
  ensureChatState();
  syncPlansDirLabels();
  if (typeof syncShowExecutedToggles === "function") syncShowExecutedToggles();
  const list = $("#plans-mgmt-list");
  const empty = $("#plans-mgmt-empty");
  if (!list) return;

  if (state.planRailLoading) {
    if (empty) empty.hidden = true;
    list.innerHTML =
      '<div class="plan-rail-loading"><span class="spinner sm" aria-hidden="true"></span>扫描计划…</div>';
    renderPlansMgmtDetail(null);
    return;
  }

  const root = state.selectedPath;
  const selectedPath =
    state.planRailSelected ||
    state.selectedPlan ||
    state.chatDraftPlan ||
    null;
  const activePath =
    typeof normalizePlanPath === "function" && selectedPath
      ? normalizePlanPath(selectedPath, root) || selectedPath
      : selectedPath;
  const pinPaths = [state.chatDraftPlan, state.selectedPlan, state.planRailSelected, activePath]
    .filter(Boolean)
    .map((p) =>
      typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p
    );

  // E4：默认只显示 plans_dir；勾选「显示其它位置」展开全量
  const showOther = !!$("#plans-mgmt-show-other")?.checked;
  const dirParts = partitionByPlansDir(state.planRailItems || [], {
    plansDir: getPlansDir(),
    root,
    pinPaths,
    showOther,
  });
  const dirItems = dirParts.primary;
  const otherCount = dirParts.otherCount || 0;

  // 空态：可点按钮（显示其它 / 选文件 / 打开夹 / 聊天）
  const emptyActionsHtml = (opts = {}) => {
    const { otherN = 0, dirLabel = "plans/" } = opts;
    const otherBtn =
      otherN > 0
        ? `<button type="button" class="btn primary sm" id="btn-plans-empty-show-other">显示其它位置（${otherN}）</button>`
        : "";
    return (
      `<div class="plans-mgmt-empty-card">` +
      `<p class="plans-mgmt-empty-msg">本夹「${chatEsc(dirLabel)}」暂无计划` +
      (otherN > 0
        ? ` · 另有 <strong>${otherN}</strong> 份在其它位置`
        : " · 可新建或从磁盘选择") +
      `</p>` +
      `<div class="plans-mgmt-empty-actions">` +
      otherBtn +
      `<button type="button" class="btn ghost sm" id="btn-plans-empty-pick">选择计划文件…</button>` +
      `<button type="button" class="btn ghost sm" id="btn-plans-empty-open-dir">打开此夹</button>` +
      `<button type="button" class="btn ghost sm" id="btn-plans-empty-to-chat">用聊天写计划</button>` +
      `</div></div>`
    );
  };

  if (!(state.planRailItems || []).length) {
    list.innerHTML = emptyActionsHtml({
      otherN: 0,
      dirLabel: getPlansDir() + "/",
    });
    if (empty) empty.hidden = true;
    renderPlansMgmtDetail(null);
    return;
  }
  if (!dirItems.length) {
    list.innerHTML = emptyActionsHtml({
      otherN: otherCount,
      dirLabel: getPlansDir() + "/",
    });
    if (empty) empty.hidden = true;
    renderPlansMgmtDetail(null);
    return;
  }
  if (empty) empty.hidden = true;

  const parts =
    typeof partitionPlanItems === "function"
      ? partitionPlanItems(dirItems, {
          showExecuted: !!state.showExecutedPlans,
          pinPaths,
        })
      : { visible: dirItems, historyHidden: false, historyCount: 0 };

  const latestPath = pickLatestPlanPath(parts.visible);
  const latestNorm =
    latestPath && typeof normalizePlanPath === "function"
      ? normalizePlanPath(latestPath, root) || latestPath
      : latestPath;

  const rows = parts.visible.map((it) => {
    const path = it.path || "";
    const rawTitle = it.title || planRailTitleFromPath(path);
    const title = sanitizePlanTitle(rawTitle) || planRailTitleFromPath(path);
    const badge = planRailBadgeInfo(it);
    const norm =
      typeof normalizePlanPath === "function" ? normalizePlanPath(path, root) || path : path;
    const selected =
      activePath && (norm === activePath || path === activePath) ? " is-selected" : "";
    const isLatest =
      latestNorm && (norm === latestNorm || path === latestPath) ? " is-latest" : "";
    const latestMark = isLatest
      ? `<span class="plan-latest-tag">最新</span>`
      : "";
    return (
      `<button type="button" class="plans-mgmt-item${selected}${isLatest}" data-plans-mgmt="${chatEsc(path)}" title="${chatEsc(path)}">` +
      `<div class="plans-mgmt-item-title">${chatEsc(title)}${latestMark}</div>` +
      `<div class="plans-mgmt-item-path">${chatEsc(path)}</div>` +
      `<div class="plans-mgmt-item-meta"><span class="plan-rail-badge ${badge.cls}">${chatEsc(badge.label)}</span></div>` +
      `</button>`
    );
  });
  if (parts.historyHidden) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">已隐藏 ${parts.historyCount} 份已执行 · 勾选「显示已执行」</div>`
    );
  }
  if (!showOther && otherCount > 0) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">` +
        `另有 ${otherCount} 份在其它位置 · ` +
        `<button type="button" class="linkish" id="btn-plans-hint-show-other">点此显示</button>` +
        `</div>`
    );
  }
  list.innerHTML = rows.join("");

  const pool = dirItems.length ? dirItems : state.planRailItems || [];
  const selItem =
    pool.find((it) => {
      const p = it.path || "";
      const n =
        typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p;
      return n === activePath || p === activePath;
    }) || null;
  renderPlansMgmtDetail(selItem || (activePath ? { path: activePath } : null));
}

async function renderPlansMgmtDetail(item) {
  const empty = $("#plans-mgmt-detail-empty");
  const detail = $("#plans-mgmt-detail");
  if (!detail) return;
  if (!item?.path || !state.selectedPath) {
    if (empty) empty.hidden = false;
    detail.hidden = true;
    return;
  }
  if (empty) empty.hidden = true;
  detail.hidden = false;

  const root = state.selectedPath;
  const path =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(item.path, root) || item.path
      : item.path;
  const titleEl = $("#plans-mgmt-detail-title");
  const pathEl = $("#plans-mgmt-detail-path");
  const badgeEl = $("#plans-mgmt-detail-badge");
  const bodyEl = $("#plans-mgmt-detail-body");
  const btnAssign = $("#btn-plans-assign");

  let markdown = "";
  try {
    markdown = await invoke("read_plan_md_cmd", { project: root, plan: path });
  } catch (e) {
    markdown = `（无法读取：${e?.message || e}）`;
  }
  const title =
    sanitizePlanTitle(item.title) ||
    planTitleFromMarkdown(markdown) ||
    planRailTitleFromPath(path);
  const badge = planRailBadgeInfo(item);

  if (titleEl) titleEl.textContent = title || "—";
  if (pathEl) pathEl.textContent = path || "—";
  if (badgeEl) {
    badgeEl.textContent = badge.label;
    badgeEl.className = `plan-rail-badge ${badge.cls}`;
  }
  if (bodyEl) bodyEl.textContent = String(markdown || "").slice(0, 12000);
  if (btnAssign) {
    btnAssign.disabled = !path;
    btnAssign.dataset.plan = path;
  }
  const btnPreview = $("#btn-plans-preview");
  if (btnPreview) btnPreview.dataset.plan = path;
}

/** 计划管理页：单击选中并刷新详情 */
function selectPlansMgmtItem(planPath) {
  selectPlanRailItem(planPath);
  renderPlansMgmtPage();
}

/** 计划管理页：双击 / 全文编辑 */
async function openPlansMgmtItem(planPath) {
  selectPlanRailItem(planPath);
  await openPlanFullView(planPath);
  if (state.page === "plans") renderPlansMgmtPage();
}

/** 计划管理页主 CTA → 统一执行入口（E1） */
async function assignFromPlansMgmt() {
  const path =
    $("#btn-plans-assign")?.dataset?.plan ||
    state.planRailSelected ||
    state.selectedPlan;
  if (!path) {
    toast("请先选中一份计划");
    return;
  }
  selectPlanRailItem(path);
  state.chatDraftPlan = path;
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(path, { source: "plans" });
    return;
  }
  // fallback
  try {
    if (typeof selectPlan === "function") await selectPlan(path);
    showPage("workspace");
    if (typeof openPlanChooser === "function") openPlanChooser(true);
    toast("已选中计划 · 确认选项后点「开始拆分」");
  } catch (e) {
    toast(String(e?.message || e || "无法打开执行选项"));
  }
}

/* ── G4 attachments ── */
const CHAT_ATT_MAX_BYTES = 5 * 1024 * 1024;
const CHAT_ATT_MAX_COUNT = 4;
const CHAT_ATT_MIME = new Set(["image/png", "image/jpeg", "image/jpg", "image/webp", "image/gif"]);

function fileToDataUrl(file) {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result);
    r.onerror = () => reject(new Error("read file failed"));
    r.readAsDataURL(file);
  });
}

function dataUrlToBase64(dataUrl) {
  const s = String(dataUrl || "");
  const i = s.indexOf(",");
  return i >= 0 ? s.slice(i + 1) : s;
}

async function addChatAttachments(fileList) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const files = Array.from(fileList || []);
  for (const f of files) {
    if (state.chatPendingAttachments.length >= CHAT_ATT_MAX_COUNT) {
      toast(`每条消息最多 ${CHAT_ATT_MAX_COUNT} 张图`);
      break;
    }
    const mime = (f.type || "").toLowerCase();
    if (!CHAT_ATT_MIME.has(mime)) {
      toast(`不支持的类型：${f.name || mime}`);
      continue;
    }
    if (f.size > CHAT_ATT_MAX_BYTES) {
      toast(`${f.name || "图片"} 超过 5MB`);
      continue;
    }
    try {
      const dataUrl = await fileToDataUrl(f);
      state.chatPendingAttachments.push({
        name: f.name || "image.png",
        mime: mime === "image/jpg" ? "image/jpeg" : mime,
        dataUrl,
        size: f.size,
      });
    } catch (e) {
      toast(String(e?.message || e));
    }
  }
  renderChatAttachPreview();
}

function removeChatAttachment(idx) {
  ensureChatState();
  if (idx < 0 || idx >= state.chatPendingAttachments.length) return;
  state.chatPendingAttachments.splice(idx, 1);
  renderChatAttachPreview();
}

function clearChatAttachments() {
  ensureChatState();
  state.chatPendingAttachments = [];
  renderChatAttachPreview();
}

function renderChatAttachPreview() {
  ensureChatState();
  const box = $("#chat-attach-preview");
  if (!box) return;
  const items = state.chatPendingAttachments || [];
  if (!items.length) {
    box.hidden = true;
    box.innerHTML = "";
    return;
  }
  box.hidden = false;
  box.innerHTML = items
    .map(
      (a, i) =>
        `<div class="chat-attach-thumb" data-att-idx="${i}">` +
        `<img class="chat-img-zoomable" src="${a.dataUrl}" alt="${chatEsc(a.name)}" data-img-src="${chatEsc(a.dataUrl)}" data-img-name="${chatEsc(a.name)}" title="点击放大" />` +
        `<button type="button" class="chat-attach-remove" data-att-remove="${i}" title="移除">×</button>` +
        `<span class="chat-attach-name">${chatEsc(a.name)}</span>` +
        `</div>`
    )
    .join("");
}

/** 图片放大 lightbox */
function openImageLightbox(src, name) {
  if (!src) return;
  const box = $("#img-lightbox");
  const img = $("#img-lightbox-img");
  const cap = $("#img-lightbox-caption");
  if (!box || !img) return;
  img.src = src;
  img.alt = name || "图片";
  if (cap) cap.textContent = name || "";
  box.hidden = false;
}

function closeImageLightbox() {
  const box = $("#img-lightbox");
  const img = $("#img-lightbox-img");
  if (img) img.removeAttribute("src");
  if (box) box.hidden = true;
}

/** Ctrl/Cmd+V 或剪贴板图片 → 附件队列 */
async function handleChatPaste(e) {
  if (!state.selectedPath || state.page !== "chat") return;
  const cd = e.clipboardData || e.originalEvent?.clipboardData;
  if (!cd) return;
  const files = [];
  // Prefer items (screenshot paste)
  if (cd.items && cd.items.length) {
    for (const it of cd.items) {
      if (it.kind === "file" && it.type && it.type.startsWith("image/")) {
        const f = it.getAsFile();
        if (f) files.push(f);
      }
    }
  }
  if (!files.length && cd.files && cd.files.length) {
    for (const f of cd.files) {
      if (f.type && f.type.startsWith("image/")) files.push(f);
    }
  }
  if (!files.length) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    await addChatAttachments(files);
    toast(`已粘贴 ${files.length} 张图片`);
  } catch (err) {
    toast(String(err?.message || err));
  }
}

function pickChatAttachments() {
  const input = $("#chat-file-input");
  if (input) {
    input.value = "";
    input.click();
  }
}

async function uploadPendingAttachments() {
  ensureChatState();
  const pending = state.chatPendingAttachments || [];
  if (!pending.length) return [];
  const out = [];
  for (const p of pending) {
    const resp = await invoke("chat_save_attachment_cmd", {
      project: state.selectedPath,
      sessionId: state.chatSession?.session_id || "default",
      fileName: p.name,
      mime: p.mime,
      dataBase64: dataUrlToBase64(p.dataUrl),
    });
    out.push({
      path: resp.path || resp.path,
      mime: resp.mime,
      name: resp.name,
    });
  }
  return out;
}

/** Cache key: project path + session id (C3 multi-session). */
function chatCacheKey(path, sessionId) {
  const p = path || state.selectedPath || state.chatProjectPath || "";
  const sid = sessionId || state.chatSession?.session_id || "default";
  return `${p}::${sid}`;
}

/** Snapshot current chat UI into per-project+session cache (survive page/session switches). */
function stashChatSession(path, sessionId) {
  ensureChatState();
  const p = path || state.selectedPath || state.chatProjectPath;
  if (!p) return;
  const sid = sessionId || state.chatSession?.session_id || "default";
  const key = chatCacheKey(p, sid);
  state.chatSessions[key] = {
    session_id: sid,
    messages: Array.isArray(state.chatSession?.messages)
      ? state.chatSession.messages.slice()
      : [],
    draft_plan: state.chatSession?.draft_plan
      ? { ...state.chatSession.draft_plan }
      : null,
    draftPath: state.chatDraftPlan || null,
    fake: !!state.chatFake,
    envNote: state.chatEnvNote || null,
    busy: !!state.chatBusy,
    waitStartedAt: state.chatWaitStartedAt || 0,
    title: state.chatSession?.title || null,
  };
  // Legacy single-key (project only) for older page-hop paths still reading it.
  state.chatSessions[p] = state.chatSessions[key];
}

/** Restore cached chat UI for a project (+ optional session). Returns true if cache hit. */
function restoreChatSession(path, sessionId) {
  ensureChatState();
  const p = path || state.selectedPath;
  if (!p) return false;
  const sid =
    sessionId ||
    state.chatSession?.session_id ||
    "default";
  const key = chatCacheKey(p, sid);
  const c = state.chatSessions[key] || state.chatSessions[p];
  if (!c) return false;
  // If legacy cache has a different session, only accept when sessionId not forced.
  if (sessionId && c.session_id && c.session_id !== sessionId) {
    if (!state.chatSessions[key]) return false;
  }
  state.chatProjectPath = p;
  state.chatSession = {
    session_id: c.session_id || sid || "default",
    messages: Array.isArray(c.messages) ? c.messages.slice() : [],
    draft_plan: c.draft_plan ? { ...c.draft_plan } : null,
    title: c.title || null,
  };
  state.chatDraftPlan = c.draftPath || null;
  state.chatFake = !!c.fake;
  state.chatEnvNote = c.envNote || null;
  // Do not restore busy across project/session switches; only same-session page hops.
  if (
    state.chatBusy &&
    state.chatProjectPath === p &&
    (state.chatSession?.session_id || "default") === (c.session_id || sid)
  ) {
    /* keep in-flight send */
  } else {
    state.chatBusy = !!c.busy;
    state.chatWaitStartedAt = c.waitStartedAt || 0;
  }
  return true;
}

/** Elapsed wait label while Claude CLI runs in the background. */
function chatWaitLabel() {
  const started = state.chatWaitStartedAt || 0;
  if (!started) return "AI 正在思考…";
  const sec = Math.max(0, Math.floor((Date.now() - started) / 1000));
  if (sec < 5) return "AI 正在思考…";
  if (sec < 60) return `AI 正在思考…（已等 ${sec}s）`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `AI 正在思考…（已等 ${m}分${s}s，可稍候）`;
}

/** Paint the pending assistant bubble (wait label or streaming partial). */
function paintChatPendingBubble() {
  const pending = document.querySelector(".chat-msg-pending .chat-msg-body");
  if (!pending) return;
  const stream = String(state.chatStreamText || "").trim();
  if (stream) {
    pending.classList.add("chat-msg-streaming");
    // Cap paint size so a huge partial doesn't thrash the DOM every tick.
    const shown =
      stream.length > 6000 ? "…\n" + stream.slice(-6000) : stream;
    pending.innerHTML =
      chatEsc(shown) + '<span class="chat-stream-cursor" aria-hidden="true">▍</span>';
  } else {
    pending.classList.remove("chat-msg-streaming");
    pending.textContent = chatWaitLabel();
  }
}

/** C3: poll stdout partial while chat_send runs; failure → keep wait label. */
async function pollChatStreamPartial() {
  if (!state.chatBusy || !state.selectedPath) return;
  try {
    const resp = await invoke("chat_stream_partial_cmd", {
      project: state.selectedPath,
      sessionId: state.chatSession?.session_id || "default",
    });
    const text = String(resp?.text || "").trim();
    // Only advance (never shrink) so a late shorter extract doesn't flicker.
    if (text && text.length >= String(state.chatStreamText || "").length) {
      state.chatStreamText = text;
    } else if (text && !state.chatStreamText) {
      state.chatStreamText = text;
    }
  } catch (_) {
    // Soft degrade: leave wait label; final reply still comes from chat_send.
  }
}

let _chatWaitTick = null;
let _chatStreamTick = null;
function startChatWaitTicker() {
  stopChatWaitTicker();
  paintChatPendingBubble();
  _chatWaitTick = setInterval(() => {
    if (!state.chatBusy) {
      stopChatWaitTicker();
      return;
    }
    // Refresh only the pending bubble + send label without full re-render of history.
    paintChatPendingBubble();
    const sendBtn = $("#btn-chat-send");
    if (sendBtn && state.chatBusy) {
      sendBtn.textContent = state.chatStreamText ? "生成中…" : "思考中…";
    }
  }, 1000);
  // Stream poll slightly faster than wait label (best-effort; spawn_blocking free).
  _chatStreamTick = setInterval(() => {
    if (!state.chatBusy) return;
    pollChatStreamPartial().then(() => paintChatPendingBubble()).catch(() => {});
  }, 700);
  // First poll immediately so early deltas show up.
  pollChatStreamPartial().then(() => paintChatPendingBubble()).catch(() => {});
}
function stopChatWaitTicker() {
  if (_chatWaitTick) {
    clearInterval(_chatWaitTick);
    _chatWaitTick = null;
  }
  if (_chatStreamTick) {
    clearInterval(_chatStreamTick);
    _chatStreamTick = null;
  }
}

function applyChatDraftFromSession(sess) {
  ensureChatState();
  if (!sess) {
    state.chatDraftPlan = null;
    return;
  }
  const d = sess.draft_plan || null;
  state.chatSession = {
    session_id: sess.session_id || "default",
    messages: Array.isArray(sess.messages) ? sess.messages : [],
    draft_plan: d,
    title: sess.title || null,
  };
  if (d && d.saved && d.path) {
    state.chatDraftPlan = d.path;
  } else if (d && d.path && d.saved) {
    state.chatDraftPlan = d.path;
  } else {
    // keep path if previously saved in this UI session
    if (d?.path && d.saved) state.chatDraftPlan = d.path;
    else if (!d?.saved) {
      /* unsaved draft markdown only */
      if (!state.chatDraftPlan) state.chatDraftPlan = null;
    }
  }
  // Prefer server truth for saved path
  if (d?.saved && d.path) state.chatDraftPlan = d.path;
  else if (!d?.saved) {
    // do not clear a previously saved path unless server says different project load
  }
}

/**
 * Load chat for the selected project.
 * - Prefer in-memory cache (page hop) so history never blanks.
 * - Skip disk reload while a send is in flight (would race and wipe optimistic msgs).
 * - Disk load uses a sequence token so stale responses cannot clobber newer state.
 */
/** C3: load session list for switcher (does not change current session). */
async function loadChatSessionList() {
  ensureChatState();
  if (!state.selectedPath) {
    state.chatSessionList = [{ session_id: "default", title: null, message_count: 0 }];
    renderChatSessionSelect();
    return;
  }
  state.chatSessionListLoading = true;
  try {
    const list = await invoke("chat_list_sessions_cmd", {
      project: state.selectedPath,
    });
    state.chatSessionList = Array.isArray(list) ? list : [];
    if (!state.chatSessionList.length) {
      state.chatSessionList = [
        { session_id: "default", title: null, message_count: 0 },
      ];
    }
  } catch (e) {
    console.warn("chat_list_sessions failed", e);
    if (!state.chatSessionList?.length) {
      state.chatSessionList = [
        {
          session_id: state.chatSession?.session_id || "default",
          title: state.chatSession?.title || null,
          message_count: (state.chatSession?.messages || []).length,
        },
      ];
    }
  } finally {
    state.chatSessionListLoading = false;
    renderChatSessionSelect();
  }
}

function chatSessionLabel(row) {
  if (!row) return "默认";
  const id = row.session_id || "default";
  if (id === "default") {
    const t = row.title || row.preview || row.draft_plan_title;
    return t ? `默认 · ${t}` : "默认";
  }
  const t = row.title || row.preview || row.draft_plan_title;
  if (t) return t;
  // Compact id: s-20260720-153045 → 07-20 15:30
  const m = /^s-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(id);
  if (m) return `${m[2]}-${m[3]} ${m[4]}:${m[5]}`;
  return id;
}

function renderChatSessionSelect() {
  ensureChatState();
  const sel = $("#chat-session-select");
  const delBtn = $("#btn-chat-session-del");
  const newBtn = $("#btn-chat-session-new");
  if (!sel) return;
  const cur = state.chatSession?.session_id || "default";
  const list = Array.isArray(state.chatSessionList) ? state.chatSessionList : [];
  // Ensure current is in options even if list lagging
  const ids = new Set(list.map((r) => r.session_id));
  const rows = list.slice();
  if (!ids.has(cur)) {
    rows.unshift({
      session_id: cur,
      title: state.chatSession?.title || null,
      message_count: (state.chatSession?.messages || []).length,
    });
  }
  const prev = sel.value;
  sel.innerHTML = rows
    .map((r) => {
      const id = chatEsc(r.session_id || "default");
      const label = chatEsc(chatSessionLabel(r));
      const n = r.message_count != null ? r.message_count : 0;
      const suffix = n > 0 ? ` (${n})` : "";
      return `<option value="${id}">${label}${suffix}</option>`;
    })
    .join("");
  sel.value = ids.has(cur) || rows.some((r) => r.session_id === cur) ? cur : rows[0]?.session_id || "default";
  // If value set failed (missing option), force
  if (sel.value !== cur && rows.some((r) => r.session_id === cur)) {
    sel.value = cur;
  }
  void prev;
  sel.disabled = !state.selectedPath || !!state.chatBusy || !!state.chatSessionListLoading;
  if (newBtn) newBtn.disabled = !state.selectedPath || !!state.chatBusy;
  if (delBtn) {
    // Allow delete always when a session exists; default clears to empty
    delBtn.disabled =
      !state.selectedPath ||
      !!state.chatBusy ||
      (!list.length && cur === "default" && !(state.chatSession?.messages || []).length);
    delBtn.title =
      cur === "default"
        ? "清空默认会话（删除磁盘记录）"
        : "删除当前会话";
  }
}

/** C3: switch to another session id (stash current, load target). */
async function switchChatSession(sessionId) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const sid = String(sessionId || "default").trim() || "default";
  const cur = state.chatSession?.session_id || "default";
  if (sid === cur && state.chatProjectPath === state.selectedPath) {
    renderChatSessionSelect();
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再切换会话");
    renderChatSessionSelect();
    return;
  }
  stashChatSession(state.selectedPath, cur);
  state.chatSession = { session_id: sid, messages: [], draft_plan: null, title: null };
  state.chatDraftPlan = null;
  state.chatFake = false;
  state.chatEnvNote = null;
  // Prefer cache for instant paint
  restoreChatSession(state.selectedPath, sid);
  renderChatPage();
  await loadChatSession({ force: true });
  await loadChatSessionList();
}

/** C3: create empty session and switch to it. */
async function newChatSession() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再新建");
    return;
  }
  const cur = state.chatSession?.session_id || "default";
  stashChatSession(state.selectedPath, cur);
  try {
    const sess = await invoke("chat_new_session_cmd", {
      project: state.selectedPath,
      title: null,
    });
    const sid = sess?.session_id || "default";
    state.chatSession = {
      session_id: sid,
      messages: Array.isArray(sess?.messages) ? sess.messages : [],
      draft_plan: sess?.draft_plan || null,
      title: sess?.title || null,
    };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    state.chatProjectPath = state.selectedPath;
    stashChatSession(state.selectedPath, sid);
    toast(`已新建会话`);
    renderChatPage();
    await loadChatSessionList();
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** C3: delete current session (confirm), then switch to default or first remaining. */
async function deleteChatSession() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.chatBusy) {
    toast("AI 正在回复，请稍后再删");
    return;
  }
  const sid = state.chatSession?.session_id || "default";
  const label = chatSessionLabel(
    (state.chatSessionList || []).find((r) => r.session_id === sid) || {
      session_id: sid,
      title: state.chatSession?.title,
    }
  );
  const ok = window.confirm(
    sid === "default"
      ? "清空默认会话的聊天记录与草稿绑定？计划文件本身不会删除。"
      : `删除会话「${label}」？计划文件本身不会删除。`
  );
  if (!ok) return;
  try {
    await invoke("chat_delete_session_cmd", {
      project: state.selectedPath,
      sessionId: sid,
    });
    // Drop cache for this session
    const key = chatCacheKey(state.selectedPath, sid);
    delete state.chatSessions[key];
    if (state.chatSessions[state.selectedPath]?.session_id === sid) {
      delete state.chatSessions[state.selectedPath];
    }
    // Switch to default (or list head after refresh)
    state.chatSession = {
      session_id: "default",
      messages: [],
      draft_plan: null,
      title: null,
    };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    toast(sid === "default" ? "已清空默认会话" : "已删除会话");
    await loadChatSessionList();
    const next =
      (state.chatSessionList || []).find((r) => r.session_id === "default")
        ?.session_id ||
      state.chatSessionList?.[0]?.session_id ||
      "default";
    await switchChatSession(next);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

async function loadChatSession(opts) {
  ensureChatState();
  const force = !!(opts && opts.force);
  if (!state.selectedPath) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null, title: null };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    state.chatProjectPath = null;
    state.chatSessionList = [];
    renderChatPage();
    return;
  }
  const path = state.selectedPath;

  // Leaving a different project's chat: stash previous, then restore/load this one.
  if (state.chatProjectPath && state.chatProjectPath !== path) {
    stashChatSession(state.chatProjectPath);
  }

  // Same project + in-flight send: never re-fetch (keeps optimistic user + pending bubble).
  if (!force && state.chatBusy && state.chatProjectPath === path) {
    renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    return;
  }

  // Page hop back to same project: restore cache first so UI is never empty,
  // then optionally refresh from disk if we have no local messages yet.
  if (state.chatProjectPath === path && (state.chatSession?.messages || []).length) {
    stashChatSession(path);
    renderChatPage();
    // Background refresh only when idle and not forced skip.
    if (!force && !state.chatBusy) {
      /* keep showing cache; soft refresh below still runs for disk truth */
    } else {
      return;
    }
  } else if (restoreChatSession(path) && (state.chatSession?.messages || []).length) {
    renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    // Fall through to soft disk refresh when idle so multi-device/disk edits land.
    if (state.chatBusy) return;
  }

  const seq = ++state._chatLoadSeq;
  const sid = state.chatSession?.session_id || "default";
  try {
    const sess = await invoke("chat_session_get_cmd", {
      project: path,
      sessionId: sid,
    });
    // Stale or project switched mid-flight → drop.
    if (seq !== state._chatLoadSeq || state.selectedPath !== path) return;
    // In-flight send still owns the UI.
    if (state.chatBusy && state.chatProjectPath === path) {
      renderChatPage();
      return;
    }

    const diskMsgs = Array.isArray(sess?.messages) ? sess.messages : [];
    const memMsgs = state.chatSession?.messages || [];
    // Prefer longer history (disk after successful send, or mem if send just finished
    // and disk lag / concurrent get). Never replace a non-empty mem with empty disk
    // unless force.
    const takeDisk =
      force ||
      diskMsgs.length > memMsgs.length ||
      (diskMsgs.length === memMsgs.length && diskMsgs.length > 0) ||
      memMsgs.length === 0;

    if (takeDisk) {
      applyChatDraftFromSession(sess);
      if (sess?.draft_plan?.saved && sess.draft_plan.path) {
        state.chatDraftPlan = sess.draft_plan.path;
      } else if (!sess?.draft_plan?.saved) {
        // Keep mem draftPath if we only had unsaved markdown; clear only when empty mem.
        if (!state.chatDraftPlan || memMsgs.length === 0) {
          state.chatDraftPlan = null;
        }
      }
    }
    state.chatProjectPath = path;
    stashChatSession(path);
  } catch (e) {
    console.warn("chat_session_get failed", e);
    if (!state.chatSession?.messages?.length) {
      // Last resort: cache for this path if any.
      if (!restoreChatSession(path)) {
        state.chatSession = { session_id: "default", messages: [], draft_plan: null };
        state.chatFake = false;
      }
    }
    state.chatProjectPath = path;
  }
  renderChatPage();
  if (state.chatBusy) startChatWaitTicker();
}

function chatEsc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Parse plan markdown for card outline: first # title + up to 4 outline lines. */
function chatPlanOutline(md) {
  const lines = String(md || "").split(/\r?\n/);
  let title = "计划草稿";
  const outline = [];
  for (const line of lines) {
    const t = line.trim();
    if (!t) continue;
    if (title === "计划草稿" && /^#\s+/.test(t)) {
      title = t.replace(/^#\s+/, "").trim() || title;
      continue;
    }
    // Prefer numbered / bullet / ## headings as outline
    if (
      outline.length < 4 &&
      (/^(\d+[\.\)]\s+|[-*•]\s+|#{2,3}\s+)/.test(t) ||
        (outline.length === 0 && t.length < 80 && !t.startsWith("```")))
    ) {
      let item = t
        .replace(/^#{1,3}\s+/, "")
        .replace(/^(\d+[\.\)]\s+|[-*•]\s+)/, "")
        .trim();
      if (item && item !== title) outline.push(item);
    }
  }
  // Fallback: first non-title non-empty lines
  if (outline.length === 0) {
    for (const line of lines) {
      const t = line.trim().replace(/^#+\s+/, "");
      if (!t || t === title || t.startsWith("```")) continue;
      outline.push(t.length > 72 ? t.slice(0, 70) + "…" : t);
      if (outline.length >= 4) break;
    }
  }
  return { title, outline };
}

/** Line-start ``` only (mirrors services/chat.rs fence helpers). */
function chatIsLineStartFence(s, idx) {
  return idx === 0 || s[idx - 1] === "\n" || s[idx - 1] === "\r";
}

function chatFenceLangTagLen(after) {
  let n = 0;
  for (const ch of after) {
    if (/[A-Za-z0-9_+-]/.test(ch)) n += 1;
    else break;
  }
  return n;
}

function chatFindLineFence(s, from) {
  if (from >= s.length) return -1;
  let i = from;
  if (i > 0 && s[i - 1] !== "\n" && s[i - 1] !== "\r") {
    const nl = s.indexOf("\n", i);
    if (nl < 0) return -1;
    i = nl + 1;
  }
  while (i < s.length) {
    if (s.startsWith("```", i) && chatIsLineStartFence(s, i)) return i;
    const nl = s.indexOf("\n", i);
    if (nl < 0) break;
    i = nl + 1;
  }
  return -1;
}

/** Close fence body with nested ```lang … ``` support. Returns [end, cont] or null. */
function chatCloseFenceBody(body) {
  let depth = 1;
  let pos = 0;
  while (true) {
    const j = chatFindLineFence(body, pos);
    if (j < 0) return null;
    const after = body.slice(j + 3);
    const tlen = chatFenceLangTagLen(after);
    const tag = after.slice(0, tlen);
    if (tag) {
      depth += 1;
      pos = j + 3 + tlen;
    } else {
      depth -= 1;
      if (depth === 0) return [j, j + 3];
      pos = j + 3;
    }
  }
}

/**
 * Split assistant markdown into text / plan / code segments.
 * Nested ```text diagrams inside ```plan stay inside the plan body (not cut early).
 */
function chatSegmentMarkdown(text) {
  const s = String(text || "");
  const out = [];
  let i = 0;
  while (i < s.length) {
    const idx = s.indexOf("```", i);
    if (idx < 0) {
      if (i < s.length) out.push({ type: "text", body: s.slice(i) });
      break;
    }
    if (!chatIsLineStartFence(s, idx)) {
      // mid-line triple-backtick: keep as text through this marker
      if (idx > i) out.push({ type: "text", body: s.slice(i, idx + 3) });
      i = idx + 3;
      continue;
    }
    if (idx > i) out.push({ type: "text", body: s.slice(i, idx) });

    // Absolute offsets into s:
    //   opener at idx..idx+3
    //   tag at idx+3 .. idx+3+tlen
    //   body starts after tag + optional spaces + one newline
    const tagStart = idx + 3;
    const tlen = chatFenceLangTagLen(s.slice(tagStart));
    const tag = s.slice(tagStart, tagStart + tlen);
    let bodyStart = tagStart + tlen;
    while (bodyStart < s.length && (s[bodyStart] === " " || s[bodyStart] === "\t")) {
      bodyStart += 1;
    }
    if (s.startsWith("\r\n", bodyStart)) bodyStart += 2;
    else if (s[bodyStart] === "\n" || s[bodyStart] === "\r") bodyStart += 1;

    const body = s.slice(bodyStart);
    const closed = chatCloseFenceBody(body);
    if (!closed) {
      out.push({ type: "text", body: s.slice(idx) });
      break;
    }
    const [end, cont] = closed;
    const block = body.slice(0, end).replace(/\s+$/, "");
    if (tag.toLowerCase() === "plan") {
      out.push({ type: "plan", body: block });
    } else {
      out.push({ type: "code", lang: tag || "", body: block });
    }
    i = bodyStart + cont;
  }
  return out;
}

/** Pull last ```plan body from free text (nesting-aware). */
function chatExtractPlanFence(text) {
  const segs = chatSegmentMarkdown(text);
  let best = null;
  for (const seg of segs) {
    if (seg.type === "plan" && seg.body && seg.body.trim()) best = seg.body.trim();
  }
  return best;
}

function chatNormMdKey(md) {
  return String(md || "")
    .replace(/\r\n/g, "\n")
    .replace(/\s+$/gm, "")
    .trim();
}

/**
 * Footer CTAs live on the plan card (not sticky ready-bar).
 * @param {string} md
 * @param {{ active?: boolean }} opts  active = latest plan in latest assistant reply
 */
function chatPlanCardActionsHtml(md, opts = {}) {
  ensureChatState();
  const active = opts.active !== false;
  const draft = state.chatSession?.draft_plan;
  const savedPath = state.chatDraftPlan || (draft?.saved ? draft.path : null);
  const draftKey = chatNormMdKey(draft?.markdown || "");
  const cardKey = chatNormMdKey(md);
  // Prefer exact body match; fall back to "active card + has draft" so structure
  // normalize diffs still light the right footer.
  const isThisDraft =
    !!(draftKey && cardKey && draftKey === cardKey) ||
    !!(active && draft && (draft.markdown || savedPath));
  const isSaved = !!(savedPath && isThisDraft && (draft?.saved || state.chatDraftPlan));
  const busy = !!state.chatBusy;
  const runLocked = typeof hasActiveRun === "function" ? hasActiveRun() : false;

  const expand =
    `<button type="button" class="btn ghost sm btn-chat-plan-expand">展开全文</button>`;

  // Historical plan cards: expand only (no sticky-like duplicate CTAs)
  if (!active) {
    return (
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `</div>`
    );
  }

  if (isSaved) {
    const canExec = !runLocked && !busy;
    return (
      `<span class="chat-plan-card-saved muted">已保存：${chatEsc(savedPath)}</span>` +
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `<button type="button" class="btn ghost sm btn-chat-plan-adopt" ${busy ? "disabled" : ""} title="覆盖保存到本地计划文件">重新保存</button>` +
      `<button type="button" class="btn primary sm btn-chat-plan-assign" ${canExec ? "" : "disabled"} title="${
        runLocked ? "运行中，请先停止后再执行新计划" : "带上当前计划进入拆分执行"
      }">执行此计划</button>` +
      `</div>`
    );
  }

  return (
    `<div class="chat-plan-card-actions-btns">` +
    expand +
    `<button type="button" class="btn primary sm btn-chat-plan-adopt" ${busy || !md ? "disabled" : ""}>保存为计划</button>` +
    `</div>`
  );
}

function chatFormatPlanCard(rawMd, opts = {}) {
  const md = String(rawMd || "").trim();
  const { title, outline } = chatPlanOutline(md);
  const outlineHtml =
    outline.length > 0
      ? `<ul class="chat-plan-outline">${outline
          .map((o) => `<li>${chatEsc(o)}</li>`)
          .join("")}</ul>`
      : `<p class="chat-plan-outline-empty muted">（暂无大纲条目）</p>`;
  // Full body kept in hidden pre for expand; adopt uses same markdown via saveChatPlan
  return (
    `<div class="chat-plan-card" data-plan-md="1">` +
    `<div class="chat-plan-card-label">计划草稿</div>` +
    `<div class="chat-plan-card-title">${chatEsc(title)}</div>` +
    `<div class="chat-plan-summary">` +
    outlineHtml +
    `</div>` +
    `<pre class="chat-plan-pre chat-plan-full" hidden>${chatEsc(md)}</pre>` +
    `<div class="chat-plan-card-actions">` +
    chatPlanCardActionsHtml(md, opts) +
    `</div>` +
    `</div>`
  );
}

/**
 * @param {string} text
 * @param {{ activePlan?: boolean }} opts  when true, last ```plan in this body gets save/exec CTAs
 */
function chatFormatBody(text, opts = {}) {
  // Parse fences on raw text first (nesting-aware), then escape each segment.
  const segs = chatSegmentMarkdown(text);
  let lastPlanIdx = -1;
  if (opts.activePlan) {
    for (let i = 0; i < segs.length; i++) {
      if (segs[i].type === "plan") lastPlanIdx = i;
    }
  }
  return segs
    .map((seg, i) => {
      if (seg.type === "plan") {
        return chatFormatPlanCard(seg.body, {
          active: opts.activePlan && i === lastPlanIdx,
        });
      }
      if (seg.type === "code") {
        return `<pre class="chat-code-block">${chatEsc(seg.body)}</pre>`;
      }
      let t = chatEsc(seg.body || "");
      t = t.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
      t = t.replace(/\n/g, "<br/>");
      return t;
    })
    .join("");
}

/** Toggle plan card full markdown (expand/collapse). */
function toggleChatPlanExpand(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const summary = card.querySelector(".chat-plan-summary");
  if (!full) return;
  const open = full.hidden;
  full.hidden = !open;
  if (summary) summary.hidden = open;
  btn.textContent = open ? "收起全文" : "展开全文";
}

/** Card「采用并保存」→ same as ready-bar saveChatPlan. */
function adoptChatPlanFromCard(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const md = full?.textContent?.trim();
  if (!md) {
    toast("卡片中没有可保存的计划正文");
    return;
  }
  ensureChatState();
  // Seed draft_plan so saveChatPlan uses this markdown
  if (!state.chatSession.draft_plan) {
    state.chatSession.draft_plan = {
      path: "",
      saved: false,
      markdown: md,
      title: null,
    };
  } else {
    state.chatSession.draft_plan.markdown = md;
  }
  stashChatSession(state.selectedPath || state.chatProjectPath);
  return saveChatPlan();
}

function fillChatExample(text) {
  const input = $("#chat-input");
  if (!input || !state.selectedPath) return;
  input.value = text;
  input.focus();
}

function renderChatMessages() {
  const list = $("#chat-messages");
  if (!list) return;
  ensureChatState();
  const msgs = state.chatSession.messages || [];
  if (!msgs.length && !state.chatBusy) {
    list.innerHTML = `
      <div class="chat-empty muted">
        <p>用自然语言说明你要做什么。AI 会先帮你写成一份<strong>计划文档</strong>，保存后再点「执行此计划」进入拆分执行。</p>
        <p class="chat-hint">点下面示例填入输入框，改完再发送：</p>
        <div class="chat-example-chips">
          <button type="button" class="chat-example-chip" data-chat-example="优化登录与注册体验，写清范围和验收">优化登录体验</button>
          <button type="button" class="chat-example-chip" data-chat-example="排查并修复 flaky 测试，列出可疑用例与步骤">修 flaky 测试</button>
          <button type="button" class="chat-example-chip" data-chat-example="为当前模块补用户文档与上手步骤">补模块文档</button>
        </div>
      </div>`;
    return;
  }
  // Only the last assistant message's plan card gets save/execute CTAs
  let lastAssistantIdx = -1;
  for (let i = msgs.length - 1; i >= 0; i--) {
    if (msgs[i]?.role === "assistant") {
      lastAssistantIdx = i;
      break;
    }
  }
  let html = msgs
    .map((m, mi) => {
      const role = m.role === "assistant" ? "assistant" : m.role === "system" ? "system" : "user";
      const label = role === "assistant" ? "AI" : role === "system" ? "系统" : "我";
      const atts = Array.isArray(m.attachments) ? m.attachments : [];
      const attHtml = atts.length
        ? `<div class="chat-msg-atts">${atts
            .map((a) => {
              const src = a._preview || "";
              const name = chatEsc(a.name || a.path || "图");
              if (src) {
                return (
                  `<div class="chat-msg-att">` +
                  `<img class="chat-img-zoomable" src="${src}" alt="${name}" data-img-src="${chatEsc(src)}" data-img-name="${name}" title="点击放大" />` +
                  `<span>${name}</span></div>`
                );
              }
              return `<div class="chat-msg-att chat-msg-att-path" title="${chatEsc(a.path || "")}">📎 ${name}</div>`;
            })
            .join("")}</div>`
        : "";
      const activePlan = role === "assistant" && mi === lastAssistantIdx;
      return `<div class="chat-msg chat-msg-${role}">
        <div class="chat-msg-role">${label}</div>
        <div class="chat-msg-body">${chatFormatBody(m.content || "", { activePlan })}${attHtml}</div>
      </div>`;
    })
    .join("");
  // Waiting bubble: user already sent; UI must stay responsive while CLI runs.
  // C3: if stream partial arrived, show it in place of the wait label.
  if (state.chatBusy) {
    const stream = String(state.chatStreamText || "").trim();
    if (stream) {
      const shown =
        stream.length > 6000 ? "…\n" + stream.slice(-6000) : stream;
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending chat-msg-streaming">${chatEsc(
        shown
      )}<span class="chat-stream-cursor" aria-hidden="true">▍</span></div>
    </div>`;
    } else {
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending">
        <span class="chat-pending-dots" aria-hidden="true"></span>
        ${chatEsc(chatWaitLabel())}
      </div>
    </div>`;
    }
  }
  list.innerHTML = html;
  list.scrollTop = list.scrollHeight;
}

function renderChatEnvBar() {
  const bar = $("#chat-env-bar");
  if (!bar) return;
  ensureChatState();
  const note = state.chatEnvNote;
  // forced fake 联调也可显示简短 mock 条；有 env_note 优先
  const show = !!(note && String(note).trim());
  bar.hidden = !show;
  const noteEl = $("#chat-env-note");
  if (noteEl && show) noteEl.textContent = String(note).trim();
}

function dismissChatEnvBar() {
  state.chatEnvNote = null;
  stashChatSession(state.selectedPath || state.chatProjectPath);
  renderChatEnvBar();
}

function openChatEnvDoctor() {
  try {
    if (typeof showPage === "function") showPage("doctor");
    else if (typeof openDoctorPage === "function") openDoctorPage();
  } catch (_) {
    toast("请从侧栏打开「环境检查」");
  }
  try {
    if (typeof runDoctor === "function") runDoctor();
    else if (typeof loadDoctor === "function") loadDoctor();
  } catch (_) {}
}

/** G0b: re-structure current draft via chat_normalize_plan_cmd. */
async function normalizeChatDraft(hint) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return null;
  }
  const draft = state.chatSession?.draft_plan;
  let md = draft?.markdown;
  if (!md) {
    const msgs = state.chatSession?.messages || [];
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].role === "assistant") {
        const m = String(msgs[i].content || "").match(/```plan\s*([\s\S]*?)```/i);
        if (m) {
          md = m[1].trim();
          break;
        }
      }
    }
  }
  if (!md) {
    toast("还没有可整理的计划草稿");
    return null;
  }
  state.chatBusy = true;
  renderChatPage();
  try {
    const resp = await invoke("chat_normalize_plan_cmd", {
      project: state.selectedPath,
      markdown: md,
      hint: hint || null,
    });
    const out = resp?.markdown || md;
    const title = resp?.title || planTitleFromMarkdown(out);
    if (!state.chatSession.draft_plan) {
      state.chatSession.draft_plan = {
        path: "",
        saved: false,
        markdown: out,
        title,
      };
    } else {
      state.chatSession.draft_plan.markdown = out;
      state.chatSession.draft_plan.title = title;
      if (!state.chatSession.draft_plan.path) {
        state.chatSession.draft_plan.saved = false;
      }
    }
    stashChatSession(state.selectedPath);
    toast(
      resp?.used_cli
        ? "已用 CLI 整理计划结构"
        : "已整理计划结构（本地模板补全）"
    );
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  } finally {
    state.chatBusy = false;
    stashChatSession(state.selectedPath);
    renderChatPage();
  }
}

/**
 * Sticky ready-bar retired: save / re-save / execute live only on the plan card
 * footer inside the assistant reply (bottom of that message). Keep this function
 * so old call sites stay safe; always hide the bar and its fixed buttons.
 */
function renderChatReadyBar() {
  const bar = $("#chat-ready-bar");
  if (bar) {
    bar.hidden = true;
    bar.classList.remove("is-fake");
  }
  const saveBtn = $("#btn-chat-save");
  const assignBtn = $("#btn-chat-assign");
  const previewBtn = $("#btn-chat-preview");
  const normalizeBtn = $("#btn-chat-normalize");
  if (saveBtn) saveBtn.hidden = true;
  if (assignBtn) assignBtn.hidden = true;
  if (previewBtn) previewBtn.hidden = true;
  if (normalizeBtn) normalizeBtn.hidden = true;
}

function renderChatPage() {
  const projLabel = $("#chat-project-label");
  if (projLabel) {
    projLabel.textContent = state.selectedPath
      ? chatProjectName()
      : "未选择项目";
  }
  const input = $("#chat-input");
  const sendBtn = $("#btn-chat-send");
  const attachBtn = $("#btn-chat-attach");
  if (input) {
    // Keep the composer editable while waiting so the app never feels frozen;
    // only the send button is gated (double-send guard).
    input.disabled = !state.selectedPath;
    input.placeholder = !state.selectedPath
      ? "请先在左侧选择项目"
      : state.chatBusy
        ? "AI 正在回复，可先写下一条…"
        : "说清目标与约束；可附图；满意后让 AI 生成计划…";
  }
  if (sendBtn) {
    // Disabled while waiting = prevent double-send, NOT app freeze.
    // Backend chat_send runs on a worker thread so the rest of the UI stays live.
    sendBtn.disabled = !state.selectedPath || !!state.chatBusy;
    sendBtn.textContent = state.chatBusy ? "思考中…" : "发送";
    sendBtn.title = state.chatBusy
      ? "正在等待本机 Claude CLI 回复，请稍候"
      : "发送消息";
  }
  if (attachBtn) {
    attachBtn.disabled = !state.selectedPath || !!state.chatBusy;
  }
  renderChatSessionSelect();
  renderChatAttachPreview();
  renderChatMessages();
  renderChatEnvBar();
  renderChatReadyBar();
  renderPlanRail();
  renderPlanFullView();
}

async function openChatPage() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  // Leaving another page: keep current chat in cache first.
  if (state.chatProjectPath) stashChatSession(state.chatProjectPath);
  // G0: re-read per-project rail open preference when switching projects
  const railKey = `cco.planRailOpen:${state.selectedPath}`;
  state.planRailOpen = localStorage.getItem(railKey) === "1";
  showPage("chat");
  // Restore immediately so history is never blank while disk loads.
  restoreChatSession(state.selectedPath);
  applyPlanRailVisibility();
  renderChatPage();
  await loadChatSession();
  // C3: session switcher list (best-effort)
  try {
    await loadChatSessionList();
  } catch (_) {}
  // G0/G1: only scan rail when user has opened 计划管理
  if (state.planRailOpen) {
    try {
      await loadPlanRail();
    } catch (_) {}
  }
}

async function sendChatMessage() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const input = $("#chat-input");
  const text = (input?.value || "").trim();
  const hasAtt = (state.chatPendingAttachments || []).length > 0;
  if (!text && !hasAtt) return;
  if (state.chatBusy) return;

  const projectPath = state.selectedPath;
  state.chatProjectPath = projectPath;
  state.chatBusy = true;
  state.chatWaitStartedAt = Date.now();
  state.chatStreamText = "";
  if (input) input.value = "";
  const pendingSnap = (state.chatPendingAttachments || []).slice();
  // optimistic user bubble + pending AI bubble (renderChatMessages)
  const optContent =
    text ||
    (pendingSnap.length ? `（附图 ${pendingSnap.length} 张）` : "");
  state.chatSession.messages = [
    ...(state.chatSession.messages || []),
    {
      role: "user",
      content: optContent,
      attachments: pendingSnap.map((p) => ({
        name: p.name,
        mime: p.mime,
        path: "",
        _preview: p.dataUrl,
      })),
    },
  ];
  clearChatAttachments();
  stashChatSession(projectPath);
  renderChatPage();
  startChatWaitTicker();

  try {
    // G4: upload pending images first, then send with attachment meta
    let attachments = [];
    if (pendingSnap.length) {
      // restore pending temporarily for upload helper
      state.chatPendingAttachments = pendingSnap;
      try {
        attachments = await uploadPendingAttachments();
      } finally {
        state.chatPendingAttachments = [];
      }
    }
    // Non-blocking for the webview: Tauri command is async + spawn_blocking.
    // User sees "思考中…" bubble; send is disabled only to avoid double-send.
    const resp = await invoke("chat_send_cmd", {
      project: projectPath,
      message: text || (attachments.length ? "（见附图）" : ""),
      sessionId: state.chatSession.session_id || "default",
      attachments: attachments.length ? attachments : null,
    });
    // If user switched project mid-send, still write into that project's cache.
    if (state.selectedPath !== projectPath) {
      const sid = resp.session_id || "default";
      const key = chatCacheKey(projectPath, sid);
      const snap = {
        session_id: sid,
        messages: Array.isArray(resp.messages) ? resp.messages : [],
        draft_plan: resp.draft_plan || null,
        draftPath:
          resp.draft_plan?.saved && resp.draft_plan.path
            ? resp.draft_plan.path
            : state.chatSessions[key]?.draftPath ||
              state.chatSessions[projectPath]?.draftPath ||
              null,
        fake: !!resp.fake,
        envNote: resp.env_note || null,
        busy: false,
        waitStartedAt: 0,
      };
      state.chatSessions[key] = snap;
      state.chatSessions[projectPath] = snap;
    } else {
      applyChatDraftFromSession({
        session_id: resp.session_id,
        messages: resp.messages,
        draft_plan: resp.draft_plan,
      });
      if (resp.draft_plan?.saved && resp.draft_plan.path) {
        state.chatDraftPlan = resp.draft_plan.path;
      }
      // 有 markdown 时记 fake；真实 AI 成功则清掉
      state.chatFake = !!resp.fake;
      // 生产 soft-fallback：env_note 进系统条；forced fake 无 env_note 时用简短 mock 提示
      if (resp.env_note) {
        state.chatEnvNote = String(resp.env_note);
      } else if (resp.fake) {
        state.chatEnvNote = "本地模板联调（CCO_CHAT_FAKE / provider=fake）· 非真实 AI";
      } else {
        state.chatEnvNote = null;
      }
      state.chatProjectPath = projectPath;
      stashChatSession(projectPath);
      // C3: refresh switcher counts/preview after a successful turn
      try {
        await loadChatSessionList();
      } catch (_) {}
    }
    if (resp.fake) {
      if (resp.env_note) {
        toast("本机 Claude CLI 暂不可用，请查看上方环境提示");
      } else {
        toast("当前是本地模板联调（非真实 AI）");
      }
    }
  } catch (e) {
    if (state.selectedPath === projectPath) {
      state.chatSession.messages.push({
        role: "system",
        content: `发送失败：${e?.message || e}`,
      });
      stashChatSession(projectPath);
    }
    toast(String(e?.message || e));
  } finally {
    if (state.selectedPath === projectPath) {
      state.chatBusy = false;
      state.chatWaitStartedAt = 0;
      state.chatStreamText = "";
      stopChatWaitTicker();
      stashChatSession(projectPath);
      renderChatPage();
      input?.focus();
    } else if (state.chatSessions[projectPath]) {
      state.chatSessions[projectPath].busy = false;
      state.chatSessions[projectPath].waitStartedAt = 0;
    }
  }
}

async function saveChatPlan(opts) {
  ensureChatState();
  if (!state.selectedPath) return;
  const draft = state.chatSession?.draft_plan;
  let md = (opts && opts.markdown) || draft?.markdown;
  if (!md) {
    // try extract from last assistant message (nesting-aware; do not cut at ```text)
    const msgs = state.chatSession.messages || [];
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].role === "assistant") {
        const extracted = chatExtractPlanFence(msgs[i].content || "");
        if (extracted) {
          md = extracted;
          break;
        }
      }
    }
  }
  if (!md) {
    toast("还没有可保存的计划草稿，请先让 AI 生成计划");
    return;
  }
  // Overwrite only when re-saving an already-saved draft (H1 未执行可改).
  // Unsaved new draft → planRel null → 新建 chat-*.md；asCopy 强制新建。
  const overwriteRel =
    opts && opts.asCopy
      ? null
      : (opts && opts.planRel) ||
        (draft?.saved && draft?.path ? draft.path : null) ||
        null;
  const plansDir = getPlansDir();
  // G2: one path confirm before write (skip when opts.skipConfirm or asCopy from full-view)
  if (!(opts && opts.skipConfirm)) {
    const previewPath =
      overwriteRel ||
      `${plansDir}/chat-${new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "").slice(0, 13)}.md`;
    const ok = window.confirm(
      overwriteRel
        ? `将覆盖已保存计划：\n${overwriteRel}\n\n确定保存？`
        : `将保存到：\n${previewPath}\n\n确定保存？`
    );
    if (!ok) return null;
  }
  state.chatBusy = true;
  renderChatPage();
  try {
    const resp = await invoke("chat_save_plan_cmd", {
      project: state.selectedPath,
      markdown: md,
      sessionId: state.chatSession.session_id || "default",
      title: (opts && opts.title) || draft?.title || null,
      planRel: overwriteRel || null,
      plansDir: overwriteRel ? null : plansDir,
    });
    state.chatDraftPlan = resp.plan_rel;
    state.chatProjectPath = state.selectedPath;
    if (state.chatSession.draft_plan) {
      state.chatSession.draft_plan.path = resp.plan_rel;
      state.chatSession.draft_plan.saved = true;
      state.chatSession.draft_plan.markdown = md;
    } else {
      state.chatSession.draft_plan = {
        path: resp.plan_rel,
        saved: true,
        markdown: md,
        title: draft?.title || null,
      };
    }
    stashChatSession(state.selectedPath);
    // refresh plans list so chooser + rail see it
    try {
      await loadPlansForPicker();
    } catch (_) {}
    // 刷新列表：右栏打开或在计划管理页时
    if (state.planRailOpen || state.page === "plans") {
      try {
        await loadPlanRail();
      } catch (_) {}
    }
    toast(`计划已保存：${resp.plan_rel}`);
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  } finally {
    state.chatBusy = false;
    stashChatSession(state.selectedPath);
    renderChatPage();
  }
}

async function assignFromChat() {
  ensureChatState();
  if (!state.chatDraftPlan) {
    toast("请先保存计划");
    return;
  }
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(state.chatDraftPlan, {
      source: "chat",
      fakeNote: !!state.chatFake,
    });
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("执行此计划");
    return;
  }
  if (state.chatFake) {
    toast("注意：当前计划来自本地模板（非真实 AI），确认后仍将进入执行");
  }
  try {
    await selectPlan(state.chatDraftPlan);
    showPage("workspace");
    openPlanChooser(true);
    updateChooserAssignState();
    toast("已选中计划 · 确认选项后点「开始拆分」");
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** Ready-bar「打开预览」→ App 内全文 modal（不默认 open_path）. */
async function previewChatPlan() {
  if (!state.chatDraftPlan || !state.selectedPath) return;
  await openPlanFullView(state.chatDraftPlan);
}

/* ══════════════════════════════════════════════
 * H1 — plan-rail list + plan-full-view modal
 * ══════════════════════════════════════════════ */

function planRailTitleFromPath(path) {
  if (typeof planDisplayName === "function") return planDisplayName(path);
  const parts = String(path || "").split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || path || "—";
}

/** H2: alias → shared planExecBadgeInfo (chooser / rail 同一规则) */
function planRailBadgeInfo(item) {
  if (typeof planExecBadgeInfo === "function") return planExecBadgeInfo(item);
  if (!item) return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
  if (item.ever_completed || item.everCompleted) {
    return { label: "已执行", cls: "plan-rail-badge-done", kind: "done" };
  }
  const st = String(item.last_run_status || item.lastRunStatus || "").toLowerCase();
  if (st && ["failed", "aborted", "timeout", "stopped"].includes(st)) {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  if (st && st !== "completed" && st !== "done") {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
}

function planTitleFromMarkdown(md) {
  if (!md) return null;
  // Prefer line-based scan; also handle single-line walls (no \n).
  const text = String(md);
  for (const line of text.split("\n")) {
    const t = line.trim();
    if (t.startsWith("# ")) {
      const title = sanitizePlanTitle(t.slice(2));
      if (title) return title;
    }
    if (t.startsWith("#") && !t.startsWith("##")) {
      const title = sanitizePlanTitle(t.slice(1));
      if (title) return title;
    }
  }
  return null;
}

async function loadPlanRail() {
  ensureChatState();
  if (!state.selectedPath) {
    state.planRailItems = [];
    state.planRailLoading = false;
    renderPlanRail();
    return [];
  }
  state.planRailLoading = true;
  renderPlanRail();
  const root = state.selectedPath;
  try {
    let items = [];
    // Prefer H2 meta when available; fall back to plain path list.
    try {
      const metas = await invoke("get_plan_meta", { project: root });
      if (Array.isArray(metas) && metas.length) {
        if (typeof applyPlanMetaItems === "function") {
          items = applyPlanMetaItems(metas, root);
        } else {
          items = metas.map((m) => ({
            path: normalizePlanPath(m.path, root) || m.path,
            title: m.title || null,
            ever_completed: !!m.ever_completed,
            last_run_status: m.last_run_status || null,
            last_run_id: m.last_run_id || null,
            last_run_finished_at: m.last_run_finished_at || null,
          }));
        }
      }
    } catch (_) {
      /* meta cmd may be absent in older builds — fall through */
    }
    if (!items.length) {
      const plans = (await invoke("get_plans", { project: root })) || [];
      items = (Array.isArray(plans) ? plans : []).map((p) => {
        const path = normalizePlanPath(p, root) || p;
        return {
          path,
          title: null,
          ever_completed: false,
          last_run_status: null,
        };
      });
      if (typeof applyPlanMetaItems === "function") {
        items = applyPlanMetaItems(items, root);
      }
    }
    // Keep only under project
    items = items
      .map((it) => ({
        ...it,
        path: normalizePlanPath(it.path, root) || it.path,
      }))
      .filter((it) => {
        if (typeof isPlanUnderProject === "function") {
          return isPlanUnderProject(it.path, root);
        }
        return !!it.path;
      });
    // Also merge chooser state.plans if longer (manual picks)
    if (Array.isArray(state.plans) && state.plans.length) {
      for (const p of state.plans) {
        const path = normalizePlanPath(p, root) || p;
        if (!items.some((it) => it.path === path)) {
          items.push({
            path,
            title: null,
            ever_completed: false,
            last_run_status: null,
          });
        }
      }
    }
    // E4：全量保留在 planRailItemsAll；默认展示按 plans_dir 过滤
    state.planRailItemsAll = items;
    // Sync chooser path list when rail loads first（仍用全量，换文件可跨夹）
    if (items.length && (!state.plans || !state.plans.length)) {
      state.plans = items.map((it) => it.path);
    } else if (items.length && Array.isArray(state.plans)) {
      for (const it of items) {
        if (it.path && !state.plans.includes(it.path)) {
          state.plans.push(it.path);
        }
      }
    }
    state.planRailItems = items;
  } catch (e) {
    console.warn("loadPlanRail", e);
    state.planRailItems = [];
  } finally {
    state.planRailLoading = false;
    renderPlanRail();
    if (state.page === "plans") {
      try {
        renderPlansMgmtPage();
      } catch (_) {}
    }
  }
  return state.planRailItems;
}

function renderPlanRail() {
  ensureChatState();
  applyPlanRailVisibility();
  syncPlansDirLabels();
  const list = $("#plan-rail-list");
  const empty = $("#plan-rail-empty");
  if (!list) return;
  // G0: rail closed → skip heavy list paint
  if (!state.planRailOpen) return;
  if (typeof syncShowExecutedToggles === "function") syncShowExecutedToggles();
  if (state.planRailLoading) {
    if (empty) empty.hidden = true;
    list.innerHTML =
      '<div class="plan-rail-loading"><span class="spinner sm" aria-hidden="true"></span>扫描计划…</div>';
    return;
  }
  const root = state.selectedPath;
  const selectedPath =
    state.planRailSelected ||
    state.selectedPlan ||
    (state.planFull?.open && state.planFull.path) ||
    state.chatDraftPlan ||
    null;
  const activePath =
    typeof normalizePlanPath === "function" && selectedPath
      ? normalizePlanPath(selectedPath, root) || selectedPath
      : selectedPath;
  // 未保存/草稿 + 当前选中/打开 优先露出（即使已执行也不藏）
  const pinPaths = [
    state.chatDraftPlan,
    state.selectedPlan,
    state.planRailSelected,
    activePath,
    state.planFull?.path,
  ]
    .filter(Boolean)
    .map((p) => (typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p));

  // E4：右栏默认本夹
  const dirParts = partitionByPlansDir(state.planRailItems || [], {
    plansDir: getPlansDir(),
    root,
    pinPaths,
    showOther: false,
  });
  const dirItems = dirParts.primary;
  if (!(state.planRailItems || []).length || !dirItems.length) {
    list.innerHTML = "";
    if (empty) {
      empty.hidden = false;
      empty.textContent = `「${getPlansDir()}/」暂无 · 保存后出现在这里`;
    }
    return;
  }
  if (empty) empty.hidden = true;

  const parts =
    typeof partitionPlanItems === "function"
      ? partitionPlanItems(dirItems, {
          showExecuted: !!state.showExecutedPlans,
          pinPaths,
        })
      : {
          visible: dirItems,
          historyHidden: false,
          historyCount: 0,
        };

  const latestPath = pickLatestPlanPath(parts.visible);
  const latestNorm =
    latestPath && typeof normalizePlanPath === "function"
      ? normalizePlanPath(latestPath, root) || latestPath
      : latestPath;

  const rows = parts.visible.map((it) => {
    const path = it.path || "";
    const rawTitle = it.title || planRailTitleFromPath(path);
    const title = sanitizePlanTitle(rawTitle) || planRailTitleFromPath(path);
    const badge = planRailBadgeInfo(it);
    const norm =
      typeof normalizePlanPath === "function" ? normalizePlanPath(path, root) || path : path;
    const active = norm && activePath && norm === activePath ? " is-active" : "";
    const selected =
      state.planRailSelected &&
      (state.planRailSelected === path || state.planRailSelected === norm)
        ? " is-selected"
        : "";
    const isLatest =
      latestNorm && (norm === latestNorm || path === latestPath) ? " is-latest" : "";
    const latestMark = isLatest
      ? `<span class="plan-latest-tag">最新</span>`
      : "";
    return (
      `<button type="button" class="plan-rail-item${active}${selected}${isLatest}" data-plan-rail="${chatEsc(path)}" title="${chatEsc(path)}">` +
      `<div class="plan-rail-item-title">${chatEsc(title)}${latestMark}</div>` +
      `<div class="plan-rail-item-path">${chatEsc(path)}</div>` +
      `<div class="plan-rail-item-meta"><span class="plan-rail-badge ${badge.cls}">${chatEsc(badge.label)}</span></div>` +
      `</button>`
    );
  });
  if (parts.historyHidden) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">` +
        `已隐藏 ${parts.historyCount} 份已执行 · 勾选「显示已执行」` +
        `</div>`
    );
  }
  list.innerHTML = rows.join("");
}

/** 从路径/时间戳猜「最新」计划（chat-YYYYMMDD-HHMM 优先，否则列表首项）. */
function pickLatestPlanPath(items) {
  if (!Array.isArray(items) || !items.length) return null;
  let best = null;
  let bestKey = "";
  for (const it of items) {
    const p = String(it.path || "");
    const base = p.split(/[/\\]/).pop() || p;
    // chat-20260719-2245.md / cco-plan-...
    const m = base.match(/(\d{8})[-_]?(\d{4,6})?/);
    const key = m ? `${m[1]}${m[2] || "0000"}` : "";
    if (key && key >= bestKey) {
      bestKey = key;
      best = p;
    }
  }
  if (best) return best;
  // fallback: first visible / last in array (often newest scan order)
  return items[0]?.path || null;
}

/** G1: single-click selects plan (no modal). */
function selectPlanRailItem(planPath) {
  ensureChatState();
  if (!planPath || !state.selectedPath) return;
  const root = state.selectedPath;
  const path =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(planPath, root) || planPath
      : planPath;
  state.planRailSelected = path;
  if (typeof selectPlan === "function") {
    try {
      selectPlan(path);
    } catch (_) {
      state.selectedPlan = path;
    }
  } else {
    state.selectedPlan = path;
  }
  renderPlanRail();
  if (state.page === "plans") {
    try {
      renderPlansMgmtPage();
    } catch (_) {}
  }
  if (typeof renderChatReadyBar === "function") renderChatReadyBar();
}

/** G1: double-click opens full view (edit path). */
async function openPlanRailItem(planPath) {
  selectPlanRailItem(planPath);
  await openPlanFullView(planPath);
}

function planFullState() {
  ensureChatState();
  return state.planFull;
}

function closePlanFullView() {
  ensureChatState();
  const pf = state.planFull;
  if (pf?.dirty && (pf.editing || pf.diffing)) {
    const ok = window.confirm("有未保存改动，确定关闭？");
    if (!ok) return;
  }
  state.planFull = {
    open: false,
    path: null,
    title: null,
    markdown: "",
    original: "",
    editing: false,
    dirty: false,
    everCompleted: false,
    lastRunStatus: null,
    saving: false,
    diffing: false,
    diffLeft: "",
    diffRight: "",
  };
  renderPlanFullView();
  renderPlanRail();
}

/**
 * C3/P2-9: line-level LCS diff (left=disk, right=current draft).
 * Pure local; no cloud. Returns rows: {tag:'eq'|'del'|'add', text}.
 */
function computeLineDiff(leftText, rightText) {
  const a = String(leftText || "").replace(/\r\n/g, "\n").split("\n");
  const b = String(rightText || "").replace(/\r\n/g, "\n").split("\n");
  const n = a.length;
  const m = b.length;
  // Cap for UI safety (very large plans): still usable, O(n*m) memory.
  if (n * m > 400_000) {
    // Fallback: simple prefix/suffix + middle as del/add blocks
    let i = 0;
    while (i < n && i < m && a[i] === b[i]) i += 1;
    let j = 0;
    while (j < n - i && j < m - i && a[n - 1 - j] === b[m - 1 - j]) j += 1;
    const rows = [];
    for (let k = 0; k < i; k++) rows.push({ tag: "eq", text: a[k] });
    for (let k = i; k < n - j; k++) rows.push({ tag: "del", text: a[k] });
    for (let k = i; k < m - j; k++) rows.push({ tag: "add", text: b[k] });
    for (let k = n - j; k < n; k++) rows.push({ tag: "eq", text: a[k] });
    return rows;
  }
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i] === b[j]) dp[i][j] = dp[i + 1][j + 1] + 1;
      else dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const rows = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      rows.push({ tag: "eq", text: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      rows.push({ tag: "del", text: a[i] });
      i += 1;
    } else {
      rows.push({ tag: "add", text: b[j] });
      j += 1;
    }
  }
  while (i < n) {
    rows.push({ tag: "del", text: a[i] });
    i += 1;
  }
  while (j < m) {
    rows.push({ tag: "add", text: b[j] });
    j += 1;
  }
  return rows;
}

function renderPlanDiffHtml(rows) {
  if (!rows || !rows.length) {
    return `<div class="plan-diff-row eq empty"><span class="mark"> </span><span class="txt">（两边皆空）</span></div>`;
  }
  return rows
    .map((r) => {
      const mark = r.tag === "del" ? "−" : r.tag === "add" ? "+" : " ";
      const txt = r.text === "" ? " " : chatEsc(r.text);
      return `<div class="plan-diff-row ${r.tag}"><span class="mark">${mark}</span><span class="txt">${txt}</span></div>`;
    })
    .join("");
}

function openPlanFullDiff() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  // Left = disk/original; Right = current editor/draft
  const editor = $("#plan-full-editor");
  const current =
    pf.editing && editor ? editor.value : pf.markdown != null ? pf.markdown : pf.original || "";
  pf.diffLeft = pf.original || "";
  pf.diffRight = current || "";
  pf.diffing = true;
  // Leave edit mode chrome but keep draft text
  if (pf.editing && editor) {
    pf.markdown = editor.value;
    pf.dirty = pf.markdown !== pf.original;
  }
  pf.editing = false;
  renderPlanFullView();
}

function closePlanFullDiff() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  pf.diffing = false;
  renderPlanFullView();
}

/**
 * Adopt left (disk) or right (current) into draft, then enter edit mode.
 * Does NOT auto-save; user still uses chat_save_plan via Save.
 */
function adoptPlanDiffSide(side) {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.diffing) return;
  if (pf.everCompleted && side === "left") {
    // adopting disk into edit is fine (no overwrite yet); keep as-is
  }
  const text = side === "left" ? pf.diffLeft || "" : pf.diffRight || "";
  pf.markdown = text;
  pf.dirty = text !== (pf.original || "");
  pf.diffing = false;
  pf.editing = true;
  renderPlanFullView();
  const editor = $("#plan-full-editor");
  if (editor) {
    editor.value = text;
    editor.focus();
  }
  toast(side === "left" ? "已采用磁盘稿（未保存）" : "已保留当前稿（未保存）");
}

async function openPlanFullView(planPath, meta) {
  ensureChatState();
  if (!state.selectedPath || !planPath) return;
  const root = state.selectedPath;
  const path = normalizePlanPath(planPath, root) || planPath;
  // Resolve meta from rail if not provided
  const rail = (state.planRailItems || []).find((it) => it.path === path) || meta || {};
  const everCompleted = !!(rail.ever_completed || rail.everCompleted);
  const lastRunStatus = rail.last_run_status || rail.lastRunStatus || null;

  let markdown = "";
  try {
    markdown = await invoke("read_plan_md_cmd", {
      project: root,
      plan: path,
    });
  } catch (e) {
    toast(String(e?.message || e));
    return;
  }
  const title =
    rail.title || planTitleFromMarkdown(markdown) || planRailTitleFromPath(path);
  state.planFull = {
    open: true,
    path,
    title,
    markdown: String(markdown || ""),
    original: String(markdown || ""),
    editing: false,
    dirty: false,
    everCompleted,
    lastRunStatus,
    saving: false,
    diffing: false,
    diffLeft: "",
    diffRight: "",
  };
  renderPlanFullView();
  renderPlanRail();
}

function renderPlanFullView() {
  ensureChatState();
  const modal = $("#plan-full-view");
  if (!modal) return;
  const pf = state.planFull || { open: false };
  modal.hidden = !pf.open;
  if (!pf.open) return;

  const titleEl = $("#plan-full-title");
  const pathEl = $("#plan-full-path");
  const badgeEl = $("#plan-full-status-badge");
  const dirtyEl = $("#plan-full-dirty");
  const viewBody = $("#plan-full-view-body");
  const editBody = $("#plan-full-edit-body");
  const diffBody = $("#plan-full-diff-body");
  const mdEl = $("#plan-full-md");
  const editor = $("#plan-full-editor");
  const editHint = $("#plan-full-edit-hint");
  const diffEl = $("#plan-full-diff");
  const diffStats = $("#plan-diff-stats");

  const btnEdit = $("#btn-plan-full-edit");
  const btnDiff = $("#btn-plan-full-diff");
  const btnDiffClose = $("#btn-plan-full-diff-close");
  const btnDiffLeft = $("#btn-plan-full-diff-left");
  const btnDiffRight = $("#btn-plan-full-diff-right");
  const btnSave = $("#btn-plan-full-save");
  const btnSaveAs = $("#btn-plan-full-save-as");
  const btnCancel = $("#btn-plan-full-cancel-edit");
  const btnAssign = $("#btn-plan-full-assign");

  if (titleEl) titleEl.textContent = pf.title || planRailTitleFromPath(pf.path) || "计划全文";
  if (pathEl) pathEl.textContent = pf.path || "—";

  const badge = planRailBadgeInfo({
    ever_completed: pf.everCompleted,
    last_run_status: pf.lastRunStatus,
  });
  if (badgeEl) {
    badgeEl.textContent = badge.label;
    badgeEl.className = `plan-rail-badge ${badge.cls}`;
  }
  if (dirtyEl) dirtyEl.hidden = !pf.dirty;

  if (pf.diffing) {
    if (viewBody) viewBody.hidden = true;
    if (editBody) editBody.hidden = true;
    if (diffBody) diffBody.hidden = false;
    const rows = computeLineDiff(pf.diffLeft || "", pf.diffRight || "");
    let add = 0;
    let del = 0;
    for (const r of rows) {
      if (r.tag === "add") add += 1;
      else if (r.tag === "del") del += 1;
    }
    if (diffStats) {
      diffStats.textContent =
        add === 0 && del === 0 ? "无差异" : `+${add} / −${del} 行`;
    }
    if (diffEl) diffEl.innerHTML = renderPlanDiffHtml(rows);
  } else if (pf.editing) {
    if (viewBody) viewBody.hidden = true;
    if (editBody) editBody.hidden = false;
    if (diffBody) diffBody.hidden = true;
    if (editor && document.activeElement !== editor) {
      editor.value = pf.markdown || "";
    }
    if (editHint) {
      editHint.textContent = pf.everCompleted
        ? "该计划已有完成的执行记录，禁止原地改写；请「另存副本」后再改。"
        : "未执行计划可直接覆盖保存；保存后路径与就绪条一致。";
    }
  } else {
    if (viewBody) viewBody.hidden = false;
    if (editBody) editBody.hidden = true;
    if (diffBody) diffBody.hidden = true;
    if (mdEl) mdEl.textContent = pf.markdown || "";
  }

  // Buttons
  if (btnEdit) {
    // In view mode: show Edit; in edit/diff mode hide (use cancel / exit)
    btnEdit.hidden = !!pf.editing || !!pf.diffing;
    btnEdit.disabled = !!pf.saving;
    btnEdit.textContent = pf.everCompleted ? "另存副本再改" : "编辑";
    btnEdit.title = pf.everCompleted
      ? "已执行计划不可原地改；将复制为新计划后编辑"
      : "在 App 内编辑计划正文";
  }
  if (btnDiff) {
    // Available in view + edit (compare disk original vs current draft)
    btnDiff.hidden = !!pf.diffing;
    btnDiff.disabled = !!pf.saving;
    btnDiff.title = "对比磁盘稿与当前草稿（本机）";
  }
  if (btnDiffClose) {
    btnDiffClose.hidden = !pf.diffing;
    btnDiffClose.disabled = !!pf.saving;
  }
  if (btnDiffLeft) {
    btnDiffLeft.hidden = !pf.diffing;
    btnDiffLeft.disabled = !!pf.saving;
  }
  if (btnDiffRight) {
    btnDiffRight.hidden = !pf.diffing;
    btnDiffRight.disabled = !!pf.saving;
  }
  if (btnSave) {
    // Save overwrite: only when editing && !everCompleted
    btnSave.hidden = !pf.editing || !!pf.everCompleted || !!pf.diffing;
    btnSave.disabled = !!pf.saving || !pf.dirty;
    btnSave.textContent = pf.saving ? "保存中…" : "保存";
  }
  if (btnSaveAs) {
    // Save-as always available in edit mode (and primary path for completed)
    btnSaveAs.hidden = !pf.editing || !!pf.diffing;
    btnSaveAs.disabled = !!pf.saving;
    btnSaveAs.textContent = pf.saving ? "保存中…" : "另存副本";
  }
  if (btnCancel) {
    btnCancel.hidden = !pf.editing || !!pf.diffing;
    btnCancel.disabled = !!pf.saving;
  }
  if (btnAssign) {
    // 未保存改动禁止执行
    const canAssign = !!pf.path && !pf.dirty && !pf.editing && !pf.diffing;
    btnAssign.disabled = !canAssign || !!pf.saving;
    btnAssign.textContent = "执行此计划";
    btnAssign.title = pf.dirty
      ? "请先保存改动再执行"
      : pf.editing || pf.diffing
        ? "请先保存或取消编辑再执行"
        : "带上该计划进入执行选项";
  }
}

function beginPlanFullEdit() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  pf.diffing = false;
  if (pf.everCompleted) {
    // 已执行：走另存副本路径（先进入编辑，保存只能 save-as）
    pf.editing = true;
    pf.dirty = false;
    // Seed editor with current text; user edits then 另存副本
    renderPlanFullView();
    const editor = $("#plan-full-editor");
    editor?.focus();
    toast("已执行计划不可覆盖原文件，请编辑后点「另存副本」");
    return;
  }
  pf.editing = true;
  pf.dirty = false;
  renderPlanFullView();
  $("#plan-full-editor")?.focus();
}

function cancelPlanFullEdit() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  if (pf.dirty) {
    const ok = window.confirm("放弃未保存改动？");
    if (!ok) return;
  }
  pf.markdown = pf.original;
  pf.editing = false;
  pf.diffing = false;
  pf.dirty = false;
  renderPlanFullView();
}

function onPlanFullEditorInput() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.editing) return;
  const editor = $("#plan-full-editor");
  if (!editor) return;
  pf.markdown = editor.value;
  pf.dirty = pf.markdown !== pf.original;
  // Lightweight dirty badge + assign disable without full re-render (keeps caret)
  const dirtyEl = $("#plan-full-dirty");
  if (dirtyEl) dirtyEl.hidden = !pf.dirty;
  const btnSave = $("#btn-plan-full-save");
  if (btnSave && !pf.everCompleted) btnSave.disabled = !pf.dirty || !!pf.saving;
  const btnAssign = $("#btn-plan-full-assign");
  if (btnAssign) {
    btnAssign.disabled = true;
    btnAssign.title = "请先保存改动再执行";
  }
}

async function savePlanFullView({ asCopy = false } = {}) {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !state.selectedPath) return;
  if (!pf.editing) return;
  const editor = $("#plan-full-editor");
  const md = (editor?.value ?? pf.markdown ?? "").trim();
  if (!md) {
    toast("计划内容为空，无法保存");
    return;
  }
  if (pf.everCompleted && !asCopy) {
    toast("已执行计划禁止原地覆盖，请「另存副本」");
    return;
  }
  if (!asCopy && !pf.dirty && md === (pf.original || "").trim()) {
    toast("没有改动");
    return;
  }
  pf.saving = true;
  renderPlanFullView();
  try {
    const resp = await invoke("chat_save_plan_cmd", {
      project: state.selectedPath,
      markdown: md,
      sessionId: state.chatSession?.session_id || "default",
      title: planTitleFromMarkdown(md) || pf.title || null,
      planRel: asCopy ? null : pf.path,
      plansDir: asCopy ? getPlansDir() : null,
    });
    const newPath = resp.plan_rel;
    // Sync ready-bar path so CTA matches
    state.chatDraftPlan = newPath;
    if (state.chatSession) {
      if (!state.chatSession.draft_plan) {
        state.chatSession.draft_plan = {
          path: newPath,
          saved: true,
          markdown: md,
          title: planTitleFromMarkdown(md),
        };
      } else {
        state.chatSession.draft_plan.path = newPath;
        state.chatSession.draft_plan.saved = true;
        state.chatSession.draft_plan.markdown = md;
      }
    }
    stashChatSession(state.selectedPath);

    pf.path = newPath;
    pf.markdown = md;
    pf.original = md;
    pf.dirty = false;
    pf.editing = false;
    pf.title = planTitleFromMarkdown(md) || planRailTitleFromPath(newPath);
    // 副本视为未执行
    if (asCopy) {
      pf.everCompleted = false;
      pf.lastRunStatus = null;
    }
    try {
      await loadPlansForPicker();
    } catch (_) {}
    try {
      await loadPlanRail();
    } catch (_) {}
    toast(asCopy ? `已另存副本：${newPath}` : `已保存：${newPath}`);
    renderChatReadyBar();
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    pf.saving = false;
    renderPlanFullView();
    renderPlanRail();
  }
}

async function assignFromPlanFullView() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.path) {
    toast("请先打开计划");
    return;
  }
  if (pf.editing || pf.dirty) {
    toast("请先保存改动再执行");
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("执行此计划");
    return;
  }
  state.chatDraftPlan = pf.path;
  if (state.chatSession) {
    if (!state.chatSession.draft_plan) {
      state.chatSession.draft_plan = {
        path: pf.path,
        saved: true,
        markdown: pf.markdown || null,
        title: pf.title || null,
      };
    } else {
      state.chatSession.draft_plan.path = pf.path;
      state.chatSession.draft_plan.saved = true;
    }
  }
  stashChatSession(state.selectedPath);
  closePlanFullView();
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(pf.path, { source: "full-view" });
    return;
  }
  try {
    await selectPlan(pf.path);
    showPage("workspace");
    openPlanChooser(true);
    updateChooserAssignState();
    toast("已选中计划 · 确认选项后点「开始拆分」");
  } catch (e) {
    toast(String(e?.message || e));
  }
}
