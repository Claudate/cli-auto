/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: plan meta · executed partition · loadPlansForPicker
 * [POS]: A5-2b-fin features/project/planMeta.js
 * note: plan meta · executed partition · loadPlansForPicker
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  isRunPaused,
  isLiveStatus,
  isFailedStatus,
  toastRunLocked,
  normalizePlanPath,
  planDisplayName,
  fillPlannerLog,
  canEditSelectedTask,
  openNativeDialog,
  loadProjects,
  renderProjectList,
  renderWorkspace,
  goHome,
  closeModal,
  openChatPage,
  stashChatSession,
  restoreChatSession,
  stopChatWaitTicker,
  loadPlanRail,
  renderPlanRail,
  selectPlanRailItem,
  renderPlansMgmtPage,
  chatAssignDirectEnabled,
  flowModeLabel,
  flowModeHint,
  flowStageStripHtml,
  flowChooserSub,
  flowJoinSeriousFun,
  flowPickBlurb,
  flowPlanHowLabel,
  flowPlanningSub,
  flowSanitizeDepsLabel,
  flowRunningMonitorTitle,
  esc,
  requireGateway,
} from "./legacy.js";
import { host } from "./host.js";

/** 计划路径是否属于当前项目（相对路径，或绝对路径前缀为本项目） */
export function isPlanUnderProject(planPath, projectRoot = state.selectedPath) {
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

/** Badge from PlanMeta: 已执行 / 已拆分 / 失败过 / 未执行 */
export function planExecBadgeInfo(item) {
  if (!item) return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
  if (item.ever_completed || item.everCompleted) {
    return { label: "已执行", cls: "plan-rail-badge-done", kind: "done" };
  }
  const st = String(item.last_run_status || item.lastRunStatus || "").toLowerCase();
  // user stop / abort is not a business failure
  if (st && ["aborted", "stopped", "cancelled", "canceled"].includes(st)) {
    return { label: "已中止", cls: "plan-rail-badge-pending", kind: "stopped" };
  }
  if (st && ["failed", "timeout"].includes(st)) {
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  if (st && st !== "completed" && st !== "done" && st !== "" && st !== "paused") {
    // had a non-success terminal/partial run
    return { label: "失败过", cls: "plan-rail-badge-failed", kind: "failed" };
  }
  // Split index (SQLite plan_jobs) — restorable without having executed
  const path = item.path || item.plan_path || item.planPath || "";
  const split =
    item.split_status ||
    item.splitStatus ||
    (typeof host.planSplitForPath === "function"
      ? host.planSplitForPath(path)
      : (state.planSplitByPath &&
          (state.planSplitByPath[path] ||
            state.planSplitByPath[
              typeof normalizePlanPath === "function"
                ? normalizePlanPath(path, state.selectedPath) || path
                : path
            ]))) ||
    null;
  if (split) {
    const ss = String(split.status || item.split_status || "").toLowerCase();
    if (ss === "planning") {
      return { label: "拆分中", cls: "plan-rail-badge-planning", kind: "split" };
    }
    if (ss === "planned" || ss === "confirmed" || ss === "ready" || !ss) {
      return { label: "已拆分", cls: "plan-rail-badge-split", kind: "split" };
    }
  }
  return { label: "未执行", cls: "plan-rail-badge-pending", kind: "pending" };
}

export function planIsEverCompleted(item) {
  if (!item) return false;
  return !!(item.ever_completed || item.everCompleted);
}

/** Lookup meta for a path (relative preferred); empty stub if unknown. */
export function planMetaForPath(path, root = state.selectedPath) {
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
export function partitionPlanItems(items, { showExecuted = false, pinPaths = [] } = {}) {
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

export function setShowExecutedPlans(on) {
  state.showExecutedPlans = !!on;
  try {
    localStorage.setItem("cco.showExecutedPlans", state.showExecutedPlans ? "1" : "0");
  } catch (_) {}
  syncShowExecutedToggles();
  if (state.planChooserOpen) host.renderPlanChooser();
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

export function syncShowExecutedToggles() {
  const on = !!state.showExecutedPlans;
  for (const id of [
    "chooser-show-executed",
    // plan-rail-show-executed 已随聊天右栏撤掉
    "plans-mgmt-show-executed",
  ]) {
    const el = document.getElementById(id);
    if (el && el.type === "checkbox") el.checked = on;
  }
}

/** Normalize get_plan_meta / fallback list into state.planMetaItems + byPath. */
export function applyPlanMetaItems(items, root = state.selectedPath) {
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

export async function loadPlansForPicker() {
  if (!state.selectedPath) {
    state.plans = [];
    state.planMetaItems = [];
    state.planMetaByPath = {};
    state.planSplitByPath = {};
    state.plansLoading = false;
    if (state.planChooserOpen) host.renderPlanChooser();
    host.updateChooserAssignState();
    return [];
  }
  state.plansLoading = true;
  if (state.planChooserOpen) host.renderPlanChooser();
  try {
    const root = state.selectedPath;
    // Parallel: run meta + SQLite split index (plan list reopen / 已拆分 badge)
    try {
      if (typeof host.loadPlanSplitIndex === "function") {
        await host.loadPlanSplitIndex(root);
      }
    } catch (_) {}
    // H2: prefer list_plan_meta (path + ever_completed / last_run_*); fall back to paths
    let list = [];
    let metas = null;
    try {
      metas = await requireGateway().getPlanMeta(root);
    } catch (_) {
      metas = null;
    }
    if (Array.isArray(metas) && metas.length) {
      const applied = applyPlanMetaItems(metas, root);
      list = applied.map((m) => m.path);
    } else {
      const plans = (await requireGateway().getPlans(root)) || [];
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
    // 用户手动选的计划：仅当磁盘仍有源文件时置顶；源已删则清选中（不造幽灵）
    let selected = normalizePlanPath(state.selectedPlan, root) || state.selectedPlan;
    if (selected && isPlanUnderProject(selected, root) && !list.includes(selected)) {
      let exists = false;
      try {
        await requireGateway().readPlanMd(root, selected);
        exists = true;
      } catch (_) {
        exists = false;
      }
      if (exists) {
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
      } else {
        state.selectedPlan = null;
        selected = null;
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
  if (state.planChooserOpen) host.renderPlanChooser();
  host.renderPlanPicker();
  host.updateChooserAssignState();
  // Keep rail in sync when chooser rescans
  if (typeof loadPlanRail === "function" && state.page === "chat") {
    try {
      // meta already in state — rail can re-render without re-fetch; still refresh for safety
      if (typeof renderPlanRail === "function") renderPlanRail();
    } catch (_) {}
  }
  return state.plans;
}
