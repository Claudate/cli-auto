/**
 * [INPUT]: legacy.state · host.saveChatPlan
 * [OUTPUT]: fence parse · plan cards · format · expand/adopt
 * [POS]: A5-2a features/chat/chatFormat.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, toast, hasActiveRun } from "./legacy.js";
import { host } from "./host.js";
import { ensureChatState, stashChatSession } from "./chatState.js";

export function chatEsc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Parse plan markdown for card outline: first # title + up to 4 outline lines. */
export function chatPlanOutline(md) {
  const lines = String(md || "").split(/\r?\n/);
  let title = "计划草稿";
  const outline = [];
  for (const line of lines) {
    const t = line.trim();
    if (!t) continue;
    if (title === "计划草稿" && /^#\s+/.test(t)) {
      title = t.replace(/^#\s+/, "").trim() || title;
      continue;
    }
    // Prefer numbered / bullet / ## headings as outline
    if (
      outline.length < 4 &&
      (/^(\d+[\.\)]\s+|[-*•]\s+|#{2,3}\s+)/.test(t) ||
        (outline.length === 0 && t.length < 80 && !t.startsWith("```")))
    ) {
      let item = t
        .replace(/^#{1,3}\s+/, "")
        .replace(/^(\d+[\.\)]\s+|[-*•]\s+)/, "")
        .trim();
      if (item && item !== title) outline.push(item);
    }
  }
  // Fallback: first non-title non-empty lines
  if (outline.length === 0) {
    for (const line of lines) {
      const t = line.trim().replace(/^#+\s+/, "");
      if (!t || t === title || t.startsWith("```")) continue;
      outline.push(t.length > 72 ? t.slice(0, 70) + "…" : t);
      if (outline.length >= 4) break;
    }
  }
  return { title, outline };
}

/** Line-start ``` only (mirrors services/chat.rs fence helpers). */
export function chatIsLineStartFence(s, idx) {
  return idx === 0 || s[idx - 1] === "\n" || s[idx - 1] === "\r";
}

export function chatFenceLangTagLen(after) {
  let n = 0;
  for (const ch of after) {
    if (/[A-Za-z0-9_+-]/.test(ch)) n += 1;
    else break;
  }
  return n;
}

export function chatFindLineFence(s, from) {
  if (from >= s.length) return -1;
  let i = from;
  if (i > 0 && s[i - 1] !== "\n" && s[i - 1] !== "\r") {
    const nl = s.indexOf("\n", i);
    if (nl < 0) return -1;
    i = nl + 1;
  }
  while (i < s.length) {
    if (s.startsWith("```", i) && chatIsLineStartFence(s, i)) return i;
    const nl = s.indexOf("\n", i);
    if (nl < 0) break;
    i = nl + 1;
  }
  return -1;
}

/** Close fence body with nested ```lang … ``` support. Returns [end, cont] or null. */
export function chatCloseFenceBody(body) {
  let depth = 1;
  let pos = 0;
  while (true) {
    const j = chatFindLineFence(body, pos);
    if (j < 0) return null;
    const after = body.slice(j + 3);
    const tlen = chatFenceLangTagLen(after);
    const tag = after.slice(0, tlen);
    if (tag) {
      depth += 1;
      pos = j + 3 + tlen;
    } else {
      depth -= 1;
      if (depth === 0) return [j, j + 3];
      pos = j + 3;
    }
  }
}

/**
 * Split assistant markdown into text / plan / code segments.
 * Nested ```text diagrams inside ```plan stay inside the plan body (not cut early).
 */
export function chatSegmentMarkdown(text) {
  const s = String(text || "");
  const out = [];
  let i = 0;
  while (i < s.length) {
    const idx = s.indexOf("```", i);
    if (idx < 0) {
      if (i < s.length) out.push({ type: "text", body: s.slice(i) });
      break;
    }
    if (!chatIsLineStartFence(s, idx)) {
      // mid-line triple-backtick: keep as text through this marker
      if (idx > i) out.push({ type: "text", body: s.slice(i, idx + 3) });
      i = idx + 3;
      continue;
    }
    if (idx > i) out.push({ type: "text", body: s.slice(i, idx) });

    // Absolute offsets into s:
    //   opener at idx..idx+3
    //   tag at idx+3 .. idx+3+tlen
    //   body starts after tag + optional spaces + one newline
    const tagStart = idx + 3;
    const tlen = chatFenceLangTagLen(s.slice(tagStart));
    const tag = s.slice(tagStart, tagStart + tlen);
    let bodyStart = tagStart + tlen;
    while (bodyStart < s.length && (s[bodyStart] === " " || s[bodyStart] === "\t")) {
      bodyStart += 1;
    }
    if (s.startsWith("\r\n", bodyStart)) bodyStart += 2;
    else if (s[bodyStart] === "\n" || s[bodyStart] === "\r") bodyStart += 1;

    const body = s.slice(bodyStart);
    const closed = chatCloseFenceBody(body);
    if (!closed) {
      out.push({ type: "text", body: s.slice(idx) });
      break;
    }
    const [end, cont] = closed;
    const block = body.slice(0, end).replace(/\s+$/, "");
    if (tag.toLowerCase() === "plan") {
      out.push({ type: "plan", body: block });
    } else {
      out.push({ type: "code", lang: tag || "", body: block });
    }
    i = bodyStart + cont;
  }
  return out;
}

/** Pull last ```plan body from free text (nesting-aware). */
export function chatExtractPlanFence(text) {
  const segs = chatSegmentMarkdown(text);
  let best = null;
  for (const seg of segs) {
    if (seg.type === "plan" && seg.body && seg.body.trim()) best = seg.body.trim();
  }
  return best;
}

export function chatNormMdKey(md) {
  return String(md || "")
    .replace(/\r\n/g, "\n")
    .replace(/\s+$/gm, "")
    .trim();
}

/**
 * Footer CTAs live on the plan card (not sticky ready-bar).
 * @param {string} md
 * @param {{ active?: boolean }} opts  active = latest plan in latest assistant reply
 */
export function chatPlanCardActionsHtml(md, opts = {}) {
  ensureChatState();
  const active = opts.active !== false;
  const draft = state.chatSession?.draft_plan;
  const savedPath = state.chatDraftPlan || (draft?.saved ? draft.path : null);
  const draftKey = chatNormMdKey(draft?.markdown || "");
  const cardKey = chatNormMdKey(md);
  // Prefer exact body match; fall back to "active card + has draft" so structure
  // normalize diffs still light the right footer.
  const isThisDraft =
    !!(draftKey && cardKey && draftKey === cardKey) ||
    !!(active && draft && (draft.markdown || savedPath));
  const isSaved = !!(savedPath && isThisDraft && (draft?.saved || state.chatDraftPlan));
  const busy = !!state.chatBusy;
  const runLocked = typeof hasActiveRun === "function" ? hasActiveRun() : false;

  const expand =
    `<button type="button" class="btn ghost sm btn-chat-plan-expand">展开全文</button>`;

  // Historical plan cards: expand only (no sticky-like duplicate CTAs)
  if (!active) {
    return (
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `</div>`
    );
  }

  // B2：主 CTA 始终「拆成步骤」；仅保存 / 重新保存 为 ghost 次按钮
  const canExec = !runLocked && !busy && !!md;
  const assignTitle = runLocked
    ? "运行中，请先停止后再拆分"
    : isSaved
      ? "把计划拆成可执行步骤"
      : "先保存到本机计划，再进入拆分台";
  if (isSaved) {
    return (
      `<span class="chat-plan-card-saved muted">已保存：${chatEsc(savedPath)}</span>` +
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `<button type="button" class="btn ghost sm btn-chat-plan-adopt" ${busy ? "disabled" : ""} title="覆盖保存到本地计划文件">仅保存</button>` +
      `<button type="button" class="btn primary sm btn-chat-plan-assign" ${canExec ? "" : "disabled"} title="${assignTitle}">拆成步骤</button>` +
      `</div>`
    );
  }

  return (
    `<div class="chat-plan-card-actions-btns">` +
    expand +
    `<button type="button" class="btn ghost sm btn-chat-plan-adopt" ${busy || !md ? "disabled" : ""} title="只保存到本机，暂不拆分">仅保存</button>` +
    `<button type="button" class="btn primary sm btn-chat-plan-assign" ${canExec ? "" : "disabled"} title="${assignTitle}">拆成步骤</button>` +
    `</div>`
  );
}

export function chatFormatPlanCard(rawMd, opts = {}) {
  const md = String(rawMd || "").trim();
  const { title, outline } = chatPlanOutline(md);
  const outlineHtml =
    outline.length > 0
      ? `<ul class="chat-plan-outline">${outline
          .map((o) => `<li>${chatEsc(o)}</li>`)
          .join("")}</ul>`
      : `<p class="chat-plan-outline-empty muted">（暂无大纲条目）</p>`;
  // Full body kept in hidden pre for expand; adopt uses same markdown via saveChatPlan
  return (
    `<div class="chat-plan-card" data-plan-md="1">` +
    `<div class="chat-plan-card-label">计划草稿</div>` +
    `<div class="chat-plan-card-title">${chatEsc(title)}</div>` +
    `<div class="chat-plan-summary">` +
    outlineHtml +
    `</div>` +
    `<pre class="chat-plan-pre chat-plan-full" hidden>${chatEsc(md)}</pre>` +
    `<div class="chat-plan-card-actions">` +
    chatPlanCardActionsHtml(md, opts) +
    `</div>` +
    `</div>`
  );
}

/**
 * @param {string} text
 * @param {{ activePlan?: boolean }} opts  when true, last ```plan in this body gets save/exec CTAs
 */
export function chatFormatBody(text, opts = {}) {
  // Parse fences on raw text first (nesting-aware), then escape each segment.
  const segs = chatSegmentMarkdown(text);
  let lastPlanIdx = -1;
  if (opts.activePlan) {
    for (let i = 0; i < segs.length; i++) {
      if (segs[i].type === "plan") lastPlanIdx = i;
    }
  }
  return segs
    .map((seg, i) => {
      if (seg.type === "plan") {
        return chatFormatPlanCard(seg.body, {
          active: opts.activePlan && i === lastPlanIdx,
        });
      }
      if (seg.type === "code") {
        return `<pre class="chat-code-block">${chatEsc(seg.body)}</pre>`;
      }
      let t = chatEsc(seg.body || "");
      t = t.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
      t = t.replace(/\n/g, "<br/>");
      return t;
    })
    .join("");
}

/** Toggle plan card full markdown (expand/collapse). */
export function toggleChatPlanExpand(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const summary = card.querySelector(".chat-plan-summary");
  if (!full) return;
  const open = full.hidden;
  full.hidden = !open;
  if (summary) summary.hidden = open;
  btn.textContent = open ? "收起全文" : "展开全文";
}

/** Card「采用并保存」→ same as ready-bar saveChatPlan. */
export function adoptChatPlanFromCard(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const md = full?.textContent?.trim();
  if (!md) {
    toast("卡片中没有可保存的计划正文");
    return;
  }
  ensureChatState();
  // Seed draft_plan so saveChatPlan uses this markdown
  if (!state.chatSession.draft_plan) {
    state.chatSession.draft_plan = {
      path: "",
      saved: false,
      markdown: md,
      title: null,
    };
  } else {
    state.chatSession.draft_plan.markdown = md;
  }
  stashChatSession(state.selectedPath || state.chatProjectPath);
  return host.saveChatPlan();
}
