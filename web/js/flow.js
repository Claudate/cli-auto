/**
 * [INPUT]: state · localStorage
 * [OUTPUT]: 主路径流程文案 · 趣味旁白 · 引擎名默认隐藏
 * [POS]: web/js 共享；plan/monitor/log/confirm 只绑流程词，不绑 CLI 品牌
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — flow copy (no CLI brand on main path) */

const FLOW_FUN_KEY = "cco.flowFun";

function flowFunEnabled() {
  try {
    const v = localStorage.getItem(FLOW_FUN_KEY);
    if (v === "0" || v === "false") return false;
    return true; // default on
  } catch (_) {
    return true;
  }
}

function setFlowFunEnabled(on) {
  try {
    localStorage.setItem(FLOW_FUN_KEY, on ? "1" : "0");
  } catch (_) {}
}

/** Main path never shows engine brand; settings/advanced keep real names. */
function flowHideEngineBadge() {
  return true;
}

const FLOW_BLURBS = {
  planning: [
    "大计划切成小块，一块一块啃。",
    "先理清步骤，再动手不慌。",
    "正在画路线图，马上就好。",
    "把长文收成可执行清单…",
  ],
  confirm: [
    "菜单好了——要加菜还是直接上灶？",
    "步骤已摆好，确认后开跑。",
    "瞄一眼依赖，再按开始。",
  ],
  running: [
    "按波次推进中，候场步骤请稍等。",
    "有人开工，有人排队，都正常。",
    "一步一步来，日志在下面折叠着。",
  ],
  wait: [
    "前置还没打完，先喝口水。",
    "在等上游步骤交棒。",
  ],
  stall: [
    "好像走神了，拍拍肩膀再试一次。",
    "半天没动静，自动再推一把。",
  ],
  done: [
    "收工。勾选与报告在结果里。",
    "本轮步骤跑完了。",
  ],
  fail: [
    "先停住，看看是哪一步绊倒了。",
    "有步骤失败，已暂停后续。",
  ],
};

function flowPickBlurb(kind, salt) {
  if (!flowFunEnabled()) return "";
  const list = FLOW_BLURBS[kind] || FLOW_BLURBS.running;
  if (!list.length) return "";
  let n = 0;
  const s = String(salt || kind);
  for (let i = 0; i < s.length; i++) n = (n + s.charCodeAt(i) * (i + 1)) % 997;
  return list[n % list.length];
}

function flowJoinSeriousFun(serious, fun) {
  const a = String(serious || "").trim();
  const b = String(fun || "").trim();
  if (!b) return a;
  if (!a) return b;
  return `${a} · ${b}`;
}

/** Planning phase subtitle (no Claude/CLI brand). */
function flowPlanningSub(elapsedSec) {
  const sec = Math.max(0, Number(elapsedSec) || 0);
  const serious =
    sec > 0
      ? `正在把计划拆成可执行步骤（已等待 ${sec}s）…`
      : "正在把计划拆成可执行步骤…";
  return flowJoinSeriousFun(serious, flowPickBlurb("planning", String(sec)));
}

function flowPlanningStaticHint() {
  return flowJoinSeriousFun(
    "先理解计划结构，再生成有依赖的工作步骤；若智能拆分不可用，会自动用本地规则兜底。",
    flowPickBlurb("planning", "static")
  );
}

/** How the plan was split — product words. */
function flowPlanHowLabel(adapter) {
  const a = String(adapter || "");
  if (a.includes("heuristic")) return "本地规则拆分";
  if (a.includes("llm") || a.includes("ai")) return "智能拆分";
  if (a.includes("fake")) return "演练拆分";
  return "拆分完成";
}

function flowChooserSub(hasSelected) {
  return hasSelected
    ? "确认同时进行几步后，点「拆成步骤」"
    : "选好计划，确认同时进行几步后点「拆成步骤」；可换一份计划";
}

function flowConfirmDepsLine(kind, depTitles, opts = {}) {
  const k = kind || "必选步骤";
  if (depTitles && depTitles.length) {
    return `${k} · 等待：${depTitles.join(" · ")}`;
  }
  return `${k} · 无依赖，可进首波`;
}

function flowConfirmMetaLine(chars, editing) {
  if (editing) return `编辑中 · 说明 ${chars} 字`;
  return `说明 ${chars} 字 · 点中间卡片可切换步骤`;
}

function flowPromptLabel(editing) {
  return editing
    ? "编辑步骤说明"
    : "步骤说明（执行时按完整说明进行）";
}

function flowConfirmStartLabel(kind) {
  if (kind === "running") return "运行中…";
  if (kind === "paused") return "继续运行";
  if (kind === "again") return "再次确认并开始";
  return "确认并开始";
}

function flowSplitCtaLabel() {
  return "拆成步骤";
}

function flowReplanLabel() {
  return "重新拆分（保留你的修改）";
}

function flowSanitizeDepsLabel() {
  return "让可并行的真正并行";
}

/**
 * Confirm pane should show the *decomposed task body*, not the host worker
 * scaffold the planner wraps around it ("你是执行任务 tN…CCO_DONE ok").
 * Edit mode keeps the full prompt so advanced users can still tune it.
 */
function stripWorkerScaffold(prompt) {
  let s = String(prompt || "");
  if (!s.trim()) return s;
  // Drop leading worker identity / cwd / "依据下列说明" preamble.
  s = s.replace(
    /^你是执行任务[\s\S]*?(?=##\s*任务说明|##\s*依赖原因|按\*\*本阶段|阅读下列|根据\s*`\.cco-out|你是\*\*检验员)/u,
    ""
  );
  // Prefer content under ## 任务说明
  const m = s.match(/##\s*任务说明\s*\n([\s\S]*?)(?:\n全部完成后在最后一行输出[：:][\s\S]*)?$/u);
  if (m && m[1] && m[1].trim().length > 20) {
    s = m[1].trim();
  } else {
    // Drop trailing CCO_DONE contract line(s)
    s = s.replace(/\n*全部完成后在最后一行输出[：:][\s\S]*$/u, "").trim();
    s = s.replace(/\n*完成后输出一行[：:]\s*CCO_DONE[\s\S]*$/iu, "").trim();
  }
  // Drop empty ## 依赖原因 block noise when no deps note content left alone
  s = s.replace(/^##\s*依赖原因\s*\n等待前置步骤产物[：:][^\n]*\n*/u, "");
  return s.trim() || String(prompt || "");
}

/** Map provider id → short product label (only when advanced shows engine). */
function flowEngineLabel(provider) {
  const p = String(provider || "").toLowerCase();
  if (p === "codex") return "Codex";
  if (p === "fake") return "演练";
  if (p === "claude") return "Claude";
  return provider || "";
}

function flowBoardSectionLabel() {
  return "步骤日志";
}

function flowEmptyBoard() {
  return "暂无执行窗口 · 确认并开始后这里会按步骤出现";
}

function flowRunningMonitorTitle() {
  return "返回工作区查看执行进度";
}

function flowStallUserText(raw) {
  const s = String(raw || "").trim();
  if (!s) return flowPickBlurb("stall", "x") || "步骤较久没有新进展，正在处理…";
  // Soften engine jargon if present
  let out = s
    .replace(/CLI/gi, "执行通道")
    .replace(/provider/gi, "执行方式")
    .replace(/claude/gi, "默认通道")
    .replace(/codex/gi, "备用通道")
    .replace(/卡死/g, "卡住")
    .replace(/阈值/g, "等待上限");
  // Prefer fun blurb as suffix when serious already has numbers
  const fun = flowPickBlurb("stall", out.slice(0, 24));
  if (fun && !out.includes(fun.slice(0, 6))) {
    return flowJoinSeriousFun(out, fun);
  }
  return out;
}

function flowFiveStateHint() {
  return "排队中 · 进行中 · 已完成 · 已卡住 · 失败";
}

function flowWaveTaskMeta(id, depsText, statusHint) {
  const parts = [id, depsText].filter(Boolean);
  if (statusHint) parts.push(String(statusHint).replace(/^ · /, ""));
  return parts.join(" · ");
}

/** Human label for digest mode badge. */
function flowModeLabel(mode) {
  const m = String(mode || "").toLowerCase();
  if (m === "regression") return "回归验证";
  if (m === "greenfield") return "从零落地";
  if (m === "audit") return "只读检验";
  if (m === "mixed") return "混合";
  return "";
}

function flowModeHint(mode) {
  const m = String(mode || "").toLowerCase();
  if (m === "regression") {
    return "文档显示相关阶段已落地：默认只核对证据，仅 blocking 残差才改代码";
  }
  if (m === "greenfield") return "按可执行工作包拆分，依赖应来自真实产物/接口";
  if (m === "audit") return "以只读检验为主，少做业务改动";
  if (m === "mixed") return "已完成项走回归，未完成项可实施";
  return "";
}

/**
 * Flow stages (PRODUCT 五步压缩条): 写计划 → 拆分 → 执行 → 结果
 * planning+confirm 同属「拆分」高亮；confirm 时副文案区分核对。
 * @param {"planning"|"confirm"|"running"|"done"|"fail"|"idle"|"pick"} phase
 */
function flowStageState(phase) {
  const p = String(phase || "idle");
  // order: 0 写计划, 1 拆分, 2 执行, 3 结果
  const stages = [
    { id: "author", label: "写计划" },
    { id: "split", label: "拆分" },
    { id: "run", label: "执行" },
    { id: "result", label: "结果" },
  ];
  let active = 0;
  if (p === "planning" || p === "confirm") active = 1;
  else if (p === "running") active = 2;
  else if (p === "done") active = 3;
  else if (p === "fail") active = 2;
  else if (p === "idle" || p === "pick") active = 0;
  return { stages, active, phase: p };
}

/** HTML for the horizontal flow stage strip. opts.compact = 只阶段点、不副文案 */
function flowStageStripHtml(phase, opts = {}) {
  const { stages, active } = flowStageState(phase);
  const serious =
    opts.serious ||
    (phase === "planning"
      ? "正在把计划拆成可执行步骤"
      : phase === "confirm"
        ? "看清波次后点「确认并开始」"
        : phase === "running"
          ? "按波次推进中"
          : phase === "done"
            ? "本轮已结束"
            : phase === "fail"
              ? "有步骤失败，已暂停"
              : "准备开始");
  const fun =
    opts.fun != null
      ? opts.fun
      : phase === "planning"
        ? flowPickBlurb("planning", phase)
        : phase === "confirm"
          ? flowPickBlurb("confirm", phase)
          : phase === "running"
            ? flowPickBlurb("running", phase)
            : phase === "done"
              ? flowPickBlurb("done", phase)
              : phase === "fail"
                ? flowPickBlurb("fail", phase)
                : "";
  const line = opts.compact ? "" : flowJoinSeriousFun(serious, fun);
  const steps = stages
    .map((s, i) => {
      let cls = "flow-stage";
      if (i < active) cls += " is-done";
      else if (i === active) cls += " is-active";
      const mark = i < active ? "✓" : i === active ? "●" : "○";
      return `<span class="${cls}" data-stage="${s.id}"><span class="flow-stage-mark" aria-hidden="true">${mark}</span>${s.label}</span>`;
    })
    .join('<span class="flow-stage-sep" aria-hidden="true">→</span>');
  return (
    `<div class="flow-stage-strip${opts.compact ? " is-compact" : ""}" role="status" aria-live="polite">` +
    `<div class="flow-stage-row">${steps}</div>` +
    (line ? `<div class="flow-stage-line muted">${line}</div>` : "") +
    `</div>`
  );
}

/** Ensure strip exists under parentEl and refresh content. */
function renderFlowStageStrip(parentEl, phase, opts = {}) {
  if (!parentEl) return;
  let box = parentEl.querySelector(":scope > .flow-stage-strip");
  if (!box) {
    box = document.createElement("div");
    // renderFlowStageStripHtml returns root; replace parent prepend
    parentEl.insertAdjacentHTML("afterbegin", flowStageStripHtml(phase, opts));
    return;
  }
  // rebuild
  const html = flowStageStripHtml(phase, opts);
  const tmp = document.createElement("div");
  tmp.innerHTML = html;
  const next = tmp.firstElementChild;
  if (next) box.replaceWith(next);
}

// A5-2d: ESM settings form reads local prefs via window
if (typeof window !== "undefined") {
  window.flowFunEnabled = flowFunEnabled;
  window.setFlowFunEnabled = setFlowFunEnabled;
}
