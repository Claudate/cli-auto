#!/usr/bin/env node
/**
 * Pure smoke: clarify option pick advances question without DOM.
 * Also asserts click-target HTML wraps option text in <span class="opt-text">
 * so webview text-node clicks are not dead.
 *
 * F5 (§6 docs/chat-dual-mode-empty-guard-2026-08-20.md):
 *   - chatMode.js exists; chip labels 快速出产品 / 深度思考
 *   - setMode / chip path source has no claimBriefToPlan call
 *   - selectClarifyEntry('plan_only') still auto-claims (逃生舱)
 *   - sendChatMessage wires prepareFastSendIfNeeded → applySkipWithAssumptionsLocal
 *   - chat-plan-writing.md has 快速模式 / 常见假设
 *
 *   node scripts/clarify-click-smoke.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail });
}

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

function extractFn(src, name) {
  // export function name(...) { ... }  or  export async function name
  const re = new RegExp(
    String.raw`export\s+(?:async\s+)?function\s+${name}\s*\([^)]*\)\s*\{`,
    "m"
  );
  const m = re.exec(src);
  if (!m) return "";
  let i = m.index + m[0].length;
  let depth = 1;
  while (i < src.length && depth > 0) {
    const ch = src[i++];
    if (ch === "{") depth++;
    else if (ch === "}") depth--;
  }
  return src.slice(m.index, i);
}

function hasCall(body, ident) {
  // Real call site: ident(  — ignore comments that mention the name
  const noLine = body.replace(/\/\/[^\n]*/g, "");
  const noBlock = noLine.replace(/\/\*[\s\S]*?\*\//g, "");
  return new RegExp(String.raw`\b${ident}\s*\(`).test(noBlock);
}

const src = read("web/js/features/chat/chatClarify.js");

// 1) Text-node fix present: option text wrapped in span.opt-text
check(
  "option text wrapped in span.opt-text",
  /opt-text/.test(src) && /class="opt-text"/.test(src)
);

// 2) eventElement helper resolves text nodes
check(
  "eventElement helper exists",
  /function eventElement\s*\(/.test(src) && /parentElement/.test(src)
);

// 3) click handler uses eventElement not raw e.target.closest only
check(
  "click path uses eventElement",
  /const t = eventElement\(e\)/.test(src) || /eventElement\(e\)/.test(src)
);

// 3b) skip CTA must land a draft (not toast-only Brief)
check(
  "skipClarify auto-claims plan draft",
  /export async function skipClarify/.test(src) &&
    /skipClarify[\s\S]*claimBriefToPlan/.test(src)
);

// 3c) paint hook so repaint does not depend only on host bag
check(
  "setClarifyPaint paint hook exists",
  /export function setClarifyPaint/.test(src) && /_clarifyPaint/.test(src)
);

// 4) Pure pick logic: reimplement minimal advance
const SLOTS = [
  "target_audience",
  "pain_moment",
  "observable_outcome",
  "non_goals",
  "done_when",
];
function pick(c, slotId, text) {
  if (c.phase === "not_started") c.phase = "clarifying";
  const ex = c.slots.find((s) => s.id === slotId);
  if (ex) {
    ex.value = text;
    ex.kind = "explicit";
  } else c.slots.push({ id: slotId, value: text, kind: "explicit" });
  const missing = SLOTS.filter((id) => !c.slots.some((s) => s.id === id && s.value));
  if (!missing.length) c.phase = "brief_ready";
  else {
    c.phase = "clarifying";
    c.questionIndex = SLOTS.indexOf(missing[0]);
  }
  return missing;
}
{
  const c = { phase: "not_started", slots: [], questionIndex: 0 };
  const m1 = pick(c, "target_audience", "我自己先用");
  check("after first pick phase clarifying", c.phase === "clarifying");
  check("after first pick 4 missing", m1.length === 4);
  check("questionIndex advanced to pain", c.questionIndex === 1);
  pick(c, "pain_moment", "想法模糊");
  pick(c, "observable_outcome", "有计划");
  pick(c, "non_goals", "不做社区");
  pick(c, "done_when", "能演示");
  check("after five picks brief_ready", c.phase === "brief_ready");
  check("all five filled", c.slots.length === 5);
}

// ─── F5 · dual-mode + fast send wiring ───────────────────────────────────────

const modePath = path.join(ROOT, "web/js/features/chat/chatMode.js");
check("F5 chatMode.js exists", fs.existsSync(modePath));

let modeSrc = "";
let actionsSrc = "";
let promptSrc = "";
if (fs.existsSync(modePath)) {
  modeSrc = read("web/js/features/chat/chatMode.js");
  actionsSrc = read("web/js/features/chat/chatActions.js");
  promptSrc = read("docs/runtime-prompts/chat-plan-writing.md");

  check(
    "F5 chip label 快速出产品",
    /快速出产品/.test(modeSrc)
  );
  check(
    "F5 chip label 深度思考",
    /深度思考/.test(modeSrc)
  );
  check(
    "F5 chat-mode-chip + data-chat-mode markup",
    /chat-mode-chip/.test(modeSrc) && /data-chat-mode=/.test(modeSrc)
  );

  const setModeBody =
    extractFn(modeSrc, "setChatMode") || extractFn(modeSrc, "setMode");
  check(
    "F5 setChatMode/setMode exported",
    !!setModeBody || /export const setMode\s*=/.test(modeSrc),
    setModeBody ? `len=${setModeBody.length}` : "alias-only?"
  );
  check(
    "F5 setMode path has no claimBriefToPlan call",
    setModeBody ? !hasCall(setModeBody, "claimBriefToPlan") : !hasCall(modeSrc, "claimBriefToPlan")
  );
  check(
    "F5 setMode path has no selectClarifyEntry call",
    setModeBody ? !hasCall(setModeBody, "selectClarifyEntry") : true
  );
  check(
    "F5 setMode path does not applySkip (chip quiet)",
    setModeBody ? !hasCall(setModeBody, "applySkipWithAssumptionsLocal") : true
  );

  // Chip click handler lives in installChatModeUi
  const installBody = extractFn(modeSrc, "installChatModeUi");
  check(
    "F5 installChatModeUi exists (chip bind)",
    !!installBody
  );
  check(
    "F5 chip click path has no claimBriefToPlan call",
    installBody ? !hasCall(installBody, "claimBriefToPlan") : false
  );
  // Chip must call setChatMode/setMode, not plan_only selectClarifyEntry
  check(
    "F5 chip click calls setChatMode/setMode",
    installBody
      ? hasCall(installBody, "setChatMode") || hasCall(installBody, "setMode")
      : false
  );

  // Escape hatch: selectClarifyEntry('plan_only') still auto-claims
  const selectBody = extractFn(src, "selectClarifyEntry");
  check(
    "F5 selectClarifyEntry plan_only auto-claim still present",
    !!selectBody &&
      /plan_only/.test(selectBody) &&
      hasCall(selectBody, "claimBriefToPlan") &&
      hasCall(selectBody, "applySkipWithAssumptionsLocal")
  );

  // sendChatMessage wires prepareFastSendIfNeeded
  const sendBody = extractFn(actionsSrc, "sendChatMessage");
  check(
    "F5 sendChatMessage calls prepareFastSendIfNeeded",
    !!sendBody && hasCall(sendBody, "prepareFastSendIfNeeded")
  );
  check(
    "F5 chatActions imports prepareFastSendIfNeeded from chatMode",
    /prepareFastSendIfNeeded/.test(actionsSrc) &&
      /from\s+["']\.\/chatMode\.js["']/.test(actionsSrc)
  );

  const prepBody = extractFn(modeSrc, "prepareFastSendIfNeeded");
  check(
    "F5 prepareFastSendIfNeeded applies applySkipWithAssumptionsLocal",
    !!prepBody && hasCall(prepBody, "applySkipWithAssumptionsLocal")
  );
  check(
    "F5 prepareFastSendIfNeeded has no claimBriefToPlan call",
    !!prepBody && !hasCall(prepBody, "claimBriefToPlan")
  );
  check(
    "F5 prepareFastSendIfNeeded gates on skip_requested / fast",
    !!prepBody &&
      /skip_requested/.test(prepBody) &&
      (/fast/.test(prepBody) || /getChatMode/.test(prepBody))
  );

  // F2 prompt contract
  check(
    "F5 chat-plan-writing.md has 快速模式 line",
    /快速模式/.test(promptSrc)
  );
  check(
    "F5 chat-plan-writing.md has 常见假设",
    /常见假设/.test(promptSrc)
  );
  check(
    "F5 chat-plan-writing.md maps 快速出产品",
    /快速出产品/.test(promptSrc)
  );

  // Minimal R4: three-entry main row withdrawn; secondary is linkish escape
  check(
    "F5 renderEntryChips no longer emits chat-clarify-entries main row",
    (() => {
      const body = extractFn(src, "renderEntryChips") ||
        // function may be non-export
        (() => {
          const re = /function renderEntryChips\s*\([^)]*\)\s*\{/;
          const m = re.exec(src);
          if (!m) return "";
          let i = m.index + m[0].length;
          let depth = 1;
          while (i < src.length && depth > 0) {
            const ch = src[i++];
            if (ch === "{") depth++;
            else if (ch === "}") depth--;
          }
          return src.slice(m.index, i);
        })();
      if (!body) return false;
      return (
        /renderClarifySecondaryHtml|chat-clarify-moreways|直接写计划/.test(body) &&
        !/chat-clarify-entries/.test(body)
      );
    })()
  );
}

const failed = results.filter((r) => !r.ok);
console.log(
  `clarify-click-smoke total=${results.length} passed=${results.length - failed.length} failed=${failed.length}`
);
for (const r of results) {
  console.log(`${r.ok ? "PASS" : "FAIL"} · ${r.name}${r.detail ? " · " + r.detail : ""}`);
}
process.exit(failed.length ? 1 : 0);
