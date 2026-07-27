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
 * @param {object|null|undefined} live
 * @param {{ $?: (id: string) => HTMLElement|null }} [deps]
 */
export function renderBrowserEvidence(live, deps = {}) {
  const $ =
    typeof deps.$ === "function"
      ? deps.$
      : (id) =>
          typeof document !== "undefined" ? document.getElementById(id) : null;

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
    note.textContent = `本轮留下 ${items.length} 份网页相关证据（截图 / 说明）`;
  }

  grid.innerHTML = items
    .map((it) => {
      const kind = String((it && it.kind) || "other");
      const taskId = String((it && it.task_id) || (it && it.taskId) || "");
      const rel = String((it && it.rel_path) || (it && it.relPath) || "");
      const dataUrl =
        (it && (it.preview_data_url || it.previewDataUrl)) || "";
      const excerpt = String((it && it.excerpt) || "").trim();
      const title = `${kindLabel(kind)}${taskId ? ` · ${taskId}` : ""}`;

      if (kind === "shot" && dataUrl) {
        return `<figure class="result-browser-card is-shot">
  <img class="result-browser-shot" src="${esc(dataUrl)}" alt="${esc(title)}" loading="lazy" />
  <figcaption class="result-browser-cap muted">${esc(title)}${
          rel ? ` · ${esc(rel)}` : ""
        }</figcaption>
</figure>`;
      }

      const body = excerpt
        ? `<pre class="result-browser-excerpt">${esc(excerpt)}</pre>`
        : `<p class="muted result-browser-path">${esc(rel || "（无正文摘录）")}</p>`;
      return `<div class="result-browser-card is-text">
  <div class="result-browser-cap">${esc(title)}</div>
  ${body}
</div>`;
    })
    .join("");
}

export default { renderBrowserEvidence };
