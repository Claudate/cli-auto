/**
 * [INPUT]: ccoRun / ccoResult / runApi · gateway
 * [OUTPUT]: stop/resume/rework/accept/openTerminal/export/handoff/doctor
 * [POS]: A5-2c features/run；删 classic invoke fallback（cco* 已就绪）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：start_run 旁路；rework 只走 start_rework / ccoResult。
 */

import * as gateway from "../../shared/gateway.js";
import { isAiInteractionEvent, eventPassesFilter } from "./logFilter.js";
import {
  g,
  S,
  $,
  esc,
  toast,
  callG
} from "./logHost.js";

/** multi-cli P2-6: render handoff Board strip from live view. */
export function renderHandoffBoardStrip() {
  const strip = $("handoff-board-strip");
  const rowsEl = $("handoff-board-rows");
  if (!strip || !rowsEl) return;
  const live = S().live || {};
  const board = live.handoff_board || live.handoffBoard || [];
  const mdPath = live.handoff_md_path || live.handoffMdPath || null;
  if (!board.length && !mdPath) {
    strip.hidden = true;
    rowsEl.innerHTML = "";
    return;
  }
  strip.hidden = false;
  const openBtn = $("btn-open-handoff");
  if (openBtn) {
    openBtn.disabled = !mdPath;
    openBtn.title = mdPath ? `打开 ${mdPath}` : "暂无 handoff.md";
  }
  if (!board.length) {
    rowsEl.innerHTML =
      '<span class="muted" style="font-size:0.75rem">账本已生成，尚无条目</span>';
    return;
  }
  rowsEl.innerHTML = board
    .map((r) => {
      const stRaw = String(r.status || "").toLowerCase();
      const finished = isFinishedStatus(stRaw);
      let cls = "handoff-board-chip";
      if (
        stRaw.includes("fail") ||
        stRaw.includes("timeout") ||
        stRaw.includes("error") ||
        stRaw.includes("abort")
      ) {
        cls += " is-fail";
      } else if (!finished) {
        cls += " is-run";
      } else {
        cls += " is-done";
      }
      // 进行中：平台 · 模型 · 状态（不显示金额）
      // 结束后：平台 · 模型 · 金额（不显示状态）
      const platform = platformLabel(r.provider);
      const model = modelLabel(r.model, r.provider);
      const parts = [
        `<span class="hb-platform">${esc(platform)}</span>`,
        `<span class="hb-sep" aria-hidden="true">·</span>`,
        `<span class="hb-model">${esc(model)}</span>`,
      ];
      if (finished) {
        parts.push(
          `<span class="hb-sep" aria-hidden="true">·</span>`,
          `<span class="hb-cost">${esc(amountLabel(r.cost))}</span>`
        );
      } else {
        parts.push(
          `<span class="hb-sep" aria-hidden="true">·</span>`,
          `<span class="hb-status">${esc(statusLabel(r.status))}</span>`
        );
      }
      const tip = [r.id, r.role, r.scope, r.status].filter(Boolean).join(" · ");
      return `<span class="${cls}" title="${esc(tip)}">${parts.join("")}</span>`;
    })
    .join("");
}

/** 任务是否已终态（完成后不显示状态、改显示金额） */
function isFinishedStatus(stRaw) {
  const s = String(stRaw || "").toLowerCase();
  if (!s) return false;
  if (
    s === "running" ||
    s === "starting" ||
    s === "queued" ||
    s === "pending" ||
    s === "paused"
  ) {
    return false;
  }
  // done / completed / skipped / failed / timeout / aborted / stopped …
  return true;
}

/** AI 平台人话短名（不暴露内部枚举长文） */
function platformLabel(provider) {
  const p = String(provider || "").trim().toLowerCase();
  if (p === "claude") return "Claude";
  if (p === "codex") return "Codex";
  if (p === "fake") return "模拟";
  if (p === "sdk") return "SDK";
  return p ? p : "—";
}

/** 模型名：plan 声明优先；未声明显示「默认」 */
function modelLabel(model, provider) {
  const m = String(model || "").trim();
  if (m) return m;
  const p = String(provider || "").trim().toLowerCase();
  if (p === "fake") return "—";
  return "默认";
}

/** 状态人话（主路径不堆英文枚举） */
function statusLabel(status) {
  const s = String(status || "").trim().toLowerCase();
  if (!s) return "—";
  if (s === "running" || s === "starting") return "运行中";
  if (s === "queued" || s === "pending") return "等待";
  if (s === "done" || s === "completed" || s === "skipped") return "完成";
  if (s.includes("fail") || s.includes("error")) return "失败";
  if (s.includes("timeout")) return "超时";
  if (s.includes("abort") || s === "stopped" || s === "cancelled") return "已停";
  if (s === "paused") return "暂停";
  return String(status).trim();
}

/** 金额：有则 $x.xxx，无则 —（不伪装 $0） */
function amountLabel(cost) {
  if (cost == null || cost === "") return "—";
  const n = Number(cost);
  if (!Number.isFinite(n)) return "—";
  return `$${n.toFixed(3)}`;
}

export async function openHandoffLedger() {
  const path =
    S().live?.handoff_md_path || S().live?.handoffMdPath || null;
  if (!path) {
    toast("当前运行尚无 handoff.md");
    return;
  }
  try {
    await gateway.openPath(path);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** P2-3: export visible task logs as Markdown download. */
export function exportBoardLogsMd() {
  const tasks = Array.isArray(S().live?.tasks) ? S().live.tasks : [];
  if (!tasks.length) {
    toast("没有可导出的任务日志");
    return;
  }
  const filter = S().cliStatusFilter || "all";
  const shown =
    filter && filter !== "all"
      ? tasks.filter((t) => callG("taskBucket", t) === filter)
      : tasks;
  const runId = S().live?.run_id || S().live?.runId || "run";
  const lines = [];
  lines.push(`# cco 执行日志导出`);
  lines.push("");
  lines.push(`- run: \`${runId}\``);
  lines.push(
    `- project: \`${S().live?.project_path || S().selectedPath || ""}\``
  );
  lines.push(`- exported: ${new Date().toISOString()}`);
  lines.push(`- filter: ${filter}`);
  lines.push("");
  for (const t of shown) {
    lines.push(`## ${t.title || t.task_id} (\`${t.task_id}\`)`);
    lines.push("");
    lines.push(
      `- status: **${t.status}** · provider: \`${t.provider || "?"}\``
    );
    if (t.error_summary || t.error) {
      lines.push(`- error: ${t.error_summary || t.error}`);
    }
    lines.push("");
    const events = (Array.isArray(t.log_events) ? t.log_events : [])
      .filter(isAiInteractionEvent)
      .filter((e) => eventPassesFilter(e, S().logEventFilter));
    if (events.length) {
      lines.push("```");
      for (const e of events.slice(-80)) {
        lines.push([e.kind, e.title, e.summary].filter(Boolean).join(" · "));
      }
      lines.push("```");
    } else if (t.log_tail) {
      lines.push("```");
      lines.push(String(t.log_tail).slice(-4000));
      lines.push("```");
    } else {
      lines.push("_无日志_");
    }
    lines.push("");
  }
  const blob = new Blob([lines.join("\n")], {
    type: "text/markdown;charset=utf-8",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `cco-log-${String(runId).replace(/[^\w.-]+/g, "_")}.md`;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 0);
  toast(`已导出 ${shown.length} 个任务日志`);
}

export async function openExternalTerminal(taskId) {
  const runId = S().live?.run_id;
  if (!runId || !taskId) return toast("无运行中的任务日志可跟随");
  const cco = g("ccoRun");
  try {
    let session;
    if (cco && typeof cco.vm?.openTerminal === "function") {
      session = await cco.vm.openTerminal({
        runId,
        taskId,
        kind: "external",
      });
    } else {
      session = await gateway.openTaskTerminal({
        runId,
        taskId,
        kind: "external",
      });
    }
    const launcher = session?.launcher || "terminal";
    toast(`已打开外置终端（${launcher}）跟随 ${taskId}`);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/** Prefer ccoRun (A4+); no classic invoke fallback. */
export async function cancelTask() {
  const cco = g("ccoRun");
  if (cco && typeof cco.stopTask === "function") {
    return cco.stopTask(S().selectedTaskId);
  }
  toast("执行台未就绪，请稍后重试");
}

export async function stopAll() {
  const cco = g("ccoRun");
  if (cco && typeof cco.stopAll === "function") {
    return cco.stopAll();
  }
  toast("执行台未就绪，请稍后重试");
}

export async function resumeRun() {
  const cco = g("ccoRun");
  if (cco && typeof cco.resume === "function") {
    return cco.resume();
  }
  toast("执行台未就绪，请稍后重试");
}

/** Card「再跑一次」: re-run one failed task (not re-split). */
export async function retryTask(taskId) {
  const cco = g("ccoRun");
  const id = taskId || S().selectedTaskId;
  if (cco && typeof cco.retryTask === "function") {
    return cco.retryTask(id);
  }
  toast("执行台未就绪，请稍后重试");
}

/** P-loop L2: rework via app start_rework (ccoResult). */
export async function startReworkWave() {
  const cco = g("ccoResult");
  if (cco && typeof cco.startRework === "function") {
    return cco.startRework();
  }
  toast("结果台未就绪，请稍后重试");
}

/** P-loop L2: accept residual (ccoResult). */
export async function acceptRunResidual() {
  const cco = g("ccoResult");
  if (cco && typeof cco.acceptResidual === "function") {
    return cco.acceptResidual();
  }
  toast("结果台未就绪，请稍后重试");
}

export async function loadDoctor() {
  const cco = g("ccoSettings");
  if (cco && typeof cco.loadDoctor === "function") {
    return cco.loadDoctor();
  }
  // gateway-only fallback if settings desk not yet mounted
  try {
    const d = await gateway.doctor(S().selectedPath || null);
    S().doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    const list = $("doctor-list");
    if (list) {
      list.innerHTML = `<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th></tr></thead>
      <tbody>
        ${lines
          .map(
            (l) => `<tr>
          <td>${esc(l.name)}</td>
          <td>${callG("badge", l.ok ? "ok" : "failed") || (l.ok ? "ok" : "fail")}</td>
          <td class="muted">${esc(l.detail)}</td>
        </tr>`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">${
      d.ok ? "全部检查通过" : "存在失败项，请按详情处理"
    }</p>`;
    }
    callG("renderDoctorWarn")();
  } catch (e) {
    toast(String(e));
  }
}
