/**
 * W4-3 · chatClarify.js vertical slice #2 - Empty/Card Render Layer
 *
 * [INPUT]: state.ensureClarifyState · chatFormat.chatEsc · CLARIFY_COPY/ENTRIES/SLOT_QUESTIONS
 * [OUTPUT]: Empty/Card UI rendering (entries, guide, status, card, events)
 * [POS]: features/chat/clarify/render.js — Empty/Card 渲染层完整模块
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 职责:
 * - 常量定义：COPY/ENTRIES/SLOTS (与 domain/chat/clarify 对齐)
 * - 状态管理：defaultClarifyState, ensureClarifyState, normalizeClarifyState, clarifyToWire
 * - 渲染函数：renderEntryChips, renderGuideBlock, renderStatusBlock, renderClarifyCard
 *   renderHollowBarHtml, renderClarifyPanelHtml, renderClarifyInlineIfNeeded
 * - 事件绑定：click capture for options/entries/skips/claims
 * - CSS 注入：ensureClarifyStyles
 * - 安装入口：installClarifyUi
 *
 * 与 chatClarify.js 关系：从 chatClarify.js 搬移而来，禁止反向依赖
 * 与 briefAndClaim.js 关系：共享同一套常量定义，通过 re-export 链保持一致
 */

import { state, $, toast } from "../legacy.js";
import { chatEsc } from "../chatFormat.js";
import { stashChatSession, ensureChatState } from "../chatState.js";
import { host } from "../host.js";

// ─── Constants (Product Copy + Entry Points + Slot Questions) ────────────────

export const CLARIFY_COPY = Object.freeze({
  empty: "用一句话说你想做成什么。下面先帮你问清楚，再写成计划。",
  error: "这句还不够清楚。可以换个说法，或点「跳过，先出草稿」。",
  loading: "正在整理你的想法…",
  success: "计划草稿已写好。",
  successNext: "下一步：拆成步骤（不会自动开始执行）。",
  assignCta: "拆成步骤",
  claimCta: "写成计划",
  claimBusy: "正在写入计划草稿…",
  rechat: "再改一改",
  skipCta: "跳过，先出草稿",
  skipAlt: "其余你帮我选",
  skipHint: "跳过剩余问题，按常见假设先出一版，你还能改",
  readySkip: "已按常见假设整理好。扫一眼摘要，再写成计划。",
  readyPlanOnly: "已选直接写计划。仍会写上目标、先不做、怎样算完。",
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
});

export const CLARIFY_SCHEMA_VERSION = 1;

export const CLARIFY_ENTRIES = Object.freeze([
  {
    id: "think_first",
    label: "先只想清楚",
    hint: "先整理一页摘要，先不写成计划",
  },
  {
    id: "idea_to_plan",
    label: "帮我想清楚再写成计划",
    hint: "默认：问答 → 一页摘要 → 认领并写草稿 → 分配计划",
    isDefault: true,
  },
  {
    id: "plan_only",
    label: "直接写计划",
    hint: "略过问答；仍需补上目标／不做／验收才能分配",
  },
]);

export const DEFAULT_CLARIFY_ENTRY = "idea_to_plan";

export const SLOT_LABEL = Object.freeze({
  target_audience: "给谁用",
  pain_moment: "痛在何处",
  observable_outcome: "做到什么程度",
  non_goals: "这轮先不做",
  done_when: "怎样算做完",
});

export const CLARIFY_SLOT_QUESTIONS = Object.freeze([
  {
    id: "target_audience",
    question: "这个计划是给谁用的？自己 / 团队 / 客户？",
    options: [
      { key: "A", text: "我自己 / 小团队自用" },
      { key: "B", text: "内部产品 / 工具同学" },
      { key: "C", text: "外部客户 / 付费用户" },
    ],
  },
  {
    id: "pain_moment",
    question: "当前最痛的场景是什么？一句话描述触发时刻。",
    options: [
      { key: "A", text: "需求反复变化，不知何时该停" },
      { key: "B", text: "信息分散，写稿要到处翻文档" },
      { key: "C", text: "只有一个模糊想法，不知从何下手" },
    ],
  },
  {
    id: "observable_outcome",
    question: "做成之后，可见的可观察结果是什么？",
    options: [
      { key: "A", text: "一份可认领的计划草稿 + V1 大纲" },
      { key: "B", text: "一个最小演示路径可跑通" },
      { key: "C", text: "用户能完成关键任务闭环" },
    ],
  },
  {
    id: "non_goals",
    question: "这轮明确不做的是什么？避免范围扩散。",
    options: [
      { key: "A", text: "完整产品站 / 营销落地页" },
      { key: "B", text: "所有可能的扩展能力" },
      { key: "C", text: "复杂的权限 / 多角色体系" },
    ],
  },
  {
    id: "done_when",
    question: "怎样算做完？定义验收标准。",
    options: [
      { key: "A", text: "五槽齐全且可分配计划" },
      { key: "B", text: "用户反馈确认主路径 OK" },
      { key: "C", text: "核心指标达到预设阈值" },
    ],
  },
]);

// ─── State Management ────────────────────────────────────────────────────────

export function defaultClarifyState(entry) {
  return {
    schema_version: CLARIFY_SCHEMA_VERSION,
    entry: entry || DEFAULT_CLARIFY_ENTRY,
    phase: "not_started",
    uiStatus: "idle",
    errorText: null,
    questionIndex: 0,
    skip_requested: false,
    selectedOption: null,
    optional: [],
    slots: [],
    assumptions: [],
    _touchAt: Date.now(),
    _claimSuccess: false,
    _claimAt: 0,
    _claimBusy: false,
  };
}

export function ensureClarifyState() {
  if (!state.chatClarify || typeof state.chatClarify !== "object") {
    state.chatClarify = defaultClarifyState();
  }
  return state.chatClarify;
}

export function resetClarifyState(entry) {
  state.chatClarify = defaultClarifyState(entry || DEFAULT_CLARIFY_ENTRY);
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = clarifyToWire(state.chatClarify);
  }
  mirrorClarifyToSession(state.chatClarify);
  repaint();
}

export function hydrateClarifyFromSession(sess) {
  if (sess && sess.clarify && typeof sess.clarify === "object") {
    const prevUi = state.chatClarify?.uiStatus;
    const normalized = normalizeClarifyState(sess.clarify);
    state.chatClarify = { ...defaultClarifyState(), ...normalized };
    if (prevUi && prevUi !== "error") {
      state.chatClarify.uiStatus = prevUi;
    }
    return true;
  }
  return false;
}

export function normalizeClarifyState(raw) {
  const base = defaultClarifyState();
  if (!raw || typeof raw !== "object") return base;
  return {
    ...base,
    ...raw,
    slots: (raw.slots || []).map((s) => ({
      id: s.id || "",
      value: s.value || "",
      kind: s.kind || "assumed",
    })),
    assumptions: (raw.assumptions || []).map((a) => ({
      text: a.text || "",
      slot: a.slot || "",
    })),
    optional: (raw.optional || []).map((o) => ({
      key: o.key || "",
      value: o.value || "",
      selected: !!o.selected,
    })),
  };
}

export function clarifyToWire(c) {
  const s = c || defaultClarifyState();
  return {
    entry: s.entry || DEFAULT_CLARIFY_ENTRY,
    phase: s.phase || "not_started",
    uiStatus: s.uiStatus || "idle",
    errorText: s.errorText || null,
    questionIndex: Number(s.questionIndex) || 0,
    skip_requested: !!s.skip_requested,
    selectedOption: s.selectedOption || null,
    optional: (s.optional || []).map((o) => ({
      key: o.key || "",
      value: o.value || "",
      selected: !!o.selected,
    })),
    slots: (s.slots || []).map((slot) => ({
      id: slot.id || "",
      value: slot.value || "",
      kind: slot.kind || "assumed",
    })),
    assumptions: (s.assumptions || []).map((a) => ({
      text: a.text || "",
      slot: a.slot || "",
    })),
    _touchAt: Number(s._touchAt) || 0,
    _claimSuccess: !!s._claimSuccess,
    _claimAt: Number(s._claimAt) || 0,
    schema_version: Number(s.schema_version) || CLARIFY_SCHEMA_VERSION,
  };
}

function mirrorClarifyToSession(c) {
  const wire = clarifyToWire(c);
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = wire;
  }
  return wire;
}

function reconcileClaimedPhase() {
  ensureChatState();
  const c = state.chatClarify;
  if (!c || c.phase !== "claimed_to_plan") return;
  const sess = state.chatSession;
  if (sess?.draft_plan?.markdown && c._claimSuccess !== false) {
    // Already claimed; nothing to do
  }
}

// ─── Slot Helpers ────────────────────────────────────────────────────────────

function slotValue(c, id) {
  const s = (c?.slots || []).find((x) => x.id === id);
  return s && String(s.value || "").trim();
}

function slotKind(c, id) {
  const s = (c?.slots || []).find((x) => x.id === id);
  return s ? String(s.kind || "") : null;
}

function isEffectivelyMissing(value, kind) {
  if (!value || !String(value).trim()) return true;
  if (kind === "explicit") return false;
  return isAssumedOrPlaceholder(value, kind);
}

function isAssumedOrPlaceholder(value, kind) {
  const v = String(value || "").toLowerCase().trim();
  if (!v) return true;
  if (/（待|^\(待|待补|待写|待定|请补充|TBD|TODO|…|\.{2,}/i.test(v)) {
    return true;
  }
  if (/^假设/.test(v) || kind === "assumed") return true;
  return false;
}

function currentQuestion(c) {
  const idx = Math.max(
    0,
    Math.min(c?.questionIndex ?? 0, CLARIFY_SLOT_QUESTIONS.length - 1)
  );
  return CLARIFY_SLOT_QUESTIONS[idx] || null;
}

function missingRequiredSlots(c) {
  const cState = c || state.chatClarify;
  return CLARIFY_SLOT_QUESTIONS.filter((q) => !isSlotFilled(cState, q.id));
}

export function isSlotFilled(c, slotId) {
  const fill = (c?.slots || []).find((s) => s.id === slotId);
  return !!(fill && String(fill.value || "").trim());
}

function filledCount(c) {
  return CLARIFY_SLOT_QUESTIONS.length - missingRequiredSlots(c).length;
}

// ─── Domain Logic Helpers ────────────────────────────────────────────────────

function extractSectionBody(md, regex) {
  const match = md.match(new RegExp(`${regex.source}[\\s\\S]*?(?:\\n## |$)`));
  if (!match) return "";
  const lines = match[0].split("\n").slice(1);
  return lines.join("\n");
}

function bodyIsStub(body) {
  const t = String(body || "").trim();
  if (!t) return true;
  const lines = t.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return true;
  const contentLines = lines.filter(
    (l) => !/^[-*+]\\s*说明 [：:]/.test(l) && !/^说明 [：:]/.test(l)
  );
  if (!contentLines.length) return true;
  const stubLine = (l) => {
    const core = l.replace(/^[-*+]\\s*/, "").replace(/^\[[\\sxX ]\\]\\s*/i, "").trim();
    if (!core) return true;
    if (/^(（待|^\(待|待补|待写|待定|请补充|TBD|TODO|…|\.{2,})/i.test(core)) {
      return true;
    }
    if (/（待补|（待写|（待定|\(待补|\(待写/.test(core)) {
      return true;
    }
    if (/^假设（用户跳过/.test(core) || /待写计划时补全/.test(core)) {
      return true;
    }
    if (/^[.。…\\-\\s]*$/.test(core)) return true;
    return false;
  };
  const nonStub = contentLines.filter((l) => !stubLine(l));
  return nonStub.length === 0;
}

function normalizeEntry(id) {
  const normalized = String(id || "").trim().toLowerCase();
  if (["think_first", "idea_to_plan", "plan_only"].includes(normalized)) {
    return normalized;
  }
  return DEFAULT_CLARIFY_ENTRY;
}

function isGrillPath(c) {
  return c.entry === "idea_to_plan" || c.entry === "think_first";
}

// ─── Repaint Helper ─────────────────────────────────────────────────────────

let _clarifyPaint = null;

export function setClarifyPaint(fn) {
  _clarifyPaint = typeof fn === "function" ? fn : null;
}

function repaint() {
  const tryCall = (fn) => {
    if (typeof fn !== "function") return false;
    try {
      fn();
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
    const desk = typeof window !== "undefined" ? window.ccoChat || null : null;
    if (desk && tryCall(desk.renderChatMessages?.bind(desk))) return;
    if (desk && tryCall(desk.renderChatPage?.bind(desk))) return;
  } catch (_) {}
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

// ─── Render Functions (Entries, Guide, Status, Card) ─────────────────────────

export function renderEntryChips(c, { disabled } = {}) {
  const dis = disabled ? " disabled" : "";
  const ordered = [
    ...CLARIFY_ENTRIES.filter((e) => e.isDefault),
    ...CLARIFY_ENTRIES.filter((e) => !e.isDefault),
  ];
  return (
    `<div class="chat-clarify-entries" role="group" aria-label="开始方式">` +
    ordered
      .map((e) => {
        const active = c.entry === e.id ? " is-active" : "";
        const def = e.isDefault ? " is-default" : " is-alt";
        const aria =
          c.entry === e.id
            ? ' aria-pressed="true"'
            : ' aria-pressed="false"';
        return (
          `<button type="button" class="chat-clarify-entry${def}${active}"`
            + ` data-clarify-entry="${chatEsc(e.id)}"`
            + ` title="${chatEsc(e.hint)}"${aria}${dis}>${chatEsc(e.label)}</button>`
        );
      })
      .join("") +
    `</div>`
  );
}

function renderGuideBlock(c, mode) {
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
  // Loading
  if (
    c.uiStatus === "loading" ||
    c._claimBusy ||
    (state.chatBusy && isGrillPath(c))
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
  // Claimed success banner
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
  // Skipped / plan-only ready
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

function renderClarifyCard(c) {
  if (!shouldShowCard(c)) return "";
  const q = currentQuestion(c);
  if (!q) return "";
  const done = filledCount(c);
  const total = CLARIFY_SLOT_QUESTIONS.length;
  const pct = Math.round((done / total) * 100);
  const selected = c.selectedOption;

  const opts = (q.options || [])
    .map((o) => {
      const sel = selected === o.key ? " is-selected" : "";
      return (
        `<button type="button" class="chat-clarify-option${sel}"`
          + ` data-clarify-option="${chatEsc(o.key)}"`
          + ` data-clarify-slot="${chatEsc(q.id)}">` +
          `<span class="opt-key">${chatEsc(o.key)}</span>` +
          `<span class="opt-text">${chatEsc(o.text)}</span>` +
          `</button>`
      );
    })
    .join("");

  const step = Math.min(done + 1, total);
  const dots = Array.from({ length: total }, (_, i) => {
    const n = i + 1;
    const cls = n < step ? " is-done" : n === step ? " is-current" : "";
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

/** Whether Brief panel should show. */
export function shouldShowBrief(c) {
  const stateC = c || ensureClarifyState();
  if (stateC.phase === "claimed_to_plan") return false;
  if (stateC.phase === "brief_ready") return true;
  if (stateC.phase === "skipped_to_plan") return true;
  if (stateC.entry === "plan_only" && stateC.skip_requested) return true;
  if (
    (stateC.phase === "clarifying" || stateC.phase === "not_started") &&
    missingRequiredSlots(stateC).length === 0
  ) {
    return true;
  }
  return false;
}

function shouldShowCard(c) {
  if (c.uiStatus === "loading" || c.uiStatus === "error") return false;
  if (c.phase === "skipped_to_plan" || c.phase === "claimed_to_plan") return false;
  if (c.phase === "brief_ready") return false;
  if (c.entry === "plan_only") return false;
  return isGrillPath(c) && (c.phase === "clarifying" || c.phase === "not_started");
}

function claimPlanPreviewText(md) {
  const raw = String(md || "").trim();
  if (!raw) return "";
  const lines = raw
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((l) => l.trimEnd())
    .filter((l) => l.trim());
  const keep = [];
  for (const l of lines) {
    if (keep.length >= 8) break;
    if (/^```/.test(l)) continue;
    keep.push(l);
  }
  let text = keep.join("\n");
  if (text.length > 420) text = text.slice(0, 419) + "…";
  return text;
}

// ─── CSS Injection ────────────────────────────────────────────────────────────

const CLARIFY_STYLE_ID = "cco-clarify-style-v2";

export function ensureClarifyStyles() {
  if (typeof document === "undefined") return;
  const prev = document.getElementById("cco-clarify-style");
  if (prev) prev.remove();
  if (document.getElementById(CLARIFY_STYLE_ID)) return;
  const s = document.createElement("style");
  s.id = CLARIFY_STYLE_ID;
  s.textContent = `
.chat-clarify { margin: 0.35rem auto 0.75rem; max-width: 32rem; width: 100%; text-align: left; }
.chat-clarify-guide { text-align: center; margin: 0.15rem 0 0.45rem; }
.chat-clarify-guide-title { margin: 0; font-size: 0.98rem; font-weight: 650; color: var(--text); line-height: 1.4; }
.chat-clarify-guide-hint { margin: 0.2rem 0 0; font-size: 0.8rem; color: var(--muted); line-height: 1.4; }
.chat-clarify-entries { display: flex; flex-wrap: wrap; gap: 0.4rem; justify-content: center; align-items: center; margin: 0.35rem 0 0.55rem; }
.chat-clarify-entry { font: inherit; font-size: 0.78rem; padding: 0.35rem 0.7rem; border-radius: 999px; border: 1px solid var(--border); background: var(--bg2, #fff); color: var(--muted); cursor: pointer; line-height: 1.3; transition: border-color 0.12s ease, background 0.12s ease; }
.chat-clarify-entry:hover { border-color: color-mix(in srgb, var(--accent, #0071E3) 45%, var(--border)); color: var(--accent, #0071E3); }
.chat-clarify-entry.is-default { font-size: 0.84rem; padding: 0.42rem 0.9rem; color: var(--text); border-color: color-mix(in srgb, var(--accent, #0071E3) 35%, var(--border)); }
.chat-clarify-entry.is-active { border-color: var(--accent, #0071E3); background: color-mix(in srgb, var(--accent, #0071E3) 12%, var(--bg2, #fff)); color: var(--accent, #0071E3); font-weight: 600; }
.chat-clarify-entry.is-active.is-default { box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent, #0071E3) 25%, transparent); }
.chat-clarify-entry:disabled { opacity: 0.5; cursor: not-allowed; }
.chat-clarify-entry.is-alt { font-size: 0.74rem; opacity: 0.92; }
.chat-clarify-empty-line { text-align: center; margin: 0.15rem 0 0.35rem; font-size: 0.9rem; color: var(--muted); line-height: 1.5; }
.chat-clarify-card { margin: 0.45rem auto; max-width: 32rem; width: 100%; padding: 0.95rem 1.05rem 0.9rem; border-radius: var(--radius, 12px); border: 1px solid var(--border); background: var(--bg2, #fff); box-sizing: border-box; box-shadow: 0 1px 2px color-mix(in srgb, var(--text, #000) 4%, transparent); }
.chat-clarify-card-head { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; margin-bottom: 0.35rem; }
.chat-clarify-progress { font-size: 0.75rem; color: var(--muted); font-weight: 550; }
.chat-clarify-progress-dots { display: inline-flex; gap: 0.28rem; align-items: center; margin-left: 0.45rem; vertical-align: middle; }
.chat-clarify-progress-dots > i { width: 6px; height: 6px; border-radius: 999px; background: var(--bg3, #E5E5EA); display: inline-block; }
.chat-clarify-progress-dots > i.is-done { background: color-mix(in srgb, var(--accent, #0071E3) 55%, var(--bg3, #E5E5EA)); }
.chat-clarify-progress-dots > i.is-current { background: var(--accent, #0071E3); transform: scale(1.15); }
.chat-clarify-progress-bar { display: block; height: 3px; border-radius: 999px; background: var(--bg3, #F0F0F2); overflow: hidden; margin-top: 0.4rem; margin-bottom: 0.75rem; }
.chat-clarify-progress-bar > i { display: block; height: 100%; background: var(--accent, #0071E3); border-radius: inherit; transition: width 0.15s ease; }
.chat-clarify-question { margin: 0 0 0.7rem; font-size: 1.02rem; font-weight: 650; color: var(--text); line-height: 1.45; }
.chat-clarify-options { display: flex; flex-direction: column; gap: 0.4rem; margin: 0 0 0.75rem; }
.chat-clarify-option { font: inherit; text-align: left; font-size: 0.9rem; padding: 0.55rem 0.8rem; border-radius: 10px; border: 1px solid var(--border); background: var(--bg, #F5F5F7); color: var(--text); cursor: pointer; line-height: 1.4; transition: border-color 0.12s ease, background 0.12s ease; }
.chat-clarify-option:hover { border-color: color-mix(in srgb, var(--accent, #0071E3) 40%, var(--border)); background: color-mix(in srgb, var(--accent, #0071E3) 6%, var(--bg2, #fff)); }
.chat-clarify-option.is-selected { border-color: var(--accent, #0071E3); background: color-mix(in srgb, var(--accent, #0071E3) 12%, var(--bg2, #fff)); font-weight: 550; }
.chat-clarify-option .opt-key { display: inline-flex; align-items: center; justify-content: center; min-width: 1.25rem; height: 1.25rem; padding: 0 0.2rem; border-radius: 6px; font-size: 0.72rem; font-weight: 600; color: var(--accent, #0071E3); background: color-mix(in srgb, var(--accent, #0071E3) 12%, transparent); margin-right: 0.5rem; }
.chat-clarify-actions { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
.chat-clarify-skip-hint { font-size: 0.75rem; color: var(--muted); margin-left: auto; line-height: 1.4; }
.chat-clarify-status { font-size: 0.85rem; padding: 0.45rem 0.65rem; border-radius: 8px; background: var(--bg2, #fff); border: 1px solid var(--border); color: var(--text); line-height: 1.35; }
.chat-clarify-status.is-loading { display: flex; align-items: center; gap: 0.45rem; color: var(--muted); }
.chat-clarify-status.is-error { display: flex; flex-direction: column; gap: 0.4rem; border-color: color-mix(in srgb, red 30%, var(--border)); background: color-mix(in srgb, red 5%, var(--bg2)); }
.chat-clarify-status-actions { display: flex; gap: 0.5rem; align-items: center; }
.chat-claim-success { margin: 0.45rem auto; max-width: 32rem; width: 100%; padding: 0.85rem 0.95rem; border-radius: var(--radius, 12px); border: 1px solid var(--border); background: color-mix(in srgb, green 8%, var(--bg2, #fff)); }
.chat-claim-success-title { margin: 0; font-size: 0.98rem; font-weight: 650; color: var(--text); }
.chat-claim-success-next { margin: 0.25rem 0 0; font-size: 0.8rem; color: var(--muted); }
.chat-claim-preview { margin: 0.5rem 0; padding: 0.55rem 0.65rem; border-radius: 8px; background: var(--bg2); border: 1px solid var(--border); font-size: 0.82rem; color: var(--text); line-height: 1.4; max-height: 8rem; overflow-y: auto; }
.chat-claim-preview-label { position: absolute; left: -9999px; }
.chat-claim-success-actions { display: flex; gap: 0.5rem; margin-top: 0.5rem; }
.chat-brief { margin: 0.45rem auto; max-width: 32rem; width: 100%; padding: 0; }
.chat-brief-head { text-align: center; margin-bottom: 0.5rem; }
.chat-brief-title { margin: 0; font-size: 0.98rem; font-weight: 650; color: var(--text); }
.chat-brief-hint { margin: 0.2rem 0 0; font-size: 0.78rem; color: var(--muted); }
.chat-brief-groups { display: flex; flex-direction: column; gap: 0.5rem; margin-bottom: 0.5rem; }
.chat-brief-group { padding: 0.5rem 0.6rem; border-radius: 8px; background: var(--bg2); border: 1px solid var(--border); }
.chat-brief-group-label { margin: 0 0 0.25rem; font-size: 0.82rem; font-weight: 600; color: var(--muted); }
.chat-brief-group-body { margin: 0; font-size: 0.9rem; line-height: 1.45; color: var(--text); }
.chat-brief-tag { display: inline-flex; align-items: center; padding: 0.15rem 0.45rem; border-radius: 6px; font-size: 0.72rem; font-weight: 600; margin-right: 0.35rem; margin-bottom: 0.25rem; }
.chat-brief-tag.is-user { background: color-mix(in srgb, blue 10%, var(--bg3)); color: var(--accent, #0071E3); }
.chat-brief-tag.is-assumed { background: color-mix(in srgb, orange 10%, var(--bg3)); color: #DD8A00; }
.chat-hollow-bar { margin: 0.5rem auto; max-width: 32rem; width: 100%; padding: 0.5rem 0.6rem; border-radius: 8px; border: 1px solid color-mix(in srgb, orange 30%, var(--border)); background: color-mix(in srgb, orange 8%, var(--bg2)); font-size: 0.85rem; color: var(--text); line-height: 1.4; }
.chat-hollow-bar ul { margin: 0.35rem 0 0 1.2rem; padding: 0; font-size: 0.82rem; color: var(--muted); }
`;
  document.head.appendChild(s);
}

/** Full panel HTML (entries + empty/status/card/Brief). */
export function renderClarifyPanelHtml(opts = {}) {
  ensureClarifyStyles();
  ensureClarifyState();
  const c = state.chatClarify;
  const mode = opts.mode || "inline";
  const busy = !!state.chatBusy || !!c._claimBusy;

  const guide = renderGuideBlock(c, mode);
  let emptyLine = "";
  if (mode === "empty" && c.phase === "not_started" && c.uiStatus !== "error") {
    emptyLine =
      `<p class="chat-clarify-empty-line" data-clarify-copy="empty">` +
      `${chatEsc(CLARIFY_COPY.empty)}` +
      `</p>`;
  }

  const entries =
    c.phase === "claimed_to_plan"
      ? ""
      : renderEntryChips(c, { disabled: busy });
  const status = renderStatusBlock(c);
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
    const saved = c.phase;
    c.phase = "clarifying";
    card = renderClarifyCard(c);
    c.phase = saved;
  }

  // Brief rendering would be here (could be moved to brief module)
  // For now, return partial until briefAndClaim integration complete

  return (
    `<div class="chat-clarify" data-clarify-phase="${chatEsc(c.phase)}" data-clarify-entry="${chatEsc(
      c.entry
    )}"` +
    `>` +
    (c.phase === "claimed_to_plan" ? "" : guide) +
    (c.phase === "claimed_to_plan" ? "" : emptyLine) +
    entries +
    status +
    card +
    `</div>`
  );
}

export function shouldShowClarifyOnEmpty() {
  return true;
}

export function renderClarifyInlineIfNeeded() {
  ensureClarifyState();
  const c = state.chatClarify;
  if (c.phase === "claimed_to_plan") {
    return renderClarifyPanelHtml({ mode: "inline" });
  }
  if (
    c.phase === "not_started" &&
    c.uiStatus === "idle" &&
    !(state.chatSession?.messages || []).length
  ) {
    return "";
  }
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

// ─── Click Binding ────────────────────────────────────────────────────────────

let _clarifyClickBound = false;

function eventElement(e) {
  const t = e?.target;
  if (!t) return null;
  if (typeof t.closest === "function") return t;
  if (t.parentElement && typeof t.parentElement.closest === "function") {
    return t.parentElement;
  }
  return null;
}

export function ensureClarifyClickBinding() {
  if (typeof document === "undefined") return;
  if (_clarifyClickBound) return;
  _clarifyClickBound = true;

  const onClarifyClick = (e) => {
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

    if (!t.closest?.(
      ".chat-clarify, [data-clarify-option], [data-clarify-entry], " +
      "[data-clarify-skip], [data-clarify-claim], [data-clarify-assign], " +
      "[data-clarify-rechat], [data-clarify-retry]"
    )) {
      return;
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
      pickClarifyOption(key, slot);
      return;
    }

    const skipBtn = t.closest("[data-clarify-skip]");
    if (skipBtn) {
      e.preventDefault();
      e.stopPropagation();
      // Note: actual skip logic lives in briefAndClaim.js
      return;
    }

    const claimBtn = t.closest("[data-clarify-claim]");
    if (claimBtn) {
      e.preventDefault();
      e.stopPropagation();
      // Note: actual claim logic lives in briefAndClaim.js
      return;
    }
  };

  document.addEventListener("click", onClarifyClick, true);
}

export function installClarifyUi() {
  ensureClarifyStyles();
  ensureClarifyState();
  ensureClarifyClickBinding();
}

// ─── Entry Point & Navigation ─────────────────────────────────────────────────

export function selectClarifyEntry(entryId) {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;
  const next = normalizeEntry(entryId);
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
    c.skip_requested = true;
    c.phase = "skipped_to_plan";
  } else {
    if (c.phase === "skipped_to_plan") {
      c.skip_requested = false;
      c.slots = (c.slots || []).filter((s) => s.kind === "explicit");
      c.assumptions = [];
    }
    if (c.phase !== "claimed_to_plan") {
      c.phase = "clarifying";
      c.skip_requested = false;
    }
  }

  mirrorClarifyToSession(c);
  try {
    if (typeof stashChatSession === "function") stashChatSession(state.selectedPath);
  } catch (_) {}
  repaint();
}

export function pickClarifyOption(optionKey, slotId) {
  try {
    ensureChatState();
    ensureClarifyState();
    const c = state.chatClarify;
    if (!c || typeof c !== "object") {
      toast("澄清状态未就绪，请再点一次");
      return;
    }
    if (c.phase === "not_started") c.phase = "clarifying";
    if (c.phase === "claimed_to_plan") {
      toast("计划草稿已写好。可点「拆成步骤」，或「再改一改」");
      return;
    }
    if (c.phase === "skipped_to_plan") {
      c.skip_requested = false;
      c.slots = (c.slots || []).filter((s) => s.kind === "explicit");
      c.assumptions = [];
      c.phase = "clarifying";
    }

    const key = String(optionKey || "").trim().toUpperCase();
    if (!key) return;

    let q = null;
    if (slotId && String(slotId).trim()) {
      q = CLARIFY_SLOT_QUESTIONS.find((x) => x.id === slotId) || null;
      if (q) {
        c.questionIndex = Math.max(0, CLARIFY_SLOT_QUESTIONS.findIndex((x) => x.id === q.id));
      }
    }
    if (!q) q = currentQuestion(c);
    if (!q) {
      c.phase = "brief_ready";
      c.selectedOption = null;
      c._touchAt = Date.now();
      mirrorClarifyToSession(c);
      repaint();
      return;
    }

    let opt = (q.options || []).find(
      (o) => String(o.key || "").toUpperCase() === key
    );
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

    setSlotFillLocal(c, q.id, opt.text, "explicit");
    c.uiStatus = "idle";
    c.errorText = null;
    c._touchAt = Date.now();

    const missing = missingRequiredSlots(c);
    if (!missing.length) {
      c.phase = "brief_ready";
      c.selectedOption = null;
      toast("要点齐了，先看一页摘要");
    } else {
      c.phase = "clarifying";
      const nextId = missing[0];
      c.questionIndex = Math.max(0, CLARIFY_SLOT_QUESTIONS.findIndex((x) => x.id === nextId));
    }

    mirrorClarifyToSession(c);
    try {
      if (typeof stashChatSession === "function") stashChatSession(state.selectedPath);
    } catch (_) {}
    repaint();
  } catch (_) {}
}

function setSlotFillLocal(c, id, value, kind) {
  const v = String(value || "").trim();
  if (!v) return false;
  const existing = (c.slots || []).find((s) => s.id === id);
  if (existing) {
    existing.value = v;
    existing.kind = FILL_KINDS.has(existing.kind) ? existing.kind : kind;
  } else {
    c.slots.push({ id, value: v, kind });
  }
  return true;
}

const FILL_KINDS = new Set(["explicit", "assumed", "soft_fill"]);
