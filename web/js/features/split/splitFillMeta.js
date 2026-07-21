/**
 * [INPUT]: plan job DTO · legacy helpers (flowModeLabel / humanize / isSystemPost)
 * [OUTPUT]: 拆分台标题/meta/critic 条 + CTA 显隐（无 IPC）
 * [POS]: A5-2b 自 plan.js renderConfirmPanel 头抽出；三栏体仍在 SplitView
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：confirm_start / start_run / soft-fill / optional 策略写这里。
 */

import { isSystemPostTask } from "./splitRender.js";

function $(idOrSel) {
  if (!idOrSel) return null;
  if (idOrSel[0] === "#" || idOrSel[0] === ".") {
    return document.querySelector(idOrSel);
  }
  return document.getElementById(idOrSel) || document.querySelector(idOrSel);
}

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function isSysPost(t) {
  const fn = g("isSystemPostTask");
  if (typeof fn === "function") return fn(t);
  return isSystemPostTask(t);
}

/**
 * Paint confirm-phase chrome above the three-column desk.
 * @param {object} job
 * @param {{
 *   runLocked?: boolean,
 *   paused?: boolean,
 *   editing?: boolean,
 *   planJobId?: string|null,
 * }} [ctx]
 */
export function fillSplitMeta(job, ctx = {}) {
  if (!job) return;

  const layers = job.layers || [];
  const tasks = job.tasks || [];
  const runLocked =
    ctx.runLocked !== undefined
      ? !!ctx.runLocked
      : typeof g("hasActiveRun") === "function"
        ? !!g("hasActiveRun")()
        : false;
  const paused =
    ctx.paused !== undefined
      ? !!ctx.paused
      : typeof g("isRunPaused") === "function"
        ? !!g("isRunPaused")()
        : false;
  const editing =
    ctx.editing !== undefined
      ? !!ctx.editing
      : !!(typeof window !== "undefined" && window.state?.confirmEditing);
  const planJobId =
    ctx.planJobId !== undefined
      ? ctx.planJobId
      : typeof window !== "undefined"
        ? window.state?.planJobId
        : null;

  const st = String(job.status || "").toLowerCase();
  const reused = st === "confirmed";
  // 计划名已在顶栏 #page-title；这里只写角色，避免「标题 · 计划名」再叠一层
  const titleEl = $("confirm-title") || $("#confirm-title");
  if (titleEl) {
    titleEl.textContent = reused
      ? "历史拆分（可再次确认并开始）"
      : "拆分结果";
  }

  const mpCap = job.max_parallel ?? job.maxParallel ?? "—";
  const widestWave = layers.reduce((m, l) => Math.max(m, (l || []).length), 0);
  const parallelHint =
    typeof mpCap === "number" && widestWave > 0 && widestWave < mpCap
      ? ` · 最宽波 ${widestWave} 路（上限 ${mpCap}）`
      : widestWave > 1
        ? ` · 最多同时 ${widestWave} 路`
        : "";

  const optTasks = tasks.filter((t) => !!t.optional);
  const sysOpt = optTasks.filter((t) => isSysPost(t));
  const bizOpt = optTasks.filter((t) => !isSysPost(t));
  const sysOn = sysOpt.filter((t) => t.include !== false).length;
  let optHint = "";
  if (optTasks.length > 0) {
    const bits = [];
    if (bizOpt.length) {
      const bizOn = bizOpt.filter((t) => t.include !== false).length;
      bits.push(
        bizOn
          ? `业务可选 ${bizOn}/${bizOpt.length} 已勾`
          : `有 ${bizOpt.length} 个可选未勾选（确认后不会跑）`
      );
    }
    if (sysOpt.length) bits.push(`系统 ${sysOn}/${sysOpt.length}`);
    optHint = bits.length ? ` · ${bits.join(" · ")}` : "";
  }

  const confirmHint = runLocked
    ? "运行中（只读）"
    : paused
      ? "已暂停 · 仅未执行步骤可编辑"
      : bizOpt.length > 0
        ? "业务可选默认不跑 · 请勾选后再确认并开始"
        : sysOpt.length > 0
          ? "系统收尾默认已勾选 · 可取消后开始"
          : reused
            ? "可编辑未执行步骤后再次确认并开始"
            : "可编辑 · 确认并开始";

  const modeRaw = job.digest_mode || job.digestMode || "";
  // 模式只走 badge 行（label+hint），不塞进 meta，避免「从零落地」双份
  const nSteps = job.task_count || tasks.length;
  const metaEl = $("confirm-meta") || $("#confirm-meta");
  if (metaEl) {
    metaEl.textContent = `共 ${nSteps} 步 · 约 ${layers.length} 波${parallelHint}${optHint} · ${confirmHint}`;
  }

  const applyFlowModeBadge = g("applyFlowModeBadge");
  if (typeof applyFlowModeBadge === "function") {
    applyFlowModeBadge(
      "#confirm-mode-row",
      "#confirm-mode-badge",
      "#confirm-mode-hint",
      modeRaw
    );
  }

  paintCriticStrip(job);
  paintCriticChips(job);
  paintCriticNotesAndActions(job, {
    runLocked,
    editing,
    planJobId,
  });
}

function paintCriticStrip(job) {
  const criticEl = $("confirm-critic-note") || $("#confirm-critic-note");
  if (!criticEl) return;
  let critic = job.critic_summary || job.criticSummary || "";
  const humanize = g("humanizePlannerLogLine");
  if (critic && typeof humanize === "function") {
    critic = humanize(critic);
  }
  if (critic && String(critic).trim()) {
    criticEl.hidden = false;
    criticEl.textContent = String(critic).trim();
    const clean =
      /无需改动|未发现可疑|无需/.test(critic) &&
      !/去掉|改写|钉入|手动清理 · 去掉/.test(critic);
    criticEl.classList.toggle("is-clean", clean);
  } else {
    criticEl.hidden = true;
    criticEl.textContent = "";
    criticEl.classList.remove("is-clean");
  }
}

function paintCriticChips(job) {
  const chips = $("confirm-critic-chips") || $("#confirm-critic-chips");
  if (!chips) return;

  const nEdges = job.critic_edges_removed ?? job.criticEdgesRemoved;
  const nTitles = job.critic_titles_rewritten ?? job.criticTitlesRewritten;
  const nPrompts = job.critic_prompts_tagged ?? job.criticPromptsTagged;
  const llmUsed = job.critic_llm_used ?? job.criticLlmUsed;
  const hasStats =
    nEdges != null || nTitles != null || nPrompts != null || llmUsed != null;

  if (!hasStats) {
    chips.hidden = true;
    return;
  }
  chips.hidden = false;

  const modeChip = $("chip-critic-mode") || $("#chip-critic-mode");
  if (modeChip) {
    modeChip.hidden = false;
    if (llmUsed === true) {
      modeChip.textContent = "智能第二跳 ✓";
      modeChip.classList.add("is-llm");
      modeChip.classList.remove("is-rules", "is-zero");
      modeChip.title = "本次拆分启用了规则校对 + 智能第二跳";
    } else {
      modeChip.textContent = "规则校对";
      modeChip.classList.add("is-rules");
      modeChip.classList.remove("is-llm", "is-zero");
      modeChip.title = "仅规则校对（可在设置开启「智能第二跳校对」）";
    }
  }

  const setChip = (id, label, n) => {
    const el = $(id) || document.querySelector(id);
    if (!el) return;
    if (n == null) {
      el.hidden = true;
      return;
    }
    el.hidden = false;
    el.textContent = `${label} ${n}`;
    el.classList.toggle("is-zero", Number(n) === 0);
  };
  setChip("#chip-critic-edges", "清依赖", nEdges);
  setChip("#chip-critic-titles", "改标题", nTitles);
  setChip("#chip-critic-prompts", "钉提示", nPrompts);

  const cost = job.critic_llm_cost_usd ?? job.criticLlmCostUsd;
  const ms = job.critic_llm_ms ?? job.criticLlmMs;
  const costChip = $("chip-critic-cost") || $("#chip-critic-cost");
  const msChip = $("chip-critic-ms") || $("#chip-critic-ms");
  if (costChip) {
    if (llmUsed === true && cost != null && Number.isFinite(Number(cost))) {
      costChip.hidden = false;
      costChip.textContent = `$${Number(cost).toFixed(3)}`;
      costChip.classList.add("is-llm");
      costChip.title = "智能第二跳费用（USD）";
    } else {
      costChip.hidden = true;
    }
  }
  if (msChip) {
    if (llmUsed === true && ms != null && Number(ms) >= 0) {
      msChip.hidden = false;
      const n = Number(ms);
      msChip.textContent =
        n >= 1000 ? `${(n / 1000).toFixed(1)}s` : `${Math.round(n)}ms`;
      msChip.classList.add("is-llm");
      msChip.title = "智能第二跳耗时";
    } else {
      msChip.hidden = true;
    }
  }
}

function paintCriticNotesAndActions(job, { runLocked, editing, planJobId }) {
  const notesEl = $("confirm-critic-notes") || $("#confirm-critic-notes");
  const criticActions =
    $("confirm-critic-actions") || $("#confirm-critic-actions");
  let showInspectCta = false;
  const humanize = g("humanizePlannerLogLine");

  if (notesEl) {
    const notes = job.critic_notes || job.criticNotes || [];
    const list = Array.isArray(notes)
      ? notes.filter((n) => String(n || "").trim())
      : [];
    if (!list.length) {
      notesEl.hidden = true;
      notesEl.innerHTML = "";
    } else {
      notesEl.hidden = false;
      notesEl.innerHTML = list
        .map((n) => {
          let t = String(n);
          if (typeof humanize === "function") t = humanize(t);
          if (/检验|巡检|inspect/i.test(t)) showInspectCta = true;
          const li = document.createElement("li");
          li.textContent = t;
          return li.outerHTML;
        })
        .join("");
    }
  }

  const llmUsed = job.critic_llm_used ?? job.criticLlmUsed;
  const showCriticCta = llmUsed === false || llmUsed == null;
  if (criticActions) {
    const inspectBtn =
      $("btn-enable-post-inspect") || $("#btn-enable-post-inspect");
    const criticBtn =
      $("btn-enable-planner-critic") || $("#btn-enable-planner-critic");
    const anyCta = (showInspectCta || showCriticCta) && !runLocked;
    criticActions.hidden = !anyCta;
    if (inspectBtn) {
      inspectBtn.hidden = !showInspectCta;
      inspectBtn.disabled = !!runLocked || !!editing || !planJobId;
    }
    if (criticBtn) {
      criticBtn.hidden = !showCriticCta || !!runLocked;
      criticBtn.disabled = !!runLocked || !!editing || !planJobId;
    }
  }
}
