/**
 * [INPUT]: project DTO · window.ccoGateway.getPlanMeta/getPlans · shellUi 的项目重绘回调
 * [OUTPUT]: 项目→计划二级树 HTML、异步元数据缓存与计划选择绑定
 * [POS]: web/js/shared 的侧栏展示适配器；只读计划 DTO，不复制计划/拆分业务策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

const planCache = new Map();
const loading = new Set();

function fileName(path) {
  const parts = String(path || "")
    .replaceAll(String.fromCharCode(92), "/")
    .split("/");
  return parts[parts.length - 1] || "未命名计划";
}

function normalize(path, project) {
  const fn = window.normalizePlanPath;
  return typeof fn === "function" ? fn(path, project) || path : path;
}

function underProject(path, project) {
  if (!path || !project) return false;
  const value = String(path).replace(/^file:\/\//, "");
  if (value.startsWith("/")) {
    const root = String(project)
      .replaceAll(String.fromCharCode(92), "/")
      .replace(/\/+$/, "");
    return value === root || value.startsWith(`${root}/`);
  }
  return !value.startsWith("../") && !value.startsWith(".." + String.fromCharCode(92));
}

function asItems(raw, project) {
  return (Array.isArray(raw) ? raw : [])
    .map((item) => {
      const value = typeof item === "string" ? { path: item } : item || {};
      const path = normalize(value.path || value.plan_path || "", project);
      return { path, title: value.title || null };
    })
    .filter((item) => item.path && underProject(item.path, project));
}

async function loadPlans(project, rerender) {
  if (!project || loading.has(project)) return;
  loading.add(project);
  try {
    const gateway = window.ccoGateway;
    if (!gateway) return;
    let items = [];
    try {
      items = asItems(await gateway.getPlanMeta(project), project);
    } catch (_) {}
    if (!items.length) items = asItems(await gateway.getPlans(project), project);
    planCache.set(project, items);
  } catch (_) {
    planCache.set(project, []);
  } finally {
    loading.delete(project);
    rerender();
  }
}

export function queueSidebarPlans(projects, rerender) {
  const roots = new Set((projects || []).map((project) => project.path).filter(Boolean));
  [...planCache.keys()].forEach((key) => {
    if (!roots.has(key)) planCache.delete(key);
  });
  (projects || []).forEach((project) => loadPlans(project.path, rerender));
}

export function sidebarPlansHtml(project, { esc, selectedPlan } = {}) {
  const items = planCache.get(project.path);
  if (!items?.length) return "";
  const escape = typeof esc === "function" ? esc : (value) => String(value || "");
  return `<div class="sidebar-plan-tree" aria-label="${escape(project.name)} 的计划">
    <div class="sidebar-plan-tree-label">计划</div>
    ${items
      .map((item) => {
        const selected = String(item.path) === String(selectedPlan || "");
        const title = item.title || fileName(item.path).replace(/\.md$/i, "");
        return `<button type="button" class="sidebar-plan-item${selected ? " active" : ""}" data-sidebar-plan="${escape(item.path)}" data-sidebar-project="${escape(project.path)}" title="${escape(title)}">
          <span data-icon="file-text" data-icon-size="12" aria-hidden="true"></span>
          <span>${escape(title)}</span>
        </button>`;
      })
      .join("")}
  </div>`;
}

export function installSidebarPlanSelection(root = document) {
  if (root?.dataset?.ccoSidebarPlansWired) return;
  if (root?.dataset) root.dataset.ccoSidebarPlansWired = "1";
  root.addEventListener("click", (event) => {
    const item = event.target.closest("[data-sidebar-plan][data-sidebar-project]");
    if (!item) return;
    event.preventDefault();
    event.stopPropagation();
    const selectProject = window.selectProject;
    const selectPlan = window.selectPlanRailItem;
    if (typeof selectProject !== "function" || typeof selectPlan !== "function") return;
    Promise.resolve(selectProject(item.dataset.sidebarProject))
      .then(() => selectPlan(item.dataset.sidebarPlan))
      .catch((error) => window.toast?.(String(error?.message || error)));
  });
}
