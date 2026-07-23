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
  "details.planner-log-fold[open]",
  "details.settings-advanced[open]",
  "details.help-advanced[open]",
  // 聊天页：多会话切换（B3 默认藏进 details；点空白应收）
  "details.chat-session-more[open]",
  ".cco-menu.is-open",
  "[data-click-outside-root][data-open='1']",
];

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
    if (!t) return;
    // 点在 modal / 对话框内：不收其它层（防误关）
    if (
      t.closest &&
      t.closest(".modal, [role='dialog'], #modal, .plan-full-modal, .img-lightbox")
    ) {
      return;
    }
    for (const sel of selectors) {
      let nodes;
      try {
        nodes = document.querySelectorAll(sel);
      } catch (_) {
        continue;
      }
      nodes.forEach((node) => {
        if (node.contains(t)) return;
        // 点在「本层内部」的 select 菜单/壳（含可能 portal 的 panel）→ 不关
        const selectRoot = t.closest?.(".cco-select");
        if (selectRoot && node.contains(selectRoot)) return;
        const portal = t.closest?.(".cco-select-panel, .cco-select-dropdown");
        if (portal && node.contains(portal)) return;
        // 点在其它区域（含页面其它 select）→ 收起本层
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

  const onKey = (e) => {
    if (e.key !== "Escape" && e.key !== "Esc") return;
    // Esc：收起所有注册的展开层（含聊天会话…）
    for (const sel of selectors) {
      let nodes;
      try {
        nodes = document.querySelectorAll(sel);
      } catch (_) {
        continue;
      }
      nodes.forEach((node) => {
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
  document.addEventListener("keydown", onKey, true);
  return () => {
    document.removeEventListener("pointerdown", onPointer, true);
    document.removeEventListener("keydown", onKey, true);
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
