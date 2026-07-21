/**
 * [INPUT]: task/planner log DTO · logVirtual / logFilter / logRender
 * [OUTPUT]: panel content · planner log · fill body · plain text
 * [POS]: A5-2c features/run；自 log.js 抽出
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  isAiInteractionEvent,
  eventPassesFilter,
  ansiToHtml,
  isNoiseText,
} from "./logFilter.js";
import { renderTranscriptLine, renderLogEvent } from "./logRender.js";
import {
  LOG_VIRTUAL_THRESHOLD,
  isNearBottom,
  mountVirtualLog,
  renderLogRowHtml,
} from "./logVirtual.js";
import {
  S,
  $,
  esc,
  isLiveStatus,
  isFailedStatus,
} from "./logHost.js";

/** P1-1：日志面板内容签名；相同则跳过 innerHTML 重绘 */
export function logPanelSignature(t) {
  if (!t) return "";
  const mode = S().logViewMode || "term";
  const evFilter = S().logEventFilter || "all";
  const events = Array.isArray(t.log_events) ? t.log_events : [];
  const last = events.length ? events[events.length - 1] : null;
  const lastId = last?.id || "";
  const lastSum = last ? `${last.kind}|${last.title}|${String(last.summary || "").slice(0, 40)}` : "";
  return [
    t.task_id || "",
    t.status || "",
    t.log_bytes ?? "",
    events.length,
    lastId,
    lastSum,
    mode,
    evFilter,
    t.error_summary || t.error || "",
    t.cost_usd ?? "",
  ].join("\0");
}

/** Planner / generic：用 LogConsole 渲染事件列表（P1-3 + P2-3 filter/ANSI + 虚拟列表） */
export function renderLogConsoleHtml(events, { mode, emptyText, rawText } = {}) {
  const m = mode || S().logViewMode || "term";
  const filter = S().logEventFilter || "all";
  const list = (Array.isArray(events) ? events : []).filter((e) =>
    eventPassesFilter(e, filter)
  );
  if (m === "raw") {
    const plain =
      rawText ||
      list
        .map((ev) => [ev.kind, ev.title, ev.summary].filter(Boolean).join("\t"))
        .join("\n");
    if (!plain) {
      return `<div class="cli-empty-ai muted">${esc(emptyText || "等待输出…")}</div>`;
    }
    // P2-3: light ANSI coloring in raw mode
    return (
      '<pre class="panel-log-pre ansi-raw">' + ansiToHtml(plain) + "</pre>"
    );
  }
  if (!list.length) {
    return `<div class="cli-empty-ai muted">${esc(
      emptyText || (filter !== "all" ? "当前过滤下无事件…" : "等待输出…")
    )}</div>`;
  }
  // Small lists: full DOM. Large lists are handled by mountVirtualLog at call sites.
  if (list.length >= LOG_VIRTUAL_THRESHOLD) {
    // Marker so callers know to mount virtual (avoid double-render)
    return `<!--virt:${list.length}-->`;
  }
  if (m === "pretty") {
    return list.map((e) => renderLogEvent(e)).join("") || "";
  }
  return (
    list
      .map((e) => renderLogRowHtml(e, m))
      .filter(Boolean)
      .join("") || ""
  );
}

/** Soften planner log lines for main-path readability (engine jargon → flow words). */
export function humanizePlannerLogLine(line) {
  let s = String(line || "");
  s = s.replace(/starting Claude LLM planner \(print\)…?/gi, "开始智能拆分…");
  s = s.replace(/starting intelligent planner…?/gi, "开始智能拆分…");
  s = s.replace(/planner CLI bin\s*=\s*\S+/gi, "执行环境已就绪");
  s = s.replace(/async planner: will invoke Claude CLI[^\n]*/gi, "后台智能拆分进行中…");
  s = s.replace(/using adapter parse[^\n]*/gi, "按已有结构直接解析…");
  s = s.replace(/skipping LLM planner[^\n]*/gi, "使用本地规则拆分…");
  s = s.replace(/LLM raw output (\d+) bytes/gi, "拆分结果已收到（$1 字节）");
  s = s.replace(/LLM plan ok: (\d+) tasks/gi, "智能拆分完成：$1 个步骤");
  s = s.replace(/planned ok name=(\S+) tasks=(\d+)/gi, "拆分完成 · $1 · $2 个步骤");
  s = s.replace(/plan digest mode=(\w+)/gi, "文档模式：$1");
  s = s.replace(/sanitize deps: removed (\d+)/gi, "已清理可疑依赖 $1 条");
  s = s.replace(/critic：/gi, "拆分校对：");
  s = s.replace(/critic:/gi, "拆分校对：");
  s = s.replace(
    /LLM critic cost_usd=([0-9.]+) duration_ms=(\d+)/gi,
    "智能校对费用 $$1 · 耗时 $2ms"
  );
  s = s.replace(/LLM critic duration_ms=(\d+)/gi, "智能校对耗时 $1ms");
  s = s.replace(/LLM critic ok:[^\n]*/gi, "智能校对完成");
  s = s.replace(/LLM critic skipped[^\n]*/gi, "智能校对已跳过");
  s = s.replace(/LLM critic:[^\n]*/gi, "智能校对");
  s = s.replace(/LLM校对：/g, "智能校对：");
  s = s.replace(/\bwave (\d+):/gi, "第 $1 波：");
  s = s.replace(/\bClaude CLI\b/gi, "智能拆分");
  s = s.replace(/\bCLI\b/g, "执行通道");
  return s;
}

export function fillPlannerLog(view) {
  const logEl = $("#planner-log");
  if (!logEl) return;
  const events =
    view?.planner_log_events ||
    view?.plannerLogEvents ||
    [];
  const tailRaw =
    view?.planner_log_tail ||
    view?.plannerLogTail ||
    "";
  const tail = String(tailRaw)
    .split("\n")
    .map((l) => humanizePlannerLogLine(l))
    .join("\n");
  // 无事件时用 tail 拆行伪事件，避免 raw 墙
  let evs = Array.isArray(events) ? events : [];
  if (evs.length) {
    evs = evs.map((e) => ({
      ...e,
      summary: humanizePlannerLogLine(e.summary || e.title || ""),
      title: e.title === "log" ? "进度" : e.title,
    }));
  } else if (tail) {
    evs = String(tail)
      .split("\n")
      .filter((l) => l.trim() && !l.startsWith("… ("))
      .slice(-80)
      .map((line, i) => ({
        id: `p${i}`,
        kind: "raw_line",
        stream: "stdout",
        title: "进度",
        summary: line.replace(/^\[[^\]]+\]\s*/, ""),
        level: /fail|error|失败/i.test(line) ? "error" : "info",
      }));
  }
  const filter = S().logEventFilter || "all";
  const filtered = evs.filter((e) => eventPassesFilter(e, filter));
  const sig = [
    filtered.length,
    filtered.length
      ? filtered[filtered.length - 1]?.id || filtered[filtered.length - 1]?.summary
      : "",
    tail.length,
    S().logViewMode || "term",
    filter,
  ].join("|");
  if (logEl.dataset.sig === sig) return;
  const stick = isNearBottom(logEl);
  logEl.classList.add("log-console", "term-mode");
  logEl.classList.toggle("pretty-mode", (S().logViewMode || "term") === "pretty");
  logEl.classList.toggle("raw-mode", (S().logViewMode || "term") === "raw");
  const mode = S().logViewMode || "term";
  // Filter switch → new sig → rebuild; stick resets to bottom (documented residual).
  if (
    mode !== "raw" &&
    mountVirtualLog(logEl, filtered, { mode, stick })
  ) {
    logEl.dataset.sig = sig;
    return;
  }
  logEl.innerHTML = renderLogConsoleHtml(evs, {
    mode,
    emptyText: "正在理解计划并拆分步骤…",
    rawText: tail,
  });
  logEl.dataset.sig = sig;
  if (stick) logEl.scrollTop = logEl.scrollHeight;
}

export function aiLogPlainText(t) {
  const events = (Array.isArray(t?.log_events) ? t.log_events : [])
    .filter(isAiInteractionEvent)
    .filter((ev) => String(ev.kind || "").toLowerCase() !== "result");
  if (events.length) {
    return events
      .map((ev) => {
        const kind = ev.kind || "";
        const title = ev.title || "";
        const summary = ev.summary || "";
        return [kind, title, summary].filter(Boolean).join("\t");
      })
      .join("\n");
  }
  // 无结构化事件时：不回落整段 log_tail，避免系统噪音污染
  if (isLiveStatus(t?.status)) return "AI 运行中，等待交互输出…";
  if (isFailedStatus(t?.status)) return t?.error ? String(t.error) : "任务失败，无 AI 交互日志。";
  return "";
}

/**
 * Build panel log content.
 * @returns {{ html?: string, virtItems?: any[], mode: string, empty?: boolean }}
 * Caller prefers virtItems when present (mountVirtualLog).
 */
export function panelLogContent(t) {
  const st = String(t.status || "").toLowerCase();
  const events = (Array.isArray(t.log_events) ? t.log_events : [])
    .filter(isAiInteractionEvent)
    .filter((e) => eventPassesFilter(e, S().logEventFilter));
  const mode = S().logViewMode || "term";

  // 默认 term / pretty：只渲染 AI 事件，绝不 dump 原始 log_tail
  // result 摘要不进黑区（成功态窗外徽章已表达）
  const viewEvents = events.filter((e) => String(e.kind || "").toLowerCase() !== "result");
  if (mode !== "raw") {
    if (!viewEvents.length) {
      if (isLiveStatus(st)) {
        return {
          mode,
          empty: true,
          html: '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>',
        };
      }
      if (isFailedStatus(st)) {
        const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
        return {
          mode,
          empty: true,
          html: err
            ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
            : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>',
        };
      }
      if ((S().logEventFilter || "all") !== "all") {
        return {
          mode,
          empty: true,
          html: '<div class="cli-empty-ai muted">当前过滤下无事件…</div>',
        };
      }
      // 完成且仅有 result 摘要：黑区留空，成功由窗外徽章表达
      return { mode, empty: true, html: "" };
    }
    // P2-3 virtual list: full history, only visible window in DOM
    if (viewEvents.length >= LOG_VIRTUAL_THRESHOLD) {
      return { mode, virtItems: viewEvents };
    }
    if (mode === "pretty") {
      return {
        mode,
        html: viewEvents.map((e) => renderLogEvent(e)).join("") || "",
      };
    }
    return {
      mode,
      html:
        viewEvents
          .map((e) => renderTranscriptLine(e))
          .filter(Boolean)
          .join("") || "",
    };
  }

  // raw 模式：执行交互文本；result 摘要已在 aiLogPlainText 过滤；P2-3 轻量 ANSI
  const plain = aiLogPlainText(t);
  if (!plain) {
    if (isLiveStatus(st)) {
      return {
        mode,
        empty: true,
        html: '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>',
      };
    }
    if (isFailedStatus(st)) {
      const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
      return {
        mode,
        empty: true,
        html: err
          ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
          : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>',
      };
    }
    return { mode, empty: true, html: "" };
  }
  return {
    mode,
    html: '<pre class="panel-log-pre ansi-raw">' + ansiToHtml(plain) + "</pre>",
  };
}

/** @deprecated keep name for any external refs; string-only path (no virt). */
export function panelLogHtml(t) {
  const c = panelLogContent(t);
  if (c.virtItems) {
    // Fallback string path: last window only (virt preferred at call site)
    const slice = c.virtItems.slice(-80);
    if (c.mode === "pretty") {
      return slice.map((e) => renderLogEvent(e)).join("") || "";
    }
    return (
      slice
        .map((e) => renderTranscriptLine(e))
        .filter(Boolean)
        .join("") || ""
    );
  }
  return c.html || "";
}

/** Fill a cli-window-body (or planner log) with panel content + optional virtual list. */
export function fillPanelLogBody(body, t, { stick } = {}) {
  if (!body) return;
  const c = panelLogContent(t);
  if (c.virtItems) {
    const did = mountVirtualLog(body, c.virtItems, {
      mode: c.mode,
      stick: stick !== false && (stick || isNearBottom(body)),
    });
    if (did) return;
  }
  body.innerHTML = c.html || "";
  if (stick) body.scrollTop = body.scrollHeight;
}
