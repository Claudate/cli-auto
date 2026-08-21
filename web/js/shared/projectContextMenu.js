/**
 * [INPUT]: gateway.openPath · window.selectProject / removeSelectedProject / ccoApp
 * [OUTPUT]: showProjectContextMenu · openProjectFolder
 * [POS]: shared shell helper；自 shellUi 抽出，控制 shellUi 体量
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function g(name) {
  return typeof window !== "undefined" ? window[name] : undefined;
}

function call(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

/** Open project path in Finder / system file manager via gateway.openPath. */
export function openProjectFolder(path) {
  const p = String(path || "").trim();
  if (!p) {
    call("toast", "没有可打开的路径");
    return;
  }
  const gw =
    (typeof window !== "undefined" && window.ccoGateway) ||
    g("ccoGateway") ||
    null;
  const open =
    (gw && typeof gw.openPath === "function" && gw.openPath.bind(gw)) || null;
  if (!open) {
    call("toast", "打开文件夹不可用");
    return;
  }
  Promise.resolve(open(p))
    .then(() => call("toast", "已在访达打开"))
    .catch((err) => call("toast", String(err?.message || err || "打开失败")));
}

function copyProjectPath(path) {
  const p = String(path || "").trim();
  if (!p) {
    call("toast", "没有可复制的路径");
    return;
  }
  if (navigator?.clipboard?.writeText) {
    navigator.clipboard
      .writeText(p)
      .then(() => call("toast", "路径已复制"))
      .catch(() => call("toast", "复制失败"));
    return;
  }
  call("toast", "复制不可用");
}

function goProjectRun(path) {
  const p = String(path || "").trim();
  if (!p) return;
  const select = g("selectProject");
  if (typeof select === "function") select(p);
  try {
    if (window.ccoApp?.goRun) {
      window.ccoApp.goRun();
      return;
    }
  } catch (_) {}
  const mon = g("goToPlanMonitor");
  if (typeof mon === "function") {
    Promise.resolve(mon()).catch(() => {});
    return;
  }
  call("showPage", "workspace");
}

export function hideProjectContextMenu() {
  const m = typeof window !== "undefined" ? window.__ccoProjectCtxMenu : null;
  if (!m) return;
  try {
    if (m._ccoDocClose) {
      document.removeEventListener("pointerdown", m._ccoDocClose, true);
    }
    m.remove();
  } catch (_) {}
  if (typeof window !== "undefined") window.__ccoProjectCtxMenu = null;
}

/**
 * 项目右键菜单：打开文件夹 · 复制路径 · 查看执行/打开 · 移除。
 * @param {number} x
 * @param {number} y
 * @param {{ path: string, name?: string, live?: boolean }} opts
 */
export function showProjectContextMenu(x, y, { path, name, live } = {}) {
  hideProjectContextMenu();
  if (!path || typeof document === "undefined") return;
  const menu = document.createElement("div");
  menu.className = "cco-menu project-ctx-menu is-open";
  menu.setAttribute("role", "menu");
  menu.dataset.open = "1";
  menu.innerHTML = [
    `<button type="button" class="cco-menu-item" data-act="open" role="menuitem">打开文件夹</button>`,
    `<button type="button" class="cco-menu-item" data-act="copy" role="menuitem">复制路径</button>`,
    `<button type="button" class="cco-menu-item" data-act="run" role="menuitem">${
      live ? "查看执行" : "打开项目"
    }</button>`,
    `<div class="cco-menu-sep" role="separator"></div>`,
    `<button type="button" class="cco-menu-item is-danger" data-act="remove" role="menuitem"${
      live ? ' disabled title="运行中不可移除"' : ""
    }>从列表移除</button>`,
  ].join("");
  document.body.appendChild(menu);
  const pad = 8;
  const mw = menu.offsetWidth || 180;
  const mh = menu.offsetHeight || 160;
  menu.style.left = `${Math.min(Math.max(pad, x), window.innerWidth - mw - pad)}px`;
  menu.style.top = `${Math.min(Math.max(pad, y), window.innerHeight - mh - pad)}px`;

  menu.addEventListener("click", (ev) => {
    const btn = ev.target?.closest?.("[data-act]");
    if (!btn || btn.disabled) return;
    const act = btn.getAttribute("data-act");
    hideProjectContextMenu();
    if (act === "open") openProjectFolder(path);
    else if (act === "copy") copyProjectPath(path);
    else if (act === "run") goProjectRun(path);
    else if (act === "remove") {
      const fn = g("removeSelectedProject");
      if (typeof fn === "function") fn(path);
      else call("toast", "移除不可用");
    }
  });

  const onDoc = (ev) => {
    if (menu.contains(ev.target)) return;
    hideProjectContextMenu();
  };
  setTimeout(() => {
    document.addEventListener("pointerdown", onDoc, true);
    menu._ccoDocClose = onDoc;
  }, 0);
  window.__ccoProjectCtxMenu = menu;
  // name reserved for future title row
  void name;
}
