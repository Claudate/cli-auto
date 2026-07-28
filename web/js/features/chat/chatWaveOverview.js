/**
 * [INPUT]: wave sibling plan items + optional split index / job views
 * [OUTPUT]: W3 Bundle 总览 DTO + HTML（人话；无引擎名第一句）
 * [POS]: features/chat — pure; confirm 仍走 gateway.confirmStart
 * [PROTOCOL]: 对照 docs/.../05-modular-split-run · landing W3；禁止 start_run
 */

import {
  isWaveIndexPath,
  waveDirKeyFromPath,
  waveSiblingPlans,
} from "./chatWavePlans.js";

/**
 * @typedef {{
 *   path: string,
 *   title: string,
 *   status: string,
 *   statusLabel: string,
 *   taskCount: number|null,
 *   jobId: string|null,
 *   canConfirm: boolean,
 *   canSplit: boolean,
 *   everCompleted: boolean,
 * }} WavePlanRow
 */

/**
 * @param {object} opts
 * @param {string} opts.path current selection
 * @param {Array} opts.allItems plan list
 * @param {Record<string, object>} [opts.splitByPath]
 * @param {Record<string, object>} [opts.jobsByPath] path → plan job view
 * @param {(p:string)=>string} [opts.norm]
 */
export function buildWaveOverview(opts = {}) {
  const path = opts.path || "";
  const key = waveDirKeyFromPath(path);
  if (!key) return null;
  const all = opts.allItems || [];
  const execPlans = waveSiblingPlans(path, all);
  const norm =
    opts.norm ||
    ((p) => p);
  const splitBy = opts.splitByPath || {};
  const jobsBy = opts.jobsByPath || {};

  /** @type {WavePlanRow[]} */
  const rows = execPlans.map((it) => {
    const p = it.path || "";
    const n = norm(p);
    const job = jobsBy[n] || jobsBy[p] || null;
    const split =
      splitBy[n] ||
      splitBy[p] ||
      null;
    const st = String(job?.status || split?.status || "").toLowerCase();
    const taskCount =
      job?.task_count ??
      job?.taskCount ??
      (Array.isArray(job?.tasks) ? job.tasks.length : null) ??
      split?.task_count ??
      null;
    const ever =
      !!it.ever_completed ||
      st === "done" ||
      st === "completed" ||
      st === "running";
    let status = "idle";
    let statusLabel = "未拆";
    if (st === "planning") {
      status = "planning";
      statusLabel = "拆分中";
    } else if (st === "planned" || st === "confirmed") {
      status = "planned";
      statusLabel = st === "confirmed" ? "已确认待跑/在跑" : "已拆好·可确认";
    } else if (st === "running") {
      status = "running";
      statusLabel = "执行中";
    } else if (st === "done" || st === "completed") {
      status = "done";
      statusLabel = "本轮做完";
    } else if (st === "plan_failed" || st === "failed") {
      status = "failed";
      statusLabel = "拆分失败·可重拆";
    } else if (split || ever) {
      status = "has_split";
      statusLabel = ever ? "有过执行" : "有拆分记录";
    }
    const jobId = job?.job_id || job?.jobId || split?.job_id || null;
    return {
      path: p,
      title: it.title || p.split("/").pop() || p,
      status,
      statusLabel,
      taskCount: taskCount != null ? Number(taskCount) : null,
      jobId: jobId ? String(jobId) : null,
      canConfirm: status === "planned" && !!jobId,
      canSplit: status === "idle" || status === "failed",
      everCompleted: ever,
    };
  });

  const n = rows.length;
  const ready = rows.filter((r) => r.canConfirm).length;
  const needSplit = rows.filter((r) => r.canSplit).length;
  const done = rows.filter((r) => r.status === "done" || r.everCompleted).length;
  const failed = rows.filter((r) => r.status === "failed").length;
  const running = rows.filter((r) => r.status === "running" || r.status === "planning")
    .length;

  let closeout = "本波还在推进";
  if (n === 0) closeout = "本波还没有执行计划";
  else if (failed > 0 && done + ready === 0) closeout = "有计划拆分失败，可只重拆失败的那份";
  else if (done >= n && n > 0) closeout = "本波执行计划都有完成记录（请仍以结果台为准）";
  else if (ready === n) closeout = "本波都已拆好，可串行确认开跑";
  else if (ready > 0) closeout = `${ready} 份可确认 · ${needSplit} 份还需拆开`;
  else if (needSplit === n) closeout = "本波都还没拆，请逐份「拆成步骤」";

  return {
    key,
    label: key.split("/").filter(Boolean).pop() || key,
    isIndex: isWaveIndexPath(path),
    planCount: n,
    rows,
    readyCount: ready,
    needSplitCount: needSplit,
    doneCount: done,
    failedCount: failed,
    runningCount: running,
    /** 默认串行：不提供真并行开关 */
    parallelPolicy: "serial",
    parallelLabel: "默认一份一份来（同仓不并行开跑，避免互相踩）",
    closeout,
  };
}

/**
 * @param {ReturnType<typeof buildWaveOverview>} ov
 * @param {(s:string)=>string} esc
 */
export function renderWaveOverviewHtml(ov, esc) {
  if (!ov) return "";
  const rows = (ov.rows || [])
    .map((r, i) => {
      const tc =
        r.taskCount != null ? ` · ${r.taskCount} 步` : "";
      return (
        `<li class="wave-ov-row" data-status="${esc(r.status)}">` +
        `<button type="button" class="linkish" data-plans-mgmt="${esc(r.path)}">` +
        `${i + 1}. ${esc(r.title)}` +
        `</button>` +
        `<span class="wave-ov-st muted">${esc(r.statusLabel)}${esc(tc)}</span>` +
        `</li>`
      );
    })
    .join("");

  const confirmBtn =
    ov.readyCount > 0
      ? `<button type="button" class="btn primary sm" data-wave-confirm-batch="1" data-wave-key="${esc(
          ov.key
        )}" title="对已拆好的计划依次调用确认开跑（同一 confirm，不旁路）">确认本波已拆好的（串行 · ${
          ov.readyCount
        }）</button>`
      : "";
  const splitBtn =
    ov.needSplitCount > 0
      ? `<button type="button" class="btn ghost sm" data-wave-split-next="1" data-wave-key="${esc(
          ov.key
        )}" title="打开下一份未拆计划的拆分台">拆下一份未拆的</button>`
      : "";

  return (
    `<div class="wave-overview" data-wave-overview="1" data-wave-key="${esc(ov.key)}">` +
    `<div class="wave-ov-head">` +
    `<strong>本波 · ${esc(ov.label)}</strong>` +
    `<span class="muted"> ${ov.planCount} 份执行计划</span>` +
    `</div>` +
    `<p class="wave-ov-policy muted">${esc(ov.parallelLabel)}</p>` +
    `<p class="wave-ov-closeout">${esc(ov.closeout)}</p>` +
    (rows
      ? `<ol class="wave-ov-list">${rows}</ol>`
      : `<p class="muted">暂无执行计划文件</p>`) +
    `<div class="wave-ov-actions">${splitBtn}${confirmBtn}</div>` +
    `<p class="wave-ov-foot muted">开跑仍经确认闸；optional 不会静默勾上。失败只重拆那一份。</p>` +
    `</div>`
  );
}
