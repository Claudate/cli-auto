/**
 * [INPUT]: legacy · gateway · chatState
 * [OUTPUT]: load plan items list · select · badge info（聊天右栏 UI 已撤；renderPlanRail 已删除）
 * [POS]: A5-2a features/chat/planRail.js
 * [DEPRECATED]: 正在迁移到 plansMgmt.js；保留数据加载函数直到完成迁移
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  normalizePlanPath,
  planDisplayName,
  planExecBadgeInfo,
  applyPlanMetaItems,
  isPlanUnderProject,
  selectPlan,
} from "./legacy.js";
import gateway from "../../shared/gateway.js";
import { host } from "./host.js";
import { ensureChatState } from "./chatState.js";
import * as chatApi from "./chatApi.js";

/** 从 markdown 提取 # 标题 */
export function planTitleFromMarkdown(md) {
  if (!md) return null;
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

/** 从路径/时间戳猜「最新」计划（chat-YYYYMMDD-HHMM 优先，否则列表首项）. */
export function pickLatestPlanPath(items) {
  if (!Array.isArray(items) || !items.length) return null;
  let best = null;
  let bestKey = "";
  for (const it of items) {
    const p = String(it.path || "");
    const base = p.split(/[/\\]/).pop() || p;
    const m = base.match(/(\d{8})[-_]?(\d{4,6})?/);
    const key = m ? `${m[1]}${m[2] || "0000"}` : "";
    if (key && key >= bestKey) {
      bestKey = key;
      best = p;
    }
  }
  if (best) return best;
  return items[0]?.path || null;
}

/** 加载计划列表数据到 state.planItems */
export async function loadPlanItems() {
  ensureChatState();
  if (!state.selectedPath) {
    state.planItems = [];
    state.planItemsLoading = false;
    return [];
  }
  state.planItemsLoading = true;
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
    // 仅 pin 仍在磁盘上的选中/草稿（存在性检查并行，避免逐个 await 的切换延迟）
    const pinInject = (() => {
      const seenPin = new Set();
      return [
        { raw: state.selectedPlan },
        { raw: state.chatDraftPlan },
      ]
        .filter((e) => e.raw)
        .map((e) => ({ raw: e.raw, path: normalizePlanPath(e.raw, root) || e.raw }))
        .filter((e) => {
          if (
            !e.path ||
            (typeof isPlanUnderProject === "function" && !isPlanUnderProject(e.path, root)) ||
            items.some((it) => it.path === e.path) ||
            seenPin.has(e.path)
          ) {
            return false;
          }
          seenPin.add(e.path);
          return true;
        });
    })();
    const pinExists = await Promise.all(
      pinInject.map((e) => chatApi.planMdExists(root, e.path).catch(() => false))
    );
    pinInject.forEach(({ raw, path }, i) => {
      if (!pinExists[i]) {
        // 清掉指向已删文件的选中
        if (state.selectedPlan === raw || state.selectedPlan === path) {
          state.selectedPlan = null;
        }
        if (state.chatDraftPlan === raw || state.chatDraftPlan === path) {
          state.chatDraftPlan = null;
        }
        return;
      }
      items.unshift({
        path,
        title: null,
        ever_completed: false,
        last_run_status: null,
      });
    });
    // 全量保留在 planItemsAll
    state.planItemsAll = items;
    // Sync chooser path list when rail loads first
    if (items.length && (!state.plans || !state.plans.length)) {
      state.plans = items.map((it) => it.path);
    } else if (items.length && Array.isArray(state.plans)) {
      for (const it of items) {
        if (it.path && !state.plans.includes(it.path)) {
          state.plans.push(it.path);
        }
      }
    }
    state.planItems = items;
  } catch (e) {
    console.warn("loadPlanItems", e);
    state.planItems = [];
  } finally {
    state.planItemsLoading = false;
    if (state.page === "plans") {
      try {
        host.renderPlansMgmtPage();
      } catch (_) {}
    }
  }
  return state.planItems;
}

/** shell-chrome C1：从聊天回看拆分台。Restores from SQLite/disk when memory planJob is missing. */
export async function viewSplitFromPlanRail(planPath) {
  ensureChatState();
  const path =
    planPath ||
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

/** G1: single-click selects plan (no modal). */
export function selectPlanRailItem(planPath) {
  ensureChatState();
  if (!planPath || !state.selectedPath) return;
  const root = state.selectedPath;
  const path =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(planPath, root) || planPath
      : planPath;
  state.selectedPlan = path;
  if (typeof selectPlan === "function") {
    try {
      selectPlan(path);
    } catch (_) {}
  }
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
