/**
 * [INPUT]: legacy.state · host.saveChatPlan · shared/markdown · planSplit index/job
 * [OUTPUT]: fence parse · plan cards · format · expand/adopt · already-split → 查看拆分结果
 * [POS]: A5-2a features/chat/chatFormat.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { state, toast, hasActiveRun, normalizePlanPath } from "./legacy.js";
import { host } from "./host.js";
import { ensureChatState, stashChatSession } from "./chatState.js";
import { renderMarkdown } from "../../shared/markdown.js";
import {
  personaPrimaryCtaLabel,
  personaDirectExec,
  getPersonaProfile,
} from "./chatPersona.js";
import { planFeedbackActionsHtml } from "./chatUnderstand.js";

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

/**
 * W0: three human lines for draft card — 做成什么 / 不做 / 怎样算完.
 * @returns {{ goal: string, nonGoals: string, doneWhen: string }}
 */
export function chatPlanThreeLines(md) {
  const text = String(md || "").replace(/\r\n/g, "\n");
  const sections = {};
  let cur = null;
  for (const raw of text.split("\n")) {
    const h = raw.match(/^##\s+(.+?)\s*$/);
    if (h) {
      cur = h[1].trim();
      if (!sections[cur]) sections[cur] = [];
      continue;
    }
    if (!cur) continue;
    const t = raw.trim();
    if (!t || t.startsWith("#") || t.startsWith("```")) continue;
    sections[cur].push(t.replace(/^[-*•]\s+/, "").replace(/^\d+[\.\)]\s+/, ""));
  }
  const pick = (keys) => {
    for (const k of keys) {
      for (const name of Object.keys(sections)) {
        if (name === k || name.includes(k)) {
          const body = (sections[name] || []).filter(Boolean);
          if (body.length) {
            const s = body.slice(0, 2).join("；");
            return s.length > 96 ? s.slice(0, 94) + "…" : s;
          }
        }
      }
    }
    return "";
  };
  return {
    goal: pick(["目标", "做成", "给谁"]) || "（待补）",
    nonGoals: pick(["非目标", "不做", "先不做"]) || "（待补）",
    doneWhen: pick(["验收", "成功标准", "怎样算", "做完"]) || "（待补）",
  };
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
 * Whether this plan path already has a restorable / live split result.
 * Mirrors plansMgmt「查看拆分结果」gate: SQLite index or in-memory planJob match.
 * @param {string} planPath
 * @returns {boolean}
 */
export function chatPlanPathHasSplit(planPath) {
  if (!planPath) return false;
  const root = state.selectedPath || "";
  const path = String(planPath);
  const norm =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(path, root) || path
      : path;
  // project host is a separate bag — prefer window.planSplitForPath (installProject)
  const splitFn =
    (typeof window !== "undefined" && window.planSplitForPath) ||
    host.planSplitForPath ||
    null;
  if (typeof splitFn === "function" && splitFn(path, root)) return true;
  const by = state.planSplitByPath || {};
  if (by[path] || by[norm] || (norm && by[norm.split("/").pop()])) return true;
  const job = state.planJob;
  if (!job) return false;
  const jobPathRaw = job.plan_path || job.planPath || "";
  if (!jobPathRaw) return false;
  const jobPath =
    typeof normalizePlanPath === "function"
      ? normalizePlanPath(jobPathRaw, root) || jobPathRaw
      : jobPathRaw;
  const st = String(job.status || "").toLowerCase();
  // planning counts: user already started 拆成步骤 for this path
  const pathHit =
    jobPath === path ||
    jobPath === norm ||
    String(jobPathRaw) === String(path) ||
    String(jobPathRaw) === String(norm);
  return (
    pathHit &&
    ["planning", "planned", "confirmed", "running", "done"].includes(st)
  );
}

/**
 * Footer CTAs live on the plan card (not sticky ready-bar).
 * @param {string} md
 * @param {{ active?: boolean }} opts  active=false only for stream partials;
 *   finished assistant cards stay actionable until saved+alreadySplit
 */
export function chatPlanCardActionsHtml(md, opts = {}) {
  ensureChatState();
  const active = opts.active !== false;
  const draft = state.chatSession?.draft_plan;
  const draftKey = chatNormMdKey(draft?.markdown || "");
  const cardKey = chatNormMdKey(md);
  // Card matches session draft only when body fingerprints match.
  // Do NOT treat "active + has path" as match — that paints「已保存：旧路径」on a new plan card.
  const isThisDraft = !!(draftKey && cardKey && draftKey === cardKey);
  const savedPath =
    isThisDraft && draft?.saved && draft?.path
      ? draft.path
      : isThisDraft && state.chatDraftPlan
        ? state.chatDraftPlan
        : null;
  const isSaved = !!(
    isThisDraft &&
    draft?.saved &&
    draft?.path &&
    savedPath
  );
  // Saved + already split → no 仅保存 / 拆成步骤 (desk CTAs live on 拆分台 / 计划管理)
  const alreadySplit = isSaved && chatPlanPathHasSplit(savedPath);
  const busy = !!state.chatBusy;
  const runLocked = typeof hasActiveRun === "function" ? hasActiveRun() : false;

  const expand =
    `<button type="button" class="btn ghost sm btn-chat-plan-expand">展开全文</button>`;

  // Stream partials only: expand while fence incomplete (no save/exec yet)
  if (!active) {
    return (
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `</div>`
    );
  }

  // W0 + W0-7：唯一 primary = 画像 primary_cta；direct_exec 随画像 offer/hide/danger_only
  // 仍走 assign/direct 原链，无 start_run
  const canExec = !runLocked && !busy && !!md;
  const ctaLabel = personaPrimaryCtaLabel();
  const dex = personaDirectExec();
  const assignTitle = runLocked
    ? "运行中，请先停止后再拆分"
    : isSaved
      ? "把计划拆成可执行步骤"
      : "先保存到本机计划，再进入拆分台";
  const directTitle = runLocked
    ? "运行中，请先停止后再执行"
    : dex === "danger_only"
      ? "整份当一步风险较高，仅特殊情况使用"
      : "不拆成多步，整份计划交给一个窗口直接执行（次要）";
  const assignBtn =
    `<button type="button" class="btn primary sm btn-chat-plan-assign" ${canExec ? "" : "disabled"} title="${assignTitle}">${chatEsc(ctaLabel)}</button>`;
  const showDirect = dex === "offer" || dex === "danger_only";
  const directBtn = showDirect
    ? `<button type="button" class="btn ghost sm btn-chat-plan-direct" ${canExec ? "" : "disabled"} title="${directTitle}">直接执行</button>`
    : "";
  if (isSaved && alreadySplit) {
    return (
      `<span class="chat-plan-card-saved muted" title="已保存并拆分；改计划请到计划管理或拆分台「重新规划」">已保存：${chatEsc(savedPath)}</span>` +
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `<button type="button" class="btn ghost sm btn-chat-plan-view-split" data-plan-path="${chatEsc(savedPath)}" title="打开拆分台查看结果">查看拆分结果</button>` +
      `</div>`
    );
  }
  const saveLabel = "这版作数";
  const saveTitle = isSaved
    ? "覆盖保存到本机计划（这版作数）"
    : "保存到本机计划文件 · 这版作数（不会开跑）";
  const feedback = planFeedbackActionsHtml({ canAssign: canExec });
  if (isSaved) {
    return (
      feedback +
      `<span class="chat-plan-card-saved muted">已保存：${chatEsc(savedPath)}</span>` +
      `<div class="chat-plan-card-actions-btns">` +
      expand +
      `<button type="button" class="btn ghost sm btn-chat-plan-adopt" ${busy ? "disabled" : ""} title="${saveTitle}">${saveLabel}</button>` +
      assignBtn +
      directBtn +
      `</div>`
    );
  }

  return (
    feedback +
    `<div class="chat-plan-card-actions-btns">` +
    expand +
    `<button type="button" class="btn ghost sm btn-chat-plan-adopt" ${busy || !md ? "disabled" : ""} title="${saveTitle}">${saveLabel}</button>` +
    assignBtn +
    directBtn +
    `</div>`
  );
}

export function chatFormatPlanCard(rawMd, opts = {}) {
  const md = String(rawMd || "").trim();
  const { title } = chatPlanOutline(md);
  const lines = chatPlanThreeLines(md);
  const persona = getPersonaProfile();
  // Prefer plan body; if stub/empty, surface persona lexicon so ecom≠admin
  const goal = lines.goal !== "（待补）" ? lines.goal : persona.coach;
  const nonGoals =
    lines.nonGoals !== "（待补）" ? lines.nonGoals : persona.nonGoalHint;
  const doneWhen =
    lines.doneWhen !== "（待补）" ? lines.doneWhen : persona.doneWhenHint;
  const threeHtml =
    `<ul class="chat-plan-threelines">` +
    `<li><span class="k">做成什么</span> ${chatEsc(goal)}</li>` +
    `<li><span class="k">不做</span> ${chatEsc(nonGoals)}</li>` +
    `<li><span class="k">怎样算完</span> ${chatEsc(doneWhen)}</li>` +
    `</ul>`;
  // Expand view = rendered markdown; raw kept in hidden pre for adopt/assign.
  return (
    `<div class="chat-plan-card" data-plan-md="1">` +
    `<div class="chat-plan-card-label">计划草稿</div>` +
    `<div class="chat-plan-card-title">${chatEsc(title)}</div>` +
    `<div class="chat-plan-summary">` +
    threeHtml +
    `</div>` +
    `<pre class="chat-plan-raw" hidden>${chatEsc(md)}</pre>` +
    `<div class="chat-plan-full md-body" hidden>${renderMarkdown(md)}</div>` +
    `<div class="chat-plan-card-actions">` +
    chatPlanCardActionsHtml(md, opts) +
    `</div>` +
    `</div>`
  );
}

/** Raw plan markdown from a card (not the rendered expand view). */
export function chatPlanCardRaw(card) {
  if (!card) return "";
  const raw = card.querySelector(".chat-plan-raw");
  if (raw) return String(raw.textContent || "").trim();
  // Legacy: expand used to be a pre with raw source
  const full = card.querySelector(".chat-plan-full");
  if (full && full.tagName === "PRE") return String(full.textContent || "").trim();
  return "";
}

/**
 * @param {string} text
 * @param {{ activePlan?: boolean }} opts  when true, every ```plan fence gets save/exec CTAs
 *   (stream path forces false). Multiple fences in one reply are all actionable.
 */
export function chatFormatBody(text, opts = {}) {
  // Parse fences on raw text first (nesting-aware): plan → card, code → pre,
  // remaining prose → shared renderMarkdown (headings / tables / lists / hr).
  const segs = chatSegmentMarkdown(text);
  const planCount = segs.filter((s) => s.type === "plan").length;
  const hasWaveIndex = /```wave-index\b/i.test(String(text || ""));
  // W2: multi-plan or explicit wave-index → claim bar (save only, no run)
  const waveBar =
    opts.activePlan && (planCount >= 2 || hasWaveIndex)
      ? chatWaveClaimBarHtml(planCount, hasWaveIndex)
      : "";
  const body = segs
    .map((seg) => {
      if (seg.type === "plan") {
        return chatFormatPlanCard(seg.body, {
          active: !!opts.activePlan,
        });
      }
      if (seg.type === "code") {
        const lang = String(seg.lang || "").toLowerCase();
        if (lang === "wave-index") {
          return (
            `<div class="chat-wave-index">` +
            `<div class="chat-wave-index-label">本波索引</div>` +
            `<div class="md-body">${renderMarkdown(seg.body)}</div>` +
            `</div>`
          );
        }
        return `<pre class="chat-code-block">${chatEsc(seg.body)}</pre>`;
      }
      const body = String(seg.body || "");
      // Skip pure whitespace between fences; avoid plan-empty placeholder.
      if (!body.trim()) return "";
      return renderMarkdown(body);
    })
    .join("");
  return waveBar + body;
}

/** W2 claim bar — 认领本波落盘；≠ 开跑. */
function chatWaveClaimBarHtml(planCount, hasIndex) {
  const n = Math.max(0, planCount | 0);
  const tip = hasIndex
    ? n > 0
      ? `保存本波索引 + ${n} 份执行计划到 plans/wave-…/（不会自动开跑）`
      : "保存本波索引到 plans/wave-…/（不会自动开跑）"
    : `保存 ${n} 份执行计划到 plans/wave-…/（不会自动开跑）`;
  const label = hasIndex
    ? n > 1
      ? `认领本波（索引 + ${n} 份计划）`
      : "认领本波索引"
    : `认领本波（${n} 份计划）`;
  return (
    `<div class="chat-wave-claim" role="group" aria-label="本波多计划">` +
    `<p class="chat-wave-claim-hint muted">多件事请分开计划；认领只落盘，下一步再逐份拆步。</p>` +
    `<button type="button" class="btn primary sm" data-chat-wave-claim="1" title="${chatEsc(
      tip
    )}">${chatEsc(label)}</button>` +
    `</div>`
  );
}

/**
 * Streaming partial → rendered markdown (no active plan CTAs).
 * Caps size so a long reply doesn't thrash the DOM every poll tick.
 * @param {string} text
 * @param {{ maxChars?: number }} [opts]
 */
export function chatFormatStreamBody(text, opts = {}) {
  const max = opts.maxChars ?? 6000;
  let src = String(text || "");
  let prefix = "";
  if (src.length > max) {
    // Prefer a clean cut at a line boundary so half-tables don't explode.
    const slice = src.slice(-max);
    const nl = slice.indexOf("\n");
    src = nl >= 0 && nl < 200 ? slice.slice(nl + 1) : slice;
    prefix = `<p class="md-p chat-stream-trunc muted">…</p>`;
  }
  if (!src.trim()) return "";
  // Incomplete ```plan fences stay as text segments → still renderMarkdown.
  return prefix + chatFormatBody(src, { activePlan: false });
}

/** Toggle plan card full markdown (expand/collapse). Expand shows rendered md. */
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
  const md = chatPlanCardRaw(card);
  if (!md) {
    toast("卡片中没有可保存的计划正文");
    return;
  }
  ensureChatState();
  // Seed draft_plan so saveChatPlan uses this markdown.
  // If body diverges from the bound draft, drop path/saved so we do not overwrite
  // an unrelated plan file (new card body ≠ old plan_rel identity).
  const prevKey = chatNormMdKey(state.chatSession?.draft_plan?.markdown || "");
  const nextKey = chatNormMdKey(md);
  if (!state.chatSession.draft_plan) {
    state.chatSession.draft_plan = {
      path: "",
      saved: false,
      markdown: md,
      title: null,
    };
    state.chatDraftPlan = null;
  } else {
    state.chatSession.draft_plan.markdown = md;
    if (prevKey && nextKey && prevKey !== nextKey) {
      state.chatSession.draft_plan.path = "";
      state.chatSession.draft_plan.saved = false;
      state.chatDraftPlan = null;
    }
  }
  stashChatSession(state.selectedPath || state.chatProjectPath);
  return host.saveChatPlan();
}
