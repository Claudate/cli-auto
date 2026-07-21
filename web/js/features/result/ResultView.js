/**
 * [INPUT]: ResultViewModel · live/tasks · 既有 #result-desk DOM
 * [OUTPUT]: 结果摘要 + inspect 人话 + 回补/接受 CTA 可见性
 * [POS]: A4-3/A4-4 ResultView；禁止 invoke / 解析 VERDICT 正文
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
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
          .map(
            (t) =>
              `<li class="result-desk-item is-done"><span class="result-desk-mark" aria-hidden="true">✓</span>${esc(
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
        rows.push(
          `<li class="result-desk-item is-miss"><span class="result-desk-mark" aria-hidden="true">·</span><span class="result-desk-item-body"><strong>${esc(
            taskTitle(t)
          )}</strong><span class="muted"> · ${esc(st)}${
            sum ? " · " + esc(sum) : ""
          }</span></span></li>`
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
        missList.innerHTML = `<li class="muted">没有步骤失败；见下方是否做过对照验收</li>`;
      } else {
        missList.innerHTML = rows.join("");
      }
    }

    if (honest) {
      const h = honestInspectCopy(loop);
      honest.hidden = h.hidden;
      honest.textContent = h.text;
    }

    if (finishBtn) {
      const loopCan =
        loop &&
        (loop.can_rework ||
          loop.blocking_count > 0 ||
          String(loop.verdict || "").toUpperCase() === "FAIL" ||
          loop.residual_count > 0);
      finishBtn.hidden = false;
      finishBtn.textContent =
        loopCan && loop?.can_rework ? "结束本轮（不回补）" : "结束本轮";
    }

    const heading = $("task-dash-heading");
    if (heading) heading.textContent = "本轮结果";

    // Keep inspect strip in sync for finished state
    renderInspectLoopStrip(live, finished, active);
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
    startRework: () => vm.startRework(),
    acceptResidual: () => vm.acceptResidual(),
    finishRound: () => vm.finishRound(),
  };
}

export default bindResultView;
