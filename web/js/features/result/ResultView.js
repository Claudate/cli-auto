/**
 * [INPUT]: ResultViewModel · live/tasks · 既有 #result-desk DOM
 * [OUTPUT]: 结果摘要 + inspect 人话；结束本轮统一日志栏「结束计划」
 * [POS]: A4-3/A4-4 · P0-1/P0-4/P1-3/P2-1 ResultView；禁止 invoke / 解析 VERDICT 正文
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * P0-4: 对照计划用语经 inspectCopy（与 report fallback 同词）。
 * 费用展示在标题右侧 #result-cost-chip（shellChrome.updateBudgetChip）。
 * P1-3: miss 行展示 live.route_label（App 拼好人话）；主路径不露 raw route_source enum。
 * P2-1: live.verification → 可折叠「原计划要验收」副栏（巡检为准 / 未自动对照）。
 * W3: live.browser_evidence → 网页验收证据（截图 data URL / 摘录）。
 * 第一屏标题固定「本轮结果」，不写 run_id。
 */

import {
  inspectStripParts,
  honestInspectCopy,
  inspectActionVisibility,
} from "./inspectCopy.js";
import {
  taskBucket,
  fiveStateLabel,
} from "../run/runBuckets.js";
import { renderBrowserEvidence } from "./browserEvidence.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function $(id) {
  return document.getElementById(id);
}

function esc(s) {
  const fn = g("esc");
  if (typeof fn === "function") return fn(s);
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function formatElapsed(a, b) {
  const fn = g("formatElapsed");
  if (typeof fn === "function") return fn(a, b);
  return "";
}

function taskErrorSummary(t) {
  const fn = g("taskErrorSummary");
  if (typeof fn === "function") return fn(t);
  return "";
}

function planLabel(live) {
  const path = live?.plan_path || g("state")?.selectedPlan || "";
  if (typeof g("planDisplayName") === "function") {
    return g("planDisplayName")(path) || "本轮计划";
  }
  if (!path) return "本轮计划";
  const parts = String(path).replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || "本轮计划";
}

function taskTitle(t) {
  return (t && (t.title || t.task_id)) || "步骤";
}

/**
 * P1-3: one human route line from live DTO (App-composed route_label).
 * Falls back to product label only when label missing (defensive).
 * Never surfaces raw route_source enum tags.
 * @param {object} t live task
 * @returns {string} e.g. "执行方式：Codex · 你在拆分台指定的"
 */
function routeLine(t) {
  if (!t) return "";
  const label = String(t.route_label || "").trim();
  if (label) return `执行方式：${label}`;
  // Defensive: old payload without route_label — product-ish provider only.
  const p = String(t.provider || "").trim();
  if (!p) return "";
  const fn = g("flowEngineLabel");
  const product =
    typeof fn === "function" ? fn(p) || p : p;
  return product ? `执行方式：${product}` : "";
}

/**
 * @param {ReturnType<import("./ResultViewModel.js").createResultViewModel>} vm
 * @param {object} [bridge]
 */
export function bindResultView(vm, bridge = {}) {
  function legacy() {
    return (typeof bridge.getLegacy === "function" && bridge.getLegacy()) || {};
  }

  function pullLive(live) {
    if (live !== undefined) vm.setLive(live);
    else if (legacy().live) vm.setLive(legacy().live);
  }

  /**
   * P-loop: human inspect strip + rework actions (DTO fields only).
   * @param {object|null} live
   * @param {boolean} finished
   * @param {boolean} [active]
   */
  function renderInspectLoopStrip(live, finished, active) {
    const strip = $("inspect-loop-strip");
    // 结果台：失败且可回补时露出主 CTA；结束本轮仍用日志栏「结束计划」
    const btnRework = $("btn-ws-rework");
    const btnAccept = $("btn-ws-accept-residual");
    const loop = live?.inspect_loop;
    const vis = inspectActionVisibility(loop, { finished, active });
    if (btnRework) {
      if (vis.canRework) {
        const round = Number(loop?.rework_round) || 0;
        const max = Number(loop?.rework_max) || 2;
        const n = Math.min(round + 1, max);
        btnRework.hidden = false;
        btnRework.textContent = `回补并再巡检（第 ${n}/${max} 轮）`;
        btnRework.title = "按巡检遗漏回补并再对照计划（不是再跑考官）";
        btnRework.classList.add("primary");
      } else {
        btnRework.hidden = true;
      }
    }
    if (btnAccept) btnAccept.hidden = !vis.showAccept;
    if (!strip) return;

    const parts = inspectStripParts(loop);
    if (parts.kind === "empty" || !parts.bits.length) {
      strip.hidden = true;
      strip.textContent = "";
      return;
    }

    strip.hidden = false;
    strip.textContent = parts.bits.join(" · ");
    strip.classList.toggle("bad", parts.kind === "bad");
    strip.classList.toggle("ok", parts.kind === "ok");
  }

  /**
   * R3: fill #result-desk when run finished; hide while running.
   * @param {object|null} live
   * @param {object[]} tasks
   * @param {{ hasRun?: boolean, active?: boolean, finished?: boolean }} ctx
   */
  function renderResultDesk(live, tasks, ctx) {
    pullLive(live);
    const desk = $("result-desk");
    if (!desk) return;

    const finished = !!(ctx && ctx.finished);
    const active = !!(ctx && ctx.active);
    const show = finished && !active && !!(ctx && ctx.hasRun);
    desk.hidden = !show;
    // 顶栏「结束」已撤；统一用日志栏「结束计划」
    const finishBtn = $("btn-ws-finish");
    if (finishBtn) finishBtn.hidden = true;
    if (!show) {
      return;
    }

    const doneList = $("result-desk-done");
    const missList = $("result-desk-miss");
    const planLine = $("result-desk-plan-line");
    const honest = $("result-desk-honest");

    const done = [];
    const miss = [];
    (tasks || []).forEach((t) => {
      const b = taskBucket(t.status, t);
      if (b === "done") done.push(t);
      else miss.push(t);
    });

    if (planLine) {
      const name = planLabel(live);
      // 费用已挪到标题右侧 #result-cost-chip；计划行只留完成度/耗时
      const bits = [`《${name}》`];
      if (tasks && tasks.length) bits.push(`共 ${tasks.length} 步`);
      bits.push(`完成 ${done.length}`);
      if (miss.length) bits.push(`未完成 ${miss.length}`);
      const runEnd = (tasks || [])
        .map((t) => t.finished_at)
        .filter(Boolean)
        .sort()
        .slice(-1)[0];
      if (live?.started_at) {
        const el = formatElapsed(live.started_at, runEnd || null);
        if (el) bits.push(el);
      }
      planLine.textContent = bits.join(" · ");
    }

    if (doneList) {
      if (!done.length) {
        doneList.innerHTML = `<li class="muted">本轮没有标记为已完成的步骤</li>`;
      } else {
        doneList.innerHTML = done
          .map((t) => {
            const icon = typeof g("ccoIcon") === "function" ? g("ccoIcon")("check", { size: 14 }) : "✓";
            return `<div class="result-desk-item is-done">
              <span class="result-desk-mark" aria-hidden="true">${icon}</span>
              <div class="result-desk-item-body">
                <strong>${esc(taskTitle(t))}</strong>
              </div>
            </div>`;
          })
          .join("");
      }
    }

    const loop = live?.inspect_loop;
    const issuePreview = (loop && loop.issue_preview) || [];

    if (missList) {
      const rows = [];
      miss.forEach((t) => {
        const b = taskBucket(t.status, t);
        const st = fiveStateLabel(b);
        const sum = taskErrorSummary(t);
        const route = routeLine(t);
        // P1-3 + P4-5: 步骤状态 + 执行方式 + 原因（概念 ≤3；不露 raw enum）
        const bits = [st];
        if (route) bits.push(route);
        if (sum) bits.push(sum);
        const xMark = typeof g("ccoIcon") === "function" ? g("ccoIcon")("x", { size: 14 }) : "×";
        rows.push(
          `<div class="result-desk-item is-miss">
            <span class="result-desk-mark" aria-hidden="true">${xMark}</span>
            <div class="result-desk-item-body">
              <strong>${esc(taskTitle(t))}</strong>
              ${bits.length > 0 ? `<span class="muted">${esc(bits.join(" · "))}</span>` : ""}
            </div>
          </div>`
        );
      });
      issuePreview.slice(0, 6).forEach((line) => {
        // P4-5: icons.js 暂无 alert-triangle，用 fallback "!"
        const warnIcon = "!";
        rows.push(
          `<div class="result-desk-item is-issue">
            <span class="result-desk-mark" aria-hidden="true">${warnIcon}</span>
            <div class="result-desk-item-body">${esc(String(line))}</div>
          </div>`
        );
      });
      if (!rows.length) {
        // P0-4: point at plan-compare footer (honest), not "验收" jargon alone
        missList.innerHTML = `<li class="muted">没有步骤失败；对照计划结论见下方</li>`;
      } else {
        missList.innerHTML = rows.join("");
      }
    }

    if (honest) {
      // P0-4: honestInspectCopy ↔ report「对照计划」同轮不矛盾
      const h = honestInspectCopy(loop);
      honest.hidden = h.hidden;
      honest.textContent = h.text;
    }

    // H3-1: merge_check one-liner (app DTO only; no JS strategy)
    const mergeEl = $("result-desk-merge-check");
    if (mergeEl) {
      const mc = (live && (live.merge_check || live.mergeCheck)) || "";
      if (mc) {
        mergeEl.hidden = false;
        mergeEl.textContent = String(mc);
      } else {
        mergeEl.hidden = true;
        mergeEl.textContent = "";
      }
    }

    // P2-1: plan checklist vs inspect side-by-side (live.verification DTO)
    renderVerificationPanel(live?.verification);

    // W3: browser evidence (shots / smoke / report) — DTO only; zoom / open via gateway
    const openPath =
      typeof g("ccoGateway")?.openPath === "function"
        ? (p) => g("ccoGateway").openPath(p)
        : undefined;
    renderBrowserEvidence(live, { $, openPath });

    // 回补/再写/先这样结束/顶栏结束 均不露出；结束本轮用 #btn-log-end-plan
    const btnBack = $("btn-ws-back-chat");
    if (btnBack) btnBack.hidden = true;

    const heading = $("task-dash-heading");
    if (heading) heading.textContent = "本轮结果";

    // Keep inspect strip in sync for finished state
    renderInspectLoopStrip(live, finished, active);
  }

  /**
   * P2-1: fill collapsible「原计划要验收」from live.verification.
   * Inspect is authoritative when source=inspect; plan list is sidebar only.
   * @param {object|null|undefined} verification
   */
  function renderVerificationPanel(verification) {
    const panel = $("result-desk-verify");
    const sum = $("result-desk-verify-sum");
    const note = $("result-desk-verify-note");
    const list = $("result-desk-verify-list");
    const tasksList = $("result-desk-verify-tasks");
    if (!panel) return;

    const v = verification || null;
    const source = v && v.source ? String(v.source) : "none";
    const planItems = (v && v.plan_items) || [];
    const taskItems = (v && v.task_items) || [];
    const planCount = Number(v && v.plan_count) || planItems.length;
    const total =
      planCount + (Array.isArray(taskItems) ? taskItems.length : 0);

    if (!v || source === "none" || total === 0) {
      // Still surface plan-only note when backend set one with zero items? hide.
      panel.hidden = true;
      if (list) list.innerHTML = "";
      if (tasksList) {
        tasksList.innerHTML = "";
        tasksList.hidden = true;
      }
      if (note) {
        note.hidden = true;
        note.textContent = "";
      }
      return;
    }

    panel.hidden = false;
    // Summary: count + source hint (no raw VERDICT).
    if (sum) {
      if (source === "inspect") {
        sum.textContent = `原计划要验收 · ${total} 条（巡检为准）`;
      } else {
        sum.textContent = `原计划要验收 · ${total} 条`;
      }
    }

    if (note) {
      const n = (v.plan_note && String(v.plan_note).trim()) || "";
      if (n) {
        note.hidden = false;
        note.textContent = n;
      } else if (source === "plan_only") {
        note.hidden = false;
        note.textContent = `计划写了 ${total} 条验收，本轮未自动对照`;
      } else {
        note.hidden = true;
        note.textContent = "";
      }
    }

    if (list) {
      if (!planItems.length) {
        list.innerHTML = "";
      } else {
        list.innerHTML = planItems
          .map((it) => {
            const text = String((it && it.text) || "").trim();
            if (!text) return "";
            const checked = !!(it && it.checked);
            const mark = checked ? "☑" : "☐";
            return `<div class="result-desk-item is-plan-check">
              <span class="result-desk-mark" aria-hidden="true">${mark}</span>
              <div class="result-desk-item-body">${esc(text)}</div>
            </div>`;
          })
          .filter(Boolean)
          .join("");
      }
    }

    if (tasksList) {
      if (!taskItems.length) {
        tasksList.innerHTML = "";
        tasksList.hidden = true;
      } else {
        tasksList.hidden = false;
        tasksList.innerHTML = taskItems
          .map((it) => {
            const tid = String((it && it.task_id) || "").trim();
            const text = String((it && it.text) || "").trim();
            if (!text) return "";
            const label = tid ? `${tid} · ${text}` : text;
            return `<div class="result-desk-item is-plan-task">
              <span class="result-desk-mark" aria-hidden="true">·</span>
              <div class="result-desk-item-body">${esc(label)}</div>
            </div>`;
          })
          .filter(Boolean)
          .join("");
      }
    }
  }

  /**
   * Combined paint used by RunView bridge.
   */
  function renderInspectAndResult(live, tasks, ctx) {
    pullLive(live);
    renderInspectLoopStrip(live, !!ctx?.finished, !!ctx?.active);
    renderResultDesk(live, tasks, ctx);
  }

  return {
    renderResultDesk,
    renderInspectLoopStrip,
    renderInspectAndResult,
    renderVerificationPanel,
    renderBrowserEvidence: (live) => {
      const openPath =
        typeof g("ccoGateway")?.openPath === "function"
          ? (p) => g("ccoGateway").openPath(p)
          : undefined;
      return renderBrowserEvidence(live, { $, openPath });
    },
    startRework: () => vm.startRework(),
    acceptResidual: () => vm.acceptResidual(),
    finishRound: () => vm.finishRound(),
  };
}

export default bindResultView;
