/**
 * [INPUT]: localStorage
 * [OUTPUT]: 对内 delivery（旧 L/M/H）+ 空态 advanced 纠正 / coach 文案
 * [POS]: features/chat — W0-8 主路径不教三档；与 workStyle / persona 正交
 * [PROTOCOL]: 对外禁止 L/M/H 英雄键；对内 trial|single|bundle（存盘仍用 L/M/H 兼容）
 *
 * 只影响 author 权重；不改 plan_mode、不写 gateway、不开跑。
 * 聊天页头不再常驻全链路步骤条（方位靠 #page-title/#page-sub + 段控；绑定靠本轮上下文）。
 */

const KEY = "cco.pathMode";

/** In-memory fallback when localStorage is unavailable (tests / private mode). */
let memoryPath = /** @type {PathModeId|null} */ (null);

/** @typedef {'L'|'M'|'H'} PathModeId */
/** @typedef {'trial'|'single'|'bundle'} DeliveryId */

/** Internal delivery meta. Labels are human; never teach L/M/H on main path. */
export const PATH_MODES = Object.freeze({
  L: {
    delivery: /** @type {DeliveryId} */ ("trial"),
    label: "先看一版就好",
    hint: "一句话先验证像不像（系统内部 trial）",
  },
  M: {
    delivery: /** @type {DeliveryId} */ ("single"),
    label: "就这一件事",
    hint: "一份计划拆开做（默认 single）",
  },
  H: {
    delivery: /** @type {DeliveryId} */ ("bundle"),
    label: "好几件事一起排",
    hint: "多材料/多页时系统偏本波目录（bundle；引擎后置）",
  },
});

export const DEFAULT_PATH_MODE = /** @type {PathModeId} */ ("M");

/** @returns {PathModeId} */
export function getPathMode() {
  try {
    const raw = String(localStorage.getItem(KEY) || "").trim().toUpperCase();
    if (raw === "L" || raw === "M" || raw === "H") return raw;
  } catch (_) {}
  if (memoryPath === "L" || memoryPath === "M" || memoryPath === "H") {
    return memoryPath;
  }
  return DEFAULT_PATH_MODE;
}

/** @returns {DeliveryId} */
export function getDeliveryId() {
  return PATH_MODES[getPathMode()]?.delivery || "single";
}

/**
 * @param {PathModeId|DeliveryId|string} id
 * @returns {PathModeId}
 */
export function setPathMode(id) {
  let next = DEFAULT_PATH_MODE;
  const raw = String(id || "").trim();
  const up = raw.toUpperCase();
  if (up === "L" || raw === "trial") next = "L";
  else if (up === "H" || raw === "bundle") next = "H";
  else if (up === "M" || raw === "single") next = "M";
  memoryPath = next;
  try {
    localStorage.setItem(KEY, next);
  } catch (_) {}
  return next;
}

/**
 * W0-8: **no hero segment**. Optional advanced fold with human corrects only.
 * @param {{ advanced?: boolean }} [opts]
 */
export function pathModeSegmentHtml(opts = {}) {
  if (!opts.advanced) return "";
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
    `<details class="chat-delivery-advanced">` +
    `<summary class="chat-delivery-advanced-sum">范围不对？可改（一般不用点）</summary>` +
    `<div class="chat-path-mode chat-path-mode-advanced" role="group" aria-label="纠正范围">` +
    `<div class="chat-path-segs">${btns}</div>` +
    `<p class="chat-delivery-advanced-hint muted">默认由你说的话和上面例子自动判断；这里只是纠偏。</p>` +
    `</div></details>`
  );
}

/**
 * Empty-state coach — action on this screen only; no full-product pipeline.
 * Prefer persona coach in empty lead; this is a fallback one-liner.
 */
export function pathModeCoachHtml() {
  return (
    `<p class="chat-path-coach">` +
    `在<strong>下方输入框</strong>说要做成什么，或拖入文件；也可先点一个像你业务的例子。` +
    `计划写好后再去拆步执行。` +
    `</p>`
  );
}

/**
 * How empty-state should treat clarify grill (internal delivery).
 * @returns {'hide'|'fold'}
 */
export function pathModeClarifyWeight() {
  if (getDeliveryId() === "trial") return "hide";
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
