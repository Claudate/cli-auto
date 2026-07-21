/**
 * [INPUT]: log event objects · filter state
 * [OUTPUT]: AI 事件过滤 · 噪音 · ANSI → HTML · event filter
 * [POS]: A5-2c features/run；自 log.js 抽出；无 IPC
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  S,
  esc
} from "./logHost.js";


export function isNoiseText(s) {
  const t = String(s || "");
  if (!t.trim()) return true;
  if (/Ignoring\s+--allowedTools/i.test(t)) return true;
  if (/Ignoring\s+--[\w-]+/i.test(t)) return true;
  if (/^\s*stderr\b/i.test(t)) return true;
  if (/^\s*\[?(system|meta|debug|trace)\]?/i.test(t)) return true;
  if (/permission\s*prompt/i.test(t) && /allowedTools/i.test(t)) return true;
  if (/CLI\s*warning/i.test(t)) return true;
  if (/deprecated\s+flag/i.test(t)) return true;
  if (/node:?\s*warn/i.test(t)) return true;
  if (/experimental\s+feature/i.test(t)) return true;
  if (/^\s*warn(ing)?:/i.test(t)) return true;
  return false;
}

export function isAiInteractionEvent(e) {
  if (!e) return false;
  const k = String(e.kind || "").toLowerCase();
  // 红框3：CLI 只保留 AI 对话 / 工具调用 / 结果
  if (k === "message" || k === "tool_use" || k === "tool_result" || k === "result") return true;
  // 业务级 error 可保留；stderr / meta / system 噪音一律丢弃
  if (k === "error") {
    const lvl = String(e.level || "").toLowerCase();
    if (lvl === "debug" || lvl === "trace") return false;
    const blob = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
    if (isNoiseText(blob)) return false;
    return true;
  }
  return false;
}

/** P2-3: event-type filter (all | tool | error). */
export function eventPassesFilter(e, filter) {
  const f = filter || S().logEventFilter || "all";
  if (f === "all") return true;
  const k = String(e?.kind || "").toLowerCase();
  if (f === "tool") return k === "tool_use" || k === "tool_result";
  if (f === "error") {
    if (k === "error") return true;
    const lvl = String(e?.level || "").toLowerCase();
    if (lvl === "error" || lvl === "warn") return true;
    const blob = `${e?.title || ""} ${e?.summary || ""}`.toLowerCase();
    return /\berror\b|failed|panic|traceback|exception/.test(blob);
  }
  return true;
}

/**
 * P2-3: minimal ANSI → HTML (raw mode only).
 * Supports SGR bold/dim/colors 30–37 / 90–97 and reset. Strips other CSI.
 */
export function ansiToHtml(text) {
  const s = String(text || "");
  // Detect CSI without embedding a raw ESC in source (avoids control-char issues).
  let hasCsi = false;
  for (let j = 0; j < s.length - 1; j++) {
    if (s.charCodeAt(j) === 0x1b && s[j + 1] === "[") {
      hasCsi = true;
      break;
    }
  }
  if (!hasCsi) return esc(s);
  let out = "";
  let i = 0;
  let open = [];
  const closeAll = () => {
    while (open.length) {
      out += "</span>";
      open.pop();
    }
  };
  const classFor = (codes) => {
    const cls = [];
    for (const c of codes) {
      if (c === 1) cls.push("ansi-bold");
      else if (c === 2) cls.push("ansi-dim");
      else if (c === 31 || c === 91) cls.push("ansi-red");
      else if (c === 32 || c === 92) cls.push("ansi-green");
      else if (c === 33 || c === 93) cls.push("ansi-yellow");
      else if (c === 34 || c === 94) cls.push("ansi-blue");
      else if (c === 35 || c === 95) cls.push("ansi-magenta");
      else if (c === 36 || c === 96) cls.push("ansi-cyan");
    }
    return cls.join(" ");
  };
  while (i < s.length) {
    if (s.charCodeAt(i) === 0x1b && s[i + 1] === "[") {
      const m = s.slice(i + 2).match(/^([0-9;]*)m/);
      if (m) {
        const codes = m[1]
          ? m[1].split(";").map((x) => parseInt(x, 10) || 0)
          : [0];
        i += 2 + m[0].length;
        if (codes.includes(0) || codes.length === 0) {
          closeAll();
        } else {
          const cls = classFor(codes);
          if (cls) {
            out += `<span class="${cls}">`;
            open.push(cls);
          }
        }
        continue;
      }
      // unknown CSI — skip ESC[
      i += 2;
      continue;
    }
    out += esc(s[i]);
    i += 1;
  }
  closeAll();
  return out;
}
