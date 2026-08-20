/**
 * [INPUT]: legacy.state · chatRender hooks · chatApi.savePlan（仅认领写草稿）
 * [OUTPUT]: 三入口 · 澄清卡片 · Brief 面板 · 认领写 plan · 空心黄条（不拦）
 * [POS]: t3+t4 features/chat/chatClarify.js — 澄清相 UI；对齐 domain/chat/clarify
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 文件策略（写死）：Brief **仅 session 展示**（state.chatClarify + session.clarify）；
 * 认领后写入 session.draft_plan + 可选 disk save_plan；**不**落独立 brief 文件。
 *
 * 不做：认领链路触发 confirm_start / spawn worker / 第二 Planner / 第二框架。
 * soft-fill 不得覆盖入口默认（idea_to_plan）与文案契约。
 * 黄条不硬拦保存与「分配计划」（D0 子集）。
 */

import { state, $, toast } from "./legacy.js";
import { chatEsc } from "./chatFormat.js";
import { host } from "./host.js";
import * as chatApi from "./chatApi.js";
import { stashChatSession, ensureChatState, sanitizePlanTitle } from "./chatState.js";
import { getPlansDir } from "./planDir.js";
// note: 本文件是 clarify 唯一真源（clarify/ 纵切草稿已删，2026-08-12）。
// 禁止再建平行副本；Duplicate export 会让 type=module main.js 整链挂掉。


// ─── Copy contract (product; first sentence human · non-dev tone) ────────────
// Internal wire ids (entry/slot/phase) stay English; UI labels stay business Chinese.
// Same-screen concept budget ≤3: ① 怎么开始 ② 当前这一问 ③ 可跳过。

export const CLARIFY_COPY = Object.freeze({
  /** Empty coach — invite, not methodology. */
  empty: "用一句话说你想做成什么。下面先帮你问清楚，再写成计划。",
  error: "这句还不够清楚。可以换个说法，或点「跳过，先出草稿」。",
  loading: "正在整理你的想法…",
  /** After claim — next step is assign/split, never auto-run. */
  success: "计划草稿已写好。",
  successNext: "下一步：拆成可并行的步骤（不会自动开始执行）。",
  assignCta: "拆成步骤",
  /** Main CTA — short verb; title/tooltip carries “不会自动开始”. */
  claimCta: "写成计划",
  claimBusy: "正在写入计划草稿…",
  rechat: "再改一改",
  skipCta: "跳过，先出草稿",
  skipAlt: "其余你帮我选",
  skipHint: "跳过剩余问题，按常见假设先出一版，你还能改",
  readySkip: "已按常见假设整理好。扫一眼摘要，再写成计划。",
  readyPlanOnly: "已选直接写计划。仍会写上目标、先不做、怎样算完。",
  /** Progress reads as questions, not internal “clarify slots”. */
  progressLabel: "问题",
  guideTitle: "先问清楚，再写成计划",
  guideHint: "点选即可，大约 1 分钟；随时可跳过。",
  briefTitle: "一页摘要",
  briefHint: "写成计划只保存草稿，不会自动开始执行。",
  hollowWarn:
    "还缺「怎样算做完」或「这轮先不做」。建议补一句（仍可保存与拆成步骤）。",
  hollowNonGoals: "还没写清「这轮先不做」",
  hollowDoneWhen: "还没写清「怎样算做完」",
  moreWays: "换一种方式",
  claimTitle: "写入计划草稿，不会自动开始执行",
  claimGuide: "如有调整，直接在下方输入告诉我；满意后点「这版作数」。",
});

/** Wire schema tag — keep in sync with domain CLARIFY_SCHEMA_VERSION. */
export const CLARIFY_SCHEMA_VERSION = 1;

// ─── Three on-ramps (mirror ClarifyEntry) ────────────────────────────────────
// Default path is the product main road; other two are escapes, not equal menu.

export const CLARIFY_ENTRIES = Object.freeze([
  {
    id: "think_first",
    label: "先只想清楚",
    hint: "先整理一页摘要，先不写成计划",
  },
  {
    id: "idea_to_plan",
    label: "帮我想清楚再写成计划",
    hint: "默认：问几句 → 摘要 → 你确认后写成计划",
    isDefault: true,
  },
  {
    id: "plan_only",
    label: "我已想清，直接写计划",
    hint: "少问几句，仍会写上目标 / 先不做 / 怎样算完",
  },
]);

export const DEFAULT_CLARIFY_ENTRY = "idea_to_plan";

// ─── Required slots + local A/B/C scaffolding (UI only) ──────────────────────

/**
 * Local question bank for the clarify card.
 * Options speak the user's business language — never cco jargon
 * (no 五槽 / 澄清稿 / 主路径 / 可认领).
 * Wire ids stay stable for domain/session.
 */
export const CLARIFY_SLOT_QUESTIONS = Object.freeze([
  {
    id: "target_audience",
    label: "给谁",
    question: "这主要给谁用？",
    options: [
      { key: "A", text: "我自己先用" },
      { key: "B", text: "客户或外部用户" },
      { key: "C", text: "团队一起用" },
    ],
  },
  {
    id: "pain_moment",
    label: "卡在哪",
    question: "现在最让你头疼的是哪一步？",
    options: [
      { key: "A", text: "想法很模糊，不知道从哪下笔" },
      { key: "B", text: "需求老改，做完对不上" },
      { key: "C", text: "和别人对齐太费劲" },
    ],
  },
  {
    id: "observable_outcome",
    label: "做成什么样",
    question: "做成后，你希望别人一眼能看到什么？",
    options: [
      { key: "A", text: "有一份能照着做的计划" },
      { key: "B", text: "能马上拆开动手" },
      { key: "C", text: "先确认这件事值不值得做" },
    ],
  },
  {
    id: "non_goals",
    label: "先不做",
    question: "这轮先不做哪些，好收住范围？",
    options: [
      { key: "A", text: "先不做完整网站 / 营销页" },
      { key: "B", text: "先不做社区、打分、复杂账号" },
      { key: "C", text: "先不做多端或复杂对接" },
    ],
  },
  {
    id: "done_when",
    label: "怎样算完",
    question: "怎样算这轮做完？",
    options: [
      { key: "A", text: "计划写清，能交给下一步拆开做" },
      { key: "B", text: "我能用一分钟讲清要做什么" },
      { key: "C", text: "有几条能勾选的完成标准" },
    ],
  },
]);

const PHASES = new Set([
  "not_started",
  "clarifying",
  "brief_ready",
  "claimed_to_plan",
  "skipped_to_plan",
]);

const FILL_KINDS = new Set(["explicit", "assumed", "soft_fill"]);

// ─── Styles (inject once; reuse cco tokens — no second color kit) ────────────

const CLARIFY_STYLE_ID = "cco-clarify-style-v2";

export function ensureClarifyStyles() {
  if (typeof document === "undefined") return;
  // Drop older injected sheet so copy/UI iteration is visible without hard cache
  const prev = document.getElementById("cco-clarify-style");
  if (prev) prev.remove();
  if (document.getElementById(CLARIFY_STYLE_ID)) return;
  const s = document.createElement("style");
  s.id = CLARIFY_STYLE_ID;
  s.textContent = `
/* t3 clarify phase — light density, cco tokens only */
.chat-clarify {
  margin: 0.35rem auto 0.75rem;
  max-width: 32rem;
  width: 100%;
  text-align: left;
}
.chat-clarify-guide {
  text-align: center;
  margin: 0.15rem 0 0.45rem;
}
.chat-clarify-guide-title {
  margin: 0;
  font-size: 0.98rem;
  font-weight: 650;
  color: var(--text);
  line-height: 1.4;
}
.chat-clarify-guide-hint {
  margin: 0.2rem 0 0;
  font-size: 0.8rem;
  color: var(--muted);
  line-height: 1.4;
}
.chat-clarify-entries {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  justify-content: center;
  align-items: center;
  margin: 0.35rem 0 0.55rem;
}
.chat-clarify-entry {
  font: inherit;
  font-size: 0.78rem;
  padding: 0.35rem 0.7rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg2, #fff);
  color: var(--muted);
  cursor: pointer;
  line-height: 1.3;
  transition: border-color 0.12s ease, background 0.12s ease, color 0.12s ease;
}
.chat-clarify-entry:hover {
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 45%, var(--border));
  color: var(--leaf-alias-brand-primary, #4176E6);
}
/* Default path = primary road; others stay quiet chips */
.chat-clarify-entry.is-default {
  font-size: 0.84rem;
  padding: 0.42rem 0.9rem;
  color: var(--text);
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 35%, var(--border));
}
.chat-clarify-entry.is-active {
  border-color: var(--leaf-alias-brand-primary, #4176E6);
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 12%, var(--bg2, #fff));
  color: var(--leaf-alias-brand-primary, #4176E6);
  font-weight: 600;
}
.chat-clarify-entry.is-active.is-default {
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 25%, transparent);
}
.chat-clarify-entry:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.chat-clarify-entry.is-alt {
  font-size: 0.74rem;
  opacity: 0.92;
}
.chat-clarify-empty-line {
  text-align: center;
  margin: 0.15rem 0 0.35rem;
  font-size: 0.9rem;
  color: var(--muted);
  line-height: 1.5;
}
.chat-clarify-card {
  margin: 0.45rem auto;
  max-width: 32rem;
  width: 100%;
  padding: 0.95rem 1.05rem 0.9rem;
  border-radius: var(--radius, 12px);
  border: 1px solid var(--border);
  background: var(--bg2, #fff);
  box-sizing: border-box;
  box-shadow: 0 1px 2px color-mix(in srgb, var(--text, #000) 4%, transparent);
  transition: opacity 0.12s ease, border-color 0.12s ease;
}
.chat-clarify-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.35rem;
}
.chat-clarify-progress {
  font-size: 0.75rem;
  color: var(--muted);
  font-weight: 550;
}
.chat-clarify-progress-dots {
  display: inline-flex;
  gap: 0.28rem;
  align-items: center;
  margin-left: 0.45rem;
  vertical-align: middle;
}
.chat-clarify-progress-dots > i {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--bg3, #E5E5EA);
  display: inline-block;
}
.chat-clarify-progress-dots > i.is-done {
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 55%, var(--bg3, #E5E5EA));
}
.chat-clarify-progress-dots > i.is-current {
  background: var(--leaf-alias-brand-primary, #4176E6);
  transform: scale(1.15);
}
.chat-clarify-progress-bar {
  display: block;
  height: 3px;
  border-radius: 999px;
  background: var(--bg3, #F0F0F2);
  overflow: hidden;
  margin-top: 0.4rem;
  margin-bottom: 0.75rem;
}
.chat-clarify-progress-bar > i {
  display: block;
  height: 100%;
  background: var(--leaf-alias-brand-primary, #4176E6);
  border-radius: inherit;
  transition: width 0.15s ease;
}
/* Slot labels are internal; hide from non-dev main path */
.chat-clarify-slot-label {
  display: none;
}
.chat-clarify-question {
  margin: 0 0 0.7rem;
  font-size: 1.02rem;
  font-weight: 650;
  color: var(--text);
  line-height: 1.45;
}
.chat-clarify-options {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin: 0 0 0.75rem;
}
.chat-clarify-option {
  font: inherit;
  text-align: left;
  font-size: 0.9rem;
  padding: 0.55rem 0.8rem;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--bg, #F5F5F7);
  color: var(--text);
  cursor: pointer;
  line-height: 1.4;
  pointer-events: auto;
  position: relative;
  z-index: 1;
  transition: border-color 0.12s ease, background 0.12s ease;
}
.chat-clarify-card,
.chat-clarify-options,
.chat-clarify-actions {
  pointer-events: auto;
}
.chat-clarify-option:hover {
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 40%, var(--border));
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 6%, var(--bg2, #fff));
}
.chat-clarify-option.is-selected {
  border-color: var(--leaf-alias-brand-primary, #4176E6);
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 12%, var(--bg2, #fff));
  font-weight: 550;
}
/* Soft letter badge — not exam scoring */
.chat-clarify-option .opt-key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.25rem;
  height: 1.25rem;
  padding: 0 0.2rem;
  border-radius: 6px;
  font-size: 0.72rem;
  font-weight: 650;
  color: var(--leaf-alias-brand-primary, #4176E6);
  background: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 12%, transparent);
  margin-right: 0.5rem;
}
.chat-clarify-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.55rem 0.75rem;
}
.chat-clarify-actions .btn.ghost {
  font-size: 0.8rem;
}
.chat-clarify-skip-hint {
  flex-basis: 100%;
  margin: 0;
  font-size: 0.72rem;
  color: var(--muted);
  line-height: 1.35;
}
.chat-clarify-status {
  margin: 0.5rem auto;
  max-width: 32rem;
  padding: 0.65rem 0.85rem;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--bg2, #fff);
  font-size: 0.9rem;
  line-height: 1.45;
  color: var(--text);
}
.chat-clarify-status.is-error {
  border-color: color-mix(in srgb, var(--danger, #FF3B30) 35%, var(--border));
  background: color-mix(in srgb, var(--danger, #FF3B30) 6%, var(--bg2, #fff));
}
.chat-clarify-status.is-loading {
  color: var(--muted);
}
.chat-clarify-status.is-ready {
  border-color: color-mix(in srgb, var(--ok, #34C759) 30%, var(--border));
}
.chat-clarify-status-actions {
  margin-top: 0.45rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}
.chat-clarify .linkish {
  background: none;
  border: none;
  padding: 0;
  font: inherit;
  color: var(--leaf-alias-brand-primary, #4176E6);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.chat-clarify .linkish.muted {
  color: var(--muted);
  text-decoration: none;
}
.chat-clarify-ico {
  display: inline-flex;
  vertical-align: -0.15em;
  margin-right: 0.3rem;
  color: var(--muted);
}
.chat-empty .chat-clarify {
  margin-top: 0.15rem;
}
.chat-empty-secondary {
  margin-top: 1.1rem;
  padding-top: 0.85rem;
  border-top: 1px solid var(--border);
  opacity: 0.92;
}
.chat-empty-legacy {
  margin: 0;
  text-align: center;
  line-height: 1.55;
}
.chat-empty-legacy .chat-empty-coach {
  font-size: 0.88rem;
  font-weight: 500;
  opacity: 0.9;
}
/* when card sits above message list (non-empty) */
.chat-messages > .chat-clarify {
  align-self: center;
  max-width: min(100%, 32rem);
}
/* t4 Brief panel — grouped scan list */
.chat-brief {
  margin: 0.55rem auto 0.65rem;
  max-width: 32rem;
  width: 100%;
  padding: 0.9rem 1rem 1rem;
  border-radius: var(--radius, 12px);
  border: 1px solid var(--border);
  background: var(--bg2, #fff);
  box-sizing: border-box;
  text-align: left;
}
.chat-brief-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.55rem;
}
.chat-brief-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text);
}
.chat-brief-hint {
  margin: 0;
  font-size: 0.75rem;
  color: var(--muted);
}
.chat-brief-groups {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  margin: 0 0 0.75rem;
}
.chat-brief-group {
  padding: 0.45rem 0.55rem;
  border-radius: 10px;
  background: var(--bg, #F5F5F7);
  border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
}
.chat-brief-group-label {
  margin: 0 0 0.2rem;
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--muted);
}
.chat-brief-group-body {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.5;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
}
.chat-brief-tag {
  display: inline-block;
  margin: 0.1rem 0.3rem 0.1rem 0;
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 600;
  border: 1px solid var(--border);
  background: var(--bg2, #fff);
  color: var(--muted);
}
.chat-brief-tag.is-assumed {
  border-color: color-mix(in srgb, #F5A524 45%, var(--border));
  color: #9a6700;
  background: color-mix(in srgb, #F5A524 12%, var(--bg2, #fff));
}
.chat-brief-tag.is-user {
  border-color: color-mix(in srgb, var(--leaf-alias-brand-primary, #4176E6) 35%, var(--border));
  color: var(--leaf-alias-brand-primary, #4176E6);
}
.chat-brief-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.45rem;
}
.chat-brief-actions .btn.primary {
  font-weight: 650;
}
/* Hollow yellow bar — warn only; never disables save/assign */
.chat-hollow-bar {
  margin: 0.45rem 0 0.65rem;
  padding: 0.55rem 0.75rem;
  border-radius: 10px;
  border: 1px solid color-mix(in srgb, #F5A524 50%, var(--border));
  background: color-mix(in srgb, #F5A524 14%, var(--bg2, #fff));
  color: #7a5a00;
  font-size: 0.84rem;
  line-height: 1.45;
}
.chat-hollow-bar ul {
  margin: 0.3rem 0 0;
  padding-left: 1.1rem;
}
.chat-hollow-bar li {
  margin: 0.1rem 0;
}
.chat-claim-success {
  margin: 0.5rem auto 0.65rem;
  max-width: 32rem;
  padding: 0.85rem 1rem;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--ok, #34C759) 35%, var(--border));
  background: color-mix(in srgb, var(--ok, #34C759) 8%, var(--bg2, #fff));
  font-size: 0.9rem;
  line-height: 1.45;
  color: var(--text);
  text-align: left;
}
.chat-claim-success-title {
  font-weight: 650;
  margin: 0 0 0.2rem;
  font-size: 0.98rem;
}
.chat-claim-success-next {
  margin: 0 0 0.65rem;
  font-size: 0.82rem;
  color: var(--muted);
  line-height: 1.4;
}
.chat-claim-success-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.55rem 0.75rem;
}
.chat-claim-preview {
  margin: 0.55rem 0 0.7rem;
  padding: 0.55rem 0.7rem;
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
  background: var(--bg2, #fff);
  font-size: 0.8rem;
  line-height: 1.45;
  color: var(--text);
  max-height: 9.5rem;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
}
.chat-claim-preview-label {
  margin: 0 0 0.3rem;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--muted);
}
.chat-hollow-bar.is-soft {
  margin-top: 0.55rem;
  opacity: 0.95;
}
`;
  document.head.appendChild(s);
}

// ─── State helpers ───────────────────────────────────────────────────────────

function icon(name, size = 14) {
  if (typeof window !== "undefined" && typeof window.ccoIcon === "function") {
    return window.ccoIcon(name, { size });
  }
  return "";
}

export function defaultClarifyState(entry) {
  return {
    schema_version: CLARIFY_SCHEMA_VERSION,
    entry: entry || DEFAULT_CLARIFY_ENTRY,
    phase: "not_started",
    slots: [],
    optional: [],
    assumptions: [],
    skip_requested: false,
    // UI-only (not serialized to domain wire as strategy)
    uiStatus: "idle", // idle | loading | error | empty
    errorText: null,
    questionIndex: 0,
    selectedOption: null,
  };
}

function normalizeEntry(raw) {
  const s = String(raw || "").trim();
  if (!s) return DEFAULT_CLARIFY_ENTRY;
  if (s === "think_first" || s === "think-first" || s === "想清楚再说" || s === "想清楚") {
    return "think_first";
  }
  if (
    s === "plan_only" ||
    s === "plan-only" ||
    s === "已想清直接写计划" ||
    s === "已想清，直接写计划" ||
    s === "直接写计划"
  ) {
    return "plan_only";
  }
  if (
    s === "idea_to_plan" ||
    s === "idea-to-plan" ||
    s === "从想法到计划" ||
    s === "default"
  ) {
    return "idea_to_plan";
  }
  // Unknown → default (soft-fill must not invent a non-default)
  return DEFAULT_CLARIFY_ENTRY;
}

function normalizePhase(raw) {
  const s = String(raw || "")
    .trim()
    .toLowerCase();
  if (PHASES.has(s)) return s;
  // snake from serde
  const map = {
    notstarted: "not_started",
    briefready: "brief_ready",
    claimedtoplan: "claimed_to_plan",
    skippedtoplan: "skipped_to_plan",
  };
  return map[s.replace(/_/g, "")] || "not_started";
}

function normalizeFillKind(raw) {
  const s = String(raw || "explicit")
    .trim()
    .toLowerCase();
  if (FILL_KINDS.has(s)) return s;
  if (s === "softfill" || s === "soft-fill") return "soft_fill";
  return "explicit";
}

/**
 * Normalize wire/session clarify object into UI state.
 * Preserves Explicit fills; never upgrades Assumed → Explicit.
 */
export function normalizeClarifyState(raw) {
  const base = defaultClarifyState();
  if (!raw || typeof raw !== "object") return base;
  base.schema_version = Number(raw.schema_version) || CLARIFY_SCHEMA_VERSION;
  base.entry = normalizeEntry(raw.entry);
  base.phase = normalizePhase(raw.phase);
  base.skip_requested = !!raw.skip_requested;
  base.slots = Array.isArray(raw.slots)
    ? raw.slots
        .map((s) => {
          if (!s || typeof s !== "object") return null;
          const id = String(s.id || "").trim();
          const value = String(s.value || "").trim();
          if (!id || !value) return null;
          return {
            id,
            value,
            kind: normalizeFillKind(s.kind),
          };
        })
        .filter(Boolean)
    : [];
  base.optional = Array.isArray(raw.optional)
    ? raw.optional
        .filter((o) => o && o.key && String(o.value || "").trim())
        .map((o) => ({
          key: String(o.key),
          value: String(o.value).trim(),
          kind: normalizeFillKind(o.kind),
        }))
    : [];
  base.assumptions = Array.isArray(raw.assumptions)
    ? raw.assumptions
        .filter((a) => a && String(a.text || "").trim())
        .map((a) => ({
          slot: a.slot || null,
          text: String(a.text).trim(),
        }))
    : [];
  // UI fields if present
  if (raw.uiStatus) base.uiStatus = String(raw.uiStatus);
  if (raw.errorText != null) base.errorText = raw.errorText;
  if (raw.questionIndex != null) {
    base.questionIndex = Math.max(0, Number(raw.questionIndex) || 0);
  }
  if (raw.selectedOption != null) base.selectedOption = raw.selectedOption;
  return base;
}

/** Wire-shaped snapshot (no UI-only fields) for session.clarify mirror. */
export function clarifyToWire(c) {
  const s = c || defaultClarifyState();
  return {
    schema_version: s.schema_version || CLARIFY_SCHEMA_VERSION,
    entry: s.entry || DEFAULT_CLARIFY_ENTRY,
    phase: s.phase || "not_started",
    slots: Array.isArray(s.slots) ? s.slots.map((x) => ({ ...x })) : [],
    optional: Array.isArray(s.optional) ? s.optional.map((x) => ({ ...x })) : [],
    assumptions: Array.isArray(s.assumptions)
      ? s.assumptions.map((x) => ({ ...x }))
      : [],
    skip_requested: !!s.skip_requested,
  };
}

/**
 * Claimed residual must match reality:
 * - has plan draft → keep claimed + success panel
 * - no draft → demote to not_started / brief_ready (never show “已写成” empty success)
 */
export function reconcileClaimedPhase() {
  ensureChatState();
  const c = state.chatClarify;
  if (!c || typeof c !== "object") return c;
  const md = String(state.chatSession?.draft_plan?.markdown || "").trim();
  const hasDraft = md.length > 0;
  if (c.phase === "claimed_to_plan") {
    if (!hasDraft) {
      // Stale claimed meta (open project / skip-then-lost draft) → friendly start
      const filled =
        Array.isArray(c.slots) &&
        c.slots.some((s) => String(s?.value || "").trim());
      c.phase = filled && missingRequiredSlots(c).length === 0
        ? "brief_ready"
        : c.skip_requested
          ? "skipped_to_plan"
          : "not_started";
      c._claimSuccess = false;
      c.uiStatus = "idle";
    } else {
      c._claimSuccess = true;
    }
  } else if (hasDraft && (c.phase === "brief_ready" || c.phase === "skipped_to_plan")) {
    // Draft exists but phase lagging — soft claim residual for next-step CTA
    if (!c._rechatOpen) {
      c.phase = "claimed_to_plan";
      c._claimSuccess = true;
    }
  }
  return c;
}

export function ensureClarifyState() {
  if (!state.chatClarify || typeof state.chatClarify !== "object") {
    state.chatClarify = defaultClarifyState();
  } else {
    // Guard soft corruption without resetting user picks
    if (!state.chatClarify.entry) state.chatClarify.entry = DEFAULT_CLARIFY_ENTRY;
    if (!state.chatClarify.phase) state.chatClarify.phase = "not_started";
    if (!Array.isArray(state.chatClarify.slots)) state.chatClarify.slots = [];
    if (!Array.isArray(state.chatClarify.assumptions)) {
      state.chatClarify.assumptions = [];
    }
    if (state.chatClarify.uiStatus == null) state.chatClarify.uiStatus = "idle";
  }
  reconcileClaimedPhase();
  // Mirror onto session object for future t4 persistence
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = clarifyToWire(state.chatClarify);
  }
  return state.chatClarify;
}

export function resetClarifyState(entry) {
  state.chatClarify = defaultClarifyState(entry || DEFAULT_CLARIFY_ENTRY);
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = clarifyToWire(state.chatClarify);
  }
  return state.chatClarify;
}

/** Hydrate from ChatSession.clarify (disk / getSession). */
export function hydrateClarifyFromSession(sess) {
  if (sess && sess.clarify && typeof sess.clarify === "object") {
    const prevUi = state.chatClarify
      ? {
          uiStatus: state.chatClarify.uiStatus,
          errorText: state.chatClarify.errorText,
          questionIndex: state.chatClarify.questionIndex,
          selectedOption: state.chatClarify.selectedOption,
        }
      : null;
    state.chatClarify = normalizeClarifyState(sess.clarify);
    // Prefer draft_plan from the same session object when present
    if (sess.draft_plan && state.chatSession && !state.chatSession.draft_plan) {
      state.chatSession.draft_plan = sess.draft_plan;
    }
    reconcileClaimedPhase();
    // Keep ephemeral UI status across soft reloads when phase unchanged
    if (prevUi && state.chatClarify.phase === normalizePhase(sess.clarify.phase)) {
      if (prevUi.uiStatus && prevUi.uiStatus !== "idle") {
        state.chatClarify.uiStatus = prevUi.uiStatus;
        state.chatClarify.errorText = prevUi.errorText;
      }
      if (prevUi.questionIndex != null) {
        state.chatClarify.questionIndex = prevUi.questionIndex;
      }
      if (prevUi.selectedOption != null) {
        state.chatClarify.selectedOption = prevUi.selectedOption;
      }
    }
  } else if (!state.chatClarify) {
    state.chatClarify = defaultClarifyState();
  }
  return ensureClarifyState();
}

// ─── Pure slot helpers (UI mirror of domain rules; no strategy) ──────────────

export function isSlotFilled(c, slotId) {
  const fill = (c?.slots || []).find((s) => s.id === slotId);
  return !!(fill && String(fill.value || "").trim());
}

export function missingRequiredSlots(c) {
  return CLARIFY_SLOT_QUESTIONS.filter((q) => !isSlotFilled(c, q.id)).map(
    (q) => q.id
  );
}

export function filledCount(c) {
  return CLARIFY_SLOT_QUESTIONS.length - missingRequiredSlots(c).length;
}

export function mayProceedWithAssumptions(c) {
  return !!(c && (c.skip_requested || c.entry === "plan_only"));
}

export function setSlotFillLocal(c, id, value, kind) {
  const v = String(value || "").trim();
  if (!v) return false;
  const k = normalizeFillKind(kind || "explicit");
  const slots = Array.isArray(c.slots) ? c.slots : (c.slots = []);
  const existing = slots.find((s) => s.id === id);
  if (existing) {
    // soft-fill must not silently overwrite Explicit
    if (k === "soft_fill" && existing.kind === "explicit") return false;
    if (existing.value === v && existing.kind === k) return false;
    existing.value = v;
    existing.kind = k;
  } else {
    slots.push({ id, value: v, kind: k });
  }
  if (c.phase === "not_started") c.phase = "clarifying";
  return true;
}

/**
 * Mirror domain apply_skip_with_assumptions (presentation state only).
 * @param {object} c
 * @param {string|null} [userNote]
 */
export function applySkipWithAssumptionsLocal(c, userNote) {
  c.skip_requested = true;
  c.phase = "skipped_to_plan";
  c.uiStatus = "idle";
  c.errorText = null;
  c.selectedOption = null;
  const note = userNote && String(userNote).trim() ? String(userNote).trim() : null;
  const missing = missingRequiredSlots(c);
  if (!Array.isArray(c.assumptions)) c.assumptions = [];
  for (const id of missing) {
    const label =
      CLARIFY_SLOT_QUESTIONS.find((q) => q.id === id)?.label || id;
    const text = note
      ? `假设（用户跳过·${note}）：待写计划时补全「${label}」`
      : `假设（用户跳过）：待写计划时补全「${label}」`;
    setSlotFillLocal(c, id, text, "assumed");
    c.assumptions.push({ slot: id, text });
  }
  if (note) {
    const already = c.assumptions.some(
      (a) => !a.slot && String(a.text || "").includes(note)
    );
    if (!already) {
      c.assumptions.push({ slot: null, text: `用户跳过澄清：${note}` });
    }
  }
}

function currentQuestion(c) {
  const missing = missingRequiredSlots(c);
  if (!missing.length) return null;
  // Prefer questionIndex if still missing; else first missing
  const byIndex = CLARIFY_SLOT_QUESTIONS[c.questionIndex || 0];
  if (byIndex && missing.includes(byIndex.id)) return byIndex;
  const firstId = missing[0];
  const q = CLARIFY_SLOT_QUESTIONS.find((x) => x.id === firstId) || null;
  if (q) {
    c.questionIndex = CLARIFY_SLOT_QUESTIONS.findIndex((x) => x.id === q.id);
  }
  return q;
}

// ─── Brief model (presentation; not PlanIR) ──────────────────────────────────

const SLOT_LABEL = Object.freeze({
  target_audience: "目标对象",
  pain_moment: "痛苦时刻",
  observable_outcome: "可观察结果",
  non_goals: "明确不做",
  done_when: "怎样算做完",
});

function slotValue(c, id) {
  const fill = (c?.slots || []).find((s) => s.id === id);
  const v = fill && String(fill.value || "").trim();
  return v || "";
}

function slotKind(c, id) {
  const fill = (c?.slots || []).find((s) => s.id === id);
  return fill?.kind || null;
}

function isAssumedOrPlaceholder(value, kind) {
  if (kind === "assumed") return true;
  const v = String(value || "");
  return /假设（用户跳过/.test(v) || /^假设[：:]/.test(v);
}

function isEffectivelyMissing(value, kind) {
  const v = String(value || "").trim();
  if (!v) return true;
  if (isAssumedOrPlaceholder(v, kind)) return true;
  // Soft placeholders that are not real acceptance / non-goals
  if (/待写计划时补全|请补充|TBD|TODO|待定/i.test(v)) return true;
  return false;
}

/**
 * Evidence light tags only: 假设 / 用户原话 / 自用痛点 / 竞品缺口.
 * Derived from fill kinds + optional keys — no network.
 */
export function deriveEvidenceTags(c) {
  const tags = new Set();
  const slots = c?.slots || [];
  let hasExplicit = false;
  let hasAssumed = false;
  for (const s of slots) {
    if (!s || !String(s.value || "").trim()) continue;
    if (s.kind === "assumed" || isAssumedOrPlaceholder(s.value, s.kind)) {
      hasAssumed = true;
    } else if (s.kind === "explicit") {
      hasExplicit = true;
    }
  }
  if (hasAssumed || c?.skip_requested) tags.add("假设");
  if (hasExplicit) tags.add("用户原话");
  const optional = c?.optional || [];
  for (const o of optional) {
    const k = String(o?.key || "").toLowerCase();
    const v = String(o?.value || "").trim();
    if (!v) continue;
    if (/竞品|competitor|替代/.test(k) || /竞品|替代/.test(v)) {
      tags.add("竞品缺口");
    }
    if (/自用|自己|pain|痛点/.test(k) || /自用|自己先用/.test(v)) {
      tags.add("自用痛点");
    }
  }
  // Self-use heuristic from audience option text
  const aud = slotValue(c, "target_audience");
  if (/自己|自用|小团队|内部/.test(aud)) tags.add("自用痛点");
  return Array.from(tags);
}

/**
 * Build Brief view-model from clarify state (read-only presentation).
 * Eight fields: 问题 · 给谁 · 做/不做 · 得/失 · 证据 · 未决 · 验收 · V1
 */
export function buildBriefFromClarify(c) {
  const stateC = c || ensureClarifyState();
  const audience = slotValue(stateC, "target_audience");
  const pain = slotValue(stateC, "pain_moment");
  const outcome = slotValue(stateC, "observable_outcome");
  const nonGoals = slotValue(stateC, "non_goals");
  const doneWhen = slotValue(stateC, "done_when");

  // 问题 = pain + outcome one-liner
  let problem = "";
  if (pain && outcome) {
    problem = `在「${pain}」时，希望达到「${outcome}」`;
  } else if (pain) {
    problem = pain;
  } else if (outcome) {
    problem = outcome;
  } else {
    problem = "（待澄清：真问题一句话）";
  }

  // 做 = observable outcome; 不做 = non-goals
  const doText = outcome || "（待写：本版要交付的可观察结果）";
  const dontText = nonGoals || "（待写：明确不做）";

  // 得/失 — light derivation; loss mirrors non-goals cost
  const gain = outcome
    ? `做成后可见：${outcome}`
    : "（待写：做成得什么）";
  const loss = nonGoals
    ? `本版不覆盖：${nonGoals}（范围代价）`
    : "（待写：会失去什么 / 范围代价）";

  // 未决 = remaining assumed / missing notes
  const open = [];
  for (const id of [
    "target_audience",
    "pain_moment",
    "observable_outcome",
    "non_goals",
    "done_when",
  ]) {
    const v = slotValue(stateC, id);
    const k = slotKind(stateC, id);
    if (isEffectivelyMissing(v, k)) {
      open.push(`${SLOT_LABEL[id] || id}仍开放`);
    }
  }
  for (const a of stateC.assumptions || []) {
    if (a && a.text && !a.slot) open.push(String(a.text));
  }
  const openText = open.length ? open.join("；") : "（无）";

  const v1 =
    outcome
      ? `V1 只做到「${outcome}」可演示/可分配；其余进 V2/Later`
      : "V1：先补齐目标 / 不做 / 验收，再写可分配大纲";

  return {
    problem,
    audience: audience || "（待写：给谁）",
    doText,
    dontText,
    gain,
    loss,
    evidence: deriveEvidenceTags(stateC),
    open: openText,
    acceptance: doneWhen || "（待写：怎样算做完）",
    v1,
    entry: stateC.entry || DEFAULT_CLARIFY_ENTRY,
    phase: stateC.phase,
    skip: !!stateC.skip_requested,
  };
}

/**
 * Hollow detection for Brief / plan draft (presentation mirror of D0).
 * Missing / assumed 验收 or 非目标 → warn; never blocks.
 * @returns {{ hollow: boolean, missing: string[], message: string|null }}
 */
export function detectHollowGaps(c, planMd) {
  const missing = [];
  const stateC = c || state.chatClarify;
  const md = String(planMd || state.chatSession?.draft_plan?.markdown || "");

  // Prefer plan body when present
  let hasNonGoalsInPlan = false;
  let hasAcceptanceInPlan = false;
  if (md.trim()) {
    hasNonGoalsInPlan = /##\s*(非目标|不做|明确不做|non[- ]?goals?)/i.test(md);
    hasAcceptanceInPlan =
      /##\s*(验收|成功标准|怎样算做完|acceptance|done[- ]?when)/i.test(md);
    // Section present but stub body
    if (hasAcceptanceInPlan) {
      const body = extractSectionBody(md, /(验收|成功标准|怎样算做完|acceptance)/i);
      if (bodyIsStub(body)) hasAcceptanceInPlan = false;
    }
    if (hasNonGoalsInPlan) {
      const body = extractSectionBody(md, /(非目标|不做|明确不做|non[- ]?goals?)/i);
      if (bodyIsStub(body)) hasNonGoalsInPlan = false;
    }
  }

  const nonGoalsVal = slotValue(stateC, "non_goals");
  const doneVal = slotValue(stateC, "done_when");
  const nonGoalsOk =
    hasNonGoalsInPlan ||
    (!isEffectivelyMissing(nonGoalsVal, slotKind(stateC, "non_goals")) &&
      !!nonGoalsVal);
  const doneOk =
    hasAcceptanceInPlan ||
    (!isEffectivelyMissing(doneVal, slotKind(stateC, "done_when")) && !!doneVal);

  if (!nonGoalsOk) missing.push(CLARIFY_COPY.hollowNonGoals);
  if (!doneOk) missing.push(CLARIFY_COPY.hollowDoneWhen);

  return {
    hollow: missing.length > 0,
    missing,
    message: missing.length ? CLARIFY_COPY.hollowWarn : null,
  };
}

function extractSectionBody(md, headingRe) {
  const lines = String(md || "").replace(/\r\n/g, "\n").split("\n");
  let i = 0;
  let found = -1;
  for (; i < lines.length; i++) {
    const m = lines[i].match(/^##\s+(.+?)\s*$/);
    if (m && headingRe.test(m[1])) {
      found = i;
      break;
    }
  }
  if (found < 0) return "";
  const body = [];
  for (let j = found + 1; j < lines.length; j++) {
    if (/^##\s+/.test(lines[j])) break;
    body.push(lines[j]);
  }
  return body.join("\n").trim();
}

function bodyIsStub(body) {
  const t = String(body || "").trim();
  if (!t) return true;
  // Only empty checkboxes / placeholders / claim-time hollow fillers
  const lines = t
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  if (!lines.length) return true;
  // Drop meta lines written by claim when slots were empty (not real acceptance)
  const contentLines = lines.filter(
    (l) => !/^[-*+]\s*说明[：:]/.test(l) && !/^说明[：:]/.test(l)
  );
  if (!contentLines.length) return true;
  const stubLine = (l) => {
    // Strip list marker + optional checkbox
    const core = l
      .replace(/^[-*+]\s*/, "")
      .replace(/^\[[\sxX ]\]\s*/i, "")
      .trim();
    if (!core) return true;
    // Claim-time placeholders: （待补）… / (待写)… / 待补全…
    if (
      /^(（待|^\(待|待补|待写|待定|请补充|TBD|TODO|…|\.{2,})/i.test(core) ||
      /（待补|（待写|（待定|\(待补|\(待写/.test(core) ||
      /^假设（用户跳过/.test(core) ||
      /待写计划时补全/.test(core)
    ) {
      return true;
    }
    // Pure punctuation / ellipsis
    if (/^[.。…\-\s]*$/.test(core)) return true;
    return false;
  };
  // Hollow if every content line is stub OR any line still carries 待补 filler
  // (mixed real+stub still counts as present only when at least one non-stub line)
  const nonStub = contentLines.filter((l) => !stubLine(l));
  return nonStub.length === 0;
}

/**
 * Compose plan markdown with habitual chapters from Brief / slots.
 * Always includes: 目标 / 非目标 / 会失去什么 / 验收 / 风险 / V1 边界 /
 * 任务大纲(V1) / V2·Later(folded).
 *
 * plan-only / skip still force min chapters (目标 / 不做 / 验收).
 */
export function buildPlanMarkdownFromBrief(c, brief) {
  const stateC = c || ensureClarifyState();
  const b = brief || buildBriefFromClarify(stateC);
  const audience = slotValue(stateC, "target_audience") || b.audience;
  const pain = slotValue(stateC, "pain_moment");
  const outcome = slotValue(stateC, "observable_outcome") || b.doText;
  const nonGoals = slotValue(stateC, "non_goals") || b.dontText;
  const doneWhen = slotValue(stateC, "done_when") || b.acceptance;
  const entryLabel =
    CLARIFY_ENTRIES.find((e) => e.id === (stateC.entry || DEFAULT_CLARIFY_ENTRY))
      ?.label || "从想法到计划";

  const titleSeed =
    outcome && !isAssumedOrPlaceholder(outcome, slotKind(stateC, "observable_outcome"))
      ? String(outcome).replace(/[。．.]+$/g, "").slice(0, 40)
      : pain && !isAssumedOrPlaceholder(pain, slotKind(stateC, "pain_moment"))
        ? String(pain).replace(/[。．.]+$/g, "").slice(0, 40)
        : "澄清稿认领计划";
  const title = sanitizePlanTitle(titleSeed) || "澄清稿认领计划";

  const assumptionLines = (stateC.assumptions || [])
    .map((a) => String(a?.text || "").trim())
    .filter(Boolean);
  // Also mark assumed slots in 风险
  for (const s of stateC.slots || []) {
    if (s.kind === "assumed" && s.value) {
      const line = `${SLOT_LABEL[s.id] || s.id}：${s.value}`;
      if (!assumptionLines.includes(line)) assumptionLines.push(line);
    }
  }

  const hollow = detectHollowGaps(stateC, "");
  const nonGoalsBody = isEffectivelyMissing(
    nonGoals,
    slotKind(stateC, "non_goals")
  )
    ? "- （待补）明确不做 / 非目标\n- 说明：黄条已提醒，不拦保存与分配"
    : `- ${nonGoals}`;
  const acceptanceBody = isEffectivelyMissing(
    doneWhen,
    slotKind(stateC, "done_when")
  )
    ? "- [ ] （待补）怎样算做完\n- 说明：黄条已提醒，不拦保存与分配"
    : `- [ ] ${doneWhen}`;

  const lossBody = isEffectivelyMissing(nonGoals, slotKind(stateC, "non_goals"))
    ? "- （待补）因砍范围会失去什么"
    : `- 本版不覆盖：${nonGoals}`;

  const riskLines =
    assumptionLines.length > 0
      ? assumptionLines.map((t) => `- 假设：${t}`).join("\n")
      : hollow.hollow
        ? `- ${hollow.message}`
        : "- （暂无硬风险；有未决请补进本节）";

  const v1Body = b.v1 || "本版只做可演示主路径与可分配大纲";

  // V1 task outline default; keep short & assignable
  const tasks = [
    "整理澄清要点为可扫读范围（目标 / 不做 / 验收）",
    "补齐计划正文习惯章节并核对人话",
    "保存计划草稿并进入「分配计划」",
  ];
  if (stateC.entry === "plan_only") {
    tasks[0] = "按已想清范围写出最小章节（目标 / 不做 / 验收）";
  }

  const taskOutline = tasks
    .map((t, i) => `### T${i + 1} · ${t}\n- 说明：澄清相认领生成的 V1 步骤\n- 验收：可勾选完成`)
    .join("\n\n");

  const md = `# ${title}

## 目标
- 给谁：${audience || "（待写）"}
- 场景：${pain || "（待写：痛苦时刻 / 触发场景）"}
- 可观察结果：${outcome || "（待写）"}
- 入口：${entryLabel}

## 非目标 / 不做
${nonGoalsBody}

## 会失去什么
${lossBody}

## 验收
${acceptanceBody}

## 风险 / 待确认
${riskLines}

## V1 边界
- ${v1Body}

## 任务大纲
${taskOutline}

## V2 / Later
- （折叠）未进 V1 的愿望与扩展能力，本版不展开执行。

## 结构对齐
- 配方：R-tool · 深度 D · app-shell
- 澄清相主 CTA：认领并写成计划；后半段：分配计划
- 文件策略：Brief 仅 session 展示；认领写入 plan 草稿（无独立 brief 文件）
`;
  return md.replace(/\n{3,}/g, "\n\n").trim() + "\n";
}

/**
 * Whether Brief panel should show (slots ready or skip/plan-only path).
 */
export function shouldShowBrief(c) {
  const stateC = c || ensureClarifyState();
  if (stateC.phase === "claimed_to_plan") return false;
  if (stateC.phase === "brief_ready") return true;
  if (stateC.phase === "skipped_to_plan") return true;
  if (stateC.entry === "plan_only" && stateC.skip_requested) return true;
  // All five filled while still clarifying → treat as brief-ready
  if (
    (stateC.phase === "clarifying" || stateC.phase === "not_started") &&
    missingRequiredSlots(stateC).length === 0
  ) {
    return true;
  }
  return false;
}

// ─── Actions ─────────────────────────────────────────────────────────────────

/** @type {null | (() => void)} */
let _clarifyPaint = null;

/**
 * installChatHost registers the real renderChatMessages here so skip/pick
 * never depend solely on host-bag spread order (empty bag → toast-only bug).
 * @param {(() => void) | null} fn
 */
export function setClarifyPaint(fn) {
  _clarifyPaint = typeof fn === "function" ? fn : null;
}

function repaint() {
  // 澄清/认领等交互型 repaint：必须真正重绘（指纹空转会吞掉视觉反馈）。
  const forceMsgs = { force: true };
  const tryCall = (fn) => {
    if (typeof fn !== "function") return false;
    try {
      fn(forceMsgs);
      return true;
    } catch (err) {
      console.warn("clarify repaint step failed", err);
      return false;
    }
  };
  if (tryCall(_clarifyPaint)) return;
  if (tryCall(host.renderChatMessages)) return;
  if (tryCall(host.renderChatPage)) return;
  try {
    const desk =
      typeof window !== "undefined" ? window.ccoChat || null : null;
    if (desk && tryCall(desk.renderChatMessages?.bind(desk))) return;
    if (desk && tryCall(desk.renderChatPage?.bind(desk))) return;
  } catch (_) {}
  // Last resort: classic globals (try both)
  try {
    if (typeof window !== "undefined" && typeof window.renderChatMessages === "function") {
      window.renderChatMessages();
      return;
    }
  } catch (_) {}
  try {
    if (typeof window !== "undefined" && typeof window.renderChatPage === "function") {
      window.renderChatPage();
    }
  } catch (_) {}
}

export function selectClarifyEntry(entryId) {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;
  const next = normalizeEntry(entryId);
  // Re-tap same grill entry while already clarifying → still repaint so UI feels alive
  if (
    c.entry === next &&
    next !== "plan_only" &&
    (c.phase === "clarifying" || c.phase === "brief_ready")
  ) {
    c._touchAt = Date.now();
    repaint();
    return;
  }
  c.entry = next;
  c.uiStatus = "idle";
  c.errorText = null;
  c.selectedOption = null;
  c._touchAt = Date.now();

  if (next === "plan_only") {
    // Escape hatch: skip grilling → same one-click draft as skip CTA.
    applySkipWithAssumptionsLocal(c, "直接写计划");
    mirrorClarifyToSession(c);
    try {
      if (typeof stashChatSession === "function") stashChatSession(state.selectedPath);
    } catch (_) {}
    repaint();
    void claimBriefToPlan().catch((err) => {
      console.warn("plan_only auto-claim failed", err);
      toast("已选直接写计划，请点「写成计划」");
      repaint();
    });
    return;
  }

  // Grill paths: enter clarifying. Returning from skip drops assumed placeholders.
  if (c.phase === "skipped_to_plan") {
    c.skip_requested = false;
    c.slots = (c.slots || []).filter((s) => s.kind === "explicit");
    c.assumptions = [];
  }
  if (c.phase !== "claimed_to_plan") {
    c.phase = "clarifying";
    c.skip_requested = false;
  }

  mirrorClarifyToSession(c);
  try {
    if (typeof stashChatSession === "function") stashChatSession(state.selectedPath);
  } catch (_) {}
  repaint();
}

/**
 * @param {string} optionKey A|B|C
 * @param {string} [slotId] data-clarify-slot from the clicked button (preferred)
 */
export function pickClarifyOption(optionKey, slotId) {
  try {
    ensureChatState();
    ensureClarifyState();
    const c = state.chatClarify;
    if (!c || typeof c !== "object") {
      toast("澄清状态未就绪，请再点一次");
      return;
    }
    // Empty-state card may still show while phase is not_started — promote first.
    if (c.phase === "not_started") c.phase = "clarifying";
    // Allow picks during brief_ready (re-answer) / clarifying / not_started.
    // Only hard-block after claim; skip path uses skip buttons.
    if (c.phase === "claimed_to_plan") {
      toast("计划草稿已写好。可点「拆成步骤」，或「再改一改」");
      return;
    }
    if (c.phase === "skipped_to_plan") {
      // User is answering after skip → drop assumed placeholders for re-fill
      c.skip_requested = false;
      c.slots = (c.slots || []).filter((s) => s.kind === "explicit");
      c.assumptions = [];
      c.phase = "clarifying";
    }
    const key = String(optionKey || "").trim().toUpperCase();
    if (!key) return;

    // Prefer the slot stamped on the button — avoids stale questionIndex races.
    let q = null;
    const preferSlot = slotId && String(slotId).trim();
    if (preferSlot) {
      q = CLARIFY_SLOT_QUESTIONS.find((x) => x.id === preferSlot) || null;
      if (q) {
        c.questionIndex = Math.max(
          0,
          CLARIFY_SLOT_QUESTIONS.findIndex((x) => x.id === q.id)
        );
      }
    }
    if (!q) q = currentQuestion(c);
    if (!q) {
      // All filled → brief-ready marker (t4 will show Brief)
      c.phase = "brief_ready";
      c.selectedOption = null;
      c._touchAt = Date.now();
      mirrorClarifyToSession(c);
      try {
        if (typeof stashChatSession === "function") {
          stashChatSession(state.selectedPath);
        }
      } catch (_) {}
      repaint();
      return;
    }

    // Prefer option from the clicked question (data-clarify-slot), not only current index.
    // Fallback: match key against current question options.
    let opt = (q.options || []).find(
      (o) => String(o.key || "").toUpperCase() === key
    );
    // If key not on current q (stale DOM), search any missing question
    if (!opt) {
      for (const cand of CLARIFY_SLOT_QUESTIONS) {
        if (isSlotFilled(c, cand.id)) continue;
        const hit = (cand.options || []).find(
          (o) => String(o.key || "").toUpperCase() === key
        );
        if (hit) {
          q = cand;
          opt = hit;
          break;
        }
      }
    }
    if (!opt) {
      console.warn("pickClarifyOption: unknown option", optionKey, q?.id);
      toast("这个选项没对上，请再点一次");
      return;
    }

    c.selectedOption = key;
    setSlotFillLocal(c, q.id, opt.text, "explicit");
    c.uiStatus = "idle";
    c.errorText = null;
    c._touchAt = Date.now();
    // Advance to next missing
    const missing = missingRequiredSlots(c);
    if (!missing.length) {
      c.phase = "brief_ready";
      c.selectedOption = null;
      toast("要点齐了，先看一页摘要");
    } else {
      c.phase = "clarifying";
      const nextId = missing[0];
      c.questionIndex = Math.max(
        0,
        CLARIFY_SLOT_QUESTIONS.findIndex((x) => x.id === nextId)
      );
      c.selectedOption = null;
    }
    mirrorClarifyToSession(c);
    try {
      if (typeof stashChatSession === "function") {
        stashChatSession(state.selectedPath);
      }
    } catch (_) {}
    repaint();
  } catch (err) {
    console.error("pickClarifyOption failed", err);
    try {
      toast("点选没生效，请再点一次");
    } catch (_) {}
  }
}

/** Keep session.clarify + in-memory cache in sync so loadChatSession cannot wipe picks. */
function mirrorClarifyToSession(c) {
  const wire = clarifyToWire(c);
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = wire;
  }
  return wire;
}

/**
 * User skip:「跳过，先出草稿 / 其余你帮我选」
 * Product CTA promises a draft in one click — fill assumptions then claim.
 * (Stopping at Brief alone felt like toast-only / no reaction.)
 * @param {string} [note]
 * @returns {Promise<void>}
 */
export async function skipClarify(note) {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;
  if (c.phase === "claimed_to_plan") {
    toast("计划草稿已写好。可点「拆成步骤」，或「再改一改」");
    return;
  }
  if (c._claimBusy) return;
  const n = note && String(note).trim() ? String(note).trim() : CLARIFY_COPY.skipCta;
  applySkipWithAssumptionsLocal(c, n);
  c._touchAt = Date.now();
  mirrorClarifyToSession(c);
  try {
    if (typeof stashChatSession === "function") stashChatSession(state.selectedPath);
  } catch (_) {}
  // Paint Brief/status immediately so UI never freezes on toast alone.
  repaint();
  try {
    const result = await claimBriefToPlan();
    // claimBriefToPlan already toasts success + injects plan card.
    if (result && result.ok === false && result.error === "not_ready") {
      toast("已按常见假设整理好，请点「写成计划」");
      repaint();
    }
  } catch (err) {
    console.warn("skipClarify auto-claim failed", err);
    toast("已按常见假设整理好，请点「写成计划」");
    repaint();
  }
}

/** Set UI status for loading / error simulation & real send wiring. */
export function setClarifyUiStatus(status, errorText) {
  ensureClarifyState();
  const c = state.chatClarify;
  const s = String(status || "idle");
  c.uiStatus = ["idle", "loading", "error", "empty"].includes(s) ? s : "idle";
  c.errorText = errorText != null ? String(errorText) : null;
  if (state.chatSession) state.chatSession.clarify = clarifyToWire(c);
}

export function clearClarifyError() {
  ensureClarifyState();
  state.chatClarify.uiStatus = "idle";
  state.chatClarify.errorText = null;
  repaint();
}

/**
 * Soft re-open Brief for edits via chat (read-only Brief + rechat).
 * Does not wipe fills; just returns to brief_ready / clarifying.
 */
export function rechatFromBrief() {
  ensureClarifyState();
  const c = state.chatClarify;
  if (c.phase === "claimed_to_plan") {
    // Allow re-open after claim without undoing draft
    c.phase = "brief_ready";
    c._rechatOpen = true; // block soft re-claim promote until next claim
  } else if (c.phase === "brief_ready" || c.phase === "skipped_to_plan") {
    // Focus composer for "继续聊"
    c._rechatOpen = true;
  } else {
    c.phase = missingRequiredSlots(c).length ? "clarifying" : "brief_ready";
  }
  c.uiStatus = "idle";
  c.errorText = null;
  if (state.chatSession) state.chatSession.clarify = clarifyToWire(c);
  repaint();
  const input = $("#chat-input");
  if (input) {
    input.focus();
    toast("可继续补充要点；改完后再次认领");
  }
}

/**
 * After reload / disk hydrate: if session.draft_plan has markdown but transcript
 * has no ```plan fence, surface a synthetic assistant card so assign CTAs stay reachable.
 *
 * Soft-promotes phase → claimed_to_plan when body looks like a claim draft
 * (session.clarify is not rewritten by save_plan — no new HTTP).
 *
 * Does **not** call confirm_start / spawn.
 * @returns {boolean} true when a message was injected
 */
export function ensureClaimDraftMessageVisible() {
  ensureChatState();
  ensureClarifyState();
  const sess = state.chatSession;
  if (!sess) return false;
  const md = String(sess.draft_plan?.markdown || "").trim();
  if (!md) return false;

  const msgs = Array.isArray(sess.messages)
    ? sess.messages
    : (sess.messages = []);
  const hasPlanFence = msgs.some(
    (m) => m && /```plan\b/i.test(String(m.content || ""))
  );

  // Soft-promote claim phase from draft body (reload path).
  // Accept new human plan templates + legacy claim markers.
  const looksClaimed =
    (/##\s*目标/.test(md) &&
      (/##\s*(非目标|不做)/.test(md) || /##\s*验收/.test(md))) ||
    /澄清相主 CTA|认领并写成计划|##\s*V1 边界|##\s*会失去什么/.test(md);
  const c = state.chatClarify;
  if (
    looksClaimed &&
    c &&
    c.phase !== "claimed_to_plan" &&
    // Don't clobber an open Brief re-edit (user tapped 继续聊)
    !(c.phase === "brief_ready" && c._rechatOpen)
  ) {
    c.phase = "claimed_to_plan";
    c._claimSuccess = true;
  }
  reconcileClaimedPhase();

  // Always surface plan card when draft exists and transcript has no fence
  // (claimed residual / reload / empty-message bug).
  if (hasPlanFence) return false;

  msgs.push({
    role: "assistant",
    content: `${CLARIFY_COPY.success}\n\n\`\`\`plan\n${md}\n\`\`\``,
  });
  return true;
}

function claimPlanPreviewText(md) {
  const raw = String(md || "").trim();
  if (!raw) return "";
  // Compact preview: title + first few non-empty lines
  const lines = raw
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((l) => l.trimEnd())
    .filter((l) => l.trim());
  const keep = [];
  for (const l of lines) {
    if (keep.length >= 8) break;
    // skip pure fences
    if (/^```/.test(l)) continue;
    keep.push(l);
  }
  let text = keep.join("\n");
  if (text.length > 420) text = text.slice(0, 419) + "…";
  return text;
}

/**
 * Claim Brief → write plan draft (session + optional disk via save_plan).
 *
 * Hard rules:
 * - Writes draft_plan / save_plan only
 * - NEVER calls confirm_start / start_run / spawn worker
 * - Hollow gaps → yellow bar only; still saves
 *
 * @returns {Promise<{ok:boolean, plan_rel?:string, markdown?:string, error?:string}|null>}
 */
export async function claimBriefToPlan() {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;

  if (c._claimBusy) return null;
  if (!state.selectedPath) {
    toast("请先选择项目");
    return { ok: false, error: "no_project" };
  }

  // Promote to brief_ready if slots complete but phase lagging
  if (
    c.phase !== "brief_ready" &&
    c.phase !== "skipped_to_plan" &&
    missingRequiredSlots(c).length === 0
  ) {
    c.phase = "brief_ready";
  }
  if (
    c.phase !== "brief_ready" &&
    c.phase !== "skipped_to_plan" &&
    !(c.entry === "plan_only" && c.skip_requested)
  ) {
    toast("请先答完问题，或点「跳过，先出草稿」");
    return { ok: false, error: "not_ready" };
  }

  c._claimBusy = true;
  c.uiStatus = "loading";
  repaint();

  try {
    const brief = buildBriefFromClarify(c);
    const md = buildPlanMarkdownFromBrief(c, brief);
    const title =
      sanitizePlanTitle(
        (md.match(/^#\s+(.+)$/m) || [])[1] || "澄清稿认领计划"
      ) || "澄清稿认领计划";

    // 1) Session draft first (always) — source of truth for plan rail / card
    if (!state.chatSession) {
      state.chatSession = {
        session_id: "default",
        messages: [],
        draft_plan: null,
      };
    }
    state.chatSession.draft_plan = {
      path: "",
      saved: false,
      markdown: md,
      title,
    };
    // Unsaved fence must not keep stale plan path
    state.chatDraftPlan = null;

    // 2) Best-effort disk save via existing chat_save_plan (no new HTTP)
    let planRel = null;
    try {
      const plansDir = getPlansDir();
      const resp = await chatApi.savePlan({
        project: state.selectedPath,
        markdown: md,
        sessionId: state.chatSession.session_id || "default",
        title,
        planRel: null,
        plansDir: plansDir || "plans",
      });
      planRel = resp?.plan_rel || null;
      if (planRel) {
        state.chatDraftPlan = planRel;
        state.selectedPlan = planRel;
        state.chatSession.draft_plan.path = planRel;
        state.chatSession.draft_plan.saved = true;
        state.chatSession.draft_plan.markdown = md;
      }
    } catch (e) {
      // Disk save failed — keep session draft; user can save via plan card later
      console.warn("claimBriefToPlan: save_plan failed; session draft kept", e);
      toast("计划草稿已在会话中；稍后可在计划卡片点「这版作数」保存到本机");
    }

    // 3) Phase → claimed (still NOT confirm_start)
    c.phase = "claimed_to_plan";
    c.uiStatus = "idle";
    c.errorText = null;
    c._claimSuccess = true;
    c._claimAt = Date.now();
    c._rechatOpen = false;
    if (state.chatSession) {
      state.chatSession.clarify = clarifyToWire(c);
    }

    // Inject a short system note so transcript shows claim (human first sentence)
    try {
      const note = CLARIFY_COPY.success;
      const msgs = state.chatSession.messages || (state.chatSession.messages = []);
      const planFence = "```plan\n" + md.trim() + "\n```";
      const newContent = `${note}\n\n${planFence}\n\n${CLARIFY_COPY.claimGuide}`;
      // Replace last existing plan fence to avoid duplicates (reload / re-claim)
      let existIdx = -1;
      for (let i = msgs.length - 1; i >= 0; i--) {
        if (msgs[i] && /```plan\b/i.test(String(msgs[i].content || ""))) {
          existIdx = i;
          break;
        }
      }
      if (existIdx >= 0) {
        msgs[existIdx] = { ...msgs[existIdx], content: newContent };
      } else {
        msgs.push({ role: "assistant", content: newContent });
      }
    } catch (_) {}

    stashChatSession(state.selectedPath);
    toast(CLARIFY_COPY.success);
    repaint();
    // Refresh plan lists best-effort (rail / management)
    try {
      if (typeof host.loadPlanRail === "function") await host.loadPlanRail();
    } catch (_) {}

    return { ok: true, plan_rel: planRel || undefined, markdown: md };
  } catch (e) {
    c.uiStatus = "error";
    c.errorText = String(e?.message || e);
    toast(String(e?.message || e));
    repaint();
    return { ok: false, error: String(e?.message || e) };
  } finally {
    c._claimBusy = false;
  }
}

// ─── Render ──────────────────────────────────────────────────────────────────

/**
 * F1 最小 R4：旧三入口主按钮行已撤；委托 chatMode 逃生舱 linkish（直接写计划）。
 * 主路径 2 chip 在 composer 上方（chatMode.paintChatMode），禁止与本行双轨并排。
 */
function renderEntryChips(c, { disabled } = {}) {
  if (!c || c.phase === "claimed_to_plan") return "";
  try {
    const api =
      (typeof window !== "undefined" && window.ccoChat) || null;
    if (api && typeof api.renderClarifySecondaryHtml === "function") {
      return api.renderClarifySecondaryHtml(c, { disabled }) || "";
    }
  } catch (_) {}
  // Fallback before desk mount: single escape link (still not 3 main buttons)
  if (c.entry === "plan_only" && c.skip_requested) return "";
  const dis = disabled ? " disabled" : "";
  return (
    `<div class="chat-clarify-moreways" data-clarify-moreways="1">` +
    `<button type="button" class="linkish muted"` +
    ` data-clarify-entry="plan_only"` +
    ` title="跳过追问，立刻出一版草稿"${dis}>直接写计划</button>` +
    `</div>`
  );
}

function renderGuideBlock(c, mode) {
  // One human sentence for orientation — only when still choosing / first question.
  if (c.phase === "claimed_to_plan") return "";
  if (c.phase === "brief_ready" || c.phase === "skipped_to_plan") return "";
  if (mode !== "empty" && c.phase !== "not_started" && c.phase !== "clarifying") {
    return "";
  }
  return (
    `<div class="chat-clarify-guide">` +
    `<p class="chat-clarify-guide-title">${chatEsc(CLARIFY_COPY.guideTitle)}</p>` +
    `<p class="chat-clarify-guide-hint">${chatEsc(CLARIFY_COPY.guideHint)}</p>` +
    `</div>`
  );
}

function renderStatusBlock(c) {
  // Loading (send or claim)
  if (
    c.uiStatus === "loading" ||
    c._claimBusy ||
    (state.chatBusy && isGrillPath(c) && !shouldShowBrief(c))
  ) {
    const ico = icon("refresh", 14);
    const text = c._claimBusy ? CLARIFY_COPY.claimBusy : CLARIFY_COPY.loading;
    return (
      `<div class="chat-clarify-status is-loading" role="status">` +
      (ico ? `<span class="chat-clarify-ico">${ico}</span>` : "") +
      `${chatEsc(text)}` +
      `</div>`
    );
  }
  // Error
  if (c.uiStatus === "error") {
    const msg = CLARIFY_COPY.error;
    return (
      `<div class="chat-clarify-status is-error" role="alert">` +
      `<div>${chatEsc(msg)}</div>` +
      `<div class="chat-clarify-status-actions">` +
      `<button type="button" class="btn ghost sm" data-clarify-skip="跳过，先出草稿">${chatEsc(
        CLARIFY_COPY.skipCta
      )}</button>` +
      `<button type="button" class="linkish muted" data-clarify-retry="1">换个说法</button>` +
      `</div></div>`
    );
  }
  // Claimed success banner — primary next step is 拆成步骤, not re-coach
  if (c.phase === "claimed_to_plan" && c._claimSuccess !== false) {
    const md = String(state.chatSession?.draft_plan?.markdown || "").trim();
    const preview = claimPlanPreviewText(md);
    const hollow = renderHollowBarHtml(c, md);
    const softHollow = hollow
      ? hollow.replace(
          'class="chat-hollow-bar"',
          'class="chat-hollow-bar is-soft"'
        )
      : "";
    return (
      `<div class="chat-claim-success" role="status" data-clarify-success="1">` +
      `<p class="chat-claim-success-title">${chatEsc(CLARIFY_COPY.success)}</p>` +
      `<p class="chat-claim-success-next">${chatEsc(CLARIFY_COPY.successNext)}</p>` +
      (preview
        ? `<div class="chat-claim-preview" aria-label="计划预览">` +
          `<p class="chat-claim-preview-label">草稿预览</p>` +
          `${chatEsc(preview)}` +
          `</div>`
        : "") +
      `<div class="chat-claim-success-actions">` +
      `<button type="button" class="btn primary sm" data-clarify-assign="1" title="进入拆分台，不会在聊天里直接开跑">${chatEsc(
        CLARIFY_COPY.assignCta
      )}</button>` +
      `<button type="button" class="linkish muted" data-clarify-rechat="1">${chatEsc(
        CLARIFY_COPY.rechat
      )}</button>` +
      `</div>` +
      softHollow +
      `</div>`
    );
  }
  // Skipped / plan-only — short line above Brief (Brief itself carries CTA)
  if (
    !shouldShowBrief(c) &&
    (c.phase === "skipped_to_plan" ||
      (c.entry === "plan_only" && c.skip_requested))
  ) {
    const text =
      c.entry === "plan_only"
        ? CLARIFY_COPY.readyPlanOnly
        : CLARIFY_COPY.readySkip;
    return (
      `<div class="chat-clarify-status is-ready" role="status">` +
      `${chatEsc(text)}` +
      `</div>`
    );
  }
  return "";
}

/** Yellow hollow bar — never disables buttons. */
export function renderHollowBarHtml(c, planMd) {
  const report = detectHollowGaps(c, planMd);
  if (!report.hollow) return "";
  const items = (report.missing || [])
    .map((m) => `<li>${chatEsc(m)}</li>`)
    .join("");
  return (
    `<div class="chat-hollow-bar" role="status" data-hollow-warn="1">` +
    `<div>${chatEsc(report.message || CLARIFY_COPY.hollowWarn)}</div>` +
    (items ? `<ul>${items}</ul>` : "") +
    `</div>`
  );
}

function renderBriefPanel(c) {
  if (!shouldShowBrief(c)) return "";
  // Auto-promote phase so wire stays consistent
  if (
    c.phase !== "brief_ready" &&
    c.phase !== "skipped_to_plan" &&
    missingRequiredSlots(c).length === 0
  ) {
    c.phase = "brief_ready";
  }
  const brief = buildBriefFromClarify(c);
  const tags = (brief.evidence || [])
    .map((t) => {
      const cls =
        t === "假设" ? " is-assumed" : t === "用户原话" ? " is-user" : "";
      return `<span class="chat-brief-tag${cls}">${chatEsc(t)}</span>`;
    })
    .join("");

  // Human group labels — no V1 / 证据轻标签 jargon on the face.
  const groups = [
    ["要解决什么", brief.problem],
    ["给谁用", brief.audience],
    ["做 / 先不做", `做：${brief.doText}\n先不做：${brief.dontText}`],
    ["你会得到 / 可能失去", `得到：${brief.gain}\n可能失去：${brief.loss}`],
    ["来源", tags || "（暂无）"],
    ["还没定", brief.open],
    ["怎样算做完", brief.acceptance],
    ["这版先做到", brief.v1],
  ];

  const groupsHtml = groups
    .map(([label, body]) => {
      const isHtml = label === "来源";
      return (
        `<div class="chat-brief-group">` +
        `<p class="chat-brief-group-label">${chatEsc(label)}</p>` +
        (isHtml
          ? `<div class="chat-brief-group-body">${body}</div>`
          : `<p class="chat-brief-group-body">${chatEsc(body)}</p>`) +
        `</div>`
      );
    })
    .join("");

  const hollow = renderHollowBarHtml(c, "");
  const busy = !!(c._claimBusy || state.chatBusy);
  // think_first may claim too (writes plan draft; user can still stop before assign)
  const ctaLabel = CLARIFY_COPY.claimCta;

  return (
    `<div class="chat-brief" role="region" aria-label="一页摘要" data-brief="1">` +
    `<div class="chat-brief-head">` +
    `<p class="chat-brief-title">${chatEsc(CLARIFY_COPY.briefTitle)}</p>` +
    `<p class="chat-brief-hint">${chatEsc(CLARIFY_COPY.briefHint)}</p>` +
    `</div>` +
    `<div class="chat-brief-groups">${groupsHtml}</div>` +
    hollow +
    `<div class="chat-brief-actions">` +
    `<button type="button" class="btn primary sm" data-clarify-claim="1"` +
    (busy ? " disabled" : "") +
    ` title="${chatEsc(CLARIFY_COPY.claimTitle)}">${chatEsc(ctaLabel)}</button>` +
    `<button type="button" class="linkish muted" data-clarify-rechat="1">${chatEsc(
      CLARIFY_COPY.rechat
    )}</button>` +
    `</div></div>`
  );
}

function isGrillPath(c) {
  return c.entry === "idea_to_plan" || c.entry === "think_first";
}

function shouldShowCard(c) {
  if (c.uiStatus === "loading" || c.uiStatus === "error") return false;
  if (c.phase === "skipped_to_plan" || c.phase === "claimed_to_plan") return false;
  if (c.phase === "brief_ready") return false;
  if (c.entry === "plan_only") return false;
  // idea_to_plan / think_first while not finished
  return isGrillPath(c) && (c.phase === "clarifying" || c.phase === "not_started");
}

function renderClarifyCard(c) {
  if (!shouldShowCard(c)) return "";
  // Auto-enter clarifying when card is shown after entry pick
  if (c.phase === "not_started") {
    // Keep not_started until user engages (option/skip) OR entry was explicitly chosen
    // Show card once entry is grill path — product wants card after selecting idea_to_plan
  }
  const q = currentQuestion(c);
  if (!q) {
    // Nothing missing — treat as ready
    return "";
  }
  const done = filledCount(c);
  const total = CLARIFY_SLOT_QUESTIONS.length;
  const pct = Math.round((done / total) * 100);
  const selected = c.selectedOption;

  const opts = (q.options || [])
    .map((o) => {
      const sel = selected === o.key ? " is-selected" : "";
      // Wrap full label in <span> so clicks never land on bare Text nodes
      // (Text nodes have no .closest — that made options look dead).
      return (
        `<button type="button" class="chat-clarify-option${sel}"` +
        ` data-clarify-option="${chatEsc(o.key)}"` +
        ` data-clarify-slot="${chatEsc(q.id)}">` +
        `<span class="opt-key">${chatEsc(o.key)}</span>` +
        `<span class="opt-text">${chatEsc(o.text)}</span>` +
        `</button>`
      );
    })
    .join("");

  const step = Math.min(done + 1, total);
  const dots = Array.from({ length: total }, (_, i) => {
    const n = i + 1;
    const cls =
      n < step ? " is-done" : n === step ? " is-current" : "";
    return `<i class="${cls.trim()}"></i>`;
  }).join("");

  return (
    `<div class="chat-clarify-card" role="region" aria-label="帮你问清楚">` +
    `<div class="chat-clarify-card-head">` +
    `<span class="chat-clarify-progress">` +
    `${chatEsc(CLARIFY_COPY.progressLabel)} ${step}/${total}` +
    `<span class="chat-clarify-progress-dots" aria-hidden="true">${dots}</span>` +
    `</span>` +
    `</div>` +
    `<span class="chat-clarify-progress-bar" aria-hidden="true"><i style="width:${pct}%"></i></span>` +
    `<p class="chat-clarify-question">${chatEsc(q.question)}</p>` +
    `<div class="chat-clarify-options" role="group" aria-label="可选答案">${opts}</div>` +
    `<div class="chat-clarify-actions">` +
    `<button type="button" class="btn ghost sm" data-clarify-skip="跳过，先出草稿" title="${chatEsc(
      CLARIFY_COPY.skipHint
    )}">${chatEsc(CLARIFY_COPY.skipCta)}</button>` +
    `<button type="button" class="linkish muted" data-clarify-skip="其余你帮我选" title="${chatEsc(
      CLARIFY_COPY.skipHint
    )}">${chatEsc(CLARIFY_COPY.skipAlt)}</button>` +
    `<p class="chat-clarify-skip-hint">${chatEsc(CLARIFY_COPY.skipHint)}</p>` +
    `</div></div>`
  );
}

/**
 * Full clarify panel HTML (entries + empty/status/card + Brief + hollow).
 * @param {{ mode?: "empty"|"inline" }} [opts]
 */
export function renderClarifyPanelHtml(opts = {}) {
  ensureClarifyStyles();
  ensureClarifyState();
  const c = state.chatClarify;
  const mode = opts.mode || "inline";
  const busy = !!state.chatBusy || !!c._claimBusy;

  // Orientation: one guide + soft empty line (no methodology dump)
  const guide = renderGuideBlock(c, mode);
  let emptyLine = "";
  if (mode === "empty" && c.phase === "not_started" && c.uiStatus !== "error") {
    emptyLine =
      `<p class="chat-clarify-empty-line" data-clarify-copy="empty">` +
      `${chatEsc(CLARIFY_COPY.empty)}` +
      `</p>`;
  }

  // Hide entry chips once claimed (success banner carries next step)
  // While grilling, keep chips but default is visually primary.
  const entries =
    c.phase === "claimed_to_plan"
      ? ""
      : renderEntryChips(c, { disabled: busy });
  const status = renderStatusBlock(c);
  // Force card visible on empty for grill default (not when Brief is ready)
  let card = renderClarifyCard(c);
  if (
    !card &&
    mode === "empty" &&
    isGrillPath(c) &&
    (c.phase === "not_started" || c.phase === "clarifying") &&
    c.uiStatus === "idle" &&
    !busy &&
    !shouldShowBrief(c)
  ) {
    // Temporary phase bump for rendering only
    const saved = c.phase;
    c.phase = "clarifying";
    card = renderClarifyCard(c);
    c.phase = saved;
  }

  const brief = renderBriefPanel(c);
  // Hollow after claim lives inside success panel (with assign CTA) — avoid double bar.
  const claimed = c.phase === "claimed_to_plan";

  return (
    `<div class="chat-clarify" data-clarify-phase="${chatEsc(c.phase)}" data-clarify-entry="${chatEsc(
      c.entry
    )}"` +
    (claimed ? ` data-clarify-claimed="1"` : "") +
    `>` +
    (claimed ? "" : guide) +
    (claimed ? "" : emptyLine) +
    entries +
    status +
    card +
    brief +
    `</div>`
  );
}

/**
 * Whether empty-state should lead with clarify (always for t3 when no msgs).
 */
export function shouldShowClarifyOnEmpty() {
  return true;
}

/**
 * Inline strip above messages when conversation already started but still clarifying
 * or Brief / claim-success is relevant.
 */
export function renderClarifyInlineIfNeeded() {
  ensureClarifyState();
  const c = state.chatClarify;
  // Still show success + hollow after claim (so CTA verb / yellow bar stay visible)
  if (c.phase === "claimed_to_plan") {
    return renderClarifyPanelHtml({ mode: "inline" });
  }
  // Still show entries + card/status while clarifying / skipped ready
  if (
    c.phase === "not_started" &&
    c.uiStatus === "idle" &&
    !(state.chatSession?.messages || []).length
  ) {
    // empty path handles it
    return "";
  }
  // After first message, keep panel if still in clarify flow
  if (
    c.phase === "clarifying" ||
    c.phase === "skipped_to_plan" ||
    c.phase === "brief_ready" ||
    c.uiStatus === "loading" ||
    c.uiStatus === "error" ||
    (c.phase === "not_started" && isGrillPath(c)) ||
    shouldShowBrief(c)
  ) {
    return renderClarifyPanelHtml({ mode: "inline" });
  }
  return "";
}

// ─── Click binding (self-contained; cannot edit bindUiClick) ─────────────────

let _clarifyClickBound = false;

/**
 * Resolve click target to an Element.
 * Clicks on button label text often yield a Text node (no .closest) — that was
 * why A/B/C and skip appeared dead.
 */
function eventElement(e) {
  const t = e?.target;
  if (!t) return null;
  if (typeof t.closest === "function") return t;
  // Text / comment node → climb to parent element
  if (t.parentElement && typeof t.parentElement.closest === "function") {
    return t.parentElement;
  }
  if (typeof Node !== "undefined" && t.nodeType === Node.TEXT_NODE && t.parentElement) {
    return t.parentElement;
  }
  return null;
}

export function ensureClarifyClickBinding() {
  if (typeof document === "undefined") return;
  // Idempotent: one capture listener for the whole app lifetime
  if (_clarifyClickBound) return;
  _clarifyClickBound = true;

  const onClarifyClick = (e) => {
    // Prefer composedPath so shadow/text-node clicks still resolve to the button
    let t = null;
    try {
      const path = typeof e.composedPath === "function" ? e.composedPath() : null;
      if (Array.isArray(path)) {
        t = path.find(
          (n) =>
            n &&
            typeof n.closest === "function" &&
            (n.hasAttribute?.("data-clarify-option") ||
              n.hasAttribute?.("data-clarify-entry") ||
              n.hasAttribute?.("data-clarify-skip") ||
              n.hasAttribute?.("data-clarify-claim") ||
              n.hasAttribute?.("data-clarify-assign") ||
              n.hasAttribute?.("data-clarify-rechat") ||
              n.hasAttribute?.("data-clarify-retry") ||
              n.classList?.contains?.("chat-clarify") ||
              n.classList?.contains?.("chat-clarify-option") ||
              n.classList?.contains?.("chat-clarify-entry"))
        ) || path.find((n) => n && typeof n.closest === "function");
      }
    } catch (_) {}
    if (!t) t = eventElement(e);
    if (!t) return;

    // Only handle inside clarify UI (avoid fighting other capture handlers)
    if (!t.closest || !t.closest(".chat-clarify, [data-clarify-option], [data-clarify-entry], [data-clarify-skip], [data-clarify-claim], [data-clarify-assign], [data-clarify-rechat], [data-clarify-retry]")) {
      // still allow data-* buttons that might sit just outside during re-render glitches
      if (!t.closest?.("[data-clarify-option], [data-clarify-entry], [data-clarify-skip], [data-clarify-claim], [data-clarify-assign], [data-clarify-rechat], [data-clarify-retry]")) {
        return;
      }
    }

    const entryBtn = t.closest("[data-clarify-entry]");
    if (entryBtn) {
      e.preventDefault();
      e.stopPropagation();
      selectClarifyEntry(entryBtn.getAttribute("data-clarify-entry"));
      return;
    }
    const optBtn = t.closest("[data-clarify-option]");
    if (optBtn) {
      e.preventDefault();
      e.stopPropagation();
      const slot = optBtn.getAttribute("data-clarify-slot");
      const key = optBtn.getAttribute("data-clarify-option");
      if (slot) {
        try {
          ensureClarifyState();
          const c = state.chatClarify;
          const idx = CLARIFY_SLOT_QUESTIONS.findIndex((q) => q.id === slot);
          if (idx >= 0) c.questionIndex = idx;
          if (c.phase === "not_started" || c.phase === "brief_ready") {
            c.phase = "clarifying";
          }
          c._touchAt = Date.now();
        } catch (_) {}
      }
      // Pass slot so pick does not depend on questionIndex alone
      pickClarifyOption(key, slot);
      return;
    }
    const skipBtn = t.closest("[data-clarify-skip]");
    if (skipBtn) {
      e.preventDefault();
      e.stopPropagation();
      skipClarify(skipBtn.getAttribute("data-clarify-skip") || CLARIFY_COPY.skipCta);
      return;
    }
    const retryBtn = t.closest("[data-clarify-retry]");
    if (retryBtn) {
      e.preventDefault();
      e.stopPropagation();
      clearClarifyError();
      const input = $("#chat-input");
      if (input) input.focus();
      return;
    }
    const claimBtn = t.closest("[data-clarify-claim]");
    if (claimBtn) {
      e.preventDefault();
      e.stopPropagation();
      if (claimBtn.disabled) return;
      claimBriefToPlan();
      return;
    }
    const rechatBtn = t.closest("[data-clarify-rechat]");
    if (rechatBtn) {
      e.preventDefault();
      e.stopPropagation();
      rechatFromBrief();
      return;
    }
    const assignBtn = t.closest("[data-clarify-assign]");
    if (assignBtn) {
      e.preventDefault();
      e.stopPropagation();
      const run =
        (typeof host.assignFromChat === "function" && host.assignFromChat) ||
        (typeof window !== "undefined" &&
          window.ccoChat &&
          typeof window.ccoChat.assignFromChat === "function" &&
          window.ccoChat.assignFromChat.bind(window.ccoChat));
      if (typeof run === "function") {
        Promise.resolve(run()).catch((err) =>
          toast(String(err?.message || err || "无法进入拆成步骤"))
        );
      } else {
        toast("请用下方计划卡片上的「拆成步骤」");
      }
    }
  };

  document.addEventListener("click", onClarifyClick, true);
}

/** Call once from install / openChat. */
export function installClarifyUi() {
  ensureClarifyStyles();
  ensureClarifyState();
  ensureClarifyClickBinding();
}

// ─── Self-test helpers (dev / inspect) ───────────────────────────────────────

/**
 * Simulate loading or error for acceptance checks.
 * @param {"loading"|"error"|"idle"|"empty"} status
 */
export function simulateClarifyStatus(status) {
  if (status === "empty") {
    resetClarifyState();
    setClarifyUiStatus("idle");
  } else if (status === "loading") {
    setClarifyUiStatus("loading");
  } else if (status === "error") {
    setClarifyUiStatus("error");
  } else {
    setClarifyUiStatus("idle");
  }
  repaint();
  return {
    copy:
      status === "loading"
        ? CLARIFY_COPY.loading
        : status === "error"
          ? CLARIFY_COPY.error
          : CLARIFY_COPY.empty,
    entry: state.chatClarify?.entry,
    phase: state.chatClarify?.phase,
  };
}

export function getClarifyCopySnapshot() {
  return {
    empty: CLARIFY_COPY.empty,
    error: CLARIFY_COPY.error,
    loading: CLARIFY_COPY.loading,
    success: CLARIFY_COPY.success,
    claimCta: CLARIFY_COPY.claimCta,
    hollowWarn: CLARIFY_COPY.hollowWarn,
    defaultEntry: DEFAULT_CLARIFY_ENTRY,
    defaultEntryLabel: CLARIFY_ENTRIES.find((e) => e.isDefault)?.label,
  };
}

/**
 * Dev / inspect: fill all five slots explicitly → brief_ready.
 * Does not claim or save.
 */
export function fillClarifySlotsForTest(overrides = {}) {
  ensureClarifyState();
  const c = state.chatClarify;
  const defaults = {
    target_audience: "产品 / 运营同学（自己先用）",
    pain_moment: "只有一句模糊想法就要出稿",
    observable_outcome: "有一份可认领的澄清稿与计划",
    non_goals: "不做完整产品站 / 营销页",
    done_when: "五槽齐全且可分配计划",
  };
  const vals = { ...defaults, ...overrides };
  for (const q of CLARIFY_SLOT_QUESTIONS) {
    const v = vals[q.id];
    if (v != null && String(v).trim()) {
      setSlotFillLocal(c, q.id, String(v).trim(), "explicit");
    }
  }
  c.phase = "brief_ready";
  c.uiStatus = "idle";
  c.skip_requested = false;
  if (state.chatSession) state.chatSession.clarify = clarifyToWire(c);
  repaint();
  return buildBriefFromClarify(c);
}

/**
 * Dev / inspect: leave acceptance or non_goals empty → hollow bar.
 * @param {"done_when"|"non_goals"|"both"} which
 */
export function forceHollowForTest(which = "both") {
  ensureClarifyState();
  const c = state.chatClarify;
  // Ensure base fills
  fillClarifySlotsForTest();
  if (which === "done_when" || which === "both") {
    c.slots = (c.slots || []).filter((s) => s.id !== "done_when");
  }
  if (which === "non_goals" || which === "both") {
    c.slots = (c.slots || []).filter((s) => s.id !== "non_goals");
  }
  c.phase = "brief_ready";
  if (state.chatSession) state.chatSession.clarify = clarifyToWire(c);
  repaint();
  return detectHollowGaps(c, "");
}
