#!/usr/bin/env node
/**
 * TRIAL · 认领边界检查（可保留 / 可遗弃）
 *
 * 用途：验证「Brief 认领」只写草稿、不旁路开跑；黄条不硬拦 claim。
 * 范围：**停在认领** —— 不测拆分台、confirm、run、worker、GUI 点击。
 *
 * 跑法：
 *   node scripts/claim-boundary-check.mjs
 *
 * 去留：
 *   - 你测过通过 → 可保留（可选写进 smoke / inspect 备注）
 *   - 不通过或不要 → **删本文件即可**，无其它接线
 *
 * 不做：DOM / Tauri / Playwright / assign→split→run 长链路
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

function extractFn(src, name) {
  // export async function name | export function name | async function name
  const re = new RegExp(
    String.raw`(?:export\s+)?(?:async\s+)?function\s+${name}\s*\([^)]*\)\s*\{`,
    "m"
  );
  const m = re.exec(src);
  if (!m) return null;
  let i = m.index + m[0].length;
  let depth = 1;
  while (i < src.length && depth > 0) {
    const ch = src[i++];
    if (ch === "{") depth++;
    else if (ch === "}") depth--;
  }
  return src.slice(m.index, i);
}

function extractMethodish(src, marker, window = 80) {
  const idx = src.indexOf(marker);
  if (idx < 0) return null;
  return src.slice(idx, Math.min(src.length, idx + window * 40));
}

// ── Load sources ──────────────────────────────────────────────────────────
const clarifyRel = "web/js/features/chat/chatClarify.js";
const planOpsRel = "web/js/features/chat/chatPlanOps.js";
const boundaryRel = "docs/clarify-phase-vibe-check-subset.md";

const clarifySrc = read(clarifyRel);
const planOpsSrc = read(planOpsRel);
const boundarySrc = read(boundaryRel);

const claimFn = extractFn(clarifySrc, "claimBriefToPlan");
const hollowFn = extractFn(clarifySrc, "detectHollowGaps");
const briefPanel = extractMethodish(clarifySrc, "data-clarify-claim=", 30);
const assignFn = extractFn(planOpsSrc, "assignFromChat");

// ── 1. claim function exists ──────────────────────────────────────────────
check("claimBriefToPlan exported", !!claimFn, claimFn ? "" : "missing fn");

// ── 2. claim writes draft / savePlan only path ────────────────────────────
if (claimFn) {
  check(
    "claim writes session draft_plan",
    /draft_plan\s*=/.test(claimFn),
    "must assign session draft_plan"
  );
  check(
    "claim calls chatApi.savePlan (best-effort disk)",
    /chatApi\.savePlan\s*\(/.test(claimFn) || /savePlan\s*\(/.test(claimFn),
    "must attempt save_plan"
  );
  check(
    "claim phase → claimed_to_plan",
    /phase\s*=\s*["']claimed_to_plan["']/.test(claimFn),
    "must set claimed_to_plan"
  );

  // ── 3. NEVER open-run from claim ────────────────────────────────────────
  const forbidden = [
    ["confirm_start", /\bconfirm_start\b|\.confirmStart\b|\bconfirmStart\b/],
    ["start_run", /\bstart_run\b|\.startRun\b|\bstartRun\b/],
    ["spawn worker", /\bspawn\b/],
    ["startExecuteFromSelection", /\bstartExecuteFromSelection\b/],
    ["assignFromChat", /\bassignFromChat\b/],
  ];
  for (const [label, re] of forbidden) {
    // allow comments that say NEVER / 不 / 禁
    const hits = [];
    const lines = claimFn.split("\n");
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("/*")) {
        continue; // comment / jsdoc may mention forbidden names as bans
      }
      if (re.test(line)) hits.push(trimmed);
    }
    check(`claim body has no live ${label}`, hits.length === 0, hits.join(" | "));
  }
}

// ── 4. Hollow warn never disables claim CTA ───────────────────────────────
check("detectHollowGaps exists", !!hollowFn);
if (hollowFn) {
  check(
    "hollow returns warn fields only",
    /hollow:\s*missing\.length\s*>\s*0/.test(hollowFn) ||
      /hollow:\s*!!/.test(hollowFn) ||
      /hollow:/.test(hollowFn),
    "must expose hollow flag"
  );
  check(
    "hollow fn does not call claim/disable",
    !/\.disabled\s*=/.test(hollowFn) && !/disable/.test(hollowFn),
    "hollow must not toggle disabled"
  );
}

if (briefPanel) {
  // claim button: disabled only when busy, not when hollow
  check(
    "claim CTA disabled only via busy",
    /\(busy\s*\?\s*["'] disabled["']\s*:\s*["']["']\)/.test(briefPanel) ||
      /\(busy \? " disabled" : ""\)/.test(briefPanel),
    "expected `(busy ? \" disabled\" : \"\")`"
  );
  check(
    "claim CTA not gated on hollow",
    !/hollow.*disabled|disabled.*hollow/.test(briefPanel),
    "hollow must not gate claim button"
  );
  check(
    "claim CTA title says will not auto-start",
    /不会自动开始|不会开跑|claimTitle/.test(briefPanel) ||
      /claimTitle:\s*["'][^"']*不会自动开始/.test(clarifySrc),
    "product affordance on button title"
  );
} else {
  check("claim CTA markup found", false, "data-clarify-claim missing");
}

// ── 5. Product copy contract (claim surface) ──────────────────────────────
check(
  "claim CTA copy is human short verb",
  /claimCta:\s*["']写成计划["']/.test(clarifySrc) ||
    /claimCta:\s*["']认领并写成计划["']/.test(clarifySrc)
);
check(
  "claim success points to next step without open-run",
  /success:\s*["'][^"']*计划草稿[^"']*["']/.test(clarifySrc) &&
    !/success:\s*["'][^"']*confirm_start[^"']*["']/.test(clarifySrc)
);
check(
  "hollow warn allows save/assign",
  /仍可保存/.test(clarifySrc)
);
check(
  "claim title says will not auto-start",
  /不会自动开始/.test(clarifySrc) || /不会开跑/.test(clarifySrc)
);

// ── 6. Handoff to assign still exists (not inside claim) ──────────────────
// Proves post-claim path is separate — assign requires saved draft path.
check("assignFromChat still exists", !!assignFn);
if (assignFn) {
  check(
    "assign requires chatDraftPlan (saved path)",
    /chatDraftPlan/.test(assignFn),
    "handoff still plan-path based"
  );
  check(
    "assign is not confirm_start",
    !/\bconfirm_start\b/.test(assignFn.split("\n").filter((l) => !l.trim().startsWith("//")).join("\n")),
    "assign must not embed confirm_start"
  );
}

// ── 7. Boundary doc still hard-bans claim→run ─────────────────────────────
check(
  "boundary doc: 认领 ≠ 开跑",
  /认领\s*≠\s*开跑|认领 != 开跑|认领≠开跑/.test(boundarySrc) ||
    /认领.*开跑/.test(boundarySrc)
);
check(
  "boundary doc bans confirm_start on claim",
  /confirm_start/.test(boundarySrc) && /禁止|不|禁/.test(boundarySrc)
);

// ── Report ────────────────────────────────────────────────────────────────
const failed = results.filter((r) => !r.ok);
const passed = results.length - failed.length;

console.log("claim-boundary-check · TRIAL (keep if pass, delete if reject)");
console.log(`total=${results.length} passed=${passed} failed=${failed.length}`);
for (const r of results) {
  console.log(
    `${r.ok ? "PASS" : "FAIL"} · ${r.name}${r.detail ? " · " + r.detail : ""}`
  );
}

if (failed.length) {
  console.log("\nREJECT/FAIL → fix claim path, or delete this script if abandoning the trial.");
  process.exit(1);
}

console.log("\nPASS · claim boundary holds (draft only; no open-run). Keep or discard as you like.");
process.exit(0);
