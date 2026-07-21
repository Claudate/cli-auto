/**
 * [INPUT]: PlanJobView DTO · selectedId · live helpers
 * [OUTPUT]: 三栏 HTML 片段（波次 · 卡片 · 不写 IPC）
 * [POS]: A3-1 features/split 纯渲染；策略在 Rust
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function esc(s) {
  const fn = g("esc");
  if (typeof fn === "function") return fn(s);
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** First non-empty sentence-ish line for card one-liner. */
export function oneLiner(task) {
  const fn = g("splitOneLiner");
  if (typeof fn === "function") return fn(task);
  const full =
    task?.prompt ||
    task?.prompt_preview ||
    task?.promptPreview ||
    task?.acceptance ||
    task?.done_when ||
    task?.doneWhen ||
    "";
  let body = String(full || "")
    .replace(/^#+\s+/gm, "")
    .replace(/^\s*[-*]\s+/gm, "")
    .trim();
  if (!body) return "点选查看完整说明";
  const line = body.split(/\n+/).find((l) => l.trim().length > 4) || body;
  const s = line.replace(/\s+/g, " ").trim();
  return s.length > 72 ? s.slice(0, 70) + "…" : s;
}

/** Display role badge (做/查/系统) — not Worker soft-fill. */
export function roleBadge(task) {
  const fn = g("splitRoleBadge");
  if (typeof fn === "function") return fn(task);
  const id = String(task?.id || "");
  const role = String(task?.role || task?.group || "").toLowerCase();
  if (
    id.startsWith("sys-post-") ||
    role.includes("系统")
  ) {
    return { kind: "sys", label: "系统" };
  }
  if (
    role.includes("inspect") ||
    /inspect|巡检|验收/.test(id) ||
    /inspect|巡检|验收/.test(String(task?.title || ""))
  ) {
    return { kind: "check", label: "查" };
  }
  return { kind: "do", label: "做" };
}

export function isSystemPostTask(t) {
  const fn = g("isSystemPostTask");
  if (typeof fn === "function") return fn(t);
  if (!t) return false;
  const id = String(t.id || "");
  if (id.startsWith("sys-post-")) return true;
  return String(t.group || "") === "系统收尾";
}

export function partitionTasks(tasks) {
  const required = [];
  const optional = [];
  const system = [];
  for (const t of tasks || []) {
    if (isSystemPostTask(t)) system.push(t);
    else if (t.optional) optional.push(t);
    else required.push(t);
  }
  return { required, optional, system };
}

export function waitLine(task, byId) {
  const deps = task?.depends_on || [];
  if (!deps.length) return "可进首波";
  const titles = deps.map((id) => {
    const d = byId[id];
    return d ? d.title || id : id;
  });
  return `等待：${titles.join(" · ")}`;
}

/**
 * Human provider label (PM 文案；非引擎名第一句).
 * @param {string} provider
 */
export function engineLabel(provider) {
  const fn = g("flowEngineLabel");
  if (typeof fn === "function") return fn(provider);
  const p = String(provider || "claude").toLowerCase();
  if (p === "codex") return "备用通道";
  if (p === "fake") return "演练";
  return "默认通道";
}

/**
 * Route summary for advanced fold: provider + display role + optional scope hint.
 * Does not invent soft-fill — only surfaces DTO fields when present.
 * @param {object} task
 * @param {object} job
 */
export function routeSummary(task, job) {
  const provider = (
    task?.provider ||
    job?.provider ||
    "claude"
  ).toLowerCase();
  const badge = roleBadge(task);
  // DTO may later expose role/scope; tolerate snake/camel without requiring them.
  const roleRaw =
    task?.role ||
    task?.worker_role ||
    task?.workerRole ||
    null;
  const roleLabel =
    roleRaw != null && String(roleRaw).trim()
      ? String(roleRaw)
      : badge.label === "查"
        ? "检验"
        : badge.label === "系统"
          ? "系统收尾"
          : "实现";
  const scope =
    task?.scope ||
    task?.scope_paths ||
    task?.scopePaths ||
    null;
  let scopeText = "";
  if (Array.isArray(scope) && scope.length) {
    scopeText = scope.slice(0, 4).join(" · ");
    if (scope.length > 4) scopeText += "…";
  } else if (scope && typeof scope === "object") {
    const paths = scope.paths || scope.Paths || [];
    if (Array.isArray(paths) && paths.length) {
      scopeText = paths.slice(0, 4).join(" · ");
      if (paths.length > 4) scopeText += "…";
    }
  }
  return {
    provider,
    providerLabel: engineLabel(provider),
    roleLabel,
    badge,
    scopeText: scopeText || "由计划声明（未单独改写）",
    hasExplicitScope: !!scopeText,
  };
}

/** Left wave timeline HTML. */
export function timelineHtml(layers, byId, selectedId) {
  if (!layers?.length) {
    return '<p class="muted split-timeline-empty">暂无波次</p>';
  }
  const maxW = Math.max(1, ...layers.map((l) => (l || []).length));
  return layers
    .map((layer, i) => {
      const n = (layer || []).length;
      const parallel = n > 1;
      const cells = (layer || [])
        .map((id) => {
          const t = byId[id] || { id, title: id };
          const sel = selectedId === id ? " is-selected" : "";
          const short = String(t.title || id);
          const label = short.length > 14 ? short.slice(0, 12) + "…" : short;
          return `<button type="button" class="split-tl-cell${sel}" data-id="${esc(id)}" title="${esc(short)}">${esc(label)}</button>`;
        })
        .join("");
      return (
        `<div class="split-tl-wave" data-wave="${i + 1}">` +
        `<div class="split-tl-label">W${i + 1}${parallel ? ` · ${n} 并行` : ""}</div>` +
        `<div class="split-tl-row" style="--tl-max:${maxW}">${cells}</div>` +
        `</div>`
      );
    })
    .join("");
}

function cardGroupHtml(title, tasks, byId, opts = {}) {
  if (!tasks?.length) return "";
  const { runLocked, selectedId, liveTask } = opts;
  const rows = tasks
    .map((t) => {
      const id = t.id;
      const sel = selectedId === id ? " selected" : "";
      const live =
        typeof liveTask === "function"
          ? liveTask(id)
          : typeof g("liveTaskById") === "function"
            ? g("liveTaskById")(id)
            : null;
      const liveSt = live?.status || "";
      const pending =
        !live ||
        (typeof g("isTaskPendingStatus") === "function"
          ? g("isTaskPendingStatus")(liveSt)
          : true);
      const isOpt = !!t.optional;
      const included = isOpt ? t.include !== false : true;
      const optClass = isOpt
        ? included
          ? " optional-on"
          : " optional-off"
        : "";
      const role = roleBadge(t);
      const wait = waitLine(t, byId);
      const one = oneLiner(t);
      const statusHint = liveSt
        ? typeof g("statusLabel") === "function"
          ? g("statusLabel")(liveSt)
          : liveSt
        : "";
      const checkHtml = isOpt
        ? `<label class="wave-task-check" title="${
            role.kind === "sys"
              ? "系统收尾：默认勾选；取消则本次不跑"
              : "可选：勾选后才会执行"
          }" data-check-for="${esc(id)}">
            <input type="checkbox" class="wave-opt-check" data-id="${esc(id)}" ${
              included ? "checked" : ""
            } ${runLocked || !pending ? "disabled" : ""} />
          </label>`
        : `<span class="wave-task-req muted" title="必选">必</span>`;
      return (
        `<div class="wave-task-row split-card${sel}${pending ? "" : " done-ish"}${optClass}" data-id="${esc(id)}">` +
        checkHtml +
        `<button type="button" class="wave-task" data-id="${esc(id)}">` +
        `<div class="split-card-top">` +
        `<span class="split-role split-role-${role.kind}">${role.label}</span>` +
        (isOpt
          ? role.kind === "sys"
            ? `<span class="opt-badge opt-badge-sys">系统</span>`
            : `<span class="opt-badge">可选</span>`
          : "") +
        `<div class="wave-task-title">${esc(t.title || id)}</div>` +
        `</div>` +
        `<div class="split-card-one muted">${esc(one)}</div>` +
        `<div class="wave-task-meta muted">${esc(wait)}${
          statusHint ? " · " + esc(statusHint) : ""
        }</div>` +
        `</button></div>`
      );
    })
    .join("");
  return (
    `<div class="split-group">` +
    `<div class="split-group-label">${esc(title)} · ${tasks.length}</div>` +
    rows +
    `</div>`
  );
}

/** Middle column: 必做 / 可选 / 系统. */
export function cardsHtml(job, byId, opts = {}) {
  const tasks = job?.tasks || [];
  const layers = job?.layers || [];
  if (!tasks.length) return '<p class="muted">暂无步骤</p>';
  const { required, optional, system } = partitionTasks(tasks);
  const order = [];
  const seen = new Set();
  for (const layer of layers) {
    for (const id of layer || []) {
      if (!seen.has(id)) {
        seen.add(id);
        order.push(id);
      }
    }
  }
  for (const t of tasks) {
    if (!seen.has(t.id)) {
      seen.add(t.id);
      order.push(t.id);
    }
  }
  const sortByOrder = (list) => {
    const idx = Object.fromEntries(order.map((id, i) => [id, i]));
    return [...list].sort((a, b) => (idx[a.id] ?? 999) - (idx[b.id] ?? 999));
  };
  return (
    cardGroupHtml("必做", sortByOrder(required), byId, opts) +
    cardGroupHtml("可选", sortByOrder(optional), byId, opts) +
    cardGroupHtml("系统", sortByOrder(system), byId, opts)
  );
}

export { esc };
