/**
 * [INPUT]: legacy · gateway.openPath · host rail/mgmt
 * [OUTPUT]: rail visibility · plansDir · partition
 * [POS]: A5-2a features/chat/planDir.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast, openNativeDialog, normalizePlanPath, pickPlanFileForPicker } from "./legacy.js";
import gateway from "../../shared/gateway.js";
import { host } from "./host.js";
import { ensureChatState } from "./chatState.js";

export function setPlanRailOpen(open, { persist = true } = {}) {
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

export function applyPlanRailVisibility() {
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
export function toggleChatPlanRail() {
  ensureChatState();
  setPlanRailOpen(!state.planRailOpen);
  if (state.planRailOpen) {
    Promise.resolve(host.loadPlanRail()).catch(() => {});
  }
  host.renderPlanRail();
}

export function syncPlansDirLabels() {
  const d = getPlansDir();
  const text = d.endsWith("/") ? d : `${d}/`;
  for (const id of ["plan-rail-dir-label", "plans-mgmt-dir-label"]) {
    const el = $(id);
    if (el) el.textContent = text;
  }
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
  Promise.resolve(host.loadPlanRail())
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
 * 换夹：优先系统选目录（项目内），回退 prompt。
 * 选完立刻刷新列表。
 */
export async function promptPlansDir() {
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
export async function openPlansDirInFinder() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const root = String(state.selectedPath).replace(/[/\\]+$/, "");
  const dir = getPlansDir();
  // 尾斜杠提示 open_path 按目录创建
  const abs = `${root}/${dir}/`.replace(/\/+/g, "/");
  try {
    await gateway.openPath(abs );
    toast(`已打开 ${dir}/`);
  } catch (e1) {
    try {
      await gateway.openPath(root );
      toast("已打开项目根（计划夹创建失败）");
    } catch (e) {
      toast(String(e?.message || e || e1 || "无法打开文件夹"));
    }
  }
}

/** 空态一键：勾选「显示其它位置」并刷新 */
export function showOtherPlansLocations() {
  const cb = $("#plans-mgmt-show-other");
  if (cb) {
    cb.checked = true;
  }
  try {
    host.renderPlansMgmtPage();
  } catch (_) {}
  toast("已显示其它位置的计划");
}

/** 管理页：选一个计划文件并选中（可跨 plans_dir） */
export async function pickPlanFileForMgmt() {
  try {
    if (typeof pickPlanFileForPicker === "function") {
      await pickPlanFileForPicker();
      // 手动选中后并入列表扫描；必要时打开「其它位置」
      try {
        await host.loadPlanRail();
      } catch (_) {}
      const path = state.selectedPlan;
      if (path) {
        if (typeof selectPlanRailItem === "function") host.selectPlanRailItem(path);
        const root = state.selectedPath;
        if (
          typeof isPathInPlansDir === "function" &&
          !isPathInPlansDir(path, getPlansDir(), root)
        ) {
          const cb = $("#plans-mgmt-show-other");
          if (cb) cb.checked = true;
        }
        if (state.page === "plans") host.renderPlansMgmtPage();
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
