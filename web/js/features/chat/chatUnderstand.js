/**
 * [INPUT]: chat session draft / plan md / clarify slots
 * [OUTPUT]: 「当前理解」短条 HTML + 反馈主动作文案
 * [POS]: features/chat — W1 边聊/反馈（02-iterate-clarity）
 * [PROTOCOL]: 变更对照 docs/path-depth-wave-2026-07-28/02；不 spawn / 不 confirm
 *
 * 对外人话：陪说清楚 / 按我说的改 / 这版去拆步。禁止教 P 代号。
 */

import { chatPlanThreeLines, chatEsc, chatExtractPlanFence } from "./chatFormat.js";

/**
 * @param {object} state legacy state bag
 * @returns {{ who: string, goal: string, nonGoals: string, source: string }|null}
 */
export function extractUnderstanding(state) {
  if (!state?.chatSession) return null;
  const sess = state.chatSession;
  const clarify = state.chatClarify || sess.clarify || null;

  let who = "";
  let goal = "";
  let nonGoals = "";

  // 1) clarify slots (边聊中)
  if (clarify && Array.isArray(clarify.slots)) {
    for (const s of clarify.slots) {
      if (!s || !s.value) continue;
      const id = String(s.id || "");
      const v = String(s.value || s.label || "").trim();
      if (!v) continue;
      if (id === "target_audience" || /给谁/.test(String(s.label || ""))) who = v;
      if (id === "observable_outcome" || /做成/.test(String(s.label || ""))) goal = v;
      if (id === "non_goals" || /不做/.test(String(s.label || ""))) nonGoals = v;
    }
  }

  // 2) draft / last plan fence
  let md = String(sess.draft_plan?.markdown || "").trim();
  if (!md && Array.isArray(sess.messages)) {
    for (let i = sess.messages.length - 1; i >= 0; i--) {
      const m = sess.messages[i];
      if (!m || m.role !== "assistant") continue;
      const body = chatExtractPlanFence(m.content || "");
      if (body) {
        md = body;
        break;
      }
    }
  }
  if (md) {
    const lines = chatPlanThreeLines(md);
    if (!goal && lines.goal && lines.goal !== "（待补）") goal = lines.goal;
    if (!nonGoals && lines.nonGoals && lines.nonGoals !== "（待补）") {
      nonGoals = lines.nonGoals;
    }
    // crude who from 目标 first clause（给X… → X，止于的/做/用）
    if (!who && lines.goal && lines.goal !== "（待补）") {
      const m = lines.goal.match(/给([^，,。；;\s的做用]{1,12})/);
      if (m) who = m[1].trim();
    }
  }

  // 3) last user message fallback for goal
  if (!goal && Array.isArray(sess.messages)) {
    for (let i = sess.messages.length - 1; i >= 0; i--) {
      const m = sess.messages[i];
      if (m?.role === "user" && String(m.content || "").trim()) {
        const t = String(m.content).trim().replace(/\s+/g, " ");
        goal = t.length > 72 ? t.slice(0, 70) + "…" : t;
        break;
      }
    }
  }

  if (!who && !goal && !nonGoals) return null;
  return {
    who: who || "（还在聊）",
    goal: goal || "（还在聊）",
    nonGoals: nonGoals || "（未写）",
    source: md ? "plan" : clarify ? "clarify" : "chat",
  };
}

/**
 * Sticky-ish understanding strip — shown when conversation started.
 * @param {object} state
 */
export function renderUnderstandingBarHtml(state) {
  const u = extractUnderstanding(state);
  if (!u) return "";
  return (
    `<div class="chat-understand" role="status" data-understand="1">` +
    `<div class="chat-understand-head">` +
    `<span class="chat-understand-title">当前理解</span>` +
    `<span class="chat-understand-hint muted">不对就改下面输入，或点「按我说的改」</span>` +
    `</div>` +
    `<ul class="chat-understand-lines">` +
    `<li><span class="k">给谁</span> ${chatEsc(u.who)}</li>` +
    `<li><span class="k">做成什么</span> ${chatEsc(u.goal)}</li>` +
    `<li><span class="k">不做</span> ${chatEsc(u.nonGoals)}</li>` +
    `</ul>` +
    `</div>`
  );
}

/**
 * Feedback primary actions for plan card (02 · W0-4 / W1).
 * Main assign still rendered by chatFormat; this is the human revise row.
 * @param {{ canAssign?: boolean }} [opts]
 */
export function planFeedbackActionsHtml(opts = {}) {
  const can = opts.canAssign !== false;
  return (
    `<div class="chat-plan-feedback" role="group" aria-label="对这版草稿">` +
    `<button type="button" class="btn ghost sm" data-chat-revise="1" title="继续在输入框说明哪里不对，再生成一版">按我说的改</button>` +
    `<button type="button" class="btn ghost sm" data-chat-pivot="1" title="换目标或范围，输入新方向">换个方向</button>` +
    (can
      ? `<span class="chat-plan-feedback-note muted">对了就点下方主按钮去拆步</span>`
      : "") +
    `</div>`
  );
}

/** Focus composer with revise coach placeholder. */
export function beginReviseInComposer(kind = "revise") {
  if (typeof document === "undefined") return;
  const input = document.getElementById("chat-input");
  if (!input) return;
  if (kind === "pivot") {
    input.setAttribute(
      "placeholder",
      "换个方向：新的给谁 / 做成什么 / 先不做啥？（发送后按新说法重写草稿）"
    );
  } else {
    input.setAttribute(
      "placeholder",
      "按我说的改：哪里不对？例如合并两步、主 CTA 改成…、先不做登录"
    );
  }
  input.focus();
  try {
    input.scrollIntoView({ block: "nearest", behavior: "smooth" });
  } catch (_) {}
}
