#!/usr/bin/env node
/**
 * Smoke: chat fold policy (Claude Code / Codex-like — fold = single-line summary,
 * only for genuinely long messages; folding must save real space).
 * Run: node scripts/chat-fold-smoke.mjs
 *
 * Loads chatMsgEnhance with a tiny mock of legacy/chatFormat/chatState.
 */
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readFileSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const srcPath = join(root, "web/js/features/chat/chatMsgEnhance.js");
const src = readFileSync(srcPath, "utf8");

// Rewrite relative imports to a temp mock barrel so node can load the module.
const dir = join(tmpdir(), `cco-fold-smoke-${process.pid}`);
mkdirSync(dir, { recursive: true });
const mockLegacy = `
export const state = {
  chatQuizDraft: {},
  chatMsgFold: {},
  chatMsgBodyOpen: {},
  selectedPath: null,
  chatBusy: false,
};
export const $ = () => null;
export const toast = () => {};
`;
const mockFormat = `
export function chatEsc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
`;
const mockState = `
export function ensureChatState() {}
export function stashChatSession() {}
`;
writeFileSync(join(dir, "legacy.js"), mockLegacy);
writeFileSync(join(dir, "chatFormat.js"), mockFormat);
writeFileSync(join(dir, "chatState.js"), mockState);

const rewritten = src
  .replace(/from "\.\/legacy\.js"/g, `from "${pathToFileURL(join(dir, "legacy.js")).href}"`)
  .replace(/from "\.\/chatFormat\.js"/g, `from "${pathToFileURL(join(dir, "chatFormat.js")).href}"`)
  .replace(/from "\.\/chatState\.js"/g, `from "${pathToFileURL(join(dir, "chatState.js")).href}"`);
const entry = join(dir, "chatMsgEnhance.js");
writeFileSync(entry, rewritten);

const mod = await import(pathToFileURL(entry).href);
const {
  shouldFoldMessage,
  shouldClampBody,
  shouldShowBodyCollapse,
  shouldShowFoldAgain,
  renderFoldBarHtml,
  wrapExpandedBody,
  isLongChatBody,
} = mod;
const { state } = await import(pathToFileURL(join(dir, "legacy.js")).href);

function assert(cond, msg) {
  if (!cond) {
    console.error("FAIL:", msg);
    process.exitCode = 1;
  } else {
    console.log("ok:", msg);
  }
}

// Short session: never auto-fold
assert(
  shouldFoldMessage(0, 6, "很长".repeat(200), { role: "assistant" }) === false,
  "short session stays open"
);

// Recent tail open even in long session
assert(
  shouldFoldMessage(20, 22, "很长".repeat(200), { role: "assistant" }) === false,
  "recent tail open"
);

// Short user ping never folds
assert(
  shouldFoldMessage(0, 20, "截图显示出来", { role: "user" }) === false,
  "short user never folds"
);

// Old genuinely-long assistant can fold
assert(
  shouldFoldMessage(0, 20, "截图报告\n".repeat(40), { role: "assistant" }) === true,
  "old long assistant folds"
);

// Mid-size old message stays open — a fold bar wouldn't save real space
assert(
  shouldFoldMessage(0, 20, "中等长度的内容".repeat(20), { role: "assistant" }) === false,
  "mid old message stays open (fold must save real space)"
);
assert(
  shouldFoldMessage(0, 20, "一行不折的短句\n".repeat(6), { role: "assistant" }) === false,
  "few-line old message stays open"
);

// Clamp only for long bodies
assert(shouldClampBody("短", 0, {}) === false, "short not clamped");
assert(
  shouldClampBody("行\n".repeat(30), 0, {}) === true,
  "long lines clamped"
);

// Fold bar summary strips md — single line + line count meta
const bar = renderFoldBarHtml("小叶", `## 标题 **加粗** 内容说明\n${"细节行\n".repeat(5)}`, 0, {
  role: "assistant",
});
assert(bar.includes("标题 加粗"), "summary strips md markers");
assert(!bar.includes("##"), "no raw ## in summary");
assert(bar.includes("chat-msg-fold-meta"), "has meta line");
assert(bar.includes("7 行"), "meta shows line count");
assert(bar.includes("展开"), "has expand cta");

// After 展开全部 → show 收起
const longBody = "行\n".repeat(30);
assert(isLongChatBody(longBody) === true, "long body detected");
state.chatMsgBodyOpen = { m0: true };
assert(
  shouldShowBodyCollapse(longBody, 0, {}) === true,
  "expanded long body offers collapse"
);
const expanded = wrapExpandedBody("<p>x</p>", 0);
assert(expanded.includes("data-chat-body-less"), "expanded wrap has 收起");
assert(expanded.includes("收起"), "expanded wrap label 收起");

// Fold-again chrome: ONLY on messages the user manually unfolded
state.chatMsgFold = {};
assert(
  shouldShowFoldAgain(0) === false,
  "no fold-again chrome by default"
);
state.chatMsgFold = { m0: false };
assert(
  shouldShowFoldAgain(0) === true,
  "manual unfold shows fold-again"
);

// Cleanup
try {
  rmSync(dir, { recursive: true, force: true });
} catch (_) {}

if (process.exitCode) {
  console.error("chat-fold-smoke: failed");
  process.exit(1);
}
console.log("chat-fold-smoke: all ok");
