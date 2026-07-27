#!/usr/bin/env node
/**
 * Ensure V3 proxy smoke (static): inspect gate fail card primary CTA = rework, not re-examiner.
 *
 *   node scripts/ensure-v3-cta-smoke.mjs
 *
 * Does **not** replace wros human V1–V5; only locks E4 UI copy in source.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const src = fs.readFileSync(
  path.join(ROOT, "web/js/features/run/logBoardCard.js"),
  "utf8"
);

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail });
}

check(
  "E4 comment present",
  /Ensure E4:\s*inspect gate fail/.test(src)
);
check(
  "primary rework label 回补并再巡检",
  /回补并再巡检（第 \$\{n\}\/\$\{max\} 轮）/.test(src) ||
    /回补并再巡检/.test(src)
);
check(
  "rework button is btn primary + cli-rework-btn",
  /btn primary sm cli-rework-btn/.test(src) && /data-rework=/.test(src)
);
check(
  "re-run examiner is ghost secondary when inspect+canRework",
  /isInspect && canRework/.test(src) &&
    /btn ghost sm cli-rerun-btn/.test(src) &&
    /仅当怀疑巡检本身坏了时再跑考官/.test(src)
);
check(
  "non-inspect fail still primary 再跑一次",
  /title="再跑这一步">再跑一次/.test(src)
);

const failed = results.filter((r) => !r.ok);
for (const r of results) {
  console.log(`${r.ok ? "PASS" : "FAIL"} · ${r.name}${r.detail ? " · " + r.detail : ""}`);
}
console.log(
  `ensure-v3-cta-smoke total=${results.length} passed=${results.length - failed.length} failed=${failed.length}`
);
process.exit(failed.length ? 1 : 0);
