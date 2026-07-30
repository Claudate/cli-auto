/**
 * [INPUT]: 原始 Markdown 文本（确认屏 / 计划说明 / 聊天气泡）
 * [OUTPUT]: 安全 HTML 字符串（无外部依赖）
 * [POS]: D9 自 state.js 抽出；classic 经 installMarkdown → window.renderMarkdown；chatFormatBody 复用
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 * note: bare http(s) → .md-ext-link；点击经 ccoGateway.openPath → 系统浏览器
 * note: ![alt](src) → img；本地相对路径 data-md-img-path 由 chat 异步灌 data URL
 * note: 单独一行的项目相对 .png/.jpg… 路径也会变成图（截图报告）
 * smoke: node scripts/chat-md-image-smoke.mjs
 */

import { esc } from "./statusUi.js";

/** 轻量 Markdown → 安全 HTML（确认屏/计划说明用） */
export function renderMarkdown(src) {
  const raw = String(src ?? "");
  if (!raw.trim()) return '<p class="md-empty">（无任务说明）</p>';

  // 1) 抽出 fenced code，避免内部被二次处理
  const fences = [];
  let text = raw.replace(/```([\w-]+)?\n([\s\S]*?)```/g, (_, lang, code) => {
    const i = fences.length;
    fences.push({ lang: (lang || "").trim(), code: code.replace(/\n$/, "") });
    return `\n\n%%FENCE${i}%%\n\n`;
  });

  // 2) 按行做块级解析
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const blocks = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    const fm = line.trim().match(/^%%FENCE(\d+)%%$/);
    if (fm) {
      blocks.push({ type: "fence", idx: Number(fm[1]) });
      i++;
      continue;
    }
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(line.trim())) {
      blocks.push({ type: "hr" });
      i++;
      continue;
    }
    const hm = line.match(/^(#{1,6})\s+(.+)$/);
    if (hm) {
      blocks.push({ type: "h", level: hm[1].length, text: hm[2].trim() });
      i++;
      continue;
    }
    if (/^>\s?/.test(line)) {
      const qs = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) {
        qs.push(lines[i].replace(/^>\s?/, ""));
        i++;
      }
      blocks.push({ type: "quote", text: qs.join("\n") });
      continue;
    }
    if (
      line.includes("|") &&
      i + 1 < lines.length &&
      /^\s*\|?\s*:?-{3,}/.test(lines[i + 1])
    ) {
      const rows = [];
      while (i < lines.length && lines[i].includes("|")) {
        if (/^\s*\|?\s*:?-{3,}/.test(lines[i])) {
          i++;
          continue;
        }
        const cells = lines[i]
          .trim()
          .replace(/^\|/, "")
          .replace(/\|$/, "")
          .split("|")
          .map((c) => c.trim());
        rows.push(cells);
        i++;
      }
      blocks.push({ type: "table", rows });
      continue;
    }
    if (/^\s*[-*+]\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      blocks.push({ type: "ul", items });
      continue;
    }
    if (/^\s*\d+\.\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+\.\s+/, ""));
        i++;
      }
      blocks.push({ type: "ol", items });
      continue;
    }
    const paras = [line];
    i++;
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^#{1,6}\s/.test(lines[i]) &&
      !/^>\s?/.test(lines[i]) &&
      !/^\s*[-*+]\s+/.test(lines[i]) &&
      !/^\s*\d+\.\s+/.test(lines[i]) &&
      !/^%%FENCE\d+%%$/.test(lines[i].trim()) &&
      !/^(-{3,}|\*{3,}|_{3,})$/.test(lines[i].trim()) &&
      !(
        lines[i].includes("|") &&
        i + 1 < lines.length &&
        /^\s*\|?\s*:?-{3,}/.test(lines[i + 1] || "")
      )
    ) {
      paras.push(lines[i]);
      i++;
    }
    blocks.push({ type: "p", text: paras.join("\n") });
  }

  function peelUrlTrail(url) {
    let u = url;
    let trail = "";
    while (u.length > 8) {
      const last = u[u.length - 1];
      if (/[.,;:!?]$/.test(last)) {
        trail = last + trail;
        u = u.slice(0, -1);
        continue;
      }
      // Prose often glues ')' after a URL; only peel when parens are unbalanced.
      if (last === ")") {
        const open = (u.match(/\(/g) || []).length;
        const close = (u.match(/\)/g) || []).length;
        if (close > open) {
          trail = ")" + trail;
          u = u.slice(0, -1);
          continue;
        }
      }
      break;
    }
    return { url: u, trail };
  }

  function extAnchor(url, label) {
    const safe = String(url || "");
    const text = label != null ? label : safe;
    return `<a class="md-ext-link" href="${safe}" target="_blank" rel="noopener noreferrer">${text}</a>`;
  }

  function isImagePath(src) {
    const s = String(src || "").trim();
    if (!s) return false;
    if (/^data:image\//i.test(s)) return true;
    // strip optional title: path "title"
    const bare = s.replace(/\s+".*"$/, "").replace(/\s+'.*'$/, "").trim();
    if (/^https?:\/\//i.test(bare)) {
      return /\.(png|jpe?g|webp|gif|svg)(\?|#|$)/i.test(bare);
    }
    return /\.(png|jpe?g|webp|gif|svg)$/i.test(bare.split("?")[0].split("#")[0]);
  }

  function peelImageSrc(raw) {
    let s = String(raw || "").trim();
    // optional markdown title after path
    const m = s.match(/^(\S+)(?:\s+["'].*["'])?$/);
    if (m) s = m[1];
    return s;
  }

  function mdImageHtml(alt, rawSrc) {
    const src = peelImageSrc(rawSrc);
    const altSafe = esc(alt || "");
    if (/^data:image\//i.test(src)) {
      return (
        `<img class="md-img chat-img-zoomable" src="${esc(src)}" alt="${altSafe}" ` +
        `data-img-src="${esc(src)}" data-img-name="${altSafe}" loading="lazy" title="点击放大" />`
      );
    }
    if (/^https?:\/\//i.test(src) && isImagePath(src)) {
      // External image hosts rarely allowed by CSP (img-src self data) — still emit
      // for browser/dev; Tauri may block. Prefer local project paths in product.
      return (
        `<img class="md-img chat-img-zoomable" src="${esc(src)}" alt="${altSafe}" ` +
        `data-img-src="${esc(src)}" data-img-name="${altSafe}" loading="lazy" title="点击放大" />`
      );
    }
    // Local / project-relative path — placeholder; chat hydrates via data URL.
    const path = src.replace(/^\.\//, "");
    return (
      `<span class="md-img-pending" data-md-img-path="${esc(path)}" data-md-img-alt="${altSafe}">` +
      `<img class="md-img chat-img-zoomable is-pending" alt="${altSafe}" ` +
      `data-img-name="${altSafe}" loading="lazy" title="${esc(path)}" />` +
      `<span class="md-img-cap muted">${altSafe || esc(path)}</span>` +
      `</span>`
    );
  }

  function inlineMd(s) {
    // Pull images out on raw text first (avoid double-esc + link rule eating ![x](y))
    const imgs = [];
    let raw = String(s ?? "");
    raw = raw.replace(
      /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g,
      (_, alt, url) => {
        const i = imgs.length;
        imgs.push(mdImageHtml(alt, url));
        return `%%IMG${i}%%`;
      }
    );
    // Sole bare project-relative image path (AI screenshot reports often list paths alone)
    raw = raw.replace(
      /(^|\n)(\.?\.?\/?[\w.-]+(?:\/[\w.-]+)+\.(?:png|jpe?g|webp|gif|svg))(?=\n|$)/gi,
      (full, pre, path) => {
        const i = imgs.length;
        const base = path.split("/").pop() || path;
        imgs.push(mdImageHtml(base, path));
        return `${pre}%%IMG${i}%%`;
      }
    );
    let x = esc(raw);
    x = x.replace(/`([^`]+)`/g, "<code>$1</code>");
    x = x.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    x = x.replace(/__([^_]+)__/g, "<strong>$1</strong>");
    x = x.replace(/(^|[^*])\*([^*]+)\*(?![*])/g, "$1<em>$2</em>");
    x = x.replace(/(^|[^_])_([^_]+)_(?!_)/g, "$1<em>$2</em>");
    // Markdown links: 外链可点开；相对/本地路径只展示样式
    x = x.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, url) => {
      if (/^https?:\/\//i.test(url) || /^mailto:/i.test(url)) {
        return extAnchor(url, label);
      }
      return `<span class="md-local-link" title="${url}">${label}</span>`;
    });
    // Bare http(s) URLs (e.g. 本地地址：http://localhost:4322/) — not already in href="…"
    x = x.replace(/(^|[^"'=\]>])(https?:\/\/[^\s<>"']+)/gi, (full, pre, rawUrl) => {
      const { url, trail } = peelUrlTrail(rawUrl);
      return `${pre}${extAnchor(url)}${trail}`;
    });
    x = x.replace(/\n/g, "<br>");
    // Restore image HTML (placeholders survived esc)
    x = x.replace(/%%IMG(\d+)%%/g, (_, idx) => imgs[Number(idx)] || "");
    return x;
  }

  const html = blocks
    .map((b) => {
      if (b.type === "fence") {
        const f = fences[b.idx] || { lang: "", code: "" };
        const lang = f.lang ? ` data-lang="${esc(f.lang)}"` : "";
        return `<pre class="md-code"${lang}><code>${esc(f.code)}</code></pre>`;
      }
      if (b.type === "hr") return '<hr class="md-hr">';
      if (b.type === "h") {
        const lv = Math.min(6, Math.max(1, b.level));
        return `<h${lv} class="md-h">${inlineMd(b.text)}</h${lv}>`;
      }
      if (b.type === "quote") {
        return `<blockquote class="md-quote">${inlineMd(b.text)}</blockquote>`;
      }
      if (b.type === "ul") {
        return `<ul class="md-ul">${b.items
          .map((it) => `<li>${inlineMd(it)}</li>`)
          .join("")}</ul>`;
      }
      if (b.type === "ol") {
        return `<ol class="md-ol">${b.items
          .map((it) => `<li>${inlineMd(it)}</li>`)
          .join("")}</ol>`;
      }
      if (b.type === "table") {
        if (!b.rows.length) return "";
        const head = b.rows[0];
        const body = b.rows.slice(1);
        return `<table class="md-table"><thead><tr>${head
          .map((c) => `<th>${inlineMd(c)}</th>`)
          .join("")}</tr></thead><tbody>${body
          .map(
            (r) =>
              `<tr>${r.map((c) => `<td>${inlineMd(c)}</td>`).join("")}</tr>`
          )
          .join("")}</tbody></table>`;
      }
      if (b.type === "p") return `<p class="md-p">${inlineMd(b.text)}</p>`;
      return "";
    })
    .join("\n");

  return html || `<p class="md-p">${inlineMd(raw)}</p>`;
}

/**
 * Click external http(s)/mailto → system open (Tauri webview target=_blank is unreliable).
 * Uses ccoGateway.openPath when present (Rust handles URL scheme).
 * @param {typeof globalThis} [g]
 */
export function installMdExtLinkOpen(g = typeof window !== "undefined" ? window : globalThis) {
  if (!g?.document || g.__ccoMdExtLinkOpen) return;
  g.__ccoMdExtLinkOpen = true;
  g.document.addEventListener(
    "click",
    (e) => {
      const t = e.target;
      if (!t || typeof t.closest !== "function") return;
      const a = t.closest("a.md-ext-link, a[href]");
      if (!a) return;
      const href = (a.getAttribute("href") || "").trim();
      if (!/^https?:\/\//i.test(href) && !/^mailto:/i.test(href)) return;
      e.preventDefault();
      e.stopPropagation();
      const open =
        (g.ccoGateway && typeof g.ccoGateway.openPath === "function" && g.ccoGateway.openPath) ||
        null;
      if (open) {
        Promise.resolve(open(href)).catch((err) => {
          const msg = err?.message || String(err || "无法打开链接");
          if (typeof g.toast === "function") g.toast(msg);
        });
        return;
      }
      try {
        g.open(href, "_blank", "noopener,noreferrer");
      } catch (_) {
        /* ignore */
      }
    },
    true
  );
}

/**
 * @param {typeof globalThis} [g]
 */
export function installMarkdown(g = typeof window !== "undefined" ? window : globalThis) {
  if (!g) return;
  g.renderMarkdown = renderMarkdown;
  installMdExtLinkOpen(g);
}

export default { renderMarkdown, installMarkdown, installMdExtLinkOpen };
