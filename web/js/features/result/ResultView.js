/**
 * [INPUT]: ResultViewModel · live/tasks · 既有 #result-desk DOM
 * [OUTPUT]: 结果摘要 + inspect 人话 + live 费用句 + 回补/接受 CTA 可见性
 * [POS]: A4-3/A4-4 · P0-1/P0-4/P1-3/P2-1 ResultView；禁止 invoke / 解析 VERDICT 正文
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * P0-4: 对照计划用语经 inspectCopy（与 report fallback 同词）；费用仍用
 * resultSummary 本地拼句（未下沉 Rust — 规则简单、无第二套格式）。
 * P1-3: miss 行展示 live.route_label（App 拼好人话）；主路径不露 raw route_source enum。
 * P2-1: live.verification → 可折叠「原计划要验收」副栏（巡检为准 / 未自动对照）。
 * 第一屏标题固定「本轮结果」，不写 run_id。
 */

import {
  inspectStripParts,
  honestInspectCopy,
  inspectActionVisibility,
} from "./inspectCopy.js";
import { formatLiveCostPhrase } from "./resultSummary.js";
import {
  taskBucket,
  fiveStateLabel,
} from "../run/runBuckets.js";

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
    const btnRework = $("btn-ws-rework");
    const btnAccept = $("btn-ws-accept-residual");
    const loop = live?.inspect_loop;
    if (!strip) return;

    const parts = inspectStripParts(loop);
    if (parts.kind === "empty" || !parts.bits.length) {
      strip.hidden = true;
      strip.textContent = "";
      if (btnRework) btnRework.hidden = true;
      if (btnAccept) btnAccept.hidden = true;
      return;
    }

    strip.hidden = false;
    strip.textContent = parts.bits.join(" · ");
    strip.classList.toggle("bad", parts.kind === "bad");
    strip.classList.toggle("ok", parts.kind === "ok");

    const vis = inspectActionVisibility(loop, {
      finished: !!finished,
      active: !!active,
    });
    if (btnRework) btnRework.hidden = !vis.canRework;
    if (btnAccept) btnAccept.hidden = !vis.showAccept;
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
    const finishBtn = $("btn-ws-finish");
    if (!show) {
      if (finishBtn) finishBtn.hidden = true;
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
      // First bits stay completion ratio / elapsed; cost is a trailing phrase only.
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
      // P0-1: always append human cost (or 「费用未汇总」); never fake $0.00.
      bits.push(formatLiveCostPhrase(live));
      planLine.textContent = bits.join(" · ");
    }

    if (doneList) {
      if (!done.length) {
        doneList.innerHTML = `<li class="muted">本轮没有标记为已完成的步骤</li>`;
      } else {
        doneList.innerHTML = done
          .map(
            (t) =>
              `<li class="result-desk-item is-done"><span class="result-desk-mark" aria-hidden="true">${typeof g("ccoIcon") === "function" ? g("ccoIcon")("check", { size: 12 }) : "✓"}</span>${esc(
                taskTitle(t)
              )}</li>`
          )
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
        // P1-3: 步骤状态 + 执行方式 + 原因（概念 ≤3；不露 raw enum）
        const bits = [st];
        if (route) bits.push(route);
        if (sum) bits.push(sum);
        rows.push(
          `<li class="result-desk-item is-miss"><span class="result-desk-mark" aria-hidden="true">·</span><span class="result-desk-item-body"><strong>${esc(
            taskTitle(t)
          )}</strong><span class="muted"> · ${esc(
            bits.join(" · ")
          )}</span></span></li>`
        );
      });
      issuePreview.slice(0, 6).forEach((line) => {
        rows.push(
          `<li class="result-desk-item is-issue"><span class="result-desk-mark" aria-hidden="true">!</span>${esc(
            String(line)
          )}</li>`
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

    // P2-1: plan checklist vs inspect side-by-side (live.verification DTO)
    renderVerificationPanel(live?.verification);

    // C3: decision tree — miss → rework/accept; clean → 完成并回写计划 + 再写一份
    const hasMiss =
      miss.length > 0 ||
      (loop &&
        (loop.can_rework ||
          loop.blocking_count > 0 ||
          String(loop.verdict || "").toUpperCase() === "FAIL" ||
          loop.residual_count > 0));
    const btnBack = $("btn-ws-back-chat");
    if (finishBtn) {
      if (hasMiss && loop?.can_rework) {
        // accept residual is the soft exit; avoid three similar CTAs
        finishBtn.hidden = true;
      } else if (hasMiss) {
        finishBtn.hidden = false;
        finishBtn.textContent = "先这样结束";
        finishBtn.classList.remove("primary");
        finishBtn.classList.add("ghost");
      } else {
        finishBtn.hidden = false;
        finishBtn.textContent = "完成并回写计划";
        finishBtn.classList.add("primary");
        finishBtn.classList.remove("ghost");
      }
    }
    if (btnBack) {
      btnBack.hidden = false;
      btnBack.textContent = "再写一份";
    }

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
            return `<li class="result-desk-item is-plan-check"><span class="result-desk-mark" aria-hidden="true">${mark}</span>${esc(
              text
            )}</li>`;
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
            return `<li class="result-desk-item is-plan-task"><span class="result-desk-mark" aria-hidden="true">·</span>${esc(
              label
            )}</li>`;
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
    startRework: () => vm.startRework(),
    acceptResidual: () => vm.acceptResidual(),
    finishRound: () => vm.finishRound(),
  };
}

export default bindResultView;
