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
      ? "历史拆分（可再次执行规划）"
      : "拆分结果";
  }

  const widestWave = layers.reduce((m, l) => Math.max(m, (l || []).length), 0);
  const nBatches = layers.length || 1;
  // S1: product-manager tone — not scheduler jargon.
  let scheduleHint = "";
  if (nBatches <= 1 && widestWave <= 1) {
    scheduleHint = "将按顺序一个一个做";
  } else if (widestWave <= 1) {
    scheduleHint = `大约分 ${nBatches} 批 · 将按顺序做`;
  } else {
    scheduleHint = `大约分 ${nBatches} 批 · 同一批最多 ${widestWave} 步一起`;
  }

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

  const externalOn = tasks.some((t) => {
    const rc = String(t.risk_class || t.riskClass || "").toLowerCase();
    if (rc === "external" && t.include !== false) return true;
    const id = String(t.id || "");
    return (
      (id.includes("git-push") || id.includes("open-pr")) && t.include !== false
    );
  });
  const hasExternal = tasks.some((t) => {
    const rc = String(t.risk_class || t.riskClass || "").toLowerCase();
    const id = String(t.id || "");
    return (
      rc === "external" || id.includes("git-push") || id.includes("open-pr")
    );
  });
  const riskHint = externalOn
    ? "含已勾选的外发步骤（推送/开 PR）"
    : hasExternal
      ? "默认不外发 · 推送/开 PR 需勾选"
      : "会改本地 · 默认不推远端";

  const confirmHint = runLocked
    ? "运行中（只读）"
    : paused
      ? "已暂停 · 仅未执行步骤可编辑"
      : bizOpt.length > 0
        ? "业务可选默认不跑 · 请勾选后再点「执行规划」"
        : sysOpt.length > 0
          ? "系统收尾可勾选 · 外发默认关"
          : reused
            ? "可编辑未执行步骤后再点「执行规划」"
            : "核对后点「执行规划」";

  const modeRaw = job.digest_mode || job.digestMode || "";
  // 模式只走 badge 行（label+hint），不塞进 meta，避免「从零落地」双份
  const nSteps = job.task_count || tasks.length;
  // Q1: always-visible split source (智能 / 本地规则 / …) — not engine jargon.
  const sourceLabel = splitSourceLabel(job);
  const metaEl = $("confirm-meta") || $("#confirm-meta");
  if (metaEl) {
    const sourceBit = sourceLabel ? `${sourceLabel} · ` : "";
    metaEl.textContent = `共 ${nSteps} 步 · ${sourceBit}${scheduleHint}${optHint} · ${riskHint} · ${confirmHint}`;
  }
  // Confirm CTA title: business vs external (still one confirm_start).
  try {
    const btn = $("btn-confirm-start");
    if (btn && !runLocked) {
      btn.title = externalOn
        ? "开始执行（含已勾选的推送/开 PR 等外发）"
        : "开始执行业务步骤（默认不推远端；外发看勾选）";
      btn.textContent = externalOn ? "执行规划（含外发）" : "执行规划";
    }
  } catch (_) {}
  // W3-1: when local / smart-missed, offer one-click smart re-split (sets plan_mode=ai).
  paintSmartResplitHint(sourceLabel, { runLocked });

  // B6：业务可选未勾 — 固定非 dismiss 提示条（与卡片 include===false 同构）
  let optBanner = $("split-optional-banner") || $("#split-optional-banner");
  if (!optBanner) {
    const head = document.querySelector("#plan-phase-confirm .split-head-main");
    if (head) {
      optBanner = document.createElement("div");
      optBanner.id = "split-optional-banner";
      optBanner.className = "split-optional-banner";
      optBanner.setAttribute("role", "status");
      head.appendChild(optBanner);
    }
  }
  if (optBanner) {
    const offCount = bizOpt.filter((t) => t.include === false).length;
    if (offCount > 0 && !runLocked) {
      optBanner.hidden = false;
      optBanner.textContent = `有 ${offCount} 个可选步骤未勾选：点「执行规划」后它们不会执行。需要跑的请先在列表里勾选。`;
    } else {
      optBanner.hidden = true;
      optBanner.textContent = "";
    }
  }

  // P1-4: plan acceptance stub/missing — yellow bar; never disables confirm.
  paintAcceptanceBanner(job, { runLocked });

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

/**
 * P1-4: yellow bar when plan-level acceptance is stub/missing.
 * Does **not** disable confirm; CTA demotion lives in paintChrome.
 */
function paintAcceptanceBanner(job, { runLocked } = {}) {
  let bar = $("split-acceptance-banner") || $("#split-acceptance-banner");
  if (!bar) {
    const head = document.querySelector("#plan-phase-confirm .split-head-main");
    if (head) {
      bar = document.createElement("div");
      bar.id = "split-acceptance-banner";
      bar.className = "split-acceptance-banner";
      bar.setAttribute("role", "status");
      // After optional banner if present, else at end of head-main.
      const opt = $("split-optional-banner") || head.querySelector("#split-optional-banner");
      if (opt && opt.parentNode === head) {
        opt.insertAdjacentElement("afterend", bar);
      } else {
        head.appendChild(bar);
      }
    }
  }
  if (!bar) return;

  const isStub =
    job.acceptance_is_stub === true ||
    job.acceptanceIsStub === true ||
    String(job.acceptance_is_stub || job.acceptanceIsStub || "") === "true";
  const hint = String(
    job.acceptance_hint || job.acceptanceHint || ""
  ).trim();

  if (isStub && hint && !runLocked) {
    bar.hidden = false;
    bar.textContent = hint;
  } else if (isStub && !runLocked) {
    bar.hidden = false;
    bar.textContent =
      "计划验收仍是占位或未写清，建议补充「怎样算做完」后再开始（仍可确认）";
  } else {
    bar.hidden = true;
    bar.textContent = "";
  }
}

/** Q1 · 人话：本次拆分来源（禁止 adapter / layers 裸词） */
function splitSourceLabel(job) {
  const mode = String(job.plan_mode || job.planMode || "").toLowerCase();
  const adapter = String(job.adapter || "").toLowerCase();
  if (mode === "fake" || adapter.includes("fake")) return "演练拆分";
  if (mode === "parse" || adapter.includes("parse")) return "按文档结构拆分";
  if (
    adapter.includes("llm") ||
    adapter.includes("split-agent") ||
    (mode === "ai" && !adapter.includes("heuristic"))
  ) {
    return "智能拆分";
  }
  if (mode === "fast" || adapter.includes("heuristic") || mode === "ai") {
    // ai 失败落到 heuristic 时 adapter 含 heuristic
    if (mode === "ai" && adapter.includes("heuristic")) {
      return "本地规则拆分（智能未用上）";
    }
    if (mode === "fast" || adapter.includes("heuristic")) return "本地规则拆分";
  }
  if (mode === "ai") return "智能拆分";
  return "";
}

/**
 * W3-1: local-source desk shows a secondary CTA → force ai + replan.
 * @param {string} sourceLabel
 * @param {{ runLocked?: boolean }} [ctx]
 */
function paintSmartResplitHint(sourceLabel, ctx = {}) {
  const head =
    document.querySelector("#plan-phase-confirm .split-head-main") ||
    document.querySelector("#plan-phase-confirm");
  let el = $("split-smart-resplit") || $("#split-smart-resplit");
  if (!el && head) {
    el = document.createElement("p");
    el.id = "split-smart-resplit";
    el.className = "split-smart-resplit muted";
    el.setAttribute("role", "note");
    const meta = $("confirm-meta") || $("#confirm-meta");
    if (meta && meta.parentNode) {
      meta.parentNode.insertBefore(el, meta.nextSibling);
    } else {
      head.appendChild(el);
    }
  }
  if (!el) return;
  const local =
    sourceLabel === "本地规则拆分" ||
    sourceLabel === "本地规则拆分（智能未用上）";
  if (!local || ctx.runLocked) {
    el.hidden = true;
    el.textContent = "";
    el.onclick = null;
    return;
  }
  el.hidden = false;
  el.innerHTML =
    '这次没走智能拆分。' +
    '<button type="button" class="linkish" data-smart-resplit="1">用智能再拆一次</button>' +
    '（会想依赖与并行，可能要几分钟）';
  el.onclick = (e) => {
    const btn = e.target?.closest?.("[data-smart-resplit]");
    if (!btn) return;
    e.preventDefault();
    // Force next analyzePlanFromPicker to ai even if #pp-plan-mode still fast.
    try {
      if (typeof window !== "undefined" && window.state) {
        window.state.forcePlanModeAi = true;
      }
    } catch (_) {
      /* ignore */
    }
    const pm = $("pp-plan-mode") || $("#pp-plan-mode");
    if (pm) {
      pm.value = "ai";
      try {
        pm.dataset.touched = "1";
      } catch (_) {
        /* ignore */
      }
    }
    const replan =
      $("btn-replan") ||
      document.getElementById("btn-replan") ||
      document.querySelector("[data-action='replan']");
    if (replan && typeof replan.click === "function") {
      replan.click();
      return;
    }
    const start =
      typeof g("replanFromConfirm") === "function"
        ? g("replanFromConfirm")
        : typeof g("startPlanFromChooser") === "function"
          ? g("startPlanFromChooser")
          : typeof window !== "undefined" &&
            (window.ccoProject?.replanFromConfirm ||
              window.ccoProject?.startPlanFromChooser);
    if (typeof start === "function") start();
  };
}

function paintCriticStrip(job) {
  const criticEl = $("confirm-critic-note") || $("#confirm-critic-note");
  if (!criticEl) return;
  let critic = job.critic_summary || job.criticSummary || "";
  const humanize = g("humanizePlannerLogLine");
  if (critic && typeof humanize === "function") {
    critic = humanize(critic);
  }
  // S1-3: serial graph + "未发现可疑" → honest plain language (no adapter/layers).
  const layers = job.layers || [];
  const widest = layers.reduce((m, l) => Math.max(m, (l || []).length), 0);
  const nSteps = (job.tasks || []).length || job.task_count || 0;
  const looksSerial = widest <= 1 && nSteps >= 4;
  const cleanish =
    critic &&
    /无需改动|未发现可疑|无需/.test(String(critic)) &&
    !/去掉|改写|钉入|手动清理 · 去掉/.test(String(critic));
  if (looksSerial && cleanish) {
    critic =
      "当前按顺序执行；未检测出互相打架的等待关系";
  }
  if (critic && String(critic).trim()) {
    criticEl.hidden = false;
    criticEl.textContent = String(critic).trim();
    const clean =
      /无需改动|未发现可疑|无需|按顺序执行/.test(critic) &&
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
