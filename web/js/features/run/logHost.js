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

/**
 * Resolve a DOM node.
 * Accepts bare id (`cli-board`) or CSS selector (`#cli-board`, `.row`).
 * Classic window.$ is querySelector — bare ids must go through getElementById.
 */
export function $(idOrSel) {
  if (!idOrSel) return null;
  const s = String(idOrSel);
  // CSS selector form → querySelector (classic $ or native)
  if (/^[.#\[]/.test(s) || /[\s>+~:\[,]/.test(s)) {
    const fn = g("$");
    if (typeof fn === "function") {
      try {
        return fn(s);
      } catch (_) {
        /* invalid selector */
      }
    }
    try {
      return document.querySelector(s);
    } catch (_) {
      return null;
    }
  }
  // bare id
  return document.getElementById(s);
}

export function $$(sel, root) {
  const scope = root || document;
  const fn = g("$$");
  if (typeof fn === "function") {
    try {
      return fn(sel, scope);
    } catch (_) {
      /* fall through */
    }
  }
  try {
    return Array.from(scope.querySelectorAll(sel));
  } catch (_) {
    return [];
  }
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
  // stop/abort are not business failures
  return /fail|error|timeout/i.test(String(s || "")) && !/abort|stop|cancel/i.test(String(s || ""));
}

export function isStoppedStatus(s) {
  const fn = g("isStoppedStatus");
  if (typeof fn === "function") return fn(s);
  return /stop|abort|cancel/i.test(String(s || ""));
}

/**
 * Call a classic window global.
 *
 * Supports both:
 *   callG("taskBucket", t)     — direct
 *   callG("taskBucket")(t)     — curry (used widely in logBoard*)
 *
 * When `args` is empty, returns a bound function so curry form works.
 * When the global is missing, returns a no-op function (curry-safe) so
 * board render never throws on missing helpers.
 */
export function callG(name, ...args) {
  const fn = g(name);
  if (typeof fn !== "function") {
    if (args.length === 0) return () => undefined;
    return undefined;
  }
  if (args.length === 0) {
    return (...rest) => fn(...rest);
  }
  return fn(...args);
}
