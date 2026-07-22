/**
 * [INPUT]: legacy · chatApi
 * [OUTPUT]: attachments · paste · upload (images + common docs/code)
 * [POS]: A5-2a features/chat/chatAttachments.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast } from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { ensureChatState } from "./chatState.js";
import { chatEsc } from "./chatFormat.js";

export const CHAT_ATT_MAX_BYTES = 8 * 1024 * 1024;
export const CHAT_ATT_MAX_COUNT = 6;

/** Blocked extensions (executables / installers). */
const BLOCKED_EXT = new Set([
  "exe",
  "dll",
  "so",
  "dylib",
  "bat",
  "cmd",
  "com",
  "msi",
  "scr",
  "ps1",
  "vbs",
  "wsf",
  "app",
  "dmg",
  "pkg",
  "deb",
  "rpm",
  "apk",
  "jar",
  "class",
  "wasm",
]);

const DOC_EXT = new Set([
  "png",
  "jpg",
  "jpeg",
  "webp",
  "gif",
  "svg",
  "pdf",
  "md",
  "markdown",
  "txt",
  "csv",
  "tsv",
  "json",
  "yaml",
  "yml",
  "toml",
  "xml",
  "html",
  "htm",
  "css",
  "rs",
  "ts",
  "tsx",
  "js",
  "jsx",
  "py",
  "go",
  "java",
  "c",
  "h",
  "cpp",
  "hpp",
  "cs",
  "sql",
  "doc",
  "docx",
  "xls",
  "xlsx",
  "ppt",
  "pptx",
  "rtf",
  "log",
]);

const ALLOWED_MIME = new Set([
  "image/png",
  "image/jpeg",
  "image/jpg",
  "image/webp",
  "image/gif",
  "image/svg+xml",
  "text/plain",
  "text/markdown",
  "text/x-markdown",
  "text/csv",
  "text/tab-separated-values",
  "text/html",
  "text/css",
  "text/xml",
  "text/rtf",
  "text/yaml",
  "text/x-yaml",
  "application/json",
  "application/ld+json",
  "application/xml",
  "application/pdf",
  "application/rtf",
  "application/msword",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "application/vnd.ms-excel",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "application/vnd.ms-powerpoint",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  "application/x-yaml",
  "application/yaml",
  "text/javascript",
  "application/javascript",
  "application/typescript",
  "text/x-python",
  "text/x-rust",
  "text/x-go",
  "application/sql",
  "text/x-sql",
]);

function fileExt(name) {
  const m = String(name || "")
    .toLowerCase()
    .match(/\.([a-z0-9]{1,12})$/);
  return m ? m[1] : "";
}

function isImageMime(mime) {
  return String(mime || "")
    .toLowerCase()
    .startsWith("image/");
}

function isAllowedFile(file) {
  const mime = (file.type || "").toLowerCase();
  const ext = fileExt(file.name);
  if (ext && BLOCKED_EXT.has(ext)) return { ok: false, reason: `不允许上传 .${ext}` };
  if (mime && ALLOWED_MIME.has(mime)) return { ok: true };
  if (ext && DOC_EXT.has(ext)) return { ok: true };
  if (mime.startsWith("text/")) return { ok: true };
  return {
    ok: false,
    reason: `不支持的类型：${file.name || mime || "未知"}`,
  };
}

function normalizeMime(file) {
  let mime = (file.type || "").toLowerCase();
  if (mime === "image/jpg") mime = "image/jpeg";
  const ext = fileExt(file.name);
  const map = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    webp: "image/webp",
    gif: "image/gif",
    svg: "image/svg+xml",
    pdf: "application/pdf",
    md: "text/markdown",
    markdown: "text/markdown",
    txt: "text/plain",
    log: "text/plain",
    csv: "text/csv",
    json: "application/json",
    yml: "text/yaml",
    yaml: "text/yaml",
    html: "text/html",
    htm: "text/html",
    css: "text/css",
    xml: "application/xml",
    rs: "text/plain",
    ts: "text/plain",
    tsx: "text/plain",
    js: "text/javascript",
    jsx: "text/javascript",
    py: "text/x-python",
    go: "text/x-go",
    java: "text/x-java-source",
    sql: "application/sql",
    toml: "text/plain",
    docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    doc: "application/msword",
    xls: "application/vnd.ms-excel",
    ppt: "application/vnd.ms-powerpoint",
    rtf: "application/rtf",
  };
  // Browsers often label .md/.txt as application/octet-stream — prefer extension.
  if (!mime || mime === "application/octet-stream") {
    return map[ext] || "application/octet-stream";
  }
  if (ALLOWED_MIME.has(mime)) return mime;
  return map[ext] || mime;
}

export function fileToDataUrl(file) {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result);
    r.onerror = () => reject(new Error("read file failed"));
    r.readAsDataURL(file);
  });
}

export function dataUrlToBase64(dataUrl) {
  const s = String(dataUrl || "");
  const i = s.indexOf(",");
  return i >= 0 ? s.slice(i + 1) : s;
}

export async function addChatAttachments(fileList) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const files = Array.from(fileList || []);
  for (const f of files) {
    if (state.chatPendingAttachments.length >= CHAT_ATT_MAX_COUNT) {
      toast(`每条消息最多 ${CHAT_ATT_MAX_COUNT} 个附件`);
      break;
    }
    const check = isAllowedFile(f);
    if (!check.ok) {
      toast(check.reason);
      continue;
    }
    if (f.size > CHAT_ATT_MAX_BYTES) {
      toast(`${f.name || "文件"} 超过 8MB`);
      continue;
    }
    try {
      const dataUrl = await fileToDataUrl(f);
      const mime = normalizeMime(f);
      state.chatPendingAttachments.push({
        name: f.name || "file",
        mime,
        dataUrl,
        size: f.size,
        isImage: isImageMime(mime),
      });
    } catch (e) {
      toast(String(e?.message || e));
    }
  }
  renderChatAttachPreview();
}

export function removeChatAttachment(idx) {
  ensureChatState();
  if (idx < 0 || idx >= state.chatPendingAttachments.length) return;
  state.chatPendingAttachments.splice(idx, 1);
  renderChatAttachPreview();
}

export function clearChatAttachments() {
  ensureChatState();
  state.chatPendingAttachments = [];
  renderChatAttachPreview();
}

export function renderChatAttachPreview() {
  ensureChatState();
  const box = $("#chat-attach-preview");
  if (!box) return;
  const items = state.chatPendingAttachments || [];
  if (!items.length) {
    box.hidden = true;
    box.innerHTML = "";
    return;
  }
  box.hidden = false;
  const ico =
    typeof window.ccoIcon === "function"
      ? (n) => window.ccoIcon(n, { size: 20 })
      : () => "📄";
  box.innerHTML = items
    .map((a, i) => {
      const removeBtn = `<button type="button" class="chat-attach-remove icon-btn sm" data-att-remove="${i}" title="移除" aria-label="移除">${
        typeof window.ccoIcon === "function"
          ? window.ccoIcon("x", { size: 12 })
          : "×"
      }</button>`;
      if (a.isImage || isImageMime(a.mime)) {
        return (
          `<div class="chat-attach-thumb" data-att-idx="${i}">` +
          `<img class="chat-img-zoomable" src="${a.dataUrl}" alt="${chatEsc(a.name)}" data-img-src="${chatEsc(a.dataUrl)}" data-img-name="${chatEsc(a.name)}" title="点击放大" />` +
          removeBtn +
          `<span class="chat-attach-name">${chatEsc(a.name)}</span>` +
          `</div>`
        );
      }
      return (
        `<div class="chat-attach-thumb chat-attach-file" data-att-idx="${i}">` +
        `<div class="chat-attach-file-ico" aria-hidden="true">${ico("file")}</div>` +
        removeBtn +
        `<span class="chat-attach-name" title="${chatEsc(a.name)}">${chatEsc(a.name)}</span>` +
        `</div>`
      );
    })
    .join("");
}

/** 图片放大 lightbox */
export function openImageLightbox(src, name) {
  if (!src) return;
  const box = $("#img-lightbox");
  const img = $("#img-lightbox-img");
  const cap = $("#img-lightbox-caption");
  if (!box || !img) return;
  img.src = src;
  img.alt = name || "图片";
  if (cap) cap.textContent = name || "";
  box.hidden = false;
}

export function closeImageLightbox() {
  const box = $("#img-lightbox");
  const img = $("#img-lightbox-img");
  if (img) img.removeAttribute("src");
  if (box) box.hidden = true;
}

/** Ctrl/Cmd+V：图片仍直接入队；其它剪贴板文件若有也入队 */
export async function handleChatPaste(e) {
  if (!state.selectedPath || state.page !== "chat") return;
  const cd = e.clipboardData || e.originalEvent?.clipboardData;
  if (!cd) return;
  const files = [];
  if (cd.items && cd.items.length) {
    for (const it of cd.items) {
      if (it.kind === "file") {
        const f = it.getAsFile();
        if (f) files.push(f);
      }
    }
  }
  if (!files.length && cd.files && cd.files.length) {
    for (const f of cd.files) files.push(f);
  }
  if (!files.length) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    const before = (state.chatPendingAttachments || []).length;
    await addChatAttachments(files);
    const n = (state.chatPendingAttachments || []).length - before;
    if (n > 0) toast(`已粘贴 ${n} 个附件`);
  } catch (err) {
    toast(String(err?.message || err));
  }
}

export function pickChatAttachments() {
  const input = $("#chat-file-input");
  if (input) {
    input.value = "";
    input.click();
  }
}

export async function uploadPendingAttachments() {
  ensureChatState();
  const pending = state.chatPendingAttachments || [];
  if (!pending.length) return [];
  const out = [];
  for (const p of pending) {
    const resp = await chatApi.saveAttachment({
      project: state.selectedPath,
      sessionId: state.chatSession?.session_id || "default",
      fileName: p.name,
      mime: p.mime,
      dataBase64: dataUrlToBase64(p.dataUrl),
    });
    out.push({
      path: resp.path || resp.path,
      mime: resp.mime,
      name: resp.name,
    });
  }
  return out;
}

/** Cache key: project path + session id (C3 multi-session). */
