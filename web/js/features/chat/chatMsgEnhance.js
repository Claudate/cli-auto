/**
 * [INPUT]: assistant plain text · state.chatQuizDraft / chatMsgFold
 * [OUTPUT]: 编号题 A/B/C 可点选 · 历史消息默认折叠自展开
 * [POS]: features/chat/chatMsgEnhance.js — 不进 chatFormat 厚文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 不做：改 Mode B / confirm；不在 JS 写业务策略；不堆进 chatFormat/chatClarify。
 */

import { state, $, toast } from "./legacy.js";
import { chatEsc } from "./chatFormat.js";
import { ensureChatState, stashChatSession } from "./chatState.js";

const STYLE_ID = "cco-chat-msg-enhance-style";
const FOLD_CHAR_SOFT = 420;
const FOLD_LINES_SOFT = 10;
/** Keep this many newest messages fully open (plus pending bubble). */
const KEEP_OPEN_TAIL = 2;

// ─── Styles ─────────────────────────────────────────────────────────────────

export function ensureChatMsgEnhanceStyles() {
  if (typeof document === "undefined") return;
  if (document.getElementById(STYLE_ID)) return;
  const s = document.createElement("style");
  s.id = STYLE_ID;
  s.textContent = `
.chat-msg.is-folded > .chat-msg-body { display: none; }
.chat-msg-fold-bar {
  display: flex; align-items: flex-start; gap: 0.5rem;
  margin: 0.15rem 0 0.1rem; padding: 0.45rem 0.65rem;
  border-radius: 10px; border: 1px solid var(--border, #e5e7eb);
  background: color-mix(in srgb, var(--bg3, #f3f4f6) 70%, transparent);
  cursor: pointer; text-align: left; width: 100%; box-sizing: border-box;
  font: inherit; color: var(--text, #111);
}
.chat-msg-fold-bar:hover {
  border-color: color-mix(in srgb, var(--accent, #2563eb) 35%, var(--border));
}
.chat-msg-fold-role {
  flex-shrink: 0; font-size: 0.72rem; font-weight: 600;
  color: var(--muted, #6b7280); min-width: 1.6rem; padding-top: 0.1rem;
}
.chat-msg-fold-sum {
  flex: 1; min-width: 0; font-size: 0.84rem; line-height: 1.4;
  color: var(--muted, #6b7280);
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;
  overflow: hidden;
}
.chat-msg-fold-cta {
  flex-shrink: 0; font-size: 0.75rem; color: var(--accent, #2563eb);
  font-weight: 550; white-space: nowrap; padding-top: 0.1rem;
}
.chat-msg-body-long {
  position: relative;
}
.chat-msg-body-long.is-clamped .chat-msg-body-inner {
  max-height: 9.5rem; overflow: hidden;
  mask-image: linear-gradient(to bottom, #000 55%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, #000 55%, transparent 100%);
}
.chat-msg-body-more {
  margin-top: 0.35rem; font: inherit; font-size: 0.78rem;
  color: var(--accent, #2563eb); background: none; border: none;
  cursor: pointer; padding: 0.15rem 0; font-weight: 550;
}
.chat-msg-body-more:hover { text-decoration: underline; }

.chat-quiz {
  margin: 0.55rem 0 0.25rem;
  padding: 0.65rem 0.75rem 0.7rem;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--accent, #2563eb) 22%, var(--border));
  background: color-mix(in srgb, var(--accent, #2563eb) 5%, var(--bg2, #fff));
  text-align: left;
}
.chat-quiz-head {
  display: flex; align-items: baseline; justify-content: space-between;
  gap: 0.5rem; margin-bottom: 0.45rem;
}
.chat-quiz-title {
  margin: 0; font-size: 0.82rem; font-weight: 650; color: var(--text);
}
.chat-quiz-hint {
  margin: 0; font-size: 0.72rem; color: var(--muted);
}
.chat-quiz-q {
  margin: 0.55rem 0 0.35rem;
  padding-top: 0.45rem;
  border-top: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
}
.chat-quiz-q:first-of-type { border-top: none; padding-top: 0; margin-top: 0.15rem; }
.chat-quiz-q-title {
  margin: 0 0 0.35rem; font-size: 0.9rem; font-weight: 600;
  color: var(--text); line-height: 1.4;
}
.chat-quiz-q-multi {
  font-size: 0.72rem; font-weight: 500; color: var(--muted); margin-left: 0.35rem;
}
.chat-quiz-opts {
  display: flex; flex-direction: column; gap: 0.3rem;
}
.chat-quiz-opt {
  font: inherit; text-align: left; font-size: 0.86rem; line-height: 1.4;
  padding: 0.42rem 0.65rem; border-radius: 9px;
  border: 1px solid var(--border); background: var(--bg, #f5f5f7);
  color: var(--text); cursor: pointer;
  display: flex; gap: 0.45rem; align-items: flex-start;
}
.chat-quiz-opt:hover {
  border-color: color-mix(in srgb, var(--accent, #2563eb) 40%, var(--border));
  background: color-mix(in srgb, var(--accent, #2563eb) 7%, var(--bg2, #fff));
}
.chat-quiz-opt.is-on {
  border-color: var(--accent, #2563eb);
  background: color-mix(in srgb, var(--accent, #2563eb) 12%, var(--bg2, #fff));
  font-weight: 550;
}
.chat-quiz-opt .qk {
  flex-shrink: 0; width: 1.35rem; height: 1.35rem;
  border-radius: 6px; display: inline-flex; align-items: center; justify-content: center;
  font-size: 0.72rem; font-weight: 700;
  background: color-mix(in srgb, var(--accent, #2563eb) 12%, transparent);
  color: var(--accent, #2563eb);
}
.chat-quiz-opt.is-on .qk {
  background: var(--accent, #2563eb); color: #fff;
}
.chat-quiz-opt .qt { flex: 1; min-width: 0; }
.chat-quiz-foot {
  display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center;
  margin-top: 0.65rem; padding-top: 0.5rem;
  border-top: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
}
.chat-quiz-draft {
  flex: 1; min-width: 6rem; font-size: 0.78rem; color: var(--muted);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.chat-quiz-foot .btn { font-size: 0.8rem; }
.chat-quiz-prose {
  font-size: 0.86rem; color: var(--muted); line-height: 1.45;
  margin: 0 0 0.5rem;
}
`;
  document.head.appendChild(s);
}

// ─── Quiz parse ──────────────────────────────────────────────────────────────

/**
 * Detect numbered questions with A/B/C(…) options in assistant prose.
 * @returns {{ lead: string, questions: Array<{n:string,title:string,multi:boolean,options:Array<{key:string,text:string}>}> } | null}
 */
export function parseAssistantQuiz(text) {
  const raw = String(text || "").replace(/\r\n/g, "\n").trim();
  if (!raw) return null;
  // Split on "1." / "1、" / "1)" at line start
  const re = /(?:^|\n)\s*(\d{1,2})\s*[.、．)]\s+/g;
  const hits = [];
  let m;
  while ((m = re.exec(raw)) !== null) {
    hits.push({ n: m[1], index: m.index + (m[0].startsWith("\n") ? 1 : 0), full: m[0] });
  }
  if (hits.length < 2) return null;

  const questions = [];
  for (let i = 0; i < hits.length; i++) {
    const start = hits[i].index;
    const end = i + 1 < hits.length ? hits[i + 1].index : raw.length;
    // body after the "N. " marker
    const marker = raw.slice(start).match(/^\s*\d{1,2}\s*[.、．)]\s*/);
    const bodyStart = start + (marker ? marker[0].length : 0);
    const block = raw.slice(bodyStart, end).trim();
    if (!block) continue;

    const lines = block.split("\n").map((l) => l.trim()).filter(Boolean);
    if (!lines.length) continue;

    const title = lines[0].replace(/\s*（可多选）\s*$/, "").trim();
    const multi =
      /可多选/.test(lines[0]) ||
      /可多选/.test(block.slice(0, 80)) ||
      lines.filter((l) => /^[A-Da-d]\s*[.、．)]\s+/.test(l)).length >= 4;

    const options = [];
    for (const line of lines.slice(1)) {
      const om = line.match(/^([A-Da-d])\s*[.、．)]\s+(.+)$/);
      if (om) {
        options.push({ key: om[1].toUpperCase(), text: om[2].trim() });
        continue;
      }
      // stop at "其他" freestyle line — keep as non-option note, ignore
      if (/^其他/.test(line)) break;
    }
    if (options.length < 2) continue;
    questions.push({ n: String(hits[i].n), title, multi, options });
  }

  if (questions.length < 2) return null;

  const firstHit = hits[0].index;
  const lead = raw.slice(0, firstHit).trim();
  return { lead, questions };
}

function quizMsgKey(msgIndex) {
  return `m${msgIndex}`;
}

function ensureQuizDraftBag() {
  ensureChatState();
  if (!state.chatQuizDraft || typeof state.chatQuizDraft !== "object") {
    state.chatQuizDraft = {};
  }
  if (!state.chatMsgFold || typeof state.chatMsgFold !== "object") {
    state.chatMsgFold = {};
  }
  if (!state.chatMsgBodyOpen || typeof state.chatMsgBodyOpen !== "object") {
    state.chatMsgBodyOpen = {};
  }
}

/**
 * Compose draft like `1B 2A 3A 4ABCD 5B` (product-friendly, matches user habit).
 */
export function composeQuizDraft(msgKey, questions) {
  ensureQuizDraftBag();
  const d = state.chatQuizDraft[msgKey] || {};
  const parts = [];
  for (const q of questions || []) {
    const v = d[q.n];
    if (v == null || v === "") continue;
    if (Array.isArray(v)) {
      const keys = v.map((x) => String(x).toUpperCase()).filter(Boolean).sort();
      if (keys.length) parts.push(`${q.n}${keys.join("")}`);
    } else {
      parts.push(`${q.n}${String(v).toUpperCase()}`);
    }
  }
  return parts.join(" ");
}

export function renderQuizPanelHtml(quiz, msgIndex) {
  if (!quiz || !quiz.questions?.length) return "";
  ensureQuizDraftBag();
  ensureChatMsgEnhanceStyles();
  const msgKey = quizMsgKey(msgIndex);
  const d = state.chatQuizDraft[msgKey] || {};
  const draftStr = composeQuizDraft(msgKey, quiz.questions);

  const qs = quiz.questions
    .map((q) => {
      const selected = d[q.n];
      const selSet = new Set(
        Array.isArray(selected)
          ? selected.map((x) => String(x).toUpperCase())
          : selected
            ? [String(selected).toUpperCase()]
            : []
      );
      const opts = q.options
        .map((o) => {
          const on = selSet.has(o.key) ? " is-on" : "";
          return (
            `<button type="button" class="chat-quiz-opt${on}"` +
            ` data-chat-quiz-opt="1"` +
            ` data-quiz-msg="${chatEsc(msgKey)}"` +
            ` data-quiz-q="${chatEsc(q.n)}"` +
            ` data-quiz-key="${chatEsc(o.key)}"` +
            ` data-quiz-multi="${q.multi ? "1" : "0"}"` +
            ` aria-pressed="${on ? "true" : "false"}">` +
            `<span class="qk">${chatEsc(o.key)}</span>` +
            `<span class="qt">${chatEsc(o.text)}</span>` +
            `</button>`
          );
        })
        .join("");
      return (
        `<div class="chat-quiz-q" data-quiz-q-block="${chatEsc(q.n)}">` +
        `<p class="chat-quiz-q-title">` +
        `${chatEsc(q.n)}. ${chatEsc(q.title)}` +
        (q.multi
          ? `<span class="chat-quiz-q-multi">可多选</span>`
          : "") +
        `</p>` +
        `<div class="chat-quiz-opts">${opts}</div>` +
        `</div>`
      );
    })
    .join("");

  const lead =
    quiz.lead && quiz.lead.length < 280
      ? `<p class="chat-quiz-prose">${chatEsc(quiz.lead)}</p>`
      : quiz.lead
        ? `<p class="chat-quiz-prose">${chatEsc(quiz.lead.slice(0, 200))}…</p>`
        : "";

  return (
    `<div class="chat-quiz" data-chat-quiz="${chatEsc(msgKey)}" data-quiz-count="${quiz.questions.length}">` +
    `<div class="chat-quiz-head">` +
    `<p class="chat-quiz-title">点选作答（可改）</p>` +
    `<p class="chat-quiz-hint">不必手打 1A 2B</p>` +
    `</div>` +
    lead +
    qs +
    `<div class="chat-quiz-foot">` +
    `<span class="chat-quiz-draft" data-quiz-draft="${chatEsc(msgKey)}">${
      draftStr
        ? chatEsc(draftStr)
        : "还没选 · 点上面选项"
    }</span>` +
    `<button type="button" class="btn ghost sm" data-chat-quiz-fill="${chatEsc(
      msgKey
    )}" title="写入下方输入框，不发送">填入输入框</button>` +
    `<button type="button" class="btn primary sm" data-chat-quiz-send="${chatEsc(
      msgKey
    )}" title="填入并发送">发送所选</button>` +
    `</div></div>`
  );
}

/**
 * @returns {{ usedQuiz: boolean, html: string }}
 */
export function enhanceAssistantBody(text, msgIndex, formatBodyFn) {
  const quiz = parseAssistantQuiz(text);
  if (!quiz) {
    return { usedQuiz: false, html: formatBodyFn(text) };
  }
  // Hide raw numbered list in md — quiz panel is the interaction surface.
  // Keep a short collapsed "原文" for power users.
  const panel = renderQuizPanelHtml(quiz, msgIndex);
  const rawId = `quiz-raw-${quizMsgKey(msgIndex)}`;
  const rawBlock =
    `<details class="chat-quiz-raw" id="${chatEsc(rawId)}">` +
    `<summary class="chat-msg-body-more" style="list-style:none;cursor:pointer">查看原文字</summary>` +
    `<div class="md-body" style="margin-top:0.4rem;opacity:0.92">${formatBodyFn(text)}</div>` +
    `</details>`;
  return { usedQuiz: true, html: panel + rawBlock };
}

// ─── Message fold ────────────────────────────────────────────────────────────

function oneLineSummary(text, max = 72) {
  const s = String(text || "")
    .replace(/```[\s\S]*?```/g, "〔计划/代码〕")
    .replace(/\s+/g, " ")
    .trim();
  if (!s) return "（空消息）";
  const chars = Array.from(s);
  if (chars.length <= max) return s;
  return chars.slice(0, max).join("") + "…";
}

export function shouldFoldMessage(msgIndex, total, content, opts = {}) {
  ensureQuizDraftBag();
  const key = quizMsgKey(msgIndex);
  // Explicit user preference wins
  if (state.chatMsgFold[key] === false) return false;
  if (state.chatMsgFold[key] === true) return true;
  // Default: fold all but last KEEP_OPEN_TAIL (unless forceOpen e.g. has quiz on latest)
  if (opts.forceOpen) return false;
  const fromEnd = total - 1 - msgIndex;
  if (fromEnd < KEEP_OPEN_TAIL) return false;
  // Always offer fold for older turns
  return true;
}

export function shouldClampBody(content, msgIndex, opts = {}) {
  if (opts.usedQuiz) return false;
  ensureQuizDraftBag();
  const key = quizMsgKey(msgIndex);
  if (state.chatMsgBodyOpen[key]) return false;
  const s = String(content || "");
  const lines = s.split(/\n/).length;
  return s.length > FOLD_CHAR_SOFT || lines > FOLD_LINES_SOFT;
}

export function wrapClampedBody(innerHtml, msgIndex, clamped) {
  if (!clamped) return innerHtml;
  const key = quizMsgKey(msgIndex);
  return (
    `<div class="chat-msg-body-long is-clamped" data-msg-body="${chatEsc(key)}">` +
    `<div class="chat-msg-body-inner">${innerHtml}</div>` +
    `<button type="button" class="chat-msg-body-more" data-chat-body-more="${chatEsc(
      key
    )}">展开全部</button>` +
    `</div>`
  );
}

export function renderFoldBarHtml(roleLabel, content, msgIndex) {
  const key = quizMsgKey(msgIndex);
  const sum = oneLineSummary(content);
  return (
    `<button type="button" class="chat-msg-fold-bar" data-chat-msg-unfold="${chatEsc(
      key
    )}" title="展开这条消息">` +
    `<span class="chat-msg-fold-role">${chatEsc(roleLabel)}</span>` +
    `<span class="chat-msg-fold-sum">${chatEsc(sum)}</span>` +
    `<span class="chat-msg-fold-cta">展开</span>` +
    `</button>`
  );
}

export function renderFoldAgainBtn(msgIndex) {
  const key = quizMsgKey(msgIndex);
  return (
    `<button type="button" class="chat-msg-body-more" data-chat-msg-fold="${chatEsc(
      key
    )}" style="margin:0.15rem 0 0.35rem">收起这条</button>`
  );
}

// ─── Actions ─────────────────────────────────────────────────────────────────

function repaintMessages() {
  try {
    if (typeof window !== "undefined" && window.ccoChat?.renderChatMessages) {
      window.ccoChat.renderChatMessages();
      return;
    }
  } catch (_) {}
  try {
    if (typeof window !== "undefined" && typeof window.renderChatMessages === "function") {
      window.renderChatMessages();
    }
  } catch (_) {}
}

export function pickChatQuizOption(msgKey, qn, optKey, multi) {
  ensureQuizDraftBag();
  const key = String(msgKey || "");
  const q = String(qn || "");
  const opt = String(optKey || "").toUpperCase();
  if (!key || !q || !opt) return;
  if (!state.chatQuizDraft[key] || typeof state.chatQuizDraft[key] !== "object") {
    state.chatQuizDraft[key] = {};
  }
  const bucket = state.chatQuizDraft[key];
  if (multi) {
    const cur = Array.isArray(bucket[q])
      ? bucket[q].map((x) => String(x).toUpperCase())
      : bucket[q]
        ? [String(bucket[q]).toUpperCase()]
        : [];
    const set = new Set(cur);
    if (set.has(opt)) set.delete(opt);
    else set.add(opt);
    const next = Array.from(set);
    if (!next.length) delete bucket[q];
    else bucket[q] = next;
  } else {
    // single: re-tap same key clears
    if (String(bucket[q] || "").toUpperCase() === opt) delete bucket[q];
    else bucket[q] = opt;
  }
  try {
    stashChatSession(state.selectedPath);
  } catch (_) {}
  repaintMessages();
}

export function fillChatQuizDraft(msgKey) {
  ensureQuizDraftBag();
  const key = String(msgKey || "");
  // Re-compose from whatever questions we can read off the live panel dataset,
  // or from draft keys alone.
  const draftObj = state.chatQuizDraft[key] || {};
  const parts = Object.keys(draftObj)
    .sort((a, b) => Number(a) - Number(b))
    .map((qn) => {
      const v = draftObj[qn];
      if (Array.isArray(v)) {
        const keys = v.map((x) => String(x).toUpperCase()).filter(Boolean).sort();
        return keys.length ? `${qn}${keys.join("")}` : null;
      }
      if (v == null || v === "") return null;
      return `${qn}${String(v).toUpperCase()}`;
    })
    .filter(Boolean);
  const text = parts.join(" ");
  if (!text) {
    toast("先点几个选项");
    return false;
  }
  const input = $("#chat-input");
  if (!input) {
    toast("找不到输入框");
    return false;
  }
  if (!state.selectedPath) {
    toast("请先选择项目");
    return false;
  }
  input.disabled = false;
  input.value = text;
  input.focus();
  try {
    input.dispatchEvent(new Event("input", { bubbles: true }));
  } catch (_) {}
  return true;
}

export function sendChatQuizDraft(msgKey) {
  if (!fillChatQuizDraft(msgKey)) return;
  // Defer to send path (busy-safe)
  try {
    if (state.chatBusy) {
      toast("AI 还在回复，已填入输入框，稍后再发");
      return;
    }
    if (typeof window !== "undefined" && window.ccoChat?.sendChatMessage) {
      window.ccoChat.sendChatMessage();
      return;
    }
    if (typeof window !== "undefined" && typeof window.sendChatMessage === "function") {
      window.sendChatMessage();
    }
  } catch (e) {
    toast(String(e?.message || e || "发送失败"));
  }
}

export function unfoldChatMessage(msgKey) {
  ensureQuizDraftBag();
  state.chatMsgFold[String(msgKey)] = false;
  try {
    stashChatSession(state.selectedPath);
  } catch (_) {}
  repaintMessages();
}

export function foldChatMessage(msgKey) {
  ensureQuizDraftBag();
  state.chatMsgFold[String(msgKey)] = true;
  try {
    stashChatSession(state.selectedPath);
  } catch (_) {}
  repaintMessages();
}

export function expandChatMsgBody(msgKey) {
  ensureQuizDraftBag();
  state.chatMsgBodyOpen[String(msgKey)] = true;
  repaintMessages();
}
