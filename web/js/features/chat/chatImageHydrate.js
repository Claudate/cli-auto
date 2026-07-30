/**
 * [INPUT]: chat DOM after render · project path · gateway readImageDataUrl
 * [OUTPUT]: fill local markdown imgs + attachment thumbs with data URLs
 * [POS]: features/chat — display only; no Mode B / confirm
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * CSP only allows img-src 'self' data: — project files need Rust → data URL.
 * Cache in-memory per project so re-render / fold expand does not re-fetch.
 */

import { state } from "./legacy.js";
import * as chatApi from "./chatApi.js";

/** @type {Map<string, string>} project::rel → dataUrl */
const cache = new Map();
/** @type {Set<string>} in-flight keys */
const inflight = new Set();

function cacheKey(project, rel) {
  return `${project}::${String(rel || "").replace(/\\/g, "/")}`;
}

function isImageName(name, mime) {
  if (mime && String(mime).toLowerCase().startsWith("image/")) return true;
  return /\.(png|jpe?g|webp|gif|svg)$/i.test(String(name || ""));
}

/**
 * @param {string} project
 * @param {string} rel
 * @returns {Promise<string|null>}
 */
async function loadDataUrl(project, rel) {
  const path = String(rel || "")
    .trim()
    .replace(/\\/g, "/")
    .replace(/^\.\//, "");
  if (!project || !path) return null;
  if (/^data:image\//i.test(path) || /^https?:\/\//i.test(path)) return path;
  const key = cacheKey(project, path);
  if (cache.has(key)) return cache.get(key);
  if (inflight.has(key)) return null;
  inflight.add(key);
  try {
    const dataUrl = await chatApi.readImageDataUrl(project, path);
    if (dataUrl && typeof dataUrl === "string" && dataUrl.startsWith("data:")) {
      cache.set(key, dataUrl);
      return dataUrl;
    }
  } catch (_) {
    /* missing / too large / escaped — leave placeholder */
  } finally {
    inflight.delete(key);
  }
  return null;
}

/**
 * Fill `[data-md-img-path]` placeholders and attachment thumbs without `_preview`.
 * Safe to call after every renderChatMessages; no-op when no project / no nodes.
 * @param {ParentNode|null} root
 */
export function hydrateChatImages(root) {
  if (!root || typeof root.querySelectorAll !== "function") return;
  const project = state.selectedPath;
  if (!project) return;

  // Markdown local images
  const pending = root.querySelectorAll("[data-md-img-path]");
  pending.forEach((wrap) => {
    const path = wrap.getAttribute("data-md-img-path");
    if (!path || wrap.getAttribute("data-md-img-done") === "1") return;
    const img = wrap.querySelector("img");
    if (!img) return;
    const key = cacheKey(project, path);
    if (cache.has(key)) {
      applyImg(wrap, img, cache.get(key), path);
      return;
    }
    loadDataUrl(project, path).then((dataUrl) => {
      if (!dataUrl) {
        wrap.setAttribute("data-md-img-done", "err");
        wrap.classList.add("is-err");
        return;
      }
      // Node may have been replaced by a later render — re-query by path
      const still =
        wrap.isConnected && wrap.getAttribute("data-md-img-path") === path
          ? wrap
          : root.querySelector(`[data-md-img-path="${cssEscape(path)}"]`);
      if (!still) return;
      const stillImg = still.querySelector("img");
      if (!stillImg) return;
      applyImg(still, stillImg, dataUrl, path);
    });
  });

  // Message attachments: path only after reload
  const attNodes = root.querySelectorAll(
    ".chat-msg-att[data-att-path], .chat-msg-att-path[data-att-path]"
  );
  attNodes.forEach((node) => {
    const path = node.getAttribute("data-att-path");
    const mime = node.getAttribute("data-att-mime") || "";
    const name = node.getAttribute("data-att-name") || path || "";
    if (!path || node.getAttribute("data-att-done") === "1") return;
    if (!isImageName(name, mime)) return;
    if (node.querySelector("img:not(.is-pending)")) {
      node.setAttribute("data-att-done", "1");
      return;
    }
    const key = cacheKey(project, path);
    const apply = (dataUrl) => {
      if (!dataUrl || !node.isConnected) return;
      const nameEsc = name;
      node.classList.remove("chat-msg-att-path");
      node.classList.add("chat-msg-att");
      node.innerHTML =
        `<img class="chat-img-zoomable" src="${dataUrl}" alt="${escapeAttr(nameEsc)}" ` +
        `data-img-src="${escapeAttr(dataUrl)}" data-img-name="${escapeAttr(nameEsc)}" title="点击放大" />` +
        `<span>${escapeAttr(nameEsc)}</span>`;
      node.setAttribute("data-att-done", "1");
    };
    if (cache.has(key)) {
      apply(cache.get(key));
      return;
    }
    loadDataUrl(project, path).then(apply);
  });
}

function applyImg(wrap, img, dataUrl, path) {
  img.src = dataUrl;
  img.classList.remove("is-pending");
  img.setAttribute("data-img-src", dataUrl);
  img.setAttribute("title", "点击放大");
  wrap.setAttribute("data-md-img-done", "1");
  wrap.classList.add("is-ready");
  wrap.classList.remove("is-err");
  // Keep path for open-in-finder if needed later
  wrap.setAttribute("data-md-img-path", path);
}

function escapeAttr(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function cssEscape(s) {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(s);
  }
  return String(s).replace(/["\\]/g, "\\$&");
}

/** Drop cache when switching projects (optional). */
export function clearChatImageCache() {
  cache.clear();
  inflight.clear();
}
