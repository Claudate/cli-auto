/**
 * [INPUT]: legacy · planDir · chatFormat · host rail/full
 * [OUTPUT]: 计划管理页（列表 + 详情；选中文件夹/文件加载）
 * [POS]: A5-2a features/chat/plansMgmt.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state, $, toast, showPage, normalizePlanPath, selectPlan,
  startExecuteFromSelection, openPlanChooser, renderPlanPicker,
  partitionPlanItems, syncShowExecutedToggles,
} from "./legacy.js";
import { host } from "./host.js";
import { ensureChatState, stashChatSession, sanitizePlanTitle } from "./chatState.js";
import {
  syncPlansDirLabels,
  getPlansMgmtScopeDir,
  partitionByPlansDir,
} from "./planDir.js";
import { chatEsc } from "./chatFormat.js";
import * as chatApi from "./chatApi.js";

export async function openPlanManagement() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (state.page === "chat" && state.chatProjectPath) {
    try {
      stashChatSession(state.chatProjectPath || state.selectedPath);
    } catch (_) {}
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
  await host.loadPlanRail();
  if (selected) {
    host.selectPlanRailItem(selected);
    try {
      if (typeof selectPlan === "function") selectPlan(selected);
      state.chatDraftPlan = selected;
    } catch (_) {}
  }
  renderPlansMgmtPage();
  if (!selected) {
    toast("从左侧选一份计划，再点「拆成步骤」");
  }
}

export function renderPlansMgmtPage() {
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

  // 选中文件夹 → 严格过滤到该夹（不带 pin 外溢）；未选则项目内全量
  const scopeDir = getPlansMgmtScopeDir();
  let dirItems = state.planRailItems || [];
  if (scopeDir) {
    const dirParts = partitionByPlansDir(dirItems, {
      plansDir: scopeDir,
      root,
      pinPaths: [],
      showOther: false,
    });
    dirItems = dirParts.primary;
  }

  if (!dirItems.length) {
    list.innerHTML = "";
    if (empty) {
      empty.hidden = false;
      empty.textContent = scopeDir
        ? `「${scopeDir}/」暂无计划 · 换文件夹或选中文件`
        : "暂无计划 · 用上方「选中文件夹」或「选中文件」加载";
    }
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

  const latestPath = host.pickLatestPlanPath(parts.visible);
  const latestNorm =
    latestPath && typeof normalizePlanPath === "function"
      ? normalizePlanPath(latestPath, root) || latestPath
      : latestPath;

  const rows = parts.visible.map((it) => {
    const path = it.path || "";
    const fileName =
      String(path)
        .split("/")
        .filter(Boolean)
        .pop() || path;
    const rawTitle = it.title || host.planRailTitleFromPath(path);
    const title = sanitizePlanTitle(rawTitle) || host.planRailTitleFromPath(path);
    const badge = host.planRailBadgeInfo(it);
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
      `<div class="plans-mgmt-item-path" title="${chatEsc(path)}">${chatEsc(fileName)}</div>` +
      `<div class="plans-mgmt-item-meta"><span class="plan-rail-badge ${badge.cls}">${chatEsc(badge.label)}</span></div>` +
      `</button>`
    );
  });
  if (parts.historyHidden) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">已隐藏 ${parts.historyCount} 份已执行 · 勾选「显示已执行」</div>`
    );
  }
  list.innerHTML = rows.join("");

  const pool = dirItems;
  const selItem =
    pool.find((it) => {
      const p = it.path || "";
      const n =
        typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p;
      return n === activePath || p === activePath;
    }) || null;
  renderPlansMgmtDetail(selItem || (activePath ? { path: activePath } : null));
}

export async function renderPlansMgmtDetail(item) {
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
    markdown = await chatApi.readPlanMd(root, path );
  } catch (e) {
    markdown = `（无法读取：${e?.message || e}）`;
  }
  const title =
    sanitizePlanTitle(item.title) ||
    host.planTitleFromMarkdown(markdown) ||
    host.planRailTitleFromPath(path);
  const badge = host.planRailBadgeInfo(item);

  if (titleEl) titleEl.textContent = title || "—";
  // 路径 + 文件名并排，避免只靠中文标题找不到 ux-nondev-*.md
  if (pathEl) {
    const base = String(path || "")
      .split("/")
      .filter(Boolean)
      .pop();
    pathEl.textContent =
      base && base !== path ? `${path}  ·  ${base}` : path || "—";
    pathEl.title = path || "";
  }
  if (badgeEl) {
    badgeEl.textContent = badge.label;
    badgeEl.className = `plan-rail-badge ${badge.cls}`;
  }
  // 全文展示：勿再 slice(12000)——中长计划（如 ux-nondev-landing）会被砍掉后半
  if (bodyEl) {
    const full = String(markdown || "");
    bodyEl.textContent = full;
    bodyEl.dataset.chars = String(full.length);
  }
  if (btnAssign) {
    btnAssign.disabled = !path;
    btnAssign.dataset.plan = path;
  }
  const btnPreview = $("#btn-plans-preview");
  if (btnPreview) btnPreview.dataset.plan = path;
  // shell-chrome C1：已有拆分（当前 job 指向该计划）→「查看拆分结果」
  const btnViewSplit = $("#btn-plans-view-split");
  if (btnViewSplit) {
    const job = state.planJob;
    const jobPath =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(job?.plan_path || job?.planPath || "", root) ||
          job?.plan_path ||
          job?.planPath ||
          ""
        : job?.plan_path || job?.planPath || "";
    const st = String(job?.status || "").toLowerCase();
    const hasSplit =
      !!job &&
      !!path &&
      jobPath &&
      (jobPath === path || String(jobPath) === String(path)) &&
      ["planned", "confirmed", "running", "done"].includes(st);
    btnViewSplit.hidden = !hasSplit;
    btnViewSplit.dataset.plan = path || "";
    btnViewSplit.title = hasSplit
      ? "查看拆分结果（可重新规划）"
      : "暂无该计划的拆分结果";
  }
}

/** 计划管理页：单击选中并刷新详情 */
export function selectPlansMgmtItem(planPath) {
  host.selectPlanRailItem(planPath);
  renderPlansMgmtPage();
}

/** 计划管理页：双击 / 全文编辑 */
export async function openPlansMgmtItem(planPath) {
  host.selectPlanRailItem(planPath);
  await host.openPlanFullView(planPath);
  if (state.page === "plans") renderPlansMgmtPage();
}

/** 计划管理页主 CTA → 统一执行入口（E1） */
export async function assignFromPlansMgmt() {
  const path =
    $("#btn-plans-assign")?.dataset?.plan ||
    state.planRailSelected ||
    state.selectedPlan;
  if (!path) {
    toast("请先选中一份计划");
    return;
  }
  host.selectPlanRailItem(path);
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
    toast("已选中计划 · 确认选项后点「拆成步骤」");
  } catch (e) {
    toast(String(e?.message || e || "无法打开执行选项"));
  }
}

/** shell-chrome C1：从计划管理回看拆分台（只读/可再规划） */
export function viewSplitFromPlansMgmt() {
  const path =
    $("#btn-plans-view-split")?.dataset?.plan ||
    state.planRailSelected ||
    state.selectedPlan;
  if (!path) {
    toast("请先选中一份计划");
    return;
  }
  if (!state.planJob) {
    toast("还没有拆分结果，请先点「拆成步骤」");
    return;
  }
  host.selectPlanRailItem?.(path);
  state.chatDraftPlan = path;
  state.selectedPlan = path;
  if (typeof window.showSplitPlanConfirm === "function") {
    window.showSplitPlanConfirm({ keepReturn: true });
    return;
  }
  if (typeof host.showSplitPlanConfirm === "function") {
    host.showSplitPlanConfirm({ keepReturn: true });
    return;
  }
  showPage("workspace");
  state.phase = "confirm";
  try {
    host.renderPhasePanels?.();
    host.renderPlanPicker?.();
  } catch (_) {}
}

