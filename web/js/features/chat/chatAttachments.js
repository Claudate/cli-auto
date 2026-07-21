/**
 * [INPUT]: legacy · chatApi
 * [OUTPUT]: attachments · paste · upload
 * [POS]: A5-2a features/chat/chatAttachments.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, $, toast } from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { ensureChatState } from "./chatState.js";
import { chatEsc } from "./chatFormat.js";

export const CHAT_ATT_MAX_BYTES = 5 * 1024 * 1024;
export const CHAT_ATT_MAX_COUNT = 4;
export const CHAT_ATT_MIME = new Set([
  "image/png",
  "image/jpeg",
  "image/jpg",
  "image/webp",
  "image/gif",
]);

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
      toast(`每条消息最多 ${CHAT_ATT_MAX_COUNT} 张图`);
      break;
    }
    const mime = (f.type || "").toLowerCase();
    if (!CHAT_ATT_MIME.has(mime)) {
      toast(`不支持的类型：${f.name || mime}`);
      continue;
    }
    if (f.size > CHAT_ATT_MAX_BYTES) {
      toast(`${f.name || "图片"} 超过 5MB`);
      continue;
    }
    try {
      const dataUrl = await fileToDataUrl(f);
      state.chatPendingAttachments.push({
        name: f.name || "image.png",
        mime: mime === "image/jpg" ? "image/jpeg" : mime,
        dataUrl,
        size: f.size,
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
  box.innerHTML = items
    .map(
      (a, i) =>
        `<div class="chat-attach-thumb" data-att-idx="${i}">` +
        `<img class="chat-img-zoomable" src="${a.dataUrl}" alt="${chatEsc(a.name)}" data-img-src="${chatEsc(a.dataUrl)}" data-img-name="${chatEsc(a.name)}" title="点击放大" />` +
        `<button type="button" class="chat-attach-remove" data-att-remove="${i}" title="移除">×</button>` +
        `<span class="chat-attach-name">${chatEsc(a.name)}</span>` +
        `</div>`
    )
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

/** Ctrl/Cmd+V 或剪贴板图片 → 附件队列 */
export async function handleChatPaste(e) {
  if (!state.selectedPath || state.page !== "chat") return;
  const cd = e.clipboardData || e.originalEvent?.clipboardData;
  if (!cd) return;
  const files = [];
  // Prefer items (screenshot paste)
  if (cd.items && cd.items.length) {
    for (const it of cd.items) {
      if (it.kind === "file" && it.type && it.type.startsWith("image/")) {
        const f = it.getAsFile();
        if (f) files.push(f);
      }
    }
  }
  if (!files.length && cd.files && cd.files.length) {
    for (const f of cd.files) {
      if (f.type && f.type.startsWith("image/")) files.push(f);
    }
  }
  if (!files.length) return;
  e.preventDefault();
  e.stopPropagation();
  try {
    await addChatAttachments(files);
    toast(`已粘贴 ${files.length} 张图片`);
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
