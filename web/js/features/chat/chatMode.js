/**
 * [INPUT]: persona pathBias · state.chatSession.clarify.entry · chatClarify helpers
 * [OUTPUT]: 两模式 chip（快速出产品 / 深度思考）· setMode 只写 entry · fast 首 send 钩子
 * [POS]: F1 docs/chat-dual-mode-empty-guard-2026-08-20.md §4.1–4.5 · features/chat/chatMode.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 硬契约：
 * - 点 chip = 只写 entry 记忆，不 claim、不出草稿（禁止走 selectClarifyEntry('plan_only') 全分支）
 * - fast 首条 send 前才 applySkipWithAssumptionsLocal；不预 claim 空模板
 * - 「直接写计划」逃生舱仍走 selectClarifyEntry('plan_only') 即时 claim
 * - domain ClarifyEntry / wire 零改动；UI 映射 fast→plan_only · deep→idea_to_plan|think_first
 */

import { state, $, toast } from "./legacy.js";
import {
  ensureChatState,
  stashChatSession,
} from "./chatState.js";
import {
  ensureClarifyState,
  applySkipWithAssumptionsLocal,
  selectClarifyEntry,
  clarifyToWire,
} from "./chatClarify.js";
import { personaPathBias } from "./chatPersona.js";
import { chatEsc } from "./chatFormat.js";

/** @typedef {'fast'|'deep'} ChatModeId */

export const CHAT_MODES = Object.freeze({
  fast: {
    id: /** @type {ChatModeId} */ ("fast"),
    label: "快速出产品",
    hint: "一句话描述 → 按常见假设先出计划（可改）",
    entry: "plan_only",
  },
  deep: {
    id: /** @type {ChatModeId} */ ("deep"),
    label: "深度思考",
    hint: "先问清楚再写成计划",
    entry: "idea_to_plan",
  },
});

const MODE_BAR_ID = "chat-mode-bar";
const FAST_SKIP_NOTE = "快速出产品";

let _modeClickBound = false;

function hasDraftPlan() {
  return !!String(state.chatSession?.draft_plan?.markdown || "").trim();
}

function mirrorAndStash(c) {
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = clarifyToWire(c);
  }
  try {
    if (typeof stashChatSession === "function") {
      stashChatSession(state.selectedPath);
    }
  } catch (_) {}
}

function repaintChat() {
  try {
    const desk =
      typeof window !== "undefined" ? window.ccoChat || null : null;
    if (desk && typeof desk.renderChatMessages === "function") {
      desk.renderChatMessages({ force: true });
      return;
    }
  } catch (_) {}
  try {
    if (typeof window !== "undefined" && typeof window.renderChatMessages === "function") {
      window.renderChatMessages({ force: true });
    }
  } catch (_) {}
}

/**
 * Map clarify.entry → UI mode.
 * @returns {ChatModeId}
 */
export function getChatMode() {
  ensureChatState();
  ensureClarifyState();
  const entry = state.chatClarify?.entry;
  if (entry === "plan_only") return "fast";
  return "deep";
}

/**
 * Soft default from persona pathBias when session has no intentional entry yet.
 * Does not override history entry / claimed / explicit slots / user pick.
 * @returns {ChatModeId}
 */
export function ensureModeDefault() {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;
  if (!c || typeof c !== "object") return "deep";
  if (c._modeUserPicked) return getChatMode();
  if (
    c.phase === "claimed_to_plan" ||
    c.phase === "skipped_to_plan" ||
    c.phase === "brief_ready" ||
    c.phase === "clarifying"
  ) {
    return getChatMode();
  }
  if (c.skip_requested) return getChatMode();
  if ((c.slots || []).some((s) => s && s.kind === "explicit")) {
    return getChatMode();
  }
  if ((state.chatSession?.messages || []).length > 0) return getChatMode();
  if (hasDraftPlan()) return getChatMode();

  // Fresh empty: L → fast (entry=plan_only, no skip); M/H stay deep default
  const bias = personaPathBias();
  if (bias === "L") {
    if (c.entry !== "plan_only") {
      c.entry = "plan_only";
      c.skip_requested = false;
      mirrorAndStash(c);
    }
    return "fast";
  }
  if (c.entry === "plan_only" && !c.skip_requested && c.phase === "not_started") {
    // Unlikely on true fresh (default idea_to_plan); leave if somehow set
  }
  return getChatMode();
}

/**
 * setMode(fast|deep): only write entry + mirror/stash + repaint.
 * **禁止** selectClarifyEntry('plan_only') 全分支（会 applySkip + claimBriefToPlan）。
 * @param {ChatModeId|string} mode
 * @param {{ silent?: boolean }} [opts]
 * @returns {ChatModeId}
 */
export function setChatMode(mode, opts = {}) {
  ensureChatState();
  ensureClarifyState();
  const c = state.chatClarify;
  const prevMode = getChatMode();
  const next = String(mode || "").toLowerCase() === "fast" ? "fast" : "deep";
  c._modeUserPicked = true;
  c._touchAt = Date.now();
  c.uiStatus = "idle";
  c.errorText = null;

  if (next === "fast") {
    c.entry = "plan_only";
    // Chip is quiet: no skip, no claim. First send applies assumptions.
    if (c.phase !== "claimed_to_plan" && c.phase !== "skipped_to_plan") {
      c.skip_requested = false;
    }
    // deep→fast: keep all slots (explicit + assumed); do not filter
  } else {
    // deep: default idea_to_plan; keep think_first if already there
    if (c.entry !== "think_first") {
      c.entry = "idea_to_plan";
    }
    // fast→deep: keep slots; do not strip assumed (differs from grill entry switch)
    if (c.phase === "skipped_to_plan" && !hasDraftPlan()) {
      c.skip_requested = false;
      c.phase = "clarifying";
    } else if (
      c.phase !== "claimed_to_plan" &&
      c.phase !== "brief_ready" &&
      c.phase !== "clarifying"
    ) {
      // stay not_started until user engages
      c.skip_requested = false;
    } else if (c.phase === "clarifying" || c.phase === "brief_ready") {
      c.skip_requested = false;
    }
  }

  mirrorAndStash(c);
  paintChatMode();
  if (!opts.silent) {
    // Full messages repaint so clarify card/fold follows entry (no draft from chip)
    repaintChat();
    // §4.2 deep→fast: tell user unanswered slots become assumptions on first send
    if (next === "fast" && prevMode === "deep") {
      const openOrPartial =
        (c.phase === "clarifying" || c.phase === "brief_ready") ||
        (c.slots || []).some(
          (s) => s && s.kind !== "explicit" && String(s.value || s.fill || "").trim() === ""
        ) ||
        (c.slots || []).some((s) => s && s.kind === "assumed");
      if (openOrPartial || (c.slots || []).length === 0) {
        try {
          toast("未答完的按常见假设处理，发第一条消息时会写进计划");
        } catch (_) {}
      }
    }
  }
  return next;
}

/** Alias used on ccoChat desk. */
export const setMode = setChatMode;

/**
 * Fast first-send hook (§4.5): if fast && no draft && not yet skip →
 * applySkipWithAssumptionsLocal + mirror; **no** claimBriefToPlan.
 * Call at the very start of sendChatMessage (before busy / network).
 * @returns {boolean} true if skip was applied this call
 */
export function prepareFastSendIfNeeded() {
  ensureChatState();
  ensureClarifyState();
  const mode = getChatMode();
  if (mode !== "fast") return false;
  const c = state.chatClarify;
  if (!c || typeof c !== "object") return false;
  if (c.phase === "claimed_to_plan") return false;
  if (hasDraftPlan()) return false;
  // Already skipped this session turn
  if (c.skip_requested && c.phase === "skipped_to_plan") return false;

  applySkipWithAssumptionsLocal(c, FAST_SKIP_NOTE);
  c._touchAt = Date.now();
  mirrorAndStash(c);
  return true;
}

/**
 * deep 内次级：「先只想清楚」→ selectClarifyEntry('think_first')（可停 Brief）
 * 仅 deep 且 phase 合适时展示。
 */
function shouldShowThinkFirstLink(c) {
  if (!c || getChatMode() !== "deep") return false;
  const phase = c.phase || "not_started";
  return (
    phase === "not_started" ||
    phase === "clarifying" ||
    phase === "brief_ready"
  );
}

/**
 * Escape / secondary links inside clarify panel (replaces three-entry main row).
 * 「直接写计划」uses data-clarify-entry=plan_only so existing handler claims (逃生舱).
 * @param {object} c
 * @param {{ disabled?: boolean }} [opts]
 */
export function renderClarifySecondaryHtml(c, opts = {}) {
  if (!c || c.phase === "claimed_to_plan") return "";
  // After skip/plan_only ready, no need for escape
  if (c.entry === "plan_only" && c.skip_requested) return "";
  const dis = opts.disabled ? " disabled" : "";
  return (
    `<div class="chat-clarify-moreways" data-clarify-moreways="1">` +
    `<button type="button" class="linkish muted chat-mode-escape"` +
    ` data-clarify-entry="plan_only"` +
    ` title="跳过追问，立刻出一版草稿（显式要现在写）"${dis}>` +
    `直接写计划` +
    `</button>` +
    `</div>`
  );
}

function modeBarHtml(c, mode) {
  const busy = !!state.chatBusy || !!c?._claimBusy;
  const dis = busy ? " disabled" : "";
  const chips = (["fast", "deep"])
    .map((id) => {
      const meta = CHAT_MODES[id];
      const active = mode === id ? " is-active" : "";
      const pressed = mode === id ? "true" : "false";
      return (
        `<button type="button" class="chat-mode-chip${active}"` +
        ` data-chat-mode="${meta.id}"` +
        ` title="${chatEsc(meta.hint)}"` +
        ` aria-pressed="${pressed}"${dis}>` +
        `${chatEsc(meta.label)}` +
        `</button>`
      );
    })
    .join("");

  let side = "";
  if (shouldShowThinkFirstLink(c) && !busy) {
    side =
      `<div class="chat-mode-side">` +
      `<button type="button" class="linkish muted chat-mode-link"` +
      ` data-chat-mode-action="think_first"` +
      ` title="先整理一页摘要，先不写成计划">` +
      `先只想清楚` +
      `</button>` +
      `</div>`;
  }

  return (
    `<div class="chat-mode-chips" role="group" aria-label="计划方式">` +
    chips +
    `</div>` +
    side
  );
}

function ensureModeBarEl() {
  if (typeof document === "undefined") return null;
  let bar = document.getElementById(MODE_BAR_ID);
  if (bar) return bar;
  const composer =
    document.querySelector(".chat-composer") || $("#chat-input")?.closest?.(".chat-composer");
  if (!composer || !composer.parentNode) return null;
  bar = document.createElement("div");
  bar.id = MODE_BAR_ID;
  bar.className = "chat-mode-bar";
  bar.setAttribute("data-chat-mode-bar", "1");
  composer.parentNode.insertBefore(bar, composer);
  return bar;
}

/**
 * Paint / update mode chips above composer.
 * Safe to call often; no-op without DOM.
 */
export function paintChatMode() {
  if (typeof document === "undefined") return;
  ensureChatState();
  ensureClarifyState();
  ensureModeDefault();
  const c = state.chatClarify;
  const mode = getChatMode();
  const bar = ensureModeBarEl();
  if (!bar) return;

  // Hide only when no project selected (composer locked)
  if (!state.selectedPath) {
    bar.hidden = true;
    bar.innerHTML = "";
    return;
  }
  bar.hidden = false;
  bar.dataset.chatMode = mode;
  bar.dataset.clarifyEntry = c?.entry || "";
  bar.innerHTML = modeBarHtml(c, mode);
}

/**
 * One-time click binding for mode chips / think_first (not plan_only claim path).
 */
export function installChatModeUi() {
  if (_modeClickBound || typeof document === "undefined") return;
  _modeClickBound = true;
  document.addEventListener(
    "click",
    (e) => {
      const t = e.target;
      if (!t || typeof t.closest !== "function") return;

      const modeBtn = t.closest("[data-chat-mode]");
      if (modeBtn && modeBtn.closest(`#${MODE_BAR_ID}, .chat-mode-bar`)) {
        e.preventDefault();
        e.stopPropagation();
        if (modeBtn.disabled) return;
        const id = modeBtn.getAttribute("data-chat-mode");
        setChatMode(id === "fast" ? "fast" : "deep");
        return;
      }

      const actionBtn = t.closest("[data-chat-mode-action]");
      if (actionBtn) {
        e.preventDefault();
        e.stopPropagation();
        if (actionBtn.disabled) return;
        const action = actionBtn.getAttribute("data-chat-mode-action");
        if (action === "think_first") {
          // Grill path only — no claim
          selectClarifyEntry("think_first");
          paintChatMode();
        }
      }
    },
    true
  );
}

/** @deprecated use setChatMode — kept for desk naming symmetry */
export function paintChatModeBar() {
  paintChatMode();
}
