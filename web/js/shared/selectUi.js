/**
 * [INPUT]: document 内原生 <select>
 * [OUTPUT]: macOS 风自定义下拉；同步 .value / change / disabled / options
 * [POS]: web/js/shared · 表单控件增强；不写业务策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 用法：installSelectUi() 一次即可；MutationObserver 跟进动态 option / 新 select。
 * 业务代码继续读 el.value / 写 el.value / 听 change，无需改调用方。
 */

const CHEVRON_SVG =
  '<svg class="cco-select__chevron" viewBox="0 0 12 8" aria-hidden="true" focusable="false"><path d="M1.5 1.75L6 6.25L10.5 1.75" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>';

const CHECK_SVG =
  '<svg class="cco-select__check" viewBox="0 0 12 12" aria-hidden="true" focusable="false"><path d="M2.2 6.2L4.8 8.8L9.8 3.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>';

/** @type {WeakMap<HTMLSelectElement, SelectShell>} */
const shells = new WeakMap();

/** @type {SelectShell | null} */
let openShell = null;

class SelectShell {
  /**
   * @param {HTMLSelectElement} select
   * @param {{ size?: "sm" | "md"; menuEnd?: boolean; inline?: boolean }} opts
   */
  constructor(select, opts = {}) {
    this.select = select;
    this.size = opts.size || (select.classList.contains("chat-session-select") || select.id === "confirm-task-provider" ? "sm" : "md");
    this.menuEnd = !!opts.menuEnd || select.id === "chat-session-select";
    this.inline = !!opts.inline || this.size === "sm" || select.classList.contains("chat-session-select");
    this.activeIndex = -1;
    this._syncing = false;
    this._rebuildQueued = false;

    this.root = document.createElement("div");
    this.root.className = "cco-select is-enhanced";
    if (this.size === "sm") this.root.classList.add("cco-select--sm");
    if (this.inline) this.root.classList.add("cco-select--inline");
    if (this.menuEnd) this.root.classList.add("cco-select--menu-end");

    this.trigger = document.createElement("button");
    this.trigger.type = "button";
    this.trigger.className = "cco-select__trigger";
    this.trigger.setAttribute("aria-haspopup", "listbox");
    this.trigger.setAttribute("aria-expanded", "false");
    if (select.id) this.trigger.id = `${select.id}__trigger`;
    if (select.getAttribute("aria-label")) {
      this.trigger.setAttribute("aria-label", select.getAttribute("aria-label"));
    }

    this.labelEl = document.createElement("span");
    this.labelEl.className = "cco-select__label";
    this.trigger.appendChild(this.labelEl);
    this.trigger.insertAdjacentHTML("beforeend", CHEVRON_SVG);

    this.menu = document.createElement("ul");
    this.menu.className = "cco-select__menu";
    this.menu.setAttribute("role", "listbox");
    this.menu.hidden = true;
    if (select.id) this.menu.id = `${select.id}__menu`;
    this.trigger.setAttribute("aria-controls", this.menu.id || "");

    const parent = select.parentNode;
    if (!parent) return;
    parent.insertBefore(this.root, select);
    this.root.appendChild(select);
    this.root.appendChild(this.trigger);
    this.root.appendChild(this.menu);
    select.classList.add("cco-select__native");
    // Keep native for form/.value sync; keyboard goes to trigger only.
    select.tabIndex = -1;
    select.setAttribute("aria-hidden", "true");

    this._onTriggerClick = (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (select.disabled) return;
      if (this.isOpen()) this.close();
      else this.open();
    };
    this._onTriggerKey = (e) => this._handleTriggerKey(e);
    this._onMenuClick = (e) => this._handleMenuClick(e);
    this._onMenuKey = (e) => this._handleMenuKey(e);
    this._onNativeChange = () => {
      if (this._syncing) return;
      this._syncFromNative();
    };
    this._onNativeFocus = () => this.trigger.focus();

    this.trigger.addEventListener("click", this._onTriggerClick);
    this.trigger.addEventListener("keydown", this._onTriggerKey);
    this.menu.addEventListener("click", this._onMenuClick);
    this.menu.addEventListener("keydown", this._onMenuKey);
    select.addEventListener("change", this._onNativeChange);
    select.addEventListener("focus", this._onNativeFocus);

    // Patch value setter so JS assignments refresh the label/menu.
    this._patchValueAccessor();

    this._optionObserver = new MutationObserver(() => this.queueRebuild());
    this._optionObserver.observe(select, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["disabled", "selected", "value", "label"],
    });

    this._attrObserver = new MutationObserver(() => this._syncDisabled());
    this._attrObserver.observe(select, {
      attributes: true,
      attributeFilter: ["disabled", "aria-label", "title"],
    });

    this.rebuild();
    this._syncDisabled();
  }

  _patchValueAccessor() {
    const sel = this.select;
    const proto = HTMLSelectElement.prototype;
    const shell = this;
    const valueDesc = Object.getOwnPropertyDescriptor(proto, "value");
    const indexDesc = Object.getOwnPropertyDescriptor(proto, "selectedIndex");
    try {
      if (valueDesc?.get && valueDesc?.set) {
        Object.defineProperty(sel, "value", {
          configurable: true,
          enumerable: true,
          get() {
            return valueDesc.get.call(this);
          },
          set(v) {
            valueDesc.set.call(this, v);
            if (!shell._syncing) shell._syncFromNative();
          },
        });
      }
      if (indexDesc?.get && indexDesc?.set) {
        Object.defineProperty(sel, "selectedIndex", {
          configurable: true,
          enumerable: true,
          get() {
            return indexDesc.get.call(this);
          },
          set(v) {
            indexDesc.set.call(this, v);
            if (!shell._syncing) shell._syncFromNative();
          },
        });
      }
    } catch {
      // Some environments disallow redefining; rebuild on change only.
    }
  }

  queueRebuild() {
    if (this._rebuildQueued) return;
    this._rebuildQueued = true;
    queueMicrotask(() => {
      this._rebuildQueued = false;
      this.rebuild();
    });
  }

  rebuild() {
    const wasOpen = this.isOpen();
    const opts = [...this.select.options];
    this.menu.innerHTML = "";
    if (!opts.length) {
      const empty = document.createElement("li");
      empty.className = "cco-select__empty";
      empty.textContent = "暂无选项";
      this.menu.appendChild(empty);
    } else {
      opts.forEach((opt, i) => {
        const li = document.createElement("li");
        li.setAttribute("role", "presentation");
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "cco-select__option";
        btn.setAttribute("role", "option");
        btn.dataset.index = String(i);
        btn.dataset.value = opt.value;
        if (opt.disabled) {
          btn.disabled = true;
          btn.classList.add("is-disabled");
        }
        const text = document.createElement("span");
        text.className = "cco-select__option-text";
        text.textContent = opt.label || opt.textContent || opt.value;
        btn.appendChild(text);
        btn.insertAdjacentHTML("beforeend", CHECK_SVG);
        li.appendChild(btn);
        this.menu.appendChild(li);
      });
    }
    this._syncFromNative();
    if (wasOpen) {
      this._placeMenu();
      this._markActive(this._selectedIndex());
    }
  }

  _syncFromNative() {
    const sel = this.select;
    const idx = sel.selectedIndex;
    const opt = idx >= 0 ? sel.options[idx] : null;
    this.labelEl.textContent = opt
      ? opt.label || opt.textContent || opt.value || "—"
      : "—";
    this.menu.querySelectorAll(".cco-select__option").forEach((btn) => {
      const i = Number(btn.dataset.index);
      const selected = i === idx;
      btn.classList.toggle("is-selected", selected);
      btn.setAttribute("aria-selected", selected ? "true" : "false");
    });
    this._syncDisabled();
  }

  _syncDisabled() {
    const d = !!this.select.disabled;
    this.trigger.disabled = d;
    this.root.classList.toggle("is-disabled", d);
    if (this.select.getAttribute("aria-label")) {
      this.trigger.setAttribute(
        "aria-label",
        this.select.getAttribute("aria-label")
      );
    }
    if (this.select.title) this.trigger.title = this.select.title;
    if (d && this.isOpen()) this.close();
  }

  isOpen() {
    return !this.menu.hidden;
  }

  open() {
    if (this.select.disabled) return;
    if (openShell && openShell !== this) openShell.close();
    this.rebuild();
    this.menu.hidden = false;
    this.trigger.setAttribute("aria-expanded", "true");
    this.root.classList.add("is-open");
    this._placeMenu();
    const idx = this._selectedIndex();
    this._markActive(idx >= 0 ? idx : 0);
    this._scrollActiveIntoView();
    openShell = this;
    // Focus first option for keyboard; keep trigger focused for Esc simplicity
    this.trigger.focus();
  }

  close() {
    if (this.menu.hidden) return;
    this.menu.hidden = true;
    this.trigger.setAttribute("aria-expanded", "false");
    this.root.classList.remove("is-open", "is-drop-up");
    this.menu.querySelectorAll(".cco-select__option.is-active").forEach((el) => {
      el.classList.remove("is-active");
    });
    this.activeIndex = -1;
    if (openShell === this) openShell = null;
  }

  _placeMenu() {
    // Prefer below; flip up if not enough room.
    this.root.classList.remove("is-drop-up");
    const rect = this.trigger.getBoundingClientRect();
    const menuH = Math.min(this.menu.scrollHeight, window.innerHeight * 0.5);
    const below = window.innerHeight - rect.bottom;
    const above = rect.top;
    if (below < menuH + 12 && above > below) {
      this.root.classList.add("is-drop-up");
    }
  }

  _selectedIndex() {
    return this.select.selectedIndex;
  }

  _optionButtons() {
    return [...this.menu.querySelectorAll(".cco-select__option:not(:disabled)")];
  }

  _markActive(index) {
    const all = [...this.menu.querySelectorAll(".cco-select__option")];
    all.forEach((btn) => btn.classList.remove("is-active"));
    const target = all.find((b) => Number(b.dataset.index) === index);
    if (target && !target.disabled) {
      target.classList.add("is-active");
      this.activeIndex = index;
    } else {
      this.activeIndex = -1;
    }
  }

  _scrollActiveIntoView() {
    const active = this.menu.querySelector(".cco-select__option.is-active");
    if (active) active.scrollIntoView({ block: "nearest" });
  }

  /** True when this shell owns focus or its menu is open. */
  isInteracting() {
    if (this.isOpen()) return true;
    const ae = document.activeElement;
    return !!(ae && this.root.contains(ae));
  }

  /**
   * @param {number} index
   * @param {{ close?: boolean }} [opts]
   */
  chooseIndex(index, opts = {}) {
    const opt = this.select.options[index];
    if (!opt || opt.disabled) return;
    const next = opt.value;
    const prev = this.select.value;
    this._syncing = true;
    this.select.selectedIndex = index;
    this.select.value = next;
    this._syncing = false;
    this._syncFromNative();
    if (prev !== this.select.value) {
      this.select.dispatchEvent(new Event("change", { bubbles: true }));
      this.select.dispatchEvent(new Event("input", { bubbles: true }));
    }
    if (opts.close !== false) {
      this.close();
      this.trigger.focus();
    }
  }

  _handleMenuClick(e) {
    const btn = e.target?.closest?.(".cco-select__option");
    if (!btn || btn.disabled) return;
    e.preventDefault();
    this.chooseIndex(Number(btn.dataset.index));
  }

  _handleTriggerKey(e) {
    const key = e.key;
    if (key === "ArrowDown" || key === "ArrowUp" || key === "Enter" || key === " ") {
      e.preventDefault();
      if (!this.isOpen()) {
        this.open();
        if (key === "ArrowUp") {
          const buttons = this._optionButtons();
          const last = buttons[buttons.length - 1];
          if (last) this._markActive(Number(last.dataset.index));
        }
      } else if (key === "Enter" || key === " ") {
        if (this.activeIndex >= 0) this.chooseIndex(this.activeIndex);
        else this.close();
      } else if (key === "ArrowDown") {
        this._moveActive(1);
      } else if (key === "ArrowUp") {
        this._moveActive(-1);
      }
    } else if (key === "Escape" && this.isOpen()) {
      e.preventDefault();
      this.close();
    } else if (key === "Home" && this.isOpen()) {
      e.preventDefault();
      const first = this._optionButtons()[0];
      if (first) {
        this._markActive(Number(first.dataset.index));
        this._scrollActiveIntoView();
      }
    } else if (key === "End" && this.isOpen()) {
      e.preventDefault();
      const buttons = this._optionButtons();
      const last = buttons[buttons.length - 1];
      if (last) {
        this._markActive(Number(last.dataset.index));
        this._scrollActiveIntoView();
      }
    }
  }

  _handleMenuKey(e) {
    // Menu itself is not focused; keys handled on trigger.
    this._handleTriggerKey(e);
  }

  _moveActive(delta) {
    const buttons = this._optionButtons();
    if (!buttons.length) return;
    const indices = buttons.map((b) => Number(b.dataset.index));
    let pos = indices.indexOf(this.activeIndex);
    if (pos < 0) pos = delta > 0 ? -1 : 0;
    pos = (pos + delta + indices.length) % indices.length;
    this._markActive(indices[pos]);
    this._scrollActiveIntoView();
  }

  destroy() {
    this.close();
    this._optionObserver?.disconnect();
    this._attrObserver?.disconnect();
    this.trigger.removeEventListener("click", this._onTriggerClick);
    this.trigger.removeEventListener("keydown", this._onTriggerKey);
    this.menu.removeEventListener("click", this._onMenuClick);
    this.menu.removeEventListener("keydown", this._onMenuKey);
    this.select.removeEventListener("change", this._onNativeChange);
    this.select.removeEventListener("focus", this._onNativeFocus);
    const parent = this.root.parentNode;
    if (parent) {
      parent.insertBefore(this.select, this.root);
      parent.removeChild(this.root);
    }
    this.select.classList.remove("cco-select__native");
    shells.delete(this.select);
  }
}

/**
 * @param {HTMLSelectElement} select
 * @param {{ size?: "sm" | "md"; menuEnd?: boolean; inline?: boolean }} [opts]
 */
export function enhanceSelect(select, opts) {
  if (!(select instanceof HTMLSelectElement)) return null;
  if (select.dataset.ccoSelect === "off") return null;
  if (shells.has(select)) return shells.get(select);
  // Skip selects already inside an enhanced root (double-run guard)
  if (select.closest(".cco-select.is-enhanced")) {
    const existing = shells.get(select);
    if (existing) return existing;
  }
  const shell = new SelectShell(select, opts || {});
  shells.set(select, shell);
  select.dataset.ccoSelect = "on";
  return shell;
}

/**
 * Enhance all selects under root (default document).
 * @param {ParentNode} [root]
 */
export function enhanceAllSelects(root = document) {
  const list = root.querySelectorAll?.("select") || [];
  list.forEach((el) => enhanceSelect(el));
}

/**
 * Whether a select (or its enhanced shell) currently has focus / open menu.
 * Use before overwriting `.value` during re-render.
 * @param {HTMLSelectElement | null | undefined} select
 */
export function isSelectBusy(select) {
  if (!(select instanceof HTMLSelectElement)) return false;
  const shell = shells.get(select);
  if (shell) return shell.isInteracting();
  if (document.activeElement === select) return true;
  const root = select.closest?.(".cco-select");
  if (root?.classList.contains("is-open")) return true;
  if (root && document.activeElement && root.contains(document.activeElement)) {
    return true;
  }
  return false;
}

function onDocumentPointerDown(e) {
  if (!openShell) return;
  if (openShell.root.contains(e.target)) return;
  openShell.close();
}

function onDocumentKeydown(e) {
  if (e.key === "Escape" && openShell) {
    openShell.close();
  }
}

function onViewportChange() {
  if (openShell) openShell._placeMenu();
}

/**
 * Install global select enhancement once.
 * @param {{ root?: ParentNode }} [opts]
 */
export function installSelectUi(opts = {}) {
  if (typeof document === "undefined") return { enhanceSelect, enhanceAllSelects };
  if (window.__ccoSelectUiInstalled) {
    enhanceAllSelects(opts.root || document);
    return window.__ccoSelectUi;
  }

  enhanceAllSelects(opts.root || document);

  document.addEventListener("pointerdown", onDocumentPointerDown, true);
  document.addEventListener("keydown", onDocumentKeydown, true);
  window.addEventListener("resize", onViewportChange);
  window.addEventListener("scroll", onViewportChange, true);

  // Catch late-mounted selects (modals, dynamic pages).
  const mo = new MutationObserver((mutations) => {
    for (const m of mutations) {
      if (m.type === "childList") {
        m.addedNodes.forEach((node) => {
          if (!(node instanceof Element)) return;
          if (node.matches?.("select")) enhanceSelect(node);
          else if (node.querySelectorAll) {
            node.querySelectorAll("select").forEach((el) => enhanceSelect(el));
          }
        });
      }
    }
  });
  mo.observe(document.documentElement, { childList: true, subtree: true });

  const api = {
    enhanceSelect,
    enhanceAllSelects,
    isSelectBusy,
    /** force rebuild for a select (e.g. after bulk option rewrite) */
    refresh(select) {
      const shell = shells.get(select);
      if (shell) shell.rebuild();
    },
  };
  window.__ccoSelectUiInstalled = true;
  window.__ccoSelectUi = api;
  window.ccoSelectUi = api;
  return api;
}

export default {
  installSelectUi,
  enhanceSelect,
  enhanceAllSelects,
  isSelectBusy,
};
