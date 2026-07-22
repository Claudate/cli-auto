/**
 * [INPUT]: document capture click · open details / menu roots
 * [OUTPUT]: close matching open panels when click is outside
 * [POS]: shell-chrome B2 · shared helper
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * note: 不误伤 native/custom select、modal、对话框；与 selectUi 并存
 */

const DEFAULT_SELECTORS = [
  "details.split-more-actions[open]",
  "details.split-route-advanced[open]",
  "details.split-quality[open]",
  "details.split-detail-tech[open]",
  "details.split-detail-full[open]",
  "details.planner-log-fold[open]",
  "details.log-advanced[open]",
  "details.settings-advanced[open]",
  "details.help-advanced[open]",
  ".cco-menu.is-open",
  "[data-click-outside-root][data-open='1']",
];

function isInsideSelectOrModal(target) {
  if (!target || !target.closest) return false;
  if (target.closest(".cco-select")) return true;
  if (target.closest("select")) return true;
  if (target.closest(".modal, [role='dialog'], #modal, .plan-full-modal, .img-lightbox")) {
    return true;
  }
  // custom select panel may portal under body
  if (target.closest(".cco-select-panel, .cco-select-dropdown")) return true;
  return false;
}

/**
 * @param {object} [opts]
 * @param {string[]} [opts.selectors] CSS selectors for open panels to close
 * @returns {() => void} dispose
 */
export function installClickOutside(opts = {}) {
  if (typeof document === "undefined") return () => {};
  if (document.documentElement.dataset.ccoClickOutside === "1") {
    return () => {};
  }
  document.documentElement.dataset.ccoClickOutside = "1";
  const selectors = opts.selectors || DEFAULT_SELECTORS;

  const onPointer = (e) => {
    const t = e.target;
    if (!t || isInsideSelectOrModal(t)) return;
    for (const sel of selectors) {
      let nodes;
      try {
        nodes = document.querySelectorAll(sel);
      } catch (_) {
        continue;
      }
      nodes.forEach((node) => {
        if (node.contains(t)) return;
        // click on summary of another details — still close this one
        if (node.tagName === "DETAILS") {
          node.open = false;
          return;
        }
        node.classList.remove("is-open");
        if (node.dataset) node.dataset.open = "0";
        if (typeof node.hidePopover === "function") {
          try {
            node.hidePopover();
          } catch (_) {}
        }
      });
    }
  };

  document.addEventListener("pointerdown", onPointer, true);
  return () => {
    document.removeEventListener("pointerdown", onPointer, true);
    delete document.documentElement.dataset.ccoClickOutside;
  };
}

export function installClickOutsideGlobal(
  target = typeof window !== "undefined" ? window : globalThis
) {
  if (!target) return () => {};
  const dispose = installClickOutside();
  target.ccoClickOutside = { install: installClickOutside, dispose };
  return dispose;
}

export default { installClickOutside, installClickOutsideGlobal };
