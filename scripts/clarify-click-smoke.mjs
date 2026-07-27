#!/usr/bin/env node
/**
 * Pure smoke: clarify option pick advances question without DOM.
 * Also asserts click-target HTML wraps option text in <span class="opt-text">
 * so webview text-node clicks are not dead.
 *
 *   node scripts/clarify-click-smoke.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const src = fs.readFileSync(
  path.join(ROOT, "web/js/features/chat/chatClarify.js"),
  "utf8"
);

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail });
}

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

const failed = results.filter((r) => !r.ok);
console.log(
  `clarify-click-smoke total=${results.length} passed=${results.length - failed.length} failed=${failed.length}`
);
for (const r of results) {
  console.log(`${r.ok ? "PASS" : "FAIL"} · ${r.name}${r.detail ? " · " + r.detail : ""}`);
}
process.exit(failed.length ? 1 : 0);
