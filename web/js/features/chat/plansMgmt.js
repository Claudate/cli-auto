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
import { renderMarkdown } from "../../shared/markdown.js";
import {
  groupPlanItemsByWave,
  renderWaveGroupHtml,
  isWaveIndexPath,
  waveSiblingPlans,
  waveDirKeyFromPath,
} from "./chatWavePlans.js";
import {
  buildWaveOverview,
  renderWaveOverviewHtml,
} from "./chatWaveOverview.js";
import {
  loadWaveJobsByPath,
  confirmWaveBatchSerial as confirmWaveBatchSerialImpl,
  splitNextInWave as splitNextInWaveImpl,
  assignFromPlansMgmtPath,
} from "./chatWaveBatch.js";

/**
 * Drop selection pointers whose source .md is gone (no ghost list rows).
 * @param {string} root
 */
async function dropMissingPlanPointers(root) {
  if (!root) return;
  const keys = ["selectedPlan", "chatDraftPlan", "planRailSelected"];
  for (const key of keys) {
    const raw = state[key];
    if (!raw) continue;
    const path =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(raw, root) || raw
        : raw;
    const ok = await chatApi.planMdExists(root, path);
    if (!ok) {
      state[key] = null;
    }
  }
}

/**
 * Ensure selected / draft plans appear when list_plans filename filter missed them.
 * **Does not** re-inject SQLite split-index paths — source deleted ⇒ list drops.
 * Only pins after disk probe succeeds.
 * @param {Array} items
 * @param {string[]} pinPaths
 * @param {string} root
 */
async function ensurePinnedPlanItems(items, pinPaths, root) {
  const list = Array.isArray(items) ? items.slice() : [];
  const seen = new Set(
    list.map((it) => {
      const p = it?.path || it;
      return typeof normalizePlanPath === "function"
        ? normalizePlanPath(p, root) || p
        : p;
    })
  );
  // Explicit user/session pins only — no planSplitByPath ghosts
  const candidates = [
    ...(pinPaths || []),
    state.chatDraftPlan,
    state.selectedPlan,
    state.planRailSelected,
  ].filter(Boolean);
  for (const raw of candidates) {
    const path =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(raw, root) || raw
        : raw;
    if (!path || seen.has(path)) continue;
    if (
      typeof host.isPlanUnderProject === "function" &&
      !host.isPlanUnderProject(path, root)
    ) {
      continue;
    }
    // Source must exist on disk; deleted chat-*.md must not reappear
    // eslint-disable-next-line no-await-in-loop
    const ok = await chatApi.planMdExists(root, path);
    if (!ok) continue;
    seen.add(path);
    const meta =
      typeof host.planMetaForPath === "function"
        ? host.planMetaForPath(path, root)
        : { path, title: null, ever_completed: false, last_run_status: null };
    list.unshift({
      ...meta,
      path,
      title: meta.title || null,
    });
  }
  return list;
}

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
  // 源文件已删 → 清掉选中指针，避免幽灵「已拆分」
  await dropMissingPlanPointers(state.selectedPath);
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
  // SQLite split index first so badges /「查看拆分结果」work without in-memory planJob
  try {
    if (typeof host.loadPlanSplitIndex === "function") {
      await host.loadPlanSplitIndex(state.selectedPath);
    } else if (typeof window.loadPlanSplitIndex === "function") {
      await window.loadPlanSplitIndex(state.selectedPath);
    }
  } catch (_) {}
  await host.loadPlanRail();
  if (selected) {
    host.selectPlanRailItem(selected);
    try {
      if (typeof selectPlan === "function") selectPlan(selected);
      state.chatDraftPlan = selected;
    } catch (_) {}
  }
  await renderPlansMgmtPage();
  if (!selected) {
    toast("从左侧选一份计划，再点「拆成步骤」");
  }
}

export async function renderPlansMgmtPage() {
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
  if (root) {
    await dropMissingPlanPointers(root);
  }
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

  // 仅 pin 磁盘仍存在的选中/草稿（不 pin 已删源文件的拆分索引）
  dirItems = await ensurePinnedPlanItems(dirItems, pinPaths, root);

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

  const rowHtml = (it) => {
    const path = it.path || "";
    const fileName =
      String(path)
        .split("/")
        .filter(Boolean)
        .pop() || path;
    const rawTitle = it.title || host.planRailTitleFromPath(path);
    let title = sanitizePlanTitle(rawTitle) || host.planRailTitleFromPath(path);
    if (isWaveIndexPath(path)) title = title || "本波索引";
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
    const indexCls = isWaveIndexPath(path) ? " is-wave-index" : "";
    return (
      `<button type="button" class="plans-mgmt-item${selected}${isLatest}${indexCls}" data-plans-mgmt="${chatEsc(path)}" title="${chatEsc(path)}">` +
      `<div class="plans-mgmt-item-title">${chatEsc(title)}${latestMark}</div>` +
      `<div class="plans-mgmt-item-path" title="${chatEsc(path)}">${chatEsc(fileName)}</div>` +
      `<div class="plans-mgmt-item-meta"><span class="plan-rail-badge ${badge.cls}">${chatEsc(badge.label)}</span></div>` +
      `</button>`
    );
  };

  // W2-5: group plans/wave-* under 本波 heads; flat list keeps other plans
  const { waves, flat } = groupPlanItemsByWave(parts.visible);
  const rows = [];
  for (const g of waves) {
    rows.push(renderWaveGroupHtml(g, rowHtml, chatEsc));
  }
  for (const it of flat) {
    rows.push(rowHtml(it));
  }
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
    markdown = await chatApi.readPlanMd(root, path);
  } catch (e) {
    // 源文件已删：从选中/列表指针清掉，不在详情里挂幽灵
    const msg = String(e?.message || e || "");
    if (/not found|No such file|无法找到|不存在/i.test(msg)) {
      if (state.planRailSelected === path || state.selectedPlan === path) {
        state.planRailSelected = null;
        if (state.selectedPlan === path) state.selectedPlan = null;
        if (state.chatDraftPlan === path) state.chatDraftPlan = null;
      }
      // Drop from rail items so list no longer shows it
      if (Array.isArray(state.planRailItems)) {
        state.planRailItems = state.planRailItems.filter((it) => {
          const p = it?.path || it;
          return p !== path;
        });
      }
      if (Array.isArray(state.plans)) {
        state.plans = state.plans.filter((p) => p !== path);
      }
      toast("源计划文件已删除，已从列表移除");
      renderPlansMgmtPage();
      return;
    }
    markdown = `（无法读取：这份计划文件打不开。可点「刷新」或换一份计划。）`;
  }
  const isIndex = isWaveIndexPath(path);
  const title =
    (isIndex ? "本波索引" : null) ||
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
    badgeEl.textContent = isIndex ? "索引" : badge.label;
    badgeEl.className = `plan-rail-badge ${isIndex ? "plan-rail-badge-pending" : badge.cls}`;
  }
  // 预览态按 Markdown 渲染（非编辑）；全文勿再 slice——中长计划会被砍掉后半
  if (bodyEl) {
    const full = String(markdown || "");
    bodyEl.classList.add("md-body");
    let html = renderMarkdown(full);
    // W2-5 / W3: wave overview + siblings
    const waveKey = waveDirKeyFromPath(path);
    const pool = state.planRailItems || [];
    if (waveKey) {
      const jobsByPath = await loadWaveJobsByPath(root, pool, waveKey);
      const ov = buildWaveOverview({
        path,
        allItems: pool,
        splitByPath: state.planSplitByPath || {},
        jobsByPath,
        norm: (p) =>
          typeof normalizePlanPath === "function"
            ? normalizePlanPath(p, root) || p
            : p,
      });
      const ovHtml = renderWaveOverviewHtml(ov, chatEsc);
      const sibs = waveSiblingPlans(path, pool);
      const others = isIndex
        ? sibs
        : sibs.filter((s) => {
            const sp = s.path || "";
            const n =
              typeof normalizePlanPath === "function"
                ? normalizePlanPath(sp, root) || sp
                : sp;
            return n !== path && sp !== path;
          });
      const links = others
        .map((s) => {
          const sp = s.path || "";
          const st =
            sanitizePlanTitle(s.title) || host.planRailTitleFromPath(sp);
          return (
            `<button type="button" class="linkish plans-wave-sib" data-plans-mgmt="${chatEsc(
              sp
            )}">${chatEsc(st)}</button>`
          );
        })
        .join(" · ");
      const note = isIndex
        ? `<p class="plans-wave-note muted">本波索引只给人核对；用总览里的执行计划「拆成步骤」。重拆一份不会取消同波其它。</p>`
        : `<p class="plans-wave-note muted">失败可只重拆这一份（按路径隔离）。${
            links ? ` 同波：${links}` : ""
          }</p>`;
      html = ovHtml + note + html;
    }
    bodyEl.innerHTML = html;
    bodyEl.dataset.chars = String(full.length);
  }
  if (btnAssign) {
    // INDEX is not executable — pick an execution plan instead
    btnAssign.disabled = !path || isIndex;
    btnAssign.dataset.plan = isIndex ? "" : path;
    btnAssign.title = isIndex
      ? "索引不能直接拆步 · 请选同波某份执行计划"
      : "把计划拆成可执行步骤";
    if (isIndex) btnAssign.textContent = "请选执行计划";
    else if (btnAssign.dataset.defaultLabel) {
      btnAssign.textContent = btnAssign.dataset.defaultLabel;
    } else {
      btnAssign.dataset.defaultLabel = btnAssign.textContent || "拆成步骤";
      btnAssign.textContent = btnAssign.dataset.defaultLabel;
    }
  }
  const btnPreview = $("#btn-plans-preview");
  if (btnPreview) btnPreview.dataset.plan = path;
  // shell-chrome C1：已有拆分 →「查看拆分结果」
  // Prefer SQLite index (planSplitByPath); fall back to in-memory planJob match.
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
    const memMatch =
      !!job &&
      !!path &&
      jobPath &&
      (jobPath === path || String(jobPath) === String(path)) &&
      ["planning", "planned", "confirmed", "running", "done"].includes(st);
    const idx =
      (typeof host.planSplitForPath === "function"
        ? host.planSplitForPath(path)
        : null) ||
      (state.planSplitByPath &&
        (state.planSplitByPath[path] ||
          state.planSplitByPath[
            typeof normalizePlanPath === "function"
              ? normalizePlanPath(path, root) || path
              : path
          ]));
    const hasSplit = memMatch || !!idx;
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
/** 计划管理页主 CTA → 统一执行入口（E1） */
export async function assignFromPlansMgmt() {
  const path =
    $("#btn-plans-assign")?.dataset?.plan ||
    state.planRailSelected ||
    state.selectedPlan;
  await assignFromPlansMgmtPath(path);
}

/** W3 re-export — batch lives in chatWaveBatch (plansMgmt line budget). */
export async function confirmWaveBatchSerial(waveKey) {
  return confirmWaveBatchSerialImpl(waveKey, {
    after: () => renderPlansMgmtPage(),
  });
}

export async function splitNextInWave(waveKey) {
  return splitNextInWaveImpl(waveKey);
}

/** shell-chrome C1：从计划管理回看拆分台（只读/可再规划） */
export async function viewSplitFromPlansMgmt() {
  const path =
    $("#btn-plans-view-split")?.dataset?.plan ||
    state.planRailSelected ||
    state.selectedPlan;
  if (!path) {
    toast("请先选中一份计划");
    return;
  }
  host.selectPlanRailItem?.(path);
  state.chatDraftPlan = path;
  state.selectedPlan = path;

  // Memory job only counts if it is for this plan path
  const root = state.selectedPath;
  const job = state.planJob;
  const jobPath =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(job?.plan_path || job?.planPath || "", root) ||
        job?.plan_path ||
        job?.planPath ||
        ""
      : job?.plan_path || job?.planPath || "";
  const norm =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(path, root) || path
      : path;
  const memOk =
    !!job &&
    jobPath &&
    (jobPath === norm || String(jobPath) === String(path)) &&
    ["planning", "planned", "confirmed"].includes(
      String(job.status || "").toLowerCase()
    );

  if (!memOk) {
    // SQLite / disk restore by plan path
    const restore =
      typeof host.tryRestorePlanJobForPlan === "function"
        ? host.tryRestorePlanJobForPlan
        : typeof window.tryRestorePlanJobForPlan === "function"
          ? window.tryRestorePlanJobForPlan
          : null;
    if (!restore) {
      toast("还没有拆分结果，请先点「拆成步骤」");
      return;
    }
    const ok = await restore(path);
    if (!ok) return;
  }

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

