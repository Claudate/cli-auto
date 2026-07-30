#!/usr/bin/env node
/**
 * Smoke: renderMarkdown emits img placeholders for ![alt](local.png)
 * and bare project-relative image paths (chat screenshot reports).
 * Run: node scripts/chat-md-image-smoke.mjs
 */
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const mdPath = join(root, "web/js/shared/markdown.js");

// Load as ESM
const mod = await import(pathToFileURL(mdPath).href);
const { renderMarkdown } = mod;
if (typeof renderMarkdown !== "function") {
  console.error("renderMarkdown missing");
  process.exit(1);
}

const samples = [
  {
    name: "bang-image",
    src: "见截图：\n\n![首页](.cco-out/screenshots/t1/index.png)\n\n完",
    need: ['data-md-img-path=".cco-out/screenshots/t1/index.png"', "md-img", "首页"],
  },
  {
    name: "bare-path-line",
    src: "本地路径：\n\n.cco-out/screenshots/20260728-182720/chat.png\n\n说明",
    need: [
      'data-md-img-path=".cco-out/screenshots/20260728-182720/chat.png"',
      "md-img",
    ],
  },
  {
    name: "not-link-only",
    src: "![模拟聊天](.cco-out/screenshots/x/chat.png)",
    need: ["md-img-pending", "模拟聊天"],
    forbid: ["md-local-link"],
  },
];

let failed = 0;
for (const s of samples) {
  const html = renderMarkdown(s.src);
  const miss = (s.need || []).filter((t) => !html.includes(t));
  const bad = (s.forbid || []).filter((t) => html.includes(t));
  if (miss.length || bad.length) {
    failed++;
    console.error(`FAIL ${s.name}`);
    if (miss.length) console.error("  missing:", miss);
    if (bad.length) console.error("  forbid hit:", bad);
    console.error("  html:", html.slice(0, 400));
  } else {
    console.log(`ok ${s.name}`);
  }
}

if (failed) {
  console.error(`${failed} failed`);
  process.exit(1);
}
console.log("chat-md-image-smoke: all ok");
