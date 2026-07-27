/**
 * [INPUT]: live.browser_evidence DTO (Rust collect_browser_evidence)
 * [OUTPUT]: result desk browser strip DOM paint helpers
 * [POS]: W3 features/result — pure render; no gateway / no strategy
 * [PROTOCOL]: 变更时更新 web/CLAUDE.md features/result 行
 */

/**
 * @param {unknown} s
 * @returns {string}
 */
function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * Human kind label (no MCP / CDP jargon).
 * @param {string} kind
 */
function kindLabel(kind) {
  switch (String(kind || "").toLowerCase()) {
    case "shot":
      return "截图";
    case "report":
      return "验收说明";
    case "smoke":
      return "冒烟记录";
    case "raw":
      return "抓取摘录";
    default:
      return "证据";
  }
}

/**
 * Lightweight lightbox for shot data URLs (no new window required).
 * @param {string} dataUrl
 * @param {string} title
 */
function openShotLightbox(dataUrl, title) {
  if (typeof document === "undefined" || !dataUrl) return;
  const existing = document.getElementById("result-browser-lightbox");
  if (existing) existing.remove();

  const root = document.createElement("div");
  root.id = "result-browser-lightbox";
  root.className = "result-browser-lightbox";
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-modal", "true");
  root.setAttribute("aria-label", title || "网页截图");
  root.innerHTML = `
    <div class="result-browser-lightbox-backdrop" data-close="1"></div>
    <div class="result-browser-lightbox-panel">
      <header class="result-browser-lightbox-h">
        <span class="result-browser-lightbox-title"></span>
        <button type="button" class="btn ghost sm" data-close="1">关闭</button>
      </header>
      <img class="result-browser-lightbox-img" alt="" />
    </div>`;
  const titleEl = root.querySelector(".result-browser-lightbox-title");
  if (titleEl) titleEl.textContent = title || "网页截图";
  const img = root.querySelector(".result-browser-lightbox-img");
  if (img) {
    img.src = dataUrl;
    img.alt = title || "网页截图";
  }
  const close = () => {
    root.remove();
    document.removeEventListener("keydown", onKey);
  };
  const onKey = (e) => {
    if (e.key === "Escape") close();
  };
  root.addEventListener("click", (e) => {
    const t = e.target;
    if (t && t.getAttribute && t.getAttribute("data-close") === "1") close();
  });
  document.addEventListener("keydown", onKey);
  document.body.appendChild(root);
}

/**
 * @param {object|null|undefined} live
 * @param {{ $?: (id: string) => HTMLElement|null, openPath?: (path: string) => Promise<unknown> }} [deps]
 */
export function renderBrowserEvidence(live, deps = {}) {
  const $ =
    typeof deps.$ === "function"
      ? deps.$
      : (id) =>
          typeof document !== "undefined" ? document.getElementById(id) : null;

  const openPath =
    typeof deps.openPath === "function"
      ? deps.openPath
      : async (path) => {
          const g =
            typeof window !== "undefined" ? window.ccoGateway : null;
          if (g && typeof g.openPath === "function") {
            return g.openPath(path);
          }
          throw new Error("openPath unavailable");
        };

  const panel = $("result-desk-browser");
  const grid = $("result-desk-browser-grid");
  const note = $("result-desk-browser-note");
  if (!panel || !grid) return;

  const items = (live && (live.browser_evidence || live.browserEvidence)) || [];
  if (!Array.isArray(items) || !items.length) {
    panel.hidden = true;
    grid.innerHTML = "";
    if (note) {
      note.hidden = true;
      note.textContent = "";
    }
    return;
  }

  panel.hidden = false;
  if (note) {
    note.hidden = false;
    note.textContent = `本轮留下 ${items.length} 份网页相关证据（点图放大 · 可打开文件）`;
  }

  grid.innerHTML = items
    .map((it, idx) => {
      const kind = String((it && it.kind) || "other");
      const taskId = String((it && it.task_id) || (it && it.taskId) || "");
      const rel = String((it && it.rel_path) || (it && it.relPath) || "");
      const abs = String((it && it.abs_path) || (it && it.absPath) || "");
      const dataUrl =
        (it && (it.preview_data_url || it.previewDataUrl)) || "";
      const excerpt = String((it && it.excerpt) || "").trim();
      const title = `${kindLabel(kind)}${taskId ? ` · ${taskId}` : ""}`;

      if (kind === "shot" && dataUrl) {
        return `<figure class="result-browser-card is-shot" data-idx="${idx}">
  <button type="button" class="result-browser-shot-btn" data-action="zoom" data-idx="${idx}" title="点击放大">
    <img class="result-browser-shot" src="${esc(dataUrl)}" alt="${esc(title)}" loading="lazy" />
  </button>
  <figcaption class="result-browser-cap muted">
    ${esc(title)}${rel ? ` · ${esc(rel)}` : ""}
    ${
      abs
        ? ` · <button type="button" class="linkish result-browser-open" data-action="open" data-path="${esc(
            abs
          )}">打开文件</button>`
        : ""
    }
  </figcaption>
</figure>`;
      }

      const body = excerpt
        ? `<pre class="result-browser-excerpt">${esc(excerpt)}</pre>`
        : `<p class="muted result-browser-path">${esc(rel || "（无正文摘录）")}</p>`;
      const openBtn = abs
        ? `<button type="button" class="linkish result-browser-open" data-action="open" data-path="${esc(
            abs
          )}">打开文件</button>`
        : "";
      return `<div class="result-browser-card is-text" data-idx="${idx}">
  <div class="result-browser-cap">${esc(title)}${openBtn ? ` · ${openBtn}` : ""}</div>
  ${body}
</div>`;
    })
    .join("");

  // One listener on grid (replace prior by cloning)
  const next = grid.cloneNode(true);
  grid.parentNode.replaceChild(next, grid);
  next.addEventListener("click", (e) => {
    const t = e.target;
    if (!t || !t.closest) return;
    const btn = t.closest("[data-action]");
    if (!btn) return;
    const action = btn.getAttribute("data-action");
    if (action === "zoom") {
      const idx = Number(btn.getAttribute("data-idx"));
      const it = items[idx];
      if (!it) return;
      const dataUrl = it.preview_data_url || it.previewDataUrl || "";
      const taskId = it.task_id || it.taskId || "";
      openShotLightbox(dataUrl, `${kindLabel(it.kind)}${taskId ? ` · ${taskId}` : ""}`);
      return;
    }
    if (action === "open") {
      const path = btn.getAttribute("data-path") || "";
      if (!path) return;
      openPath(path).catch((err) => {
        const toast = typeof window.toast === "function" ? window.toast : null;
        if (toast) toast(String(err?.message || err || "打开失败"));
      });
    }
  });
}

export default { renderBrowserEvidence };
