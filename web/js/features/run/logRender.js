/**
 * [INPUT]: log events
 * [OUTPUT]: transcript / pretty log row HTML
 * [POS]: A5-2c features/run；自 log.js 抽出
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { isAiInteractionEvent, isNoiseText } from "./logFilter.js";
import { esc } from "./logHost.js";

export function transcriptRole(e) {
  const k = String(e.kind || "");
  if (k === "tool_use" || k === "tool_result") return "tool";
  if (k === "message") return "assistant";
  if (k === "result") return "result";
  if (k === "error") return "error";
  if (k === "stderr") return "stderr";
  if (k === "meta") return "meta";
  if (k === "raw_line") return "out";
  return k || "out";
}

export function renderTranscriptLine(e) {
  if (!isAiInteractionEvent(e)) return "";
  const role = transcriptRole(e);
  const label =
    role === "tool"
      ? e.kind === "tool_result"
        ? "result"
        : "tool"
      : role === "assistant"
        ? "asst"
        : role === "out"
          ? "out"
          : role;
  const noiseProbe = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
  if (isNoiseText(noiseProbe)) return "";
  const title = e.title && e.title !== label ? esc(e.title) : "";
  const summary = esc(e.summary || "");
  // tool_result / result 默认不把超长 detail 塞进 CLI 主视图
  const detail =
    e.detail && e.kind !== "result" && e.kind !== "tool_result" ? esc(e.detail) : "";
  let body = "";
  if (title && summary) body = `<span style="opacity:.85">${title}</span>  ${summary}`;
  else body = summary || title || "…";
  // 黑区只留执行交互；result success/$cost 由窗外徽章表达
  if (e.kind === "result") return "";
  if (e.kind === "tool_result") {
    const short = (summary || title || "完成").slice(0, 280);
    return `<div class="tx-line role-result">
      <div class="tx-role">tool✓</div>
      <div class="tx-body">${short}</div>
    </div>`;
  }
  if (detail && e.kind === "tool_use" && detail.length > 160) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  if (detail && e.kind === "message" && detail.length > 220) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  return `<div class="tx-line role-${esc(role)}">
    <div class="tx-role">${esc(label)}</div>
    <div class="tx-body">${body}</div>
  </div>`;
}

export function renderLogEvent(e) {
  const kind = esc(e.kind || "raw_line");
  const level = esc(e.level || "info");
  const title = esc(e.title || kind);
  const summary = esc(e.summary || "");
  const detail = e.detail ? esc(e.detail) : "";
  const detailBlock = detail
    ? `<details><summary>展开详情</summary><div class="detail">${detail}</div></details>`
    : "";
  return `<div class="log-event kind-${kind} level-${level}">
    <div class="kind">${kind}</div>
    <div class="body">
      <div class="title">${title}</div>
      <div class="summary">${summary}</div>
      ${detailBlock}
    </div>
  </div>`;
}
