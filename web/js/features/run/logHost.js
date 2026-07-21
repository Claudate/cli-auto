/**
 * [INPUT]: window classic globals (state / $ / esc / toast / status helpers)
 * [OUTPUT]: thin bridge for features/run/log*
 * [POS]: A5-2c；避免各 log 模块重复桥接代码
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

/** Live global state object (mutates in place). */
export function S() {
  return g("state") || {};
}

export function $(id) {
  const fn = g("$");
  return typeof fn === "function" ? fn(id) : document.getElementById(id);
}

export function $$(sel, root) {
  const fn = g("$$");
  if (typeof fn === "function") return fn(sel, root);
  return Array.from((root || document).querySelectorAll(sel));
}

export function esc(s) {
  const fn = g("esc");
  if (typeof fn === "function") return fn(s);
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function toast(msg) {
  const fn = g("toast");
  if (typeof fn === "function") return fn(msg);
  console.log("[toast]", msg);
}

export function isLiveStatus(s) {
  const fn = g("isLiveStatus");
  if (typeof fn === "function") return fn(s);
  return /run|active|working|pending|starting|queued/i.test(String(s || ""));
}

export function isFailedStatus(s) {
  const fn = g("isFailedStatus");
  if (typeof fn === "function") return fn(s);
  return /fail|error|abort/i.test(String(s || ""));
}

export function callG(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}
