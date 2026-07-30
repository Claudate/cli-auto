#!/usr/bin/env node
/**
 * path-depth 波次静态契约冒烟（W0–W3 结构 · 可代 W1-6「结构/脚本」项）
 *
 * 不替代真人桌面 30–60s 抽检（见 docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md）。
 * 锁：无三档英雄键 · 场景芯片 · 当前理解 · 认领本波不旁路开跑 · 本波分组/总览/串行 confirm。
 *
 *   node scripts/path-depth-wave-smoke.mjs
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
function exists(rel) {
  return fs.existsSync(path.join(ROOT, rel));
}

const pathMode = read("web/js/features/chat/chatPathMode.js");
const persona = read("web/js/features/chat/chatPersona.js");
const understand = read("web/js/features/chat/chatUnderstand.js");
const render = read("web/js/features/chat/chatRender.js");
const format = read("web/js/features/chat/chatFormat.js");
const batch = read("web/js/features/chat/chatWaveBatch.js");
const overview = read("web/js/features/chat/chatWaveOverview.js");
const wavePlans = read("web/js/features/chat/chatWavePlans.js");
const plansMgmt = read("web/js/features/chat/plansMgmt.js");
const gateway = read("web/js/shared/gateway.js");
const chatApi = read("web/js/features/chat/chatApi.js");
const bindUi = read("web/js/features/settings/bindUiClick.js");
const prompt = read("docs/runtime-prompts/chat-plan-writing.md");
const fence = read("src/domain/chat/fence.rs");
const planMd = read("src/services/chat/plan_md.rs");
const jobRs = read("src/plan/planner/job.rs");

// ── W0-8: no hero three-mode ────────────────────────────────────────────────
check(
  "pathModeSegmentHtml default returns empty (no hero)",
  /if\s*\(\s*!opts\.advanced\s*\)\s*return\s*""/.test(pathMode) ||
    /if\s*\(\s*!opts\.advanced\s*\)\s*return\s*''/.test(pathMode)
);
check(
  "empty lead does not call pathModeSegmentHtml() bare",
  !/pathModeSegmentHtml\(\s*\)/.test(render) ||
    /pathModeSegmentHtml\(\s*\{\s*advanced:\s*true/.test(render)
);
check(
  "coach says 不必先学",
  /不必先学/.test(pathMode)
);
check(
  "HEAD_STEP_DEFAULT human skeleton",
  /说清楚/.test(pathMode) && /写成计划/.test(pathMode)
);

// ── W0-7 persona ───────────────────────────────────────────────────────────
check("persona scene chips present", /SCENE_CHIPS/.test(persona) && /上架/.test(persona));
check("persona primary_cta / direct_exec", /primaryCta/.test(persona) && /directExec/.test(persona));
check("ecom listing lexicon", /上架清单/.test(persona));
check("admin hide direct", /admin:[\s\S]*directExec:\s*"hide"/.test(persona));

// ── W1 understand / feedback ───────────────────────────────────────────────
check("当前理解 extractUnderstanding", /extractUnderstanding/.test(understand));
check("按我说的改 CTA", /按我说的改/.test(understand) || /data-chat-revise/.test(understand));
check("这版作数 save label", /这版作数/.test(format));
check("renderUnderstandingBar in chatRender", /renderUnderstandingBarHtml/.test(render));

// ── W2 wave claim ──────────────────────────────────────────────────────────
check("extract_all_plan_fences in domain", /fn extract_all_plan_fences/.test(fence));
check("extract_wave_index_fence in domain", /fn extract_wave_index_fence/.test(fence));
check("chat_save_wave_bundle service", /fn chat_save_wave_bundle/.test(planMd));
check("gateway chatSaveWaveBundle", /chat_save_wave_bundle_cmd/.test(gateway));
check("chatApi saveWaveBundle", /saveWaveBundle/.test(chatApi));
check("UI 认领本波", /认领本波/.test(format) || /data-chat-wave-claim/.test(format));
check(
  "claimWaveBundle no confirmStart live call",
  (() => {
    const fn = render.match(
      /export async function claimWaveBundle[\s\S]*?^export /m
    );
    const body = fn ? fn[0] : render;
    const lines = body.split("\n").filter((l) => {
      const t = l.trim();
      return !t.startsWith("//") && !t.startsWith("*");
    });
    const joined = lines.join("\n");
    return !/\.confirmStart\s*\(/.test(joined) && !/\bconfirm_start\b/.test(joined);
  })()
);
check("prompt teaches wave-index", /wave-index/.test(prompt));
check("prompt forbids glue long md", /粘成/.test(prompt) || /超长/.test(prompt));

// ── W2-4 supersede per path ────────────────────────────────────────────────
check(
  "supersede_planning_jobs takes plan_path",
  /fn supersede_planning_jobs\([\s\S]*plan_path:\s*&Path/.test(jobRs) ||
    /supersede_planning_jobs\([\s\S]*&req\.plan/.test(jobRs)
);
check(
  "supersede test per plan_path",
  /supersede_planning_is_per_plan_path/.test(jobRs)
);

// ── W2-5 / W3 UI ───────────────────────────────────────────────────────────
check("groupPlanItemsByWave", /groupPlanItemsByWave/.test(wavePlans));
check("plansMgmt uses wave groups", /groupPlanItemsByWave/.test(plansMgmt));
check("wave overview module", /buildWaveOverview/.test(overview));
check("serial parallelPolicy", /parallelPolicy:\s*"serial"/.test(overview));
check("confirmWaveBatchSerial uses confirmStart", /confirmStart/.test(batch));
check(
  "batch does not call start_run / startRun",
  (() => {
    // Strip comments (may say "no start_run") and ignore startExecuteFromSelection
    const code = batch
      .split("\n")
      .filter((l) => {
        const t = l.trim();
        return !t.startsWith("//") && !t.startsWith("*") && !t.startsWith("/*");
      })
      .join("\n");
    return (
      !/\bstart_run\b/.test(code) &&
      !/\.startRun\s*\(/.test(code) &&
      !/\bstartRun\s*\(/.test(code)
    );
  })()
);
check("bindUi wave confirm + split next", /data-wave-confirm-batch/.test(bindUi) && /data-wave-split-next/.test(bindUi));
check("INDEX cannot assign (isWaveIndexPath gate)", /isWaveIndexPath/.test(plansMgmt) || /isWaveIndexPath/.test(batch));

// ── V-WAVE-HARDEN：批确认 run-lock disable + 失败人话 + INDEX 误点拆步 ────
check(
  "overview confirm button disabled when runLocked",
  /runLocked\s*\?/.test(overview) && /disabled\s+data-wave-confirm-batch/.test(overview)
);
check(
  "plansMgmt passes runLocked from hasActiveRun",
  /renderWaveOverviewHtml\(ov,\s*chatEsc,\s*\{\s*runLocked\s*\}\)/.test(plansMgmt) &&
    /hasActiveRun/.test(plansMgmt)
);
check(
  "batch failure toast tells rest untouched + retry gate",
  /确认失败/.test(batch) && /未动/.test(batch) && /确认本波/.test(batch)
);
check(
  "INDEX mis-split toast is human (目录不可拆)",
  /本波索引（目录）/.test(batch) && /拆成步骤/.test(batch)
);

// ── docs ───────────────────────────────────────────────────────────────────
check("W1-6 checklist exists", exists("docs/path-depth-wave-2026-07-28/w1-6-desktop-checklist.md"));
check("W4-3 debt ledger exists", exists("docs/path-depth-wave-2026-07-28/w4-3-line-debt.md"));
check("landing exists", exists("docs/path-depth-wave-2026-07-28/landing.md"));

// ── report ─────────────────────────────────────────────────────────────────
const failed = results.filter((r) => !r.ok);
for (const r of results) {
  const mark = r.ok ? "PASS" : "FAIL";
  console.log(`${mark}  ${r.name}${r.detail ? ` — ${r.detail}` : ""}`);
}
console.log(`\npath-depth-wave-smoke: ${results.length - failed.length}/${results.length} passed`);
if (failed.length) {
  console.error("FAILED:", failed.map((f) => f.name).join(", "));
  process.exit(1);
}
process.exit(0);
