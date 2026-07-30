/**
 * [INPUT]: state.ensureClarifyState · chatFormat.chatEsc · CLARIFY_COPY/ENTRIES/SLOT_QUESTIONS
 * [OUTPUT]: Brief 构建 + Claim 执行的核心逻辑（不含 UI 渲染）
 * [POS]: t3+t4 features/chat/clarify/briefAndClaim.js — Brief/Claim 子集纵切第一刀
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 功能说明：
 * - 数据构建层：从 clarify 状态构建 Brief VM、检测空心缺口、生成计划 Markdown
 * - UI 决策层：判断何时显示 Brief 面板
 * - Action 层：rechat 重新编辑、确保 claim 消息可见、claim brief 写成计划
 * - 与原文件关系：从 chatClarify.js 搬移而来，仅读 ensureClarifyState，禁止反向依赖
 *
 * 导出清单：
 * - deriveEvidenceTags(c) — 从澄清状态推导证据标签
 * - buildBriefFromClarify(c) — 构建 Brief 视图模型
 * - detectHollowGaps(c, planMd) — 检测空心缺口（黄条提示）
 * - buildPlanMarkdownFromBrief(c, brief) — 从 Brief 生成计划 Markdown
 * - shouldShowBrief(c) — 是否显示 Brief 面板
 * - rechatFromBrief() — 重新编辑 Brief
 * - ensureClaimDraftMessageVisible() — 确保 claim 草稿消息可见
 * - claimBriefToPlan() — 将 Brief 认领写成计划草稿
 */

import { state, $, toast } from "../legacy.js";
import { chatEsc } from "../chatFormat.js";
import { stashChatSession, ensureChatState, sanitizePlanTitle } from "../chatState.js";
import { getPlansDir } from "../planDir.js";
import * as chatApi from "../chatApi.js";
import { host } from "../host.js";

// ─── Copy contract (product; first sentence human · non-dev tone) ────
// Internal wire ids (entry/slot/phase) stay English; UI labels stay business Chinese.
// Same-screen concept budget ≤3: ① 怎么开始 ② 当前这一问 ③ 可跳过。

// These constants are imported from chatClarify.js via re-export below
export const CLARIFY_COPY = null; // Placeholder - will be set by re-export
export const CLARIFY_ENTRIES = [];
export const DEFAULT_CLARIFY_ENTRY = "idea_to_plan";
export const SLOT_LABEL = {};
export const CLARIFY_SLOT_QUESTIONS = [];

/**
 * deriveEvidenceTags — 从澄清状态推导证据标签
 * 搬自 chatClarify.js line 994-1029
 * @param {any} c clarify state
 * @returns {string[]}
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
 * buildBriefFromClarify — 构建 Brief 视图模型
 * 搬自 chatClarify.js line 1031-1109
 * Eight fields: 问题 · 给谁 · 做/不做 · 得/失 · 证据 · 未决 · 验收 · V1
 * @param {any} c clarify state
 * @returns {any} Brief view model
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
 * detectHollowGaps — 检测空心缺口（黄条提示）
 * 搬自 chatClarify.js line 1110-1219
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

/**
 * Internal helper: extract section body from markdown
 * 搬自 chatClarify.js line 1153-1171 (internal)
 */
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

/**
 * Internal helper: check if section body is just a stub
 * 搬自 chatClarify.js line 1173-1211 (internal)
 */
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
    (l) => !/^[-*+]\s*说明 [：:]/.test(l) && !/^说明 [：:]/.test(l)
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
 * buildPlanMarkdownFromBrief — 从 Brief 生成计划 Markdown
 * 搬自 chatClarify.js line 1220-1331
 * 搬移注意：此函数调用 CLARIFY_ENTRIES、SLOT_LABEL、sanitizePlanTitle、detectHollowGaps
 * @param {any} c clarify state
 * @param {any} brief brief view model
 * @returns {string} markdown
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
 * shouldShowBrief — 判断是否显示 Brief 面板
 * 搬自 chatClarify.js line 1332-1346
 * @param {any} c clarify state
 * @returns {boolean}
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
 * setClarifyPaint — 注册 paint 回调（briefAndClaim 内使用）
 * 搬自 chatClarify.js line 1358-1360
 * @param {(() => void) | null} fn
 */
export function setClarifyPaint(fn) {
  _clarifyPaint = typeof fn === "function" ? fn : null;
}

/**
 * repaint — 重绘聊天气泡（内部工具函数）
 * 搬自 chatClarify.js line 1362-1395
 */
function repaint() {
  // Prefer messages-only paint; fall through on throw so skip never looks dead.
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

/**
 * selectClarifyEntry — 选择澄清入口（briefAndClaim 间接调用）
 * 搬自 chatClarify.js line 1397-1450
 * @param {string} entryId
 */
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
 * pickClarifyOption — 点击选项（briefAndClaim 间接调用）
 * 搬自 chatClarify.js line 1456-1569
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

/**
 * mirrorClarifyToSession — 镜像 clarify 状态到 session（内部工具）
 * 搬自 chatClarify.js line 1572-1578
 * @param {any} c clarify state
 * @returns {*}
 */
function mirrorClarifyToSession(c) {
  const wire = clarifyToWire(c);
  if (state.chatSession && typeof state.chatSession === "object") {
    state.chatSession.clarify = wire;
  }
  return wire;
}

/**
 * skipClarify — 跳过澄清（briefAndClaim 间接调用）
 * 搬自 chatClarify.js line 1587-1617
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

/**
 * setClarifyUiStatus — 设置 UI 状态（加载/错误模拟）
 * 搬自 chatClarify.js line 1620-1627
 * @param {string} status
 * @param {string} errorText
 */
export function setClarifyUiStatus(status, errorText) {
  ensureClarifyState();
  const c = state.chatClarify;
  const s = String(status || "idle");
  c.uiStatus = ["idle", "loading", "error", "empty"].includes(s) ? s : "idle";
  c.errorText = errorText != null ? String(errorText) : null;
  if (state.chatSession) state.chatSession.clarify = clarifyToWire(c);
}

/**
 * clearClarifyError — 清除错误提示
 * 搬自 chatClarify.js line 1629-1634
 */
export function clearClarifyError() {
  ensureClarifyState();
  state.chatClarify.uiStatus = "idle";
  state.chatClarify.errorText = null;
  repaint();
}

/**
 * rechatFromBrief — 从 Brief 重新编辑
 * 搬自 chatClarify.js line 1640-1662
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
 * ensureClaimDraftMessageVisible — 确保 claim 草稿消息可见
 * 搬自 chatClarify.js line 1674-1717
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
 * claimBriefToPlan — 将 Brief 认领写成计划草稿
 * 搬自 chatClarify.js line 1750-1874
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
      // Disk save failed — keep session draft; user can「仅保存」later
      console.warn("claimBriefToPlan: save_plan failed; session draft kept", e);
      toast("计划草稿已在会话中；落盘稍后可点「仅保存」");
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
      // Also plant a plan fence as assistant bubble so plan card appears
      const planFence = "```plan\n" + md.trim() + "\n```";
      msgs.push({
        role: "assistant",
        content: `${note}\n\n${planFence}`,
      });
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

// ─── Helper functions for briefAndClaim (internal to this module) ────────────

/**
 * isAssumedOrPlaceholder — check if fill is assumed or placeholder text
 * @param {string} value
 * @param {string} kind
 * @returns {boolean}
 */
function isAssumedOrPlaceholder(value, kind) {
  if (kind === "assumed") return true;
  const v = String(value || "").trim().toLowerCase();
  // Claim-time placeholders in slot values
  return /^(（待|\(待|待补|待写|待定|帮我选|TBD|TODO)/.test(v);
}

/**
 * slotValue — fetch slot value by id
 * @param {any} c clarify state
 * @param {string} slotId
 * @returns {string}
 */
function slotValue(c, slotId) {
  const s = (c?.slots || []).find((x) => x.id === slotId);
  return s ? String(s.value || "").trim() : "";
}

/**
 * slotKind — fetch slot kind by id
 * @param {any} c clarify state
 * @param {string} slotId
 * @returns {string}
 */
function slotKind(c, slotId) {
  const s = (c?.slots || []).find((x) => x.id === slotId);
  return s ? s.kind : "missing";
}

/**
 * isEffectivelyMissing — check if slot is effectively missing (empty or assumed placeholder)
 * @param {string} value
 * @param {string} kind
 * @returns {boolean}
 */
function isEffectivelyMissing(value, kind) {
  if (!value || !String(value).trim()) return true;
  return isAssumedOrPlaceholder(value, kind);
}

/**
 * currentQuestion — get current question index
 * @param {any} c clarify state
 * @returns {any|null}
 */
function currentQuestion(c) {
  const idx = Math.max(
    0,
    Number(c?.questionIndex) || 0
  );
  return (CLARIFY_SLOT_QUESTIONS[idx] || null);
}

/**
 * applySkipWithAssumptionsLocal — apply skip with assumptions locally
 * @param {any} c clarify state
 * @param {string} note
 */
function applySkipWithAssumptionsLocal(c, note) {
  c.skip_requested = true;
  c.phase = "skipped_to_plan";
  // Add assumed placeholders for all missing required slots
  const missing = missingRequiredSlots(c);
  for (const id of missing) {
    const label = SLOT_LABEL[id] || id;
    c.slots.push({
      id,
      value: `（假设）${label}未填写`,
      kind: "assumed",
    });
  }
}

/**
 * clarifyToWire — serialize clarify state to wire shape
 * @param {any} c clarify state
 * @returns {any}
 */
function clarifyToWire(c) {
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
 * normalizeEntry — normalize entry string to enum
 * @param {string} raw
 * @returns {string}
 */
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

/**
 * reconcileClaimedPhase — reconcile claimed phase with reality
 */
function reconcileClaimedPhase() {
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

/**
 * ensureClarifyState — ensure clarify state exists (read-only access per constraints)
 * CRITICAL: Per task constraints, briefAndClaim can ONLY read this, not call other business logic
 * This is the only allowed external dependency
 */
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

/**
 * defaultClarifyState — create new clarify state
 * @param {string} [entry]
 * @returns {any}
 */
function defaultClarifyState(entry) {
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
