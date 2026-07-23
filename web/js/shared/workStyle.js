/**
 * [INPUT]: localStorage
 * [OUTPUT]: work style profile prefs for split defaults (方案 C)
 * [POS]: shared — no IPC; used by welcome/settings/jobPoll
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 用户可选「更常拿 cco 干什么」→ 影响话术密度建议 / 并发建议 / 模板排序。
 * 不挡主路径；可跳过；计划结构仍优先于自称（拆分侧 heuristic）。
 */

const KEY = "cco.workStyle";
const SKIP_KEY = "cco.workStyle.skipped";
const ASKED_KEY = "cco.workStyle.asked";
/** W4-2: per-project override map path → WorkStyleId (localStorage JSON). */
const PROJECT_KEY = "cco.workStyle.byProject";

/** @typedef {'pm'|'gtm'|'semi'|'eng'} WorkStyleId */

/**
 * @type {Record<WorkStyleId, {
 *   label: string,
 *   hint: string,
 *   planMode: 'ai',
 *   maxParallelHint: number,
 *   copyDensity: 'plain'|'dual'|'tech_lean',
 *   parallel: 'cautious'|'balanced'|'eager',
 *   grainHint: string,
 *   templateOrder: string[]
 * }>}
 *
 * W4 / Q0: planMode is always "ai" — profiles must **not** force fast (产品默认智能拆分).
 * Fast remains a manual advanced choice on `#pp-plan-mode`.
 * grainHint: one line for ModelSplitAgent user prompt (偏粗/偏细); never changes planner default.
 */
export const WORK_STYLES = {
  pm: {
    label: "我主要写需求 / 管进度",
    hint: "说明偏结果；拆分仍走智能；能一起做的会一起做",
    planMode: "ai",
    maxParallelHint: 2,
    copyDensity: "plain",
    parallel: "balanced",
    grainHint: "偏粗：合并可同批的小改动，步骤宜少而清",
    templateOrder: ["req-outline", "overseas-landing"],
  },
  gtm: {
    label: "我做出海 / 运营 / 落地页",
    hint: "业务结果优先；模板更靠前；默认可并行",
    planMode: "ai",
    maxParallelHint: 2,
    copyDensity: "plain",
    parallel: "balanced",
    grainHint: "偏粗：按业务结果分步，少拆实现细节",
    templateOrder: ["overseas-landing", "req-outline"],
  },
  semi: {
    label: "我会看一点实现，要对齐验收",
    hint: "默认可读结果；更信计划里的先后",
    planMode: "ai",
    maxParallelHint: 2,
    copyDensity: "dual",
    parallel: "balanced",
    grainHint: "中等粒度：保留完成定义与关键路径，不过细",
    templateOrder: ["req-outline", "overseas-landing"],
  },
  eng: {
    label: "我主要改代码 / 工程落地",
    hint: "任务可含路径；可多步一起；高级仍默认折叠",
    planMode: "ai",
    maxParallelHint: 3,
    copyDensity: "tech_lean",
    parallel: "eager",
    grainHint: "偏细：按文件/模块拆开，scope_paths 尽量具体",
    templateOrder: ["req-outline", "overseas-landing"],
  },
};

/** @returns {WorkStyleId|null} */
export function getWorkStyleId() {
  try {
    const v = localStorage.getItem(KEY);
    if (v && WORK_STYLES[v]) return /** @type {WorkStyleId} */ (v);
  } catch (_) {
    /* ignore */
  }
  return null;
}

/** @param {WorkStyleId|null|string} id */
export function setWorkStyleId(id) {
  try {
    if (!id || !WORK_STYLES[id]) {
      localStorage.removeItem(KEY);
      return;
    }
    localStorage.setItem(KEY, id);
    localStorage.setItem(ASKED_KEY, "1");
    localStorage.removeItem(SKIP_KEY);
  } catch (_) {
    /* ignore */
  }
}

export function skipWorkStyle() {
  try {
    localStorage.setItem(SKIP_KEY, "1");
    localStorage.setItem(ASKED_KEY, "1");
  } catch (_) {
    /* ignore */
  }
}

/**
 * Normalize project path for map keys (trim · strip trailing slash).
 * @param {string|null|undefined} projectPath
 */
function projectKey(projectPath) {
  let p = String(projectPath || "").trim();
  if (!p) return "";
  // Unify trailing separators (mac paths rarely use \)
  while (p.length > 1 && (p.endsWith("/") || p.endsWith("\\"))) {
    p = p.slice(0, -1);
  }
  return p;
}

/** @returns {Record<string, WorkStyleId>} */
function readProjectMap() {
  try {
    const raw = localStorage.getItem(PROJECT_KEY);
    if (!raw) return {};
    const o = JSON.parse(raw);
    return o && typeof o === "object" ? o : {};
  } catch (_) {
    return {};
  }
}

/**
 * W4-2: project-level work style if set.
 * @param {string|null|undefined} projectPath
 * @returns {WorkStyleId|null}
 */
export function getProjectWorkStyleId(projectPath) {
  const k = projectKey(projectPath);
  if (!k) return null;
  try {
    const map = readProjectMap();
    const id = map[k];
    if (id && WORK_STYLES[id]) return /** @type {WorkStyleId} */ (id);
  } catch (_) {
    /* ignore */
  }
  return null;
}

/**
 * W4-2: set or clear project override.
 * @param {string|null|undefined} projectPath
 * @param {WorkStyleId|null|string} id — null/empty clears override
 */
export function setProjectWorkStyleId(projectPath, id) {
  const k = projectKey(projectPath);
  if (!k) return;
  try {
    const map = readProjectMap();
    if (!id || !WORK_STYLES[id]) {
      delete map[k];
    } else {
      map[k] = id;
    }
    localStorage.setItem(PROJECT_KEY, JSON.stringify(map));
  } catch (_) {
    /* ignore */
  }
}

/**
 * Default profile when skipped / unset = PM (product main audience).
 * @param {string|null|undefined} [projectPath] — if set, project override wins (W4-2)
 */
export function resolvedWorkStyle(projectPath) {
  const fromProject = getProjectWorkStyleId(projectPath);
  const id = fromProject || getWorkStyleId() || "pm";
  return {
    id,
    ...WORK_STYLES[id],
    fromProject: !!fromProject,
  };
}

/**
 * Show one-shot chooser only when never asked and not skipped.
 * @returns {boolean}
 */
export function shouldOfferWorkStyle() {
  try {
    if (localStorage.getItem(ASKED_KEY) === "1") return false;
    if (localStorage.getItem(SKIP_KEY) === "1") return false;
    if (getWorkStyleId()) return false;
    return true;
  } catch (_) {
    return false;
  }
}

/**
 * Suggested max_parallel for startPlanJob when UI has no explicit override.
 * @param {number} [configDefault]
 * @param {string|null|undefined} [projectPath]
 */
export function suggestedMaxParallel(configDefault, projectPath) {
  const base = Number(configDefault) > 0 ? Number(configDefault) : 2;
  const style = resolvedWorkStyle(projectPath);
  if (style.parallel === "eager") return Math.max(base, style.maxParallelHint);
  if (style.parallel === "cautious") return Math.min(base, 1);
  return Math.min(Math.max(base, 1), Math.max(style.maxParallelHint, 2));
}

/**
 * W4 grain line for ModelSplitAgent (optional). Empty when unset/skip → backend omits.
 * @param {string|null|undefined} [projectPath]
 * @returns {string}
 */
export function suggestedGrainHint(projectPath) {
  try {
    const style = resolvedWorkStyle(projectPath);
    return String(style.grainHint || "").trim();
  } catch (_) {
    return "";
  }
}

/** Apply template button order on welcome row if present. */
export function applyTemplateOrder(root) {
  const row =
    root?.querySelector?.(".welcome-template-row") ||
    document.querySelector(".welcome-template-row");
  if (!row) return;
  const style = resolvedWorkStyle();
  const order = style.templateOrder || [];
  const buttons = Array.from(row.querySelectorAll("[data-plan-template]"));
  if (buttons.length < 2) return;
  buttons
    .sort((a, b) => {
      const ia = order.indexOf(a.getAttribute("data-plan-template") || "");
      const ib = order.indexOf(b.getAttribute("data-plan-template") || "");
      return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib);
    })
    .forEach((el) => row.appendChild(el));
}

/**
 * Render optional first-run chooser into `#work-style-chooser` if present.
 * @param {{ onPick?: (id: string) => void }} [opts]
 */
export function paintWorkStyleChooser(opts = {}) {
  const host = document.getElementById("work-style-chooser");
  if (!host) return;
  if (!shouldOfferWorkStyle()) {
    host.hidden = true;
    host.innerHTML = "";
    return;
  }
  host.hidden = false;
  const cards = Object.entries(WORK_STYLES)
    .map(
      ([id, s]) =>
        `<button type="button" class="work-style-card" data-work-style="${id}">` +
        `<span class="work-style-label">${s.label}</span>` +
        `<span class="work-style-hint muted">${s.hint}</span>` +
        `</button>`
    )
    .join("");
  host.innerHTML =
    `<p class="work-style-title">你更常拿 cco 干什么？（可跳过，以后在设置里改）</p>` +
    `<div class="work-style-grid">${cards}</div>` +
    `<button type="button" class="linkish work-style-skip" data-work-style-skip="1">先跳过，按通用来</button>`;

  host.onclick = (e) => {
    const skip = e.target?.closest?.("[data-work-style-skip]");
    if (skip) {
      skipWorkStyle();
      host.hidden = true;
      host.innerHTML = "";
      return;
    }
    const btn = e.target?.closest?.("[data-work-style]");
    if (!btn) return;
    const id = btn.getAttribute("data-work-style");
    setWorkStyleId(id);
    applyTemplateOrder();
    host.hidden = true;
    host.innerHTML = "";
    if (typeof opts.onPick === "function") opts.onPick(id);
  };
}

/**
 * Fill settings `<select id="s-work-style">` if present.
 * Optionally inject W4-2 project override controls (no index.html change).
 * @param {string|null|undefined} [projectPath]
 */
export function loadWorkStyleSetting(projectPath) {
  const sel = document.getElementById("s-work-style");
  if (!sel) return;
  const proj = projectPath || "";
  const projId = getProjectWorkStyleId(proj);
  const cur = projId || getWorkStyleId() || "";
  if ([...sel.options].some((o) => o.value === cur)) {
    sel.value = cur;
  } else {
    sel.value = "";
  }
  paintProjectWorkStyleRow(proj);
}

/**
 * W4-2: thin project override UI under #s-work-style (injected).
 * @param {string} projectPath
 */
function paintProjectWorkStyleRow(projectPath) {
  const sel = document.getElementById("s-work-style");
  if (!sel || !sel.parentElement) return;
  let row = document.getElementById("s-work-style-project-row");
  if (!projectPath) {
    if (row) {
      row.hidden = true;
      row.innerHTML = "";
    }
    return;
  }
  if (!row) {
    row = document.createElement("div");
    row.id = "s-work-style-project-row";
    row.className = "settings-work-style-project muted";
    sel.parentElement.appendChild(row);
  }
  row.hidden = false;
  const active = getProjectWorkStyleId(projectPath);
  const short =
    projectPath.split(/[/\\]/).filter(Boolean).pop() || projectPath;
  row.innerHTML =
    `<label class="settings-inline">` +
    `<input type="checkbox" id="s-work-style-project-only" ${active ? "checked" : ""}/>` +
    ` 仅当前项目「${short}」用上面习惯（覆盖全局）</label>` +
    (active
      ? ` <button type="button" class="linkish" id="s-work-style-project-clear">清除项目覆盖</button>`
      : "");
  const box = document.getElementById("s-work-style-project-only");
  if (box) {
    box.onchange = () => {
      /* saved on saveWorkStyleSetting */
    };
  }
  const clear = document.getElementById("s-work-style-project-clear");
  if (clear) {
    clear.onclick = (e) => {
      e.preventDefault();
      setProjectWorkStyleId(projectPath, null);
      loadWorkStyleSetting(projectPath);
    };
  }
}

/**
 * @param {string|null|undefined} [projectPath]
 */
export function saveWorkStyleSetting(projectPath) {
  const sel = document.getElementById("s-work-style");
  if (!sel) return;
  const v = sel.value;
  const projOnly = !!document.getElementById("s-work-style-project-only")
    ?.checked;
  if (projOnly && projectPath) {
    if (v && WORK_STYLES[v]) {
      setProjectWorkStyleId(projectPath, v);
    } else {
      setProjectWorkStyleId(projectPath, null);
    }
  } else {
    if (projectPath) {
      // Saving as global: clear project override unless user left box checked
      setProjectWorkStyleId(projectPath, null);
    }
    if (!v) {
      skipWorkStyle();
      localStorage.removeItem(KEY);
    } else {
      setWorkStyleId(v);
    }
  }
  applyTemplateOrder();
  paintProjectWorkStyleRow(projectPath || "");
}
