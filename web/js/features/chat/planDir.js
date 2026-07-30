/**
 * [INPUT]: legacy · host mgmt
 * [OUTPUT]: rail closed · plansDir · mgmt scope · partition（聊天右栏 UI 已撤）
 * [POS]: A5-2a features/chat/planDir.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast, openNativeDialog, normalizePlanPath, pickPlanFileForPicker } from "./legacy.js";
import { host } from "./host.js";
import { ensureChatState } from "./chatState.js";

/**
 * 聊天右栏已撤：强制关闭、不写 localStorage、不画轨。
 * 计划列表能力在 page-plans（顶栏「计划管理」）。
 */
export function setPlanRailOpen(_open, { persist: _persist = true } = {}) {
  ensureChatState();
  state.planRailOpen = false;
  applyPlanRailVisibility();
}

export function applyPlanRailVisibility() {
  ensureChatState();
  state.planRailOpen = false;
  const rail = $("#plan-rail");
  const layout = document.querySelector("#page-chat .chat-layout");
  if (rail) rail.setAttribute("hidden", "");
  if (layout) layout.classList.remove("plan-rail-open");
}

/** 兼容旧入口：打开「计划管理」页（右栏已撤） */
export function toggleChatPlanRail() {
  ensureChatState();
  setPlanRailOpen(false);
  if (typeof host.openPlanManagement === "function") {
    Promise.resolve(host.openPlanManagement()).catch(() => {});
    return;
  }
  if (typeof window.openPlanManagement === "function") {
    Promise.resolve(window.openPlanManagement()).catch(() => {});
  }
}

export function syncPlansDirLabels() {
  // 保存目录仅用于聊天落盘；管理页已改「选中文件夹/文件」，不再展示 dir 标签
  const d = getPlansDir();
  const text = d.endsWith("/") ? d : `${d}/`;
  const el = $("plan-rail-dir-label");
  if (el) el.textContent = text;
}

/** 管理页：当前列表作用域（选中的文件夹相对路径；null = 项目全量）— 不展示 UI 标签 */
export function getPlansMgmtScopeDir() {
  ensureChatState();
  return state.plansMgmtScopeDir || null;
}

export function setPlansMgmtScopeDir(dir) {
  ensureChatState();
  const d = dir == null || dir === "" ? null : String(dir).replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  state.plansMgmtScopeDir = d || null;
}

/** G1: project-relative plans directory (default plans). */
export function getPlansDir() {
  ensureChatState();
  if (state.selectedPath) {
    const k = `cco.plansDir:${state.selectedPath}`;
    const v = localStorage.getItem(k);
    if (v) state.plansDir = v;
  }
  return (state.plansDir || "plans").replace(/^\/+|\/+$/g, "") || "plans";
}

export function setPlansDir(dir) {
  ensureChatState();
  let d = String(dir || "plans").trim().replace(/\\/g, "/");
  d = d.replace(/^\/+|\/+$/g, "");
  if (!d || d.includes("..") || d.startsWith("/") || /^[A-Za-z]:\//.test(d)) {
    toast("计划目录必须是项目内相对路径，例如 plans 或 docs/plans");
    return false;
  }
  state.plansDir = d;
  if (state.selectedPath) {
    localStorage.setItem(`cco.plansDir:${state.selectedPath}`, d);
  }
  syncPlansDirLabels();
  toast(`新计划将保存到 ${d}/ · 列表已按此目录刷新`);
  // 更换目录后立刻重扫管理页 / 右栏
  Promise.resolve(host.loadPlanItems())
    .then(() => {
      if (state.page === "plans") {
        try {
          host.renderPlansMgmtPage();
        } catch (_) {}
      }
    })
    .catch(() => {});
  return true;
}

/**
 * 更换聊天落盘目录（高级；管理页不再暴露）。
 * 仅当原生对话框不可用时才回退到文本输入。
 */
export async function promptPlansDir() {
  const cur = getPlansDir();
  const root = state.selectedPath;
  if (!root) {
    toast("请先选择项目");
    return;
  }
  const rootNorm = String(root).replace(/[/\\]+$/, "");
  let dialogOk = false;
  try {
    if (typeof openNativeDialog === "function") {
      dialogOk = true;
      const selected = await openNativeDialog({
        directory: true,
        multiple: false,
        defaultPath: `${rootNorm}/${cur}`.replace(/\/+/g, "/"),
        title: "选择计划保存目录（须在当前项目内）",
      });
      if (selected == null || selected === false || selected === "") return;
      const abs = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
      if (!abs) return;
      let rel =
        typeof normalizePlanPath === "function"
          ? normalizePlanPath(abs, rootNorm) || abs
          : abs;
      rel = String(rel || "").replace(/\\/g, "/").replace(/^\.\//, "");
      if (rel.startsWith(rootNorm + "/") || rel.startsWith(rootNorm + "\\")) {
        rel = rel.slice(rootNorm.length + 1);
      }
      if (!rel || rel === rootNorm || rel === ".") {
        toast("请选择项目内的子文件夹，例如 plans 或 docs");
        return;
      }
      if (rel.includes("..") || rel.startsWith("/") || /^[A-Za-z]:\//.test(rel)) {
        toast("计划目录必须在项目内");
        return;
      }
      setPlansDir(rel);
      return;
    }
  } catch (e) {
    console.warn("promptPlansDir dialog", e);
    dialogOk = false;
  }
  if (dialogOk) return;
  const next = window.prompt(
    "默认计划文件夹（相对项目根，例如 plans 或 docs）",
    cur
  );
  if (next == null) return;
  setPlansDir(next);
}

/**
 * 管理页：选中项目内文件夹 → 列表只显示该夹下的计划。
 * 不改聊天落盘目录；取消对话框则无操作。
 */
export async function pickPlansFolderForMgmt() {
  const root = state.selectedPath;
  if (!root) {
    toast("请先选择项目");
    return;
  }
  const rootNorm = String(root).replace(/[/\\]+$/, "");
  try {
    if (typeof openNativeDialog !== "function") {
      toast("选中文件夹不可用");
      return;
    }
    const selected = await openNativeDialog({
      directory: true,
      multiple: false,
      defaultPath: rootNorm,
      title: "选择计划文件夹（须在当前项目内）",
    });
    if (selected == null || selected === false || selected === "") return;
    const abs = String(Array.isArray(selected) ? selected[0] : selected || "").trim();
    if (!abs) return;
    let rel =
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(abs, rootNorm) || abs
        : abs;
    rel = String(rel || "").replace(/\\/g, "/").replace(/^\.\//, "");
    if (rel.startsWith(rootNorm + "/") || rel.startsWith(rootNorm + "\\")) {
      rel = rel.slice(rootNorm.length + 1);
    }
    if (!rel || rel === rootNorm || rel === ".") {
      // 选中项目根 = 全量列表
      setPlansMgmtScopeDir(null);
    } else if (rel.includes("..") || rel.startsWith("/") || /^[A-Za-z]:\//.test(rel)) {
      toast("请选择当前项目内的文件夹");
      return;
    } else {
      setPlansMgmtScopeDir(rel);
    }
    try {
      await host.loadPlanRail();
    } catch (_) {}
    if (state.page === "plans" && typeof host.renderPlansMgmtPage === "function") {
      host.renderPlansMgmtPage();
    }
    const scope = getPlansMgmtScopeDir();
    toast(scope ? `已加载「${scope}/」下的计划` : "已显示项目内全部计划");
  } catch (e) {
    toast(String(e?.message || e || "无法选中文件夹"));
  }
}

/** 管理页：选中一份计划文件并加载到列表 / 详情 */
export async function pickPlanFileForMgmt() {
  try {
    if (typeof pickPlanFileForPicker !== "function") {
      toast("选中文件不可用");
      return;
    }
    const before = state.selectedPlan;
    await pickPlanFileForPicker();
    const path = state.selectedPlan;
    if (!path || path === before) {
      return;
    }
    // 单文件加载：解除文件夹作用域限制，保证能看见刚选的文件
    setPlansMgmtScopeDir(null);
    try {
      await host.loadPlanRail();
    } catch (_) {}
    if (typeof host.selectPlanRailItem === "function") {
      host.selectPlanRailItem(path);
    }
    state.chatDraftPlan = path;
    if (state.page === "plans" && typeof host.renderPlansMgmtPage === "function") {
      host.renderPlansMgmtPage();
    }
    toast("已加载计划");
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/**
 * E4：路径是否在当前 plans_dir 下（相对项目路径）。
 * pinPaths 始终保留（选中/草稿/手动挑的文件）。
 */
export function isPathInPlansDir(path, plansDir, root) {
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
export function partitionByPlansDir(items, { plansDir, root, pinPaths = [], showOther = false } = {}) {
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
