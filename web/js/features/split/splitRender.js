/**
 * [INPUT]: PlanJobView DTO · selectedId · live helpers
 * [OUTPUT]: 步骤列 HTML（按波次顺序 · 并行外框 · 不写 IPC）
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

function isWorkerNoise(line) {
  const t = String(line || "").trim();
  if (!t) return true;
  const lower = t.toLowerCase();
  return (
    t.startsWith("你是执行") ||
    t.includes("的 worker") ||
    t.includes("的worker") ||
    lower.includes("cco_done") ||
    t.startsWith("项目根目录") ||
    t.startsWith("依据下列说明")
  );
}

function displayTitle(title) {
  return String(title || "")
    .replace(/[☐✅☑□■✗✘×]+$/g, "")
    .trim();
}

/** First non-empty sentence-ish line for card one-liner (never worker scaffold). */
export function oneLiner(task) {
  const fn = g("splitOneLiner");
  if (typeof fn === "function") return fn(task);
  const candidates = [
    task?.summary,
    task?.done_when || task?.doneWhen,
    task?.acceptance,
    displayTitle(task?.title),
    task?.prompt_preview || task?.promptPreview,
  ];
  for (const c of candidates) {
    const s = String(c || "")
      .replace(/^#+\s+/gm, "")
      .replace(/^\s*[-*]\s+/gm, "")
      .replace(/\s+/g, " ")
      .trim();
    if (s.length > 2 && !isWorkerNoise(s)) {
      return s.length > 72 ? s.slice(0, 70) + "…" : s;
    }
  }
  // Last resort: strip scaffold from full prompt
  const strip =
    typeof g("stripWorkerScaffold") === "function"
      ? g("stripWorkerScaffold")
      : (p) => p;
  let body = String(strip(task?.prompt || "") || "")
    .replace(/^#+\s+/gm, "")
    .replace(/^\s*[-*]\s+/gm, "")
    .trim();
  if (!body) return "点选查看本步";
  const line =
    body
      .split(/\n+/)
      .map((l) => l.trim())
      .find((l) => l.length > 4 && !isWorkerNoise(l)) || body;
  const s = line.replace(/\s+/g, " ").trim();
  if (isWorkerNoise(s)) return displayTitle(task?.title) || "点选查看本步";
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
  if (!deps.length) return "可马上开始";
  const titles = deps.map((id) => {
    const d = byId[id];
    const raw = d ? d.title || id : id;
    return displayTitle(raw) || id;
  });
  if (titles.length <= 2) return `等：${titles.join(" · ")}`;
  return `等：${titles.slice(0, 2).join(" · ")} 等 ${titles.length} 项`;
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
  if (p === "claude") return "默认通道";
  if (p === "gemini") return "Gemini";
  if (p === "qwen" || p === "tongyi") return "通义 Qwen";
  if (p === "kimi" || p === "moonshot") return "Kimi";
  if (p === "deepseek" || p === "codewhale" || p === "codew") return "CodeWhale";
  if (p === "copilot") return "Copilot";
  if (p === "codebuddy" || p === "cbc") return "CodeBuddy";
  if (p === "fake") return "演练";
  return p || "默认通道";
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

/**
 * Legacy left-rail timeline (kept for callers; desk now uses cardsHtml wave groups).
 * Prefer cardsHtml — wave + cards are merged into one ordered column.
 */
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

/** Single step card row (shared by wave groups). */
function taskCardHtml(t, byId, opts = {}) {
  const { runLocked, selectedId, liveTask } = opts;
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
  // Backend risk_class / risk_label (read|write_local|exec|external); fallback local.
  const riskClass = String(t.risk_class || t.riskClass || "").toLowerCase();
  const riskLabel =
    t.risk_label ||
    t.riskLabel ||
    (riskClass === "external"
      ? "会外发"
      : riskClass === "exec"
        ? "跑命令"
        : riskClass === "read"
          ? "只读"
          : riskClass === "write_local"
            ? "改本地"
            : "");
  const riskHtml = riskLabel
    ? `<span class="risk-badge risk-${esc(riskClass || "write_local")}" title="${esc(
        riskClass === "external"
          ? "会推送或发到远端"
          : riskClass === "exec"
            ? "会在本机跑检查命令"
            : riskClass === "read"
              ? "只读，不改业务文件"
              : "会改本地代码或文件"
      )}">${esc(riskLabel)}</span>`
    : "";
  const costHint = String(t.cost_route_hint || t.costRouteHint || "").trim();
  const costHtml = costHint
    ? `<span class="cost-route-chip" title="开跑时费用优选（未改你指定的通道）">${esc(
        costHint
      )}</span>`
    : "";
  // 执行通道：本步将用哪个 CLI（自动分配或手动改过的通道），一眼可辨。
  const provRaw = String(t.provider || t.provider_label || "").trim();
  const provHtml = provRaw
    ? `<span class="cost-route-chip route-provider-chip" title="本步执行通道（可选中该步后在「高级·执行通道」改）">${esc(
        engineLabel(provRaw)
      )}</span>`
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
    riskHtml +
    provHtml +
    costHtml +
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
}

/**
 * Build ordered wave layers from job.layers + orphan tasks.
 * Returns Task[][] in execution order.
 */
export function orderedWaveTasks(job, byId) {
  const tasks = job?.tasks || [];
  const layers = job?.layers || [];
  const seen = new Set();
  const waves = [];
  for (const layer of layers) {
    const wave = [];
    for (const id of layer || []) {
      if (seen.has(id)) continue;
      const t = byId[id];
      if (!t) continue;
      seen.add(id);
      wave.push(t);
    }
    if (wave.length) waves.push(wave);
  }
  // Orphans (not in layers): each alone in order of tasks array
  for (const t of tasks) {
    if (seen.has(t.id)) continue;
    seen.add(t.id);
    waves.push([t]);
  }
  return waves;
}

/**
 * Cards column: 按执行波次顺序；可并行的波用外框 + 色条区分。
 * 左侧波次轨已并入此列，不再分两栏展示。
 */
export function cardsHtml(job, byId, opts = {}) {
  const tasks = job?.tasks || [];
  if (!tasks.length) return '<p class="muted">暂无步骤</p>';
  const waves = orderedWaveTasks(job, byId);
  if (!waves.length) return '<p class="muted">暂无步骤</p>';

  return waves
    .map((waveTasks, i) => {
      const n = waveTasks.length;
      const parallel = n > 1;
      const tone = i % 6; // 0..5 色循环
      const kindClass = parallel ? " is-parallel" : " is-serial";
      // S2-3: 甲扫左栏 —「第 N 批 · M 步一起」/「按顺序」
      const label = parallel
        ? `第 ${i + 1} 批 · ${n} 步一起`
        : `第 ${i + 1} 批 · 按顺序`;
      const rows = waveTasks.map((t) => taskCardHtml(t, byId, opts)).join("");
      return (
        `<div class="split-wave-group tone-${tone}${kindClass}" data-wave="${i + 1}">` +
        `<div class="split-wave-group-head">` +
        `<span class="split-wave-group-label">${esc(label)}</span>` +
        (parallel
          ? `<span class="split-wave-parallel-tag">一起做</span>`
          : "") +
        `</div>` +
        `<div class="split-wave-group-body">${rows}</div>` +
        `</div>`
      );
    })
    .join("");
}

export { esc };
