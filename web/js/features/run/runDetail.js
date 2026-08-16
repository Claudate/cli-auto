/**
 * [INPUT]: live task DTO · logContent · logBoardCard · logHost · ccoIcon
 * [OUTPUT]: #run-detail-column 右次级列渲染（Terminal/Diff/Read 卡 · wait/stall 琥珀条 · 日志折叠）
 * [POS]: P4-4 features/run；无新 IPC · 停/续/重跑仍经 ccoRun → runApi → gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 只渲染 app 下发 DTO；琥珀条只用既有 wait（排队）/ stall（卡住）语义，不造假「等待审批」。
 * 几何瞬态（展开/折叠）只在会话内，不入 localStorage。
 */

import { S, $, esc, toast } from "./logHost.js";
import { taskBucket } from "./runBuckets.js";
import { stallStripText } from "./logBoardCard.js";
import { fillPanelLogBody } from "./logContent.js";
import { statusDot, fiveStateLabel } from "../../shared/statusUi.js";

function safeBucket(t) {
  try {
    return taskBucket(t) || "wait";
  } catch (_) {
    return "wait";
  }
}

/** 最近一次工具调用命令（tool_use.detail JSON → command/path）。 */
function parseToolCommand(e) {
  const d = e?.detail;
  if (d) {
    try {
      const o = JSON.parse(d);
      if (typeof o?.command === "string") return String(o.command);
      if (typeof o?.input?.command === "string") return String(o.input.command);
      if (typeof o?.path === "string") return String(o.path);
      if (typeof o?.file_path === "string") return String(o.file_path);
      if (typeof o?.input?.file_path === "string") return String(o.input.file_path);
    } catch (_) {
      /* detail 非 JSON → 当命令原文 */
    }
    return String(d).split("\n")[0].slice(0, 160);
  }
  return String(e?.summary || e?.title || "").slice(0, 160);
}

/** 命令 + 最近一次工具输出（诚实读 log_events；不发明内容）。 */
function toolActivity(t) {
  const events = Array.isArray(t?.log_events) ? t.log_events : [];
  let command = "";
  let output = "";
  for (const e of events) {
    const k = String(e.kind || "").toLowerCase();
    if (k === "tool_use") {
      command = parseToolCommand(e);
    } else if (k === "tool_result") {
      const txt = String(e.detail || e.summary || "").trim();
      if (txt) output = txt;
    }
  }
  return { command, output };
}

/** 读写文件路径（tool_use 事件里 JSON 的 path/file_path；数量受限，诚实计数在页脚）。 */
function filePathsFromEvents(t) {
  const writes = [];
  const reads = [];
  const events = Array.isArray(t?.log_events) ? t.log_events : [];
  const WRITE_RE = /write|edit|insert|patch|replace|create|mkdir|copy|delete|rm\b|mv\b/i;
  const READ_RE = /read|view|grep|list|ls\b|show|cat\b/i;
  for (const e of events) {
    if (String(e.kind || "").toLowerCase() !== "tool_use") continue;
    const nm = String(e.title || "");
    let p = "";
    const d = e?.detail;
    if (d) {
      try {
        const o = JSON.parse(d);
        p = String(
          o?.file_path ||
            o?.path ||
            o?.full_path ||
            o?.input?.file_path ||
            o?.input?.path ||
            ""
        );
      } catch (_) {
        /* 非 JSON */
      }
    }
    if (!p) continue;
    const rel = p.replace(/^file:\/\//, "");
    if (WRITE_RE.test(nm) && writes.length < 6) writes.push(rel);
    else if (READ_RE.test(nm) && reads.length < 6) reads.push(rel);
  }
  return { writes, reads };
}

function pillClass(bucket) {
  if (bucket === "done") return "is-ok";
  if (bucket === "fail") return "is-danger";
  if (bucket === "stall") return "is-warn";
  if (bucket === "run") return "is-brand";
  return "";
}

/** 琥珀条：只用既有 wait / stall DTO 语义承接「等待」，不造假概念。 */
function amberText(t, bucket) {
  if (bucket === "stall") {
    const s = stallStripText(t);
    return s || "本步较久没有新进展，可停止后重试";
  }
  if (bucket === "wait") {
    const w = Array.isArray(t?.waiting_on) ? t.waiting_on : [];
    return w.length
      ? `排队等待 ${w.length} 个前序步骤完成`
      : "排队等待开始";
  }
  return "";
}

function terminalHtml(t, bucket, command, output) {
  const cmd = command || "本步暂无工具调用";
  const out = output
    ? `<pre class="run-detail-term-out">${esc(String(output).slice(0, 2000))}</pre>`
    : `<div class="run-detail-term-empty">暂无命令输出</div>`;
  return `
    <div class="run-detail-term">
      <div class="run-detail-term-head">
        <span class="run-detail-term-ico" data-icon="terminal" data-icon-size="13"></span>
        <span class="run-detail-term-cmd" title="${esc(cmd)}">${esc(cmd)}</span>
        <span class="pill ${pillClass(bucket)}">${esc(fiveStateLabel(bucket))}</span>
        <button type="button" class="icon-btn sm" data-copy-term title="复制命令与输出" aria-label="复制命令与输出" data-icon="copy" data-icon-size="13"></button>
      </div>
      ${out}
    </div>`;
}

function diffBlockHtml(t, writes) {
  const commit = t?.auto_commit || null;
  const files = Array.isArray(commit?.files) ? commit.files : [];
  if (commit && files.length) {
    const hash = String(commit.commit_hash || "").slice(0, 8);
    const pushed = commit.pushed ? " · 已 Push" : "";
    const rows = files
      .slice(0, 6)
      .map(
        (f) =>
          `<div class="run-detail-file"><span data-icon="file" data-icon-size="12"></span>${esc(
            String(f).replace(/^file:\/\//, "")
          )}</div>`
      )
      .join("");
    const more = files.length > 6 ? ` … +${files.length - 6}` : "";
    return `
      <div class="diff-block">
        <div class="run-detail-block-head"><span data-icon="git-branch" data-icon-size="13"></span>自动提交</div>
        ${rows}
        <div class="diff-foot">git 记录 ${files.length} 个变更文件${hash ? ` · ${hash}` : ""}${pushed}${more}</div>
      </div>`;
  }
  if (writes.length) {
    const rows = writes
      .map(
        (f) =>
          `<div class="run-detail-file"><span data-icon="file" data-icon-size="12"></span>${esc(f)}</div>`
      )
      .join("");
    return `
      <div class="diff-block">
        <div class="run-detail-block-head"><span data-icon="git-branch" data-icon-size="13"></span>写文件</div>
        ${rows}
        <div class="diff-foot">本步写入 ${writes.length} 个文件（自动提交未记录）</div>
      </div>`;
  }
  return "";
}

function readBlockHtml(reads) {
  if (!reads.length) return "";
  const rows = reads
    .map(
      (f) =>
        `<div class="run-detail-file"><span data-icon="file" data-icon-size="12"></span>${esc(f)}</div>`
    )
    .join("");
  return `
    <div class="read-block">
      <div class="run-detail-block-head"><span data-icon="file" data-icon-size="13"></span>读文件</div>
      ${rows}
      <div class="read-foot">本步读入 ${reads.length} 个文件</div>
    </div>`;
}

function logDisclosureHtml(t, isOpen) {
  const n = Array.isArray(t?.log_events) ? t.log_events.length : 0;
  return `<details class="disclosure-row run-detail-log" data-log-disclosure${isOpen ? " open" : ""}>
    <summary><span data-icon="terminal" data-icon-size="12"></span> 详细日志${n ? ` · ${n} 条事件` : ""}</summary>
    <div class="disclosure-body">
      <div class="run-detail-log-console log-console term-mode" data-log-body></div>
    </div>
  </details>`;
}

function buildDetailHtml(t, bucket, prevOpen) {
  const { command, output } = toolActivity(t);
  const { writes, reads } = filePathsFromEvents(t);
  const amber = amberText(t, bucket);
  const sum = String(t?.error_summary || t?.error || "").trim();
  const parts = [];
  if (amber) parts.push(`<div class="run-detail-amber">${esc(amber)}</div>`);
  if (bucket === "fail" && sum) {
    parts.push(`<div class="run-detail-error">${esc(sum.slice(0, 300))}</div>`);
  }
  parts.push(terminalHtml(t, bucket, command, output));
  parts.push(diffBlockHtml(t, writes));
  parts.push(readBlockHtml(reads));
  parts.push(logDisclosureHtml(t, prevOpen));
  return parts.filter(Boolean).join("\n");
}

/** 重绘签名：内容未变则跳过 innerHTML（对齐 logPanelSignature 思路）。 */
function detailSignature(t, bucket) {
  if (!t) return "";
  const commit = t.auto_commit
    ? [
        String(t.auto_commit.commit_hash || ""),
        t.auto_commit.ok ? 1 : 0,
        (t.auto_commit.files || []).length,
      ].join("|")
    : "";
  return [
    t.task_id || "",
    bucket,
    String(t.status || ""),
    stallStripText(t) ? 1 : 0,
    String(t.error_summary || t.error || "").slice(0, 60),
    commit,
    String(t.log_tail || "").slice(-40),
    t.log_bytes ?? "",
    (Array.isArray(t.log_events) ? t.log_events : []).length,
  ].join("\0");
}

function renderHead(t) {
  const dotEl = $("run-detail-dot");
  const titleEl = $("run-detail-title");
  if (!t) {
    if (dotEl) dotEl.className = "dot";
    if (titleEl) titleEl.textContent = "任务详情";
    return;
  }
  const bucket = safeBucket(t);
  const dotCls = statusDot(String(t.status || ""), t) || "";
  if (dotEl) dotEl.className = "dot" + (dotCls ? " " + dotCls : "");
  if (titleEl) {
    titleEl.textContent = String(t.title || t.task_id || "任务详情").slice(0, 60);
    titleEl.title = String(t.title || t.task_id || "");
  }
}

function bindDetailEvents(bodyEl, t) {
  const det = bodyEl.querySelector("[data-log-disclosure]");
  if (det) {
    det.addEventListener("toggle", () => {
      if (!S().runDetailLog) S().runDetailLog = {};
      S().runDetailLog[t.task_id] = det.open;
    });
  }
  const copy = bodyEl.querySelector("[data-copy-term]");
  if (copy) {
    copy.onclick = async () => {
      const { command, output } = toolActivity(t);
      const text = [command, output].filter(Boolean).join("\n\n");
      try {
        await navigator.clipboard.writeText(text || "");
        toast("终端输出已复制");
      } catch (_) {
        toast("复制失败");
      }
    };
  }
}

/** 渲染右次级列（选中任务 → Terminal/Diff/Read · 琥珀条 · 日志折叠）。 */
export function render(tasks, live) {
  const bodyEl = $("run-detail-body");
  if (!bodyEl) return;
  const list = Array.isArray(tasks) ? tasks : [];
  const sel = S().selectedTaskId;
  const t = list.find((x) => x.task_id === sel) || list[0] || null;
  renderHead(t);
  if (!t) {
    if (bodyEl.dataset.sig !== "") {
      bodyEl.innerHTML =
        '<div class="run-detail-empty">点流程卡右上角「聚焦」查看该步骤的终端输出与文件变更。</div>';
      bodyEl.dataset.sig = "";
      if (typeof window.ccoHydrateIcons === "function") {
        window.ccoHydrateIcons(bodyEl);
      }
    }
    return;
  }
  const bucket = safeBucket(t);
  const sig = detailSignature(t, bucket);
  if (bodyEl.dataset.sig === sig) return;
  const prevOpen = !!(S().runDetailLog && S().runDetailLog[t.task_id]);
  bodyEl.innerHTML = buildDetailHtml(t, bucket, prevOpen);
  bodyEl.dataset.sig = sig;
  if (typeof window.ccoHydrateIcons === "function") {
    window.ccoHydrateIcons(bodyEl);
  }
  // 日志折叠：fillPanelLogBody 保留 logVirtual 虚拟列表
  const logBody = bodyEl.querySelector("[data-log-body]");
  if (logBody) fillPanelLogBody(logBody, t, { stick: false });
  bindDetailEvents(bodyEl, t);
}

/**
 * 绑定 #btn-run-detail-toggle / #btn-run-detail-close；窄窗优先折叠。
 * @param {{ vm?: object }} [deps] deps.vm 须暴露 detailCollapsed / toggleDetailCollapsed / setDetailCollapsed
 */
export function installRunDetail({ vm } = {}) {
  const toggle = $("btn-run-detail-toggle");
  const closeBtn = $("btn-run-detail-close");
  const aside = $("run-detail-column");

  function collapsed() {
    return !!(vm && typeof vm.getSnapshot === "function" && vm.getSnapshot().detailCollapsed);
  }
  function applyVisibility() {
    const c = collapsed();
    if (aside) aside.hidden = c;
    if (toggle) toggle.setAttribute("aria-pressed", c ? "false" : "true");
  }
  function toggleDetail() {
    if (vm && typeof vm.toggleDetailCollapsed === "function") vm.toggleDetailCollapsed();
    applyVisibility();
  }

  if (toggle && !toggle.dataset.ccoA2Wired) {
    toggle.dataset.ccoA2Wired = "1";
    toggle.onclick = toggleDetail;
  }
  if (closeBtn && !closeBtn.dataset.ccoA2Wired) {
    closeBtn.dataset.ccoA2Wired = "1";
    closeBtn.onclick = toggleDetail;
  }
  // 窄窗优先折叠：仅作会话默认，用户可再展开；几何瞬态不入 localStorage
  if (vm && typeof vm.setDetailCollapsed === "function") {
    const narrow = typeof window !== "undefined" && window.innerWidth < 1160;
    vm.setDetailCollapsed(narrow);
  }
  applyVisibility();

  return { render, toggleDetail, applyVisibility };
}

export default { render, installRunDetail };
