/**
 * [INPUT]: assistant plain text · state.chatQuizDraft / chatMsgFold
 * [OUTPUT]: 编号题 A/B/C 可点选（选项=短标签—一句说明 · 底部打字逃生口）· 历史消息折叠（Cursor 风：少折、摘要可读、气泡内渐隐）
 * [POS]: features/chat/chatMsgEnhance.js — 不进 chatFormat 厚文件
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 解析须容忍模型常见 markdown：`**1. 标题？**`、硬换行尾空格、`---` 分隔。
 * 回归：`node scripts/chat-quiz-parse-smoke.mjs`（真实会话 shape）。
 *
 * 折叠策略（对齐 Claude Code / Codex CLI：折叠 = 单行摘要，折了必须真省空间）：
 * - 折叠态只有一行：角色 · 单行摘要 · N 行（像 Codex “Called slack 3 times”）
 * - 只折真正长的旧消息（≥400 字或 ≥10 行）；摘要条本身就一两行高的消息永不折
 * - 短会话 / 最近消息 / 短用户句永远展开；超长正文走气泡内 clamp
 * - 「收起」只出现在用户手动展开过的消息上，不给尾部消息撒噪音行
 *
 * 不做：改 Mode B / confirm；不在 JS 写业务策略；不堆进 chatFormat/chatClarify。
 */

import { state, $, toast } from "./legacy.js";
import { chatEsc } from "./chatFormat.js";
import { ensureChatState, stashChatSession } from "./chatState.js";

const STYLE_ID = "cco-chat-msg-enhance-style";
/** 气泡内「展开全部」：更长才裁，给截图报告留出可视高度 */
const FOLD_CHAR_SOFT = 900;
const FOLD_LINES_SOFT = 18;
/** 最近 N 条整条展开（含用户短句）；更早的才考虑整条折 */
const KEEP_OPEN_TAIL = 8;
/** 总条数少于此，整条不自动折（短会话完整时间线） */
const MIN_TOTAL_TO_AUTO_FOLD = 12;
/** 短于此时长的内容不自动整条折（用户短句 / 短确认） */
const SHORT_MSG_CHARS = 160;
/** 折叠必须省出真实空间：低于此体量，单行摘要条 ≈ 展开后的高度，折了白折 */
const FOLD_WORTHY_CHARS = 400;
const FOLD_WORTHY_LINES = 10;

// ─── Styles ─────────────────────────────────────────────────────────────────

export function ensureChatMsgEnhanceStyles() {
  if (typeof document === "undefined") return;
  if (document.getElementById(STYLE_ID)) return;
  const s = document.createElement("style");
  s.id = STYLE_ID;
  s.textContent = `
/* 整条折起：保留左右对齐，看起来像变淡的气泡预览，而不是工具条 */
.chat-msg.is-folded {
  opacity: 0.92;
}
.chat-msg.is-folded > .chat-msg-body { display: none; }
.chat-msg-fold-bar {
  display: flex; align-items: center; gap: 0.5rem;
  margin: 0.1rem 0; padding: 0.4rem 0.7rem;
  border-radius: 14px; border: 1px solid color-mix(in srgb, var(--border, #e5e7eb) 85%, transparent);
  background: color-mix(in srgb, var(--bg2, #fff) 88%, var(--bg3, #f3f4f6));
  cursor: pointer; text-align: left; width: auto; max-width: min(100%, 36rem);
  box-sizing: border-box; font: inherit; color: var(--text, #111);
  box-shadow: 0 1px 0 color-mix(in srgb, var(--border) 35%, transparent);
  transition: border-color .12s ease, background .12s ease, box-shadow .12s ease;
}
.chat-msg-user.is-folded {
  display: flex; justify-content: flex-end;
}
.chat-msg-user.is-folded .chat-msg-fold-bar {
  margin-left: auto;
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 8%, var(--bg2, #fff));
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 18%, var(--border));
}
.chat-msg-assistant.is-folded .chat-msg-fold-bar,
.chat-msg-system.is-folded .chat-msg-fold-bar {
  max-width: min(100%, 40rem);
}
.chat-msg-fold-bar:hover {
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 40%, var(--border));
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 6%, var(--bg2, #fff));
  box-shadow: 0 2px 8px color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 10%, transparent);
}
.chat-msg-fold-bar:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 55%, transparent);
  outline-offset: 2px;
}
.chat-msg-fold-role {
  flex-shrink: 0; font-size: 0.68rem; font-weight: 650; letter-spacing: 0.02em;
  color: var(--muted, #6b7280); min-width: 1.4rem; line-height: 1.2;
  padding: 0.12rem 0.35rem; border-radius: 999px;
  background: color-mix(in srgb, var(--bg3, #f3f4f6) 80%, transparent);
}
.chat-msg-fold-main {
  flex: 1; min-width: 0; display: flex; align-items: baseline; gap: 0.5rem;
}
.chat-msg-fold-sum {
  flex: 1; min-width: 0; font-size: 0.84rem; line-height: 1.35;
  color: color-mix(in srgb, var(--text, #111) 78%, var(--muted, #6b7280));
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.chat-msg-fold-meta {
  flex-shrink: 0; font-size: 0.7rem; color: var(--muted, #6b7280); white-space: nowrap;
}
.chat-msg-fold-cta {
  flex-shrink: 0; display: inline-flex; align-items: center; gap: 0.2rem;
  font-size: 0.72rem; color: var(--leaf-alias-brand-primary, #4176E6);
  font-weight: 600; white-space: nowrap; padding: 0.15rem 0.1rem;
  opacity: 0.92;
}
.chat-msg-fold-cta::after {
  content: ""; width: 0.4rem; height: 0.4rem;
  border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor;
  transform: rotate(-45deg); margin-left: 0.05rem; opacity: 0.85;
}
/* 气泡内裁切：更高可视区 + 底部渐隐，像 Cursor「Show more」 */
.chat-msg-body-long {
  position: relative;
}
.chat-msg-body-long.is-clamped .chat-msg-body-inner {
  max-height: 22rem; overflow: hidden;
  mask-image: linear-gradient(to bottom, #000 62%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, #000 62%, transparent 100%);
}
.chat-msg-body-long.is-clamped > .chat-msg-body-more {
  position: sticky; bottom: 0;
  display: inline-flex; align-items: center; gap: 0.25rem;
  margin-top: -0.15rem; padding: 0.35rem 0.15rem 0.1rem;
  background: linear-gradient(to bottom, transparent, var(--bg2, #fff) 35%);
  width: 100%; box-sizing: border-box;
}
.chat-msg-body-more {
  margin-top: 0.35rem; font: inherit; font-size: 0.78rem;
  color: var(--leaf-alias-brand-primary, #4176E6); background: none; border: none;
  cursor: pointer; padding: 0.2rem 0; font-weight: 550;
}
.chat-msg-body-more:hover { text-decoration: underline; }
/* 展开后的「收起」：放在气泡底部，够明显可点 */
.chat-msg-collapse-row {
  display: flex; align-items: center; justify-content: flex-start;
  gap: 0.65rem; margin-top: 0.45rem; padding-top: 0.35rem;
  border-top: 1px solid color-mix(in srgb, var(--border, #e5e7eb) 70%, transparent);
}
.chat-msg-user .chat-msg-collapse-row { justify-content: flex-end; }
.chat-msg-collapse-row .chat-msg-body-more,
.chat-msg-collapse-row .chat-msg-fold-again {
  margin: 0; font-size: 0.78rem; opacity: 1; font-weight: 600;
  padding: 0.2rem 0.15rem;
}
.chat-msg-fold-again {
  font: inherit; font-size: 0.78rem;
  color: var(--leaf-alias-brand-primary, #4176E6); background: none; border: none;
  cursor: pointer; padding: 0.2rem 0; font-weight: 600;
}
.chat-msg-fold-again:hover { text-decoration: underline; }

.chat-quiz {
  margin: 0.55rem 0 0.25rem;
  padding: 0.65rem 0.75rem 0.7rem;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 22%, var(--border));
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 5%, var(--bg2, #fff));
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
  transition: border-color var(--duration, 150ms) var(--ease, ease),
    background var(--duration, 150ms) var(--ease, ease);
}
.chat-quiz-opt:hover {
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 40%, var(--border));
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 7%, var(--bg2, #fff));
}
.chat-quiz-opt:focus-visible {
  outline: 2px solid var(--leaf-alias-brand-primary, #4176E6);
  outline-offset: 2px;
}
.chat-quiz-opt:disabled { opacity: 0.45; cursor: not-allowed; }
.chat-quiz-opt.is-on {
  border-color: var(--leaf-alias-brand-primary, #4176E6);
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 12%, var(--bg2, #fff));
  font-weight: 550;
}
.chat-quiz-opt .qk {
  flex-shrink: 0; width: 1.35rem; height: 1.35rem;
  border-radius: 999px; display: inline-flex; align-items: center; justify-content: center;
  font-size: 0.72rem; font-weight: 700;
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 12%, transparent);
  color: var(--leaf-alias-brand-primary, #4176E6);
}
.chat-quiz-q.is-multi .chat-quiz-opt .qk { border-radius: 4px; }
.chat-quiz-opt.is-on .qk {
  background: var(--leaf-alias-brand-primary, #4176E6); color: #fff;
}
.chat-quiz-opt .qt { flex: 1; min-width: 0; }
.chat-quiz-opt .qt-l { display: block; }
.chat-quiz-opt .qt-d {
  display: block; font-size: 0.74rem; font-weight: 400;
  color: var(--muted); line-height: 1.35; margin-top: 0.08rem;
}
.chat-quiz-escape {
  margin: 0.45rem 0 0; font-size: 0.72rem; color: var(--muted);
}
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
 * Light markdown unwrap so quiz markers survive model formatting.
 * Real sessions often emit `**1. 标题？**` + MD hard-break trailing spaces —
 * without this, the numbered A/B/C detector never fires and the bubble
 * stays plain text (looks like “点选没做”).
 */
function normalizeQuizSource(text) {
  let s = String(text || "").replace(/\r\n/g, "\n");
  // **1. 标题（可多选）**  →  1. 标题（可多选）
  s = s.replace(
    /(^|\n)[ \t]*\*\*[ \t]*(\d{1,2})[ \t]*[.、．)][ \t]*([^*\n]+?)[ \t]*\*\*/g,
    (_, p, n, title) => `${p}${n}. ${String(title).trim()}`
  );
  // **1.** 标题  →  1. 标题
  s = s.replace(
    /(^|\n)[ \t]*\*\*[ \t]*(\d{1,2})[ \t]*[.、．)][ \t]*\*\*[ \t]*/g,
    (_, p, n) => `${p}${n}. `
  );
  // **A.** / *A.* option keys
  s = s.replace(
    /(^|\n)[ \t]*\*{1,2}[ \t]*([A-Da-d])[ \t]*[.、．)][ \t]*\*{1,2}[ \t]*/g,
    (_, p, k) => `${p}${k}. `
  );
  // Drop MD hard-break double spaces at EOL
  s = s.replace(/[ \t]{2,}$/gm, "");
  // Drop horizontal rules that only separate quiz blocks
  s = s.replace(/(^|\n)\s*-{3,}\s*(?=\n|$)/g, "$1");
  return s.trim();
}

/** Strip simple emphasis for human-facing quiz labels (panel is plain text). */
function stripMdLight(s) {
  return String(s || "")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/_([^_]+)_/g, "$1")
    .trim();
}

const QUIZ_MULTI_MARK =
  /可多选|可选多个|可同时选|可勾选多项|多选题|[（(【\[]\s*多选\s*[）)】\]]|multi[\s-]?select|\(\s*multiple\s*\)/i;
const QUIZ_MULTI_ASK = /哪些|哪几[个项题]?|明确不做/;

/**
 * Multi when the title says so, asks "哪些/明确不做", or has 4+ lettered options.
 * Models often omit "（可多选）" on the 不做/范围闸 slot — that must still toggle.
 */
function detectQuizMulti(titleRaw, blockHead, optionCount) {
  const t = String(titleRaw || "");
  const head = String(blockHead || "");
  if (QUIZ_MULTI_MARK.test(t) || QUIZ_MULTI_MARK.test(head) || QUIZ_MULTI_ASK.test(t)) {
    return true;
  }
  return Number(optionCount) >= 4;
}

/**
 * `A. 短标签 — 一句说明` → { key, label, desc }（Claude/Codex 的 label+description 结构；
 * 无破折号时整句当 label，旧格式照常渲染单行）。
 */
function splitQuizOption(key, text) {
  const full = String(text).trim();
  const parts = full.split(/\s*[—–－]\s*/);
  if (parts.length >= 2 && parts[0] && parts.slice(1).join("").trim()) {
    return { key, text: full, label: parts[0], desc: parts.slice(1).join(" — ").trim() };
  }
  return { key, text: full, label: full, desc: "" };
}

function stripQuizMultiMarker(titleRaw) {
  return String(titleRaw || "")
    .replace(
      /\s*[（(【\[]\s*(可多选|多选|multiple|multi[\s-]?select)\s*[）)】\]]\s*$/iu,
      ""
    )
    .trim();
}

/**
 * Detect numbered questions with A/B/C(…) options in assistant prose.
 * Tolerates markdown bold on titles (`**1. …**`) and hard-break spacing.
 * @returns {{ lead: string, questions: Array<{n:string,title:string,multi:boolean,options:Array<{key:string,text:string}>}> } | null}
 */
export function parseAssistantQuiz(text) {
  const raw = normalizeQuizSource(text);
  if (!raw) return null;
  // Split on "1." / "1、" / "1)" at line start (after normalize)
  // Also allow a lone bold residue that normalize missed: optional ** wrappers.
  const re = /(?:^|\n)[ \t]*(?:\*\*[ \t]*)?(\d{1,2})[ \t]*[.、．)][ \t]*(?:\*\*[ \t]*)?/g;
  const hits = [];
  let m;
  while ((m = re.exec(raw)) !== null) {
    hits.push({
      n: m[1],
      index: m.index + (m[0].startsWith("\n") ? 1 : 0),
      full: m[0],
    });
  }
  if (hits.length < 2) return null;

  const questions = [];
  for (let i = 0; i < hits.length; i++) {
    const start = hits[i].index;
    const end = i + 1 < hits.length ? hits[i + 1].index : raw.length;
    // body after the "N. " marker (same tolerant shape as re)
    const marker = raw
      .slice(start)
      .match(/^[ \t]*(?:\*\*[ \t]*)?\d{1,2}[ \t]*[.、．)][ \t]*(?:\*\*[ \t]*)?/);
    const bodyStart = start + (marker ? marker[0].length : 0);
    const block = raw.slice(bodyStart, end).trim();
    if (!block) continue;

    const lines = block
      .split("\n")
      .map((l) => l.replace(/[ \t]+$/g, "").trim())
      .filter(Boolean);
    if (!lines.length) continue;

    const titleRaw = stripMdLight(lines[0]);
    const options = [];
    for (const line of lines.slice(1)) {
      const om = line.match(
        /^(?:\*{1,2}[ \t]*)?([A-Da-d])[ \t]*[.、．)][ \t]*(?:\*{1,2}[ \t]*)?(.+)$/
      );
      if (om) {
        const optText = stripMdLight(om[2]);
        if (optText) options.push(splitQuizOption(om[1].toUpperCase(), optText));
        continue;
      }
      // stop at "其他" freestyle line — keep as non-option note, ignore
      if (/^其他/.test(stripMdLight(line))) break;
    }
    if (options.length < 2) continue;
    const multi = detectQuizMulti(titleRaw, block.slice(0, 120), options.length);
    const title = stripQuizMultiMarker(titleRaw);
    questions.push({ n: String(hits[i].n), title, multi, options });
  }

  if (questions.length < 2) return null;

  const firstHit = hits[0].index;
  const lead = stripMdLight(
    raw
      .slice(0, firstHit)
      .replace(/(^|\n)\s*-{3,}\s*/g, "$1")
      .trim()
  );
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

/** Option body: 短标签一行 + 说明一行（无说明时单行，旧文案不受影响）。 */
function quizOptBodyHtml(o) {
  const label = o.label ?? o.text ?? "";
  if (!o.desc) return chatEsc(label);
  return `<span class="qt-l">${chatEsc(label)}</span><span class="qt-d">${chatEsc(o.desc)}</span>`;
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
            `<button type="button" class="chat-quiz-opt${on}${q.multi ? " is-multi" : ""}"` +
            ` data-chat-quiz-opt="1"` +
            ` data-quiz-msg="${chatEsc(msgKey)}"` +
            ` data-quiz-q="${chatEsc(q.n)}"` +
            ` data-quiz-key="${chatEsc(o.key)}"` +
            ` data-quiz-multi="${q.multi ? "1" : "0"}"` +
            ` role="${q.multi ? "checkbox" : "radio"}"` +
            ` aria-checked="${on ? "true" : "false"}"` +
            ` aria-pressed="${on ? "true" : "false"}">` +
            `<span class="qk">${chatEsc(o.key)}</span>` +
            `<span class="qt">${quizOptBodyHtml(o)}</span>` +
            `</button>`
          );
        })
        .join("");
      return (
        `<div class="chat-quiz-q${q.multi ? " is-multi" : ""}" data-quiz-q-block="${chatEsc(q.n)}">` +
        `<p class="chat-quiz-q-title">` +
        `${chatEsc(q.n)}. ${chatEsc(q.title)}` +
        (q.multi
          ? `<span class="chat-quiz-q-multi">可多选</span>`
          : "") +
        `</p>` +
        `<div class="chat-quiz-opts"${q.multi ? "" : ` role="radiogroup" aria-label="${chatEsc(q.title)}"`}>${opts}</div>` +
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
    `</div>` +
    `<p class="chat-quiz-escape">选项不合适？直接打字回复即可</p>` +
    `</div>`
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

/** Strip md noise so folded preview reads like speech, not source. */
function cleanPreviewText(text) {
  return String(text || "")
    .replace(/```[\s\S]*?```/g, " 〔代码/计划〕 ")
    .replace(/!\[[^\]]*\]\([^)]+\)/g, " 〔图〕 ")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/(^|[^*])\*([^*]+)\*(?!\*)/g, "$1$2")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "")
    .replace(/\|/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function oneLineSummary(text, max = 96) {
  const s = cleanPreviewText(text);
  if (!s) return "（空消息）";
  const chars = Array.from(s);
  if (chars.length <= max) return s;
  return chars.slice(0, max).join("") + "…";
}

function contentStats(text) {
  const s = String(text || "");
  const lines = s ? s.split(/\n/).length : 0;
  return { chars: s.length, lines };
}

/**
 * Cursor-like whole-message fold:
 * - user preference always wins
 * - short chats stay fully open
 * - recent KEEP_OPEN_TAIL stay open
 * - short user / short assistant never auto-fold
 * - only older, longer turns collapse to a preview pill
 *
 * @param {number} msgIndex
 * @param {number} total
 * @param {string} content
 * @param {{ forceOpen?: boolean, role?: string }} [opts]
 */
export function shouldFoldMessage(msgIndex, total, content, opts = {}) {
  ensureQuizDraftBag();
  const key = quizMsgKey(msgIndex);
  // Explicit user preference wins
  if (state.chatMsgFold[key] === false) return false;
  if (state.chatMsgFold[key] === true) return true;
  if (opts.forceOpen) return false;

  const fromEnd = total - 1 - msgIndex;
  if (fromEnd < KEEP_OPEN_TAIL) return false;
  if (total < MIN_TOTAL_TO_AUTO_FOLD) return false;

  const { chars, lines } = contentStats(content);
  const role = String(opts.role || "");
  // Short user pings / short leaf acks stay visible — folding them into pills is hostile
  if (role === "user" && chars <= SHORT_MSG_CHARS * 1.5) return false;
  // 折了必须真省空间：不够长的内容折成摘要条，展开只多一两行，正是被抱怨的行为
  if (chars < FOLD_WORTHY_CHARS && lines < FOLD_WORTHY_LINES) return false;

  return true;
}

/** True if content is long enough to deserve clamp / collapse controls. */
export function isLongChatBody(content) {
  const { chars, lines } = contentStats(content);
  return chars > FOLD_CHAR_SOFT || lines > FOLD_LINES_SOFT;
}

export function shouldClampBody(content, msgIndex, opts = {}) {
  if (opts.usedQuiz) return false;
  ensureQuizDraftBag();
  const key = quizMsgKey(msgIndex);
  if (state.chatMsgBodyOpen[key]) return false;
  return isLongChatBody(content);
}

/** Body was long + user already clicked 展开全部 → show 收起. */
export function shouldShowBodyCollapse(content, msgIndex, opts = {}) {
  if (opts.usedQuiz) return false;
  ensureQuizDraftBag();
  const key = quizMsgKey(msgIndex);
  if (!state.chatMsgBodyOpen[key]) return false;
  return isLongChatBody(content);
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

/** Full body after 展开全部 — bottom 「收起」 re-clamps. */
export function wrapExpandedBody(innerHtml, msgIndex) {
  const key = quizMsgKey(msgIndex);
  return (
    `<div class="chat-msg-body-long" data-msg-body="${chatEsc(key)}">` +
    `<div class="chat-msg-body-inner">${innerHtml}</div>` +
    `<div class="chat-msg-collapse-row">` +
    `<button type="button" class="chat-msg-body-more" data-chat-body-less="${chatEsc(
      key
    )}">收起</button>` +
    `</div>` +
    `</div>`
  );
}

export function renderFoldBarHtml(roleLabel, content, msgIndex, opts = {}) {
  const key = quizMsgKey(msgIndex);
  const sum = oneLineSummary(content);
  const { lines } = contentStats(content);
  // 单行摘要条右侧只给一个事实：多少行（Codex 风），不再堆形容句
  const meta = lines > 3 ? `${lines} 行` : "展开";
  const role = String(opts.role || "");
  const aria =
    role === "user" ? "展开我的这条消息" : "展开小叶的这条消息";
  return (
    `<button type="button" class="chat-msg-fold-bar" data-chat-msg-unfold="${chatEsc(
      key
    )}" title="${chatEsc(aria)}" aria-label="${chatEsc(aria + "：" + sum)}">` +
    `<span class="chat-msg-fold-role">${chatEsc(roleLabel)}</span>` +
    `<span class="chat-msg-fold-main">` +
    `<span class="chat-msg-fold-sum">${chatEsc(sum)}</span>` +
    `<span class="chat-msg-fold-meta">${chatEsc(meta)}</span>` +
    `</span>` +
    `<span class="chat-msg-fold-cta">展开</span>` +
    `</button>`
  );
}

export function renderFoldAgainBtn(msgIndex) {
  const key = quizMsgKey(msgIndex);
  return (
    `<div class="chat-msg-collapse-row">` +
    `<button type="button" class="chat-msg-fold-again" data-chat-msg-fold="${chatEsc(
      key
    )}" title="收起这条消息">收起</button>` +
    `</div>`
  );
}

/**
 * Claude/Codex 语义：整条「收起」只出现在用户手动展开过的消息上（chatMsgFold=false）。
 * 旧版给所有 ≥2 条外的长消息都挂「收起」行，正是「展开只多一行」噪音的另一半；
 * 现在旧消息离开最近区后自然回到单行折叠条，尾部消息保持干净。
 */
export function shouldShowFoldAgain(msgIndex) {
  ensureQuizDraftBag();
  return state.chatMsgFold[quizMsgKey(msgIndex)] === false;
}

// ─── Actions ─────────────────────────────────────────────────────────────────

function repaintMessages() {
  // 交互型 repaint（点选/折叠）必须真正重绘新状态，跳过指纹空转会丢掉视觉反馈。
  const force = { force: true };
  try {
    if (typeof window !== "undefined" && window.ccoChat?.renderChatMessages) {
      window.ccoChat.renderChatMessages(force);
      return;
    }
  } catch (_) {}
  try {
    if (typeof window !== "undefined" && typeof window.renderChatMessages === "function") {
      window.renderChatMessages(force);
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
      toast("小叶还在回复，已填入输入框，稍后再发");
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

/** Reverse of 展开全部 — re-clamp long body. */
export function collapseChatMsgBody(msgKey) {
  ensureQuizDraftBag();
  const key = String(msgKey || "");
  if (!key) return;
  if (state.chatMsgBodyOpen && typeof state.chatMsgBodyOpen === "object") {
    delete state.chatMsgBodyOpen[key];
  }
  try {
    stashChatSession(state.selectedPath);
  } catch (_) {}
  repaintMessages();
}
