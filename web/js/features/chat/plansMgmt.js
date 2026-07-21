/**
 * [INPUT]: legacy · planDir · chatFormat · host rail/full
 * [OUTPUT]: 计划管理页
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
  getPlansDir, syncPlansDirLabels, partitionByPlansDir, promptPlansDir,
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

  const latestPath = host.pickLatestPlanPath(parts.visible);
  const latestNorm =
    latestPath && typeof normalizePlanPath === "function"
      ? normalizePlanPath(latestPath, root) || latestPath
      : latestPath;

  const rows = parts.visible.map((it) => {
    const path = it.path || "";
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

