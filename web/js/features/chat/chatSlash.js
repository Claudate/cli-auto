/**
 * [INPUT]: chatApi.slashCatalog · #chat-input · #chat-cli
 * [OUTPUT]: #chat-slash-menu (composer autocomplete dropdown)
 * [POS]: A5-2a features/chat — renders Rust-sourced slash catalog only;
 *   routing/scope logic lives in services/chat/commands.rs (no policy copy).
 * [PROTOCOL]: 变更时更新此头部；透传命令仅提示，是否支持看 CLI；reserved 标灰不可选。
 */

import { $ } from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { chatEsc } from "./chatFormat.js";

let catalogCache = null; // full Vec<SlashCommandInfo> (all CLIs, unfiltered)
let menuActive = false;
let menuItems = []; // rendered SlashCommandInfo rows
let menuIndex = -1; // highlighted row
let openPrefix = ""; // typed text after "/" used to filter

/** Load the full catalog once; refresh on demand via `forceRefreshCatalog`. */
export async function ensureSlashCatalog(force) {
  if (catalogCache && !force) return catalogCache;
  try {
    const rows = await chatApi.slashCatalog(null);
    if (Array.isArray(rows)) {
      catalogCache = rows;
      return rows;
    }
  } catch (_) {
    /* non-fatal: no autocomplete, chat still works */
  }
  return catalogCache || [];
}

export function forceRefreshSlashCatalog() {
  catalogCache = null;
  return ensureSlashCatalog(true);
}

/** Current picked CLI channel (matches what chat_send uses). */
function currentCli() {
  const sel = $("#chat-cli");
  return sel && sel.value ? sel.value : "claude";
}

/**
 * Compute the menu rows for the current CLI + a typed prefix.
 * Local commands show for every CLI; passthrough only for the matching CLI;
 * reserved are greyed (scope=reserved) and never completable.
 */
function filterRows(prefix) {
  const cli = currentCli();
  const rows = catalogCache || [];
  const p = prefix.trim().toLowerCase();
  return rows.filter((r) => {
    if (!p || r.cmd.toLowerCase().startsWith(p)) {
      if (r.scope === "local") return true;
      if (r.scope === "reserved") return true; // shown greyed
      if (r.scope === "passthrough") return true; // catalog already per-CLI
      return false;
    }
    return false;
  });
}

function menuEl() {
  return $("#chat-slash-menu");
}

/** Render the dropdown; returns false when nothing to show. */
function renderMenu(prefix) {
  const menu = menuEl();
  if (!menu) return false;
  const items = filterRows(prefix);
  menuItems = items;
  menuIndex = -1;
  if (!items.length) {
    menu.hidden = true;
    menuActive = false;
    return false;
  }
  menu.innerHTML = items
    .map((r, i) => {
      const args = r.args ? ` ${chatEsc(r.args)}` : "";
      const cmd = `/ ${chatEsc(r.cmd)}${args}`.replace("/ ", "/");
      const grey = r.scope === "reserved" ? " is-reserved" : "";
      const scopeTag =
        r.scope === "passthrough" ? '<span class="chat-slash-scope">透传</span>' : "";
      return (
        `<div class="chat-slash-item${grey}" data-index="${i}" ` +
        `data-cmd="${chatEsc(r.cmd)}" data-args="${chatEsc(r.args)}">` +
        `<span class="chat-slash-cmd">${cmd}</span>` +
        `<span class="chat-slash-desc">${chatEsc(r.desc)}</span>${scopeTag}</div>`
      );
    })
    .join("");
  menu.hidden = false;
  menuActive = true;
  // Default-highlight the first selectable row so Enter accepts immediately.
  const first = items.findIndex((r) => r.scope !== "reserved");
  if (first >= 0) highlightIndex(first);
  return true;
}

function highlightIndex(i) {
  menuIndex = i;
  const els = menuEl()?.querySelectorAll(".chat-slash-item");
  els?.forEach((el, idx) => {
    el.classList.toggle("is-active", idx === i);
    if (idx === i) {
      try {
        el.scrollIntoView({ block: "nearest" });
      } catch (_) {}
    }
  });
}

function moveHighlight(delta) {
  if (!menuActive || !menuItems.length) return;
  let ni = menuIndex + delta;
  if (ni < 0) ni = menuItems.length - 1;
  if (ni >= menuItems.length) ni = 0;
  // skip reserved (greyed) rows while navigating
  while (menuItems[ni]?.scope === "reserved" && menuItems.length > 1) {
    ni = (ni + delta + menuItems.length) % menuItems.length;
    if (ni === menuIndex + delta) break; // guard infinite loop
  }
  highlightIndex(ni);
}

/** Insert the chosen `/cmd [args]` into the composer and focus. */
export function acceptSlashSuggestion() {
  const menu = menuEl();
  if (!menuActive || !menu || menu.hidden) return false;
  const item = menuItems[menuIndex];
  if (!item || item.scope === "reserved") return false;
  const input = $("#chat-input");
  if (!input) return false;
  // Replace the leading "/…" token with the completed command.
  const val = input.value;
  const caret = input.selectionStart ?? val.length;
  // Find the start of the token (after whitespace) so a mid-line "/" also works.
  let tokenStart = caret;
  while (tokenStart > 0 && !/\s/.test(val[tokenStart - 1])) tokenStart--;
  const before = val.slice(0, tokenStart);
  const after = val.slice(caret);
  const insertion = `/${item.cmd}${item.args ? " " + item.args : ""} `;
  input.value = before + insertion + after;
  closeSlashMenu();
  requestAnimationFrame(() => {
    input.focus();
    const pos = before.length + insertion.length;
    input.setSelectionRange(pos, pos);
  });
  return true;
}

function closeSlashMenu() {
  menuActive = false;
  menuItems = [];
  menuIndex = -1;
  const menu = menuEl();
  if (menu) menu.hidden = true;
}

/**
 * Called from the global keydown on `#chat-input`. Returns true when the event
 * was consumed by the autocomplete (caller should not also send / newline).
 */
export function handleSlashKeydown(e) {
  if (e.isComposing || e.keyCode === 229 || e.which === 229) return false;
  const input = $("#chat-input");
  if (!input || e.target !== input) return false;

  // Esc closes the menu.
  if (e.key === "Escape" && menuActive) {
    e.preventDefault();
    closeSlashMenu();
    return true;
  }
  // Arrow navigation while open.
  if (menuActive && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
    e.preventDefault();
    moveHighlight(e.key === "ArrowDown" ? 1 : -1);
    return true;
  }
  // Enter accepts the highlighted suggestion (not reserved).
  if (menuActive && e.key === "Enter") {
    if (menuIndex >= 0 && menuItems[menuIndex]?.scope !== "reserved") {
      e.preventDefault();
      acceptSlashSuggestion();
      return true;
    }
    closeSlashMenu();
    return false; // let Enter send as usual
  }
  // Tab accepts too (like other autocomplete).
  if (menuActive && e.key === "Tab") {
    if (menuIndex >= 0 && menuItems[menuIndex]?.scope !== "reserved") {
      e.preventDefault();
      acceptSlashSuggestion();
      return true;
    }
  }

  return false;
}

/** `input` event on the composer: open/re-filter the menu on a `/` token. */
export function handleSlashInput() {
  const input = $("#chat-input");
  if (!input) return;
  const val = input.value;
  const caret = input.selectionStart ?? val.length;
  const m = val.slice(0, caret).match(/(?:^|\s)\/([^\s/]*)$/);
  if (m) {
    openPrefix = m[1];
    if (catalogCache) {
      renderMenu(m[1]);
    } else {
      // First "/" ever typed: fetch the catalog, then open.
      ensureSlashCatalog().then(() => {
        const el = $("#chat-input");
        if (el && el.value.slice(0, el.selectionStart ?? el.value.length)
          .match(/(?:^|\s)\/([^\s/]*)$/)) {
          renderMenu(openPrefix);
        }
      });
    }
  } else if (menuActive) {
    closeSlashMenu();
  }
}

/** Focus/blur: close the menu when the composer loses focus (with click guard). */
export function bindSlashMenuDismiss() {
  const input = $("#chat-input");
  const menu = menuEl();
  if (!input || !menu) return;
  input.addEventListener("blur", () => {
    // Allow click on a menu row to complete before closing.
    setTimeout(() => {
      if (document.activeElement !== input && !menu.contains(document.activeElement)) {
        closeSlashMenu();
      }
    }, 120);
  });
  menu.addEventListener("mousedown", (e) => {
    e.preventDefault(); // keep focus in textarea so blur does not fire first
  });
  // Hover follows the pointer (keyboard highlight stays in sync).
  menu.addEventListener("mousemove", (e) => {
    const row = e.target.closest(".chat-slash-item");
    if (!row) return;
    const idx = Number(row.dataset.index);
    if (!Number.isFinite(idx) || idx === menuIndex) return;
    if (menuItems[idx]?.scope === "reserved") return;
    highlightIndex(idx);
  });
  menu.addEventListener("click", (e) => {
    const row = e.target.closest(".chat-slash-item");
    if (!row) return;
    const idx = Number(row.dataset.index);
    if (menuItems[idx]?.scope === "reserved") return;
    menuIndex = idx;
    acceptSlashSuggestion();
  });
}
