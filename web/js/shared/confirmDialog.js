/**
 * [INPUT]: 无 IPC；纯 DOM overlay（复用 .modal 视觉）
 * [OUTPUT]: confirmDialog({title,body,okLabel,cancelLabel,danger}) → Promise<boolean>
 * [POS]: shared — 应用内确认层，替代 window.confirm
 *   （桌面 WKWebView 的原生 confirm/prompt 可能静默返回 null/false，且视觉割裂）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

let _overlay = null;
let _resolve = null;

// val: true (ok) | false (cancel/backdrop/Esc) | string (extraButton.value)
function closeWith(val) {
  if (_overlay) _overlay.hidden = true;
  const r = _resolve;
  _resolve = null;
  if (r) r(val);
}

function onKeydown(ev) {
  if (!_overlay || _overlay.hidden) return;
  if (ev.key === "Escape") {
    ev.preventDefault();
    closeWith(false);
  }
}

function ensureDom() {
  if (_overlay) return _overlay;
  const el = document.createElement("div");
  el.className = "modal cco-confirm";
  el.hidden = true;
  el.style.zIndex = "140"; // above #modal (100) — confirm can stack on dialogs
  el.innerHTML =
    `<div class="modal-backdrop" data-confirm-cancel></div>` +
    `<div class="modal-card cco-confirm-card" role="alertdialog" aria-modal="true" aria-labelledby="cco-confirm-title">` +
    `<div class="modal-head"><h2 id="cco-confirm-title"></h2></div>` +
    `<p class="modal-hint cco-confirm-body" style="white-space:pre-line"></p>` +
    `<div class="modal-actions">` +
    `<button type="button" class="btn ghost" data-confirm-cancel></button>` +
    `<button type="button" class="btn secondary" data-confirm-extra hidden></button>` +
    `<button type="button" class="btn primary" data-confirm-ok></button>` +
    `</div></div>`;
  el.querySelectorAll("[data-confirm-cancel]").forEach((b) => {
    b.addEventListener("click", () => closeWith(false));
  });
  el.querySelector("[data-confirm-ok]").addEventListener("click", () =>
    closeWith(true)
  );
  const extraBtn = el.querySelector("[data-confirm-extra]");
  extraBtn.addEventListener("click", () =>
    closeWith(extraBtn.dataset.confirmValue ?? "extra")
  );
  document.addEventListener("keydown", onKeydown, true);
  document.body.appendChild(el);
  _overlay = el;
  return el;
}

/**
 * In-app confirm. Resolves false on cancel / backdrop / Escape — never hangs.
 * Resolves true on ok, or extraButton.value (string) when the extra button is clicked.
 * @param {{title?:string, body?:string, okLabel?:string, cancelLabel?:string, danger?:boolean, extraButton?:{label:string,value:string}}} opts
 * @returns {Promise<boolean|string>}
 */
export function confirmDialog(opts = {}) {
  const {
    title = "确认",
    body = "",
    okLabel = "确定",
    cancelLabel = "取消",
    danger = false,
    extraButton = null,
  } = opts;
  const el = ensureDom();
  // A second confirm while one is open cancels the first (last wins).
  if (_resolve) closeWith(false);
  el.querySelector("#cco-confirm-title").textContent = title;
  el.querySelector(".cco-confirm-body").textContent = body;
  const okBtn = el.querySelector("[data-confirm-ok]");
  const cancelBtn = el.querySelector("button[data-confirm-cancel]");
  const extraBtn = el.querySelector("[data-confirm-extra]");
  okBtn.textContent = okLabel;
  cancelBtn.textContent = cancelLabel;
  okBtn.classList.toggle("danger", !!danger);
  okBtn.classList.toggle("primary", !danger);
  if (extraButton) {
    extraBtn.textContent = extraButton.label;
    extraBtn.dataset.confirmValue = extraButton.value ?? "extra";
    extraBtn.hidden = false;
  } else {
    extraBtn.hidden = true;
  }
  el.hidden = false;
  // Danger defaults focus to cancel so Enter can't destroy by accident.
  try {
    (danger ? cancelBtn : okBtn).focus();
  } catch (_) {}
  return new Promise((resolve) => {
    _resolve = resolve;
  });
}

/** Bridge for classic scripts / feature hosts. */
export function installConfirmDialog(
  target = typeof window !== "undefined" ? window : globalThis
) {
  if (!target) return;
  target.ccoConfirm = confirmDialog;
}

export default confirmDialog;
