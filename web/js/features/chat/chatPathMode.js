/**
 * [INPUT]: localStorage
 * [OUTPUT]: path L/M/H prefs + empty-state segment HTML + head-step copy
 * [POS]: features/chat — W0 工作方式（与 workStyle 职业习惯正交）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 只影响 author 空态权重与文案；不改 plan_mode、不写 gateway 策略、不开跑。
 */

const KEY = "cco.pathMode";

/** @typedef {'L'|'M'|'H'} PathModeId */

/** @type {Record<PathModeId, { label: string, hint: string, headStep: string }>} */
export const PATH_MODES = Object.freeze({
  L: {
    label: "快试",
    hint: "一句话说清，尽快看到像不像",
    headStep: "说一句 → 看结果",
  },
  M: {
    label: "写一份计划",
    hint: "经典：说清 → 计划 → 拆步 → 跑 → 验",
    headStep: "计划 → 拆 → 跑 → 验",
  },
  H: {
    label: "多需求一起排",
    hint: "多材料/多计划索引（引擎稍后）；先把主需求说清",
    headStep: "材料 → 索引 → 多计划…",
  },
});

export const DEFAULT_PATH_MODE = /** @type {PathModeId} */ ("M");

/** @returns {PathModeId} */
export function getPathMode() {
  try {
    const raw = String(localStorage.getItem(KEY) || "").trim().toUpperCase();
    if (raw === "L" || raw === "M" || raw === "H") return raw;
  } catch (_) {}
  return DEFAULT_PATH_MODE;
}

/**
 * @param {PathModeId|string} id
 * @returns {PathModeId}
 */
export function setPathMode(id) {
  const next =
    id === "L" || id === "M" || id === "H" ? id : DEFAULT_PATH_MODE;
  try {
    localStorage.setItem(KEY, next);
  } catch (_) {}
  return next;
}

/** @param {PathModeId} [mode] */
export function pathModeHeadStepText(mode) {
  const m = PATH_MODES[mode || getPathMode()] || PATH_MODES.M;
  return m.headStep;
}

/**
 * Update `.chat-head-step` lead text; keep project label span if present.
 * @param {PathModeId} [mode]
 */
export function applyPathModeHeadStep(mode) {
  if (typeof document === "undefined") return;
  const el = document.querySelector(".chat-head-step");
  if (!el) return;
  const step = pathModeHeadStepText(mode);
  const proj = el.querySelector("#chat-project-label");
  const projHtml = proj ? proj.outerHTML : "";
  el.innerHTML = projHtml
    ? `${escapeHtml(step)} · ${projHtml}`
    : escapeHtml(step);
}

/** Segment control for empty state. */
export function pathModeSegmentHtml() {
  const cur = getPathMode();
  const btns = /** @type {PathModeId[]} */ (["L", "M", "H"])
    .map((id) => {
      const meta = PATH_MODES[id];
      const active = id === cur ? " is-active" : "";
      return (
        `<button type="button" class="chat-path-seg${active}"` +
        ` data-path-mode="${id}"` +
        ` title="${escapeHtml(meta.hint)}"` +
        ` aria-pressed="${id === cur ? "true" : "false"}">` +
        `${escapeHtml(meta.label)}` +
        `</button>`
      );
    })
    .join("");
  return (
    `<div class="chat-path-mode" role="group" aria-label="本次怎么干">` +
    `<p class="chat-path-mode-label">本次怎么干？</p>` +
    `<div class="chat-path-segs">${btns}</div>` +
    `</div>`
  );
}

/** Coach under path segs — points at bottom composer. */
export function pathModeCoachHtml() {
  const cur = getPathMode();
  let line =
    "在<strong>下方输入框</strong>说要做成什么，或拖入文件。点发送后开始。";
  if (cur === "L") {
    line =
      "快试：用一句话说目标，发下去即可——<strong>不必先答问卷</strong>。输入框在下面。";
  } else if (cur === "H") {
    line =
      "多需求：先把<strong>这一波主目标</strong>写进下方输入框；多计划索引能力稍后上线，可先附材料。";
  }
  return `<p class="chat-path-coach">${line}</p>`;
}

/**
 * How empty-state should treat clarify grill.
 * @returns {'hide'|'fold'}
 */
export function pathModeClarifyWeight() {
  const m = getPathMode();
  if (m === "L") return "hide";
  return "fold";
}

/**
 * Claimed success without re-pasting plan body (avoids dual-card same text).
 * Assign still goes through existing data-clarify-assign → host.assignFromChat.
 */
export function thinClaimSuccessHtml() {
  return (
    `<div class="chat-clarify chat-clarify-thin" data-clarify-claimed="1">` +
    `<div class="chat-claim-success chat-claim-success-thin">` +
    `<p class="chat-claim-success-title">计划草稿已写好</p>` +
    `<p class="chat-claim-success-next">下一步：拆成可并行的步骤（不会自动开始执行）。</p>` +
    `<div class="chat-claim-success-actions">` +
    `<button type="button" class="btn primary sm" data-clarify-assign="1" title="进入拆分台，不会自动开始执行">拆成步骤</button>` +
    `<button type="button" class="btn ghost sm" data-clarify-rechat="1">再改一改</button>` +
    `</div></div></div>`
  );
}

function escapeHtml(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}