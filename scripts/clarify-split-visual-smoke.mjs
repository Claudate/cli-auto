#!/usr/bin/env node
/**
 * Static visual contract smoke for clarify + split replan residual.
 *
 * Does **not** replace a packaged App 30s finger-test — locks DOM/JS wiring so
 * the residual path cannot silently regress (entries · claim · replan notes · risk).
 *
 *   node scripts/clarify-split-visual-smoke.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail: detail || "" });
}
function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

const html = read("web/index.html");
const clarify = read("web/js/features/chat/chatClarify.js");
const claimOps = (() => {
  try {
    return read("web/js/features/chat/chatPlanOps.js");
  } catch {
    return "";
  }
})();
const confirmActions = read("web/js/features/project/confirmActions.js");
const jobPoll = read("web/js/features/project/jobPoll.js");
const splitRender = read("web/js/features/split/splitRender.js");
const splitFill = read("web/js/features/split/splitFillMeta.js");
const planCss = read("web/css/plan.css");

// ── Clarify three entries + claim ──────────────────────────────────────────
check(
  "HTML or clarify has three entry routes",
  /think_first|idea_to_plan|plan_only/.test(clarify) &&
    /想清楚再说|从想法到计划|已想清/.test(clarify)
);
check(
  "claim CTA 认领并写成计划",
  /认领并写成计划/.test(clarify) || /认领并写成计划/.test(html)
);
check(
  "claim path does not call confirmStart/confirm_start",
  !/confirmStart|confirm_start/.test(
    clarify.slice(
      Math.max(0, clarify.indexOf("claim") - 200),
      clarify.indexOf("claim") + 2500
    )
  ) ||
    (!/confirm_start/.test(clarify) && !/confirmStart\(/.test(clarify))
);
check(
  "claim-boundary script still present",
  fs.existsSync(path.join(ROOT, "scripts/claim-boundary-check.mjs"))
);

// ── Split replan + revision notes ───────────────────────────────────────────
check(
  "split-revision-notes input in index.html",
  /id="split-revision-notes"/.test(html) && /重拆意见|revision/.test(html)
);
check(
  "btn-replan present",
  /id="btn-replan"/.test(html) && /重新规划/.test(html)
);
check(
  "jobPoll sends revision_notes",
  /revision_notes/.test(jobPoll)
);
check(
  "confirmActions replan toast mentions feedback path",
  /replanFromConfirm/.test(confirmActions) &&
    (/revision|反馈|重拆/.test(confirmActions) ||
      /analyzePlanFromPicker/.test(confirmActions))
);
check(
  "risk badge CSS present",
  /\.risk-badge/.test(planCss) &&
    /\.risk-external/.test(planCss) &&
    /\.risk-write_local/.test(planCss)
);
check(
  "splitRender paints risk chip",
  /risk_class|riskClass/.test(splitRender) && /risk-badge/.test(splitRender)
);
check(
  "splitFillMeta external confirm hint",
  /会外发|不推远端|含外发/.test(splitFill)
);

// ── Ensure V3 still wired (cross-link residual closeout) ────────────────────
check(
  "ensure-v3 smoke script exists",
  fs.existsSync(path.join(ROOT, "scripts/ensure-v3-cta-smoke.mjs"))
);

const failed = results.filter((r) => !r.ok);
for (const r of results) {
  console.log(`${r.ok ? "PASS" : "FAIL"} · ${r.name}${r.detail ? " · " + r.detail : ""}`);
}
console.log(
  `clarify-split-visual-smoke total=${results.length} passed=${
    results.length - failed.length
  } failed=${failed.length}`
);
process.exit(failed.length ? 1 : 0);
