/**
 * [INPUT]: legacy · gateway · planDir · host mgmt/full/ready
 * [OUTPUT]: load plan meta/list · select · badge（聊天右栏 DOM 已撤；paint 仅 no-op）
 * [POS]: A5-2a features/chat/planRail.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  normalizePlanPath,
  planDisplayName,
  planExecBadgeInfo,
  applyPlanMetaItems,
  partitionPlanItems,
  isPlanUnderProject,
  selectPlan,
  syncShowExecutedToggles,
} from "./legacy.js";
import gateway from "../../shared/gateway.js";
import { host } from "./host.js";
import { ensureChatState, sanitizePlanTitle } from "./chatState.js";
import {
  getPlansDir,
  applyPlanRailVisibility,
  syncPlansDirLabels,
  partitionByPlansDir,
} from "./planDir.js";
import { chatEsc } from "./chatFormat.js";

export function planRailTitleFromPath(path) {
  if (typeof planDisplayName === "function") return planDisplayName(path);
  const parts = String(path || "").split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || path || "—";
}

/** H2: alias → shared planExecBadgeInfo (chooser / rail 同一规则) */
export function planRailBadgeInfo(item) {
  if (typeof planExecBadgeInfo === "function") return planExecBadgeInfo(item);
  if (!item) return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
  if (item.ever_completed || item.everCompleted) {
    return { label: "已执行", cls: "plan-rail-badge-done", kind: "done" };
  }
  const st = String(item.last_run_status || item.lastRunStatus || "").toLowerCase();
  if (st && ["aborted", "stopped", "cancelled", "canceled"].includes(st)) {
    return { label: "已中止", cls: "plan-rail-badge-pending", kind: "stopped" };
  }
  if (st && ["failed", "timeout"].includes(st)) {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  if (st && st !== "completed" && st !== "done" && st !== "paused") {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
}

export function planTitleFromMarkdown(md) {
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

export async function loadPlanRail() {
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
    // SQLite split index for「已拆分」badge / reopen (best-effort)
    try {
      if (typeof host.loadPlanSplitIndex === "function") {
        await host.loadPlanSplitIndex(root);
      } else if (typeof window.loadPlanSplitIndex === "function") {
        await window.loadPlanSplitIndex(root);
      }
    } catch (_) {}
    let items = [];
    // Prefer H2 meta when available; fall back to plain path list.
    try {
      const metas = await gateway.getPlanMeta(root );
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
      const plans = (await gateway.getPlans(root )) || [];
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
    // 当前选中 / 草稿 / 已有拆分索引 必须留在列表（防「再进计划管理就消失」）
    const pinInject = [
      state.selectedPlan,
      state.chatDraftPlan,
      state.planRailSelected,
      ...Object.keys(state.planSplitByPath || {}).filter((k) => k.includes("/")),
    ];
    for (const raw of pinInject) {
      if (!raw) continue;
      const path = normalizePlanPath(raw, root) || raw;
      if (!path) continue;
      if (
        typeof isPlanUnderProject === "function" &&
        !isPlanUnderProject(path, root)
      ) {
        continue;
      }
      if (items.some((it) => it.path === path)) continue;
      items.unshift({
        path,
        title: null,
        ever_completed: false,
        last_run_status: null,
      });
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
        host.renderPlansMgmtPage();
      } catch (_) {}
    }
  }
  return state.planRailItems;
}

export function renderPlanRail() {
  ensureChatState();
  applyPlanRailVisibility();
  syncPlansDirLabels();
  // 聊天右栏 DOM 已撤：仅维护 state / 目录标签；列表 UI 在 page-plans
  const list = $("#plan-rail-list");
  const empty = $("#plan-rail-empty");
  if (!list) return;
  // 兼容旧壳：若残 DOM 仍存在也不展开
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

  // shell-chrome C1：当前内存里的 planJob 指向哪份计划 → 可「查看拆分结果」
  const job = state.planJob;
  const jobSt = String(job?.status || "").toLowerCase();
  const jobPathRaw = job?.plan_path || job?.planPath || "";
  const jobPath =
    jobPathRaw && typeof normalizePlanPath === "function"
      ? normalizePlanPath(jobPathRaw, root) || jobPathRaw
      : jobPathRaw;
  const jobHasSplit =
    !!job &&
    !!jobPath &&
    ["planned", "confirmed", "running", "done", "completed"].includes(jobSt);

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
    const canViewSplit =
      jobHasSplit &&
      (norm === jobPath || path === jobPath || path === jobPathRaw);
    const viewSplitBtn = canViewSplit
      ? `<button type="button" class="plan-rail-view-split" data-plan-rail-view="${chatEsc(
          path
        )}" title="查看拆分结果">查看拆分结果</button>`
      : "";
    return (
      `<div class="plan-rail-row${active}${selected}${isLatest}">` +
      `<button type="button" class="plan-rail-item${active}${selected}${isLatest}" data-plan-rail="${chatEsc(path)}" title="${chatEsc(path)}">` +
      `<div class="plan-rail-item-title">${chatEsc(title)}${latestMark}</div>` +
      `<div class="plan-rail-item-path">${chatEsc(path)}</div>` +
      `<div class="plan-rail-item-meta"><span class="plan-rail-badge ${badge.cls}">${chatEsc(badge.label)}</span></div>` +
      `</button>` +
      viewSplitBtn +
      `</div>`
    );
  });
  if (parts.historyHidden) {
    rows.push(
      `<div class="plan-history-hint muted" role="note">` +
        `已隐藏 ${parts.historyCount} 份已执行 · 勾选「显示已执行」可展开；有拆分的可点「查看拆分结果」` +
        `</div>`
    );
  }
  list.innerHTML = rows.join("");
}

/**
 * shell-chrome C1：从聊天 plan-rail 回看拆分台。
 * Restores from SQLite/disk when memory planJob is missing or for another plan.
 * @param {string} [planPath]
 */
export async function viewSplitFromPlanRail(planPath) {
  ensureChatState();
  const path =
    planPath ||
    state.planRailSelected ||
    state.selectedPlan ||
    null;
  if (!path) {
    if (typeof window.toast === "function") window.toast("请先选中一份计划");
    return;
  }
  selectPlanRailItem(path);
  state.chatDraftPlan = path;
  state.selectedPlan = path;

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
    const restore =
      typeof host.tryRestorePlanJobForPlan === "function"
        ? host.tryRestorePlanJobForPlan
        : typeof window.tryRestorePlanJobForPlan === "function"
          ? window.tryRestorePlanJobForPlan
          : null;
    if (!restore) {
      if (typeof window.toast === "function") {
        window.toast("还没有拆分结果，请先点「拆成步骤」");
      }
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
  state.phase = "confirm";
  try {
    host.renderPhasePanels?.();
    host.renderPlanPicker?.();
  } catch (_) {}
  if (typeof window.showPage === "function") window.showPage("workspace");
}

/** 从路径/时间戳猜「最新」计划（chat-YYYYMMDD-HHMM 优先，否则列表首项）. */
export function pickLatestPlanPath(items) {
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
export function selectPlanRailItem(planPath) {
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
      host.renderPlansMgmtPage();
    } catch (_) {}
  }
  if (typeof renderChatReadyBar === "function") host.renderChatReadyBar();
}

/** G1: double-click opens full view (edit path). */
export async function openPlanRailItem(planPath) {
  selectPlanRailItem(planPath);
  await host.openPlanFullView(planPath);
}
