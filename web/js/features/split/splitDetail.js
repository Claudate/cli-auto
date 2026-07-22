/**
 * [INPUT]: job DTO · selected task · DOM
 * [OUTPUT]: 详情栏 + 高级路由折叠 paint
 * [POS]: A3-1/A3-2 features/split；View 子模块
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  routeSummary,
  oneLiner,
  roleBadge,
  esc,
  engineLabel,
} from "./splitRender.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

function $(id) {
  return document.getElementById(id);
}

/** Avoid clobbering select while custom dropdown is open/focused. */
function selectBusy(el) {
  if (!el) return false;
  const api = g("ccoSelectUi");
  if (api && typeof api.isSelectBusy === "function") return api.isSelectBusy(el);
  if (document.activeElement === el) return true;
  const root = el.closest?.(".cco-select");
  if (root?.classList.contains("is-open")) return true;
  if (root && document.activeElement && root.contains(document.activeElement)) {
    return true;
  }
  return false;
}

function toast(msg) {
  const fn = g("toast");
  if (typeof fn === "function") fn(msg);
  else console.info("[split]", msg);
}

function hasActiveRun() {
  return typeof g("hasActiveRun") === "function" ? g("hasActiveRun")() : false;
}

function isRunPaused() {
  return typeof g("isRunPaused") === "function" ? g("isRunPaused")() : false;
}

function canEditTask(taskId) {
  return typeof g("canEditSelectedTask") === "function"
    ? g("canEditSelectedTask")(taskId)
    : !hasActiveRun();
}

function stripScaffold(full) {
  return typeof g("stripWorkerScaffold") === "function"
    ? g("stripWorkerScaffold")(full)
    : String(full || "");
}

function md(src) {
  return typeof g("renderMarkdown") === "function"
    ? g("renderMarkdown")(src)
    : esc(src);
}

/**
 * Ensure advanced route fold exists (A3-2). Idempotent; default collapsed.
 */
export function ensureAdvancedRouteDom() {
  const head = document.querySelector(".confirm-detail-head");
  if (!head) return null;
  let fold = $("split-route-advanced");
  if (fold) return fold;
  fold = document.createElement("details");
  fold.id = "split-route-advanced";
  fold.className = "split-route-advanced";
  fold.innerHTML =
    `<summary class="split-route-summary muted">高级 · 执行通道与路由</summary>` +
    `<div class="split-route-body">` +
    `<p class="muted split-route-hint" id="split-route-hint">智能建议可改；不会静默覆盖你已改过的通道 / 角色 / 范围。</p>` +
    `<div class="split-route-grid">` +
    `<div class="split-route-row"><span class="muted">通道</span> <strong id="split-route-provider-label">—</strong></div>` +
    `<div class="split-route-row"><span class="muted">角色</span> <span id="split-route-role-label">—</span></div>` +
    `<div class="split-route-row"><span class="muted">范围</span> <span id="split-route-scope-label">—</span></div>` +
    `</div>` +
    `<label class="field split-route-provider-field" id="split-route-provider-field">` +
    `<span>本步骤执行通道</span>` +
    `<select id="split-route-provider">` +
    `<option value="claude">默认通道</option>` +
    `<option value="codex">备用通道</option>` +
    `<option value="fake">演练</option>` +
    `</select>` +
    `</label>` +
    `<label class="field split-route-role-field" id="split-route-role-field">` +
    `<span>协作角色</span>` +
    `<select id="split-route-role">` +
    `<option value="">未指定（按计划）</option>` +
    `<option value="scout">探查 scout</option>` +
    `<option value="implement">实现 implement</option>` +
    `<option value="integrate">整合 integrate</option>` +
    `<option value="inspect">巡检 inspect</option>` +
    `</select>` +
    `</label>` +
    `<label class="field split-route-scope-field" id="split-route-scope-field">` +
    `<span>可写范围（每行一个路径，如 src/web/**）</span>` +
    `<textarea id="split-route-scope" rows="2" spellcheck="false" placeholder="留空 = 不单独限制可写路径"></textarea>` +
    `</label>` +
    `</div>`;
  const detail = document.querySelector(".confirm-detail.split-detail");
  if (detail) {
    const promptLabel = $("confirm-prompt-label");
    if (promptLabel) detail.insertBefore(fold, promptLabel);
    else detail.appendChild(fold);
  }
  return fold;
}

/**
 * @param {object} ctx
 * @param {object} ctx.vm
 * @param {object} ctx.job
 * @param {Record<string, object>} ctx.byId
 * @param {boolean} ctx.runLocked
 * @param {() => void} ctx.render
 * @param {() => void} ctx.pushSelection
 * @param {() => void} ctx.afterMutate
 */
export function paintDetail(ctx) {
  const { vm, job, byId, runLocked, render, pushSelection, afterMutate } = ctx;
  const s = vm.getSnapshot();
  const tasks = job.tasks || [];
  const cur = byId[s.selectedTaskId] || tasks[0];
  const metaEl = $("confirm-task-meta");
  const promptEl = $("confirm-task-prompt");
  const editForm = $("confirm-edit-form");
  const editBtn = $("btn-confirm-edit");
  const deleteBtn = $("btn-confirm-delete");
  const cancelBtn = $("btn-confirm-edit-cancel");
  const saveBtn = $("btn-confirm-edit-save");
  const promptLabel = $("confirm-prompt-label");
  const providerSel = $("confirm-task-provider");
  const taskEditable = !!cur && canEditTask(cur.id);
  const editing = !!s.editing && taskEditable;
  const curProvider = (
    cur?.provider ||
    job.provider ||
    "claude"
  ).toLowerCase();

  if (cur) {
    if (s.selectedTaskId !== cur.id) {
      vm.selectTask(cur.id);
    }
    const titleEl = $("confirm-task-title");
    if (titleEl) {
      const rawT = cur.title || cur.id;
      titleEl.textContent = String(rawT)
        .replace(/[☐✅☑□■✗✘×]+$/g, "")
        .trim();
      titleEl.classList.remove("muted");
    }
    const role = roleBadge(cur);
    const kind = cur.optional
      ? cur.include !== false
        ? role?.kind === "sys"
          ? "系统 · 已勾选"
          : "可选 · 已勾选（会执行）"
        : role?.kind === "sys"
          ? "系统 · 未勾选（本次不跑）"
          : "可选 · 未勾选（默认不跑）"
      : role?.kind === "check"
        ? "查 · 必做"
        : "做 · 必做";
    let depTitles = [];
    if (cur.depends_on?.length > 0) {
      depTitles = cur.depends_on.map((id) => {
        const d = byId[id];
        const t = d ? d.title || id : id;
        return String(t)
          .replace(/[☐✅☑□■✗✘×]+$/g, "")
          .trim();
      });
    }
    const depsEl = $("confirm-task-deps");
    if (depsEl) {
      const line =
        typeof g("flowConfirmDepsLine") === "function"
          ? g("flowConfirmDepsLine")(kind, depTitles)
          : depTitles.length
            ? `${kind} · 等：${depTitles.join(" · ")}`
            : `${kind} · 可马上开始`;
      depsEl.textContent = line;
    }

    const full =
      cur.prompt || cur.prompt_preview || cur.promptPreview || "";
    const displayBody = stripScaffold(full);

    if (editing) {
      if (promptEl) promptEl.hidden = true;
      if (editForm) editForm.hidden = false;
      if (promptLabel) {
        promptLabel.textContent =
          typeof g("flowPromptLabel") === "function"
            ? g("flowPromptLabel")(true)
            : "编辑步骤说明";
      }
      const titleInput = $("confirm-edit-title");
      const promptInput = $("confirm-edit-prompt");
      const editProv = $("confirm-edit-provider");
      const depsBox = $("confirm-edit-deps");
      if (titleInput && document.activeElement !== titleInput) {
        titleInput.value = cur.title || "";
      }
      if (promptInput && document.activeElement !== promptInput) {
        promptInput.value = full;
      }
      if (editProv && !selectBusy(editProv)) {
        editProv.value = curProvider;
      }
      if (depsBox && depsBox.dataset.forTask !== cur.id) {
        depsBox.dataset.forTask = cur.id;
        const others = tasks.filter((t) => t.id !== cur.id);
        if (!others.length) {
          depsBox.innerHTML =
            '<span class="confirm-edit-deps-empty">没有其它步骤可依赖</span>';
        } else {
          const selected = new Set(cur.depends_on || []);
          depsBox.innerHTML = others
            .map((t) => {
              const checked = selected.has(t.id) ? "checked" : "";
              return (
                `<label>` +
                `<input type="checkbox" class="confirm-dep-check" value="${esc(t.id)}" ${checked} />` +
                `<span>${esc(t.title || t.id)}</span>` +
                `</label>`
              );
            })
            .join("");
        }
      }
    } else {
      // S0：默认三块 — 要做什么 · 怎样算做完 · （等待在 deps 行）
      // 技术说明默认折叠；完整说明可展开（不强制 open，对齐双受众短读）
      if (editForm) editForm.hidden = true;
      if (promptLabel) {
        promptLabel.textContent = "这一步做什么";
      }
      const bodyText = displayBody || full || "";
      const ol = oneLiner(cur) || "";
      let doneLine = "";
      if (cur.acceptance || cur.done_when || cur.doneWhen) {
        doneLine = String(
          cur.acceptance || cur.done_when || cur.doneWhen
        ).trim();
      }
      if (!doneLine) {
        const m = String(bodyText).match(
          /\|\s*\*?\*?完成定义\*?\*?\s*\|\s*([^|\n]+)/
        );
        if (m) doneLine = m[1].trim();
      }
      const shortParts = [];
      if (ol) {
        shortParts.push(
          `<p class="split-detail-short"><strong>要做什么</strong> · ${esc(ol)}</p>`
        );
      }
      if (doneLine && doneLine !== ol) {
        shortParts.push(
          `<p class="split-detail-short"><strong>怎样算做完</strong> · ${esc(doneLine)}</p>`
        );
      }
      const shortHtml = shortParts.length
        ? shortParts.join("")
        : `<p class="split-detail-short muted">下方可展开完整说明；也可点「编辑」改写</p>`;

      // S3-1: 技术说明 — 落点/步骤；自测不进默认技术说明（进完整说明）
      const techBits = [];
      const tableCell = (labels) => {
        for (const lab of labels) {
          const re = new RegExp(
            String.raw`\|\s*\*?\*?${lab}\*?\*?\s*\|\s*([^|\n]+)`,
            "i"
          );
          const m = String(bodyText).match(re);
          if (m) return m[1].trim();
        }
        return "";
      };
      const touch = tableCell(["落点", "文件", "改哪里"]);
      const steps = tableCell(["步骤", "改法", "Steps"]);
      const scopePaths = cur?.scope?.paths || cur?.scope_paths || cur?.scopePaths;
      if (touch) techBits.push(`**改哪里** · ${touch}`);
      if (Array.isArray(scopePaths) && scopePaths.length) {
        techBits.push(
          `**范围** · ${scopePaths.slice(0, 6).join(" · ")}${
            scopePaths.length > 6 ? "…" : ""
          }`
        );
      }
      if (steps) techBits.push(`**怎么做** · ${steps}`);
      const hasTech = techBits.length > 0;
      const fullHtml = md(bodyText);
      if (promptEl) {
        promptEl.hidden = false;
        promptEl.classList.add("md-body");
        const sameTask = promptEl.dataset.forTask === cur.id;
        const wasTech = sameTask
          ? !!promptEl.querySelector("details.split-detail-tech")?.open
          : false;
        const wasOpen = sameTask
          ? !!promptEl.querySelector("details.split-detail-full")?.open
          : false;
        const prevScroll = sameTask ? promptEl.scrollTop : 0;
        promptEl.dataset.forTask = cur.id;
        let html = shortHtml;
        if (hasTech) {
          html +=
            `<details class="split-detail-tech"${wasTech ? " open" : ""}>` +
            `<summary>技术说明</summary>` +
            `<div class="split-detail-tech-body md-body">${md(techBits.join("\n\n"))}</div>` +
            `</details>`;
        }
        // shell-chrome A5 纠偏：默认不强制 open（短读优先）；同任务 re-paint 尊重 wasOpen
        html +=
          `<details class="split-detail-full"${wasOpen ? " open" : ""}>` +
          `<summary>完整说明</summary>` +
          `<div class="split-detail-full-body md-body">${fullHtml}</div>` +
          `</details>`;
        promptEl.innerHTML = html;
        if (sameTask) {
          promptEl.scrollTop = prevScroll;
        } else {
          promptEl.scrollTop = 0;
        }
      }
    }
    if (metaEl) {
      metaEl.hidden = false;
      metaEl.textContent = editing
        ? "编辑中 · 保存后生效"
        : "左侧选步骤 · 下方可展开完整说明 · 可编辑";
    }
  } else {
    const titleEl = $("confirm-task-title");
    if (titleEl) {
      titleEl.textContent = "选择步骤查看说明";
      titleEl.classList.add("muted");
    }
    if ($("confirm-task-deps")) $("confirm-task-deps").textContent = "";
    if (promptEl) {
      promptEl.hidden = false;
      promptEl.innerHTML =
        `<p class="muted">点左侧步骤查看「要做什么 / 怎样算做完」；可展开完整说明，也可编辑后再执行规划。</p>`;
    }
    if (editForm) editForm.hidden = true;
    if (metaEl) {
      metaEl.hidden = false;
      metaEl.textContent = "左侧选步骤 · 下方可展开完整说明 · 可编辑";
    }
  }

  // 顶栏「默认通道」与「高级·执行通道」双控件叠层：通道只留高级折叠一处
  const providerField = $("confirm-provider-field");
  if (providerField) {
    providerField.hidden = true;
  }
  if (providerSel) {
    if (!selectBusy(providerSel)) {
      providerSel.value = cur ? curProvider : "claude";
    }
    providerSel.disabled = true;
    providerSel.onchange = null;
  }

  paintAdvancedRoute(cur, job, {
    vm,
    runLocked,
    editing,
    taskEditable,
    curProvider,
    render,
    pushSelection,
    afterMutate,
  });

  if (editBtn) {
    editBtn.hidden = !cur || editing || !taskEditable;
    editBtn.disabled = !taskEditable;
  }
  if (deleteBtn) {
    const canDelete =
      !!cur && taskEditable && !editing && tasks.length > 1 && !runLocked;
    deleteBtn.hidden = !canDelete;
    deleteBtn.disabled = !canDelete;
  }
  if (cancelBtn) cancelBtn.hidden = !editing;
  if (saveBtn) saveBtn.hidden = !editing;
}

function currentRoleValue(cur) {
  const raw = String(cur?.role || cur?.worker_role || "").trim().toLowerCase();
  if (!raw) return "";
  if (raw.includes("scout")) return "scout";
  if (raw.includes("implement") || raw === "impl") return "implement";
  if (raw.includes("integrate")) return "integrate";
  if (raw.includes("inspect") || raw.includes("review") || raw.includes("check"))
    return "inspect";
  return raw;
}

function currentScopePathsText(cur) {
  const scope = cur?.scope || cur?.scope_paths || cur?.scopePaths || null;
  if (Array.isArray(scope)) return scope.join("\n");
  if (scope && typeof scope === "object") {
    const paths = scope.paths || scope.Paths || [];
    return Array.isArray(paths) ? paths.join("\n") : "";
  }
  return "";
}

function paintAdvancedRoute(cur, job, ctx) {
  ensureAdvancedRouteDom();
  const fold = $("split-route-advanced");
  if (!fold) return;
  if (!cur) {
    fold.hidden = true;
    return;
  }
  fold.hidden = !!ctx.editing;
  const route = routeSummary(cur, job);
  const provLabel = $("split-route-provider-label");
  const roleLabelEl = $("split-route-role-label");
  const scopeLabelEl = $("split-route-scope-label");
  const advSel = $("split-route-provider");
  const roleSel = $("split-route-role");
  const scopeTa = $("split-route-scope");
  const locked = !ctx.taskEditable || ctx.editing || !!ctx.runLocked;
  if (provLabel) provLabel.textContent = route.providerLabel;
  if (roleLabelEl) roleLabelEl.textContent = route.roleLabel;
  if (scopeLabelEl) {
    scopeLabelEl.textContent = route.scopeText;
    scopeLabelEl.classList.toggle("muted", !route.hasExplicitScope);
  }
  if (advSel) {
    if (!selectBusy(advSel)) {
      advSel.value = route.provider;
    }
    advSel.disabled = locked;
    advSel.onchange = async () => {
      if (
        !cur ||
        !ctx.taskEditable ||
        hasActiveRun() ||
        ctx.vm.getSnapshot().editing
      ) {
        advSel.value = ctx.curProvider;
        return;
      }
      const next = (advSel.value || "claude").toLowerCase();
      if (next === ctx.curProvider) return;
      try {
        await ctx.vm.setProvider(cur.id, next);
        ctx.pushSelection();
        toast(`已设「${cur.title || cur.id}」→ ${engineLabel(next)}`);
        ctx.afterMutate();
        ctx.render();
      } catch (e) {
        advSel.value = ctx.curProvider;
        toast(String(e?.message || e));
      }
    };
  }
  const curRole = currentRoleValue(cur);
  if (roleSel) {
    if (!selectBusy(roleSel)) {
      roleSel.value = curRole;
    }
    roleSel.disabled = locked;
    roleSel.onchange = async () => {
      if (
        !cur ||
        !ctx.taskEditable ||
        hasActiveRun() ||
        ctx.vm.getSnapshot().editing
      ) {
        roleSel.value = curRole;
        return;
      }
      const next = String(roleSel.value || "").toLowerCase();
      if (next === curRole) return;
      try {
        await ctx.vm.setRole(cur.id, next);
        ctx.pushSelection();
        toast(
          next
            ? `已设「${cur.title || cur.id}」角色 → ${next}`
            : `已清除「${cur.title || cur.id}」角色`
        );
        ctx.afterMutate();
        ctx.render();
      } catch (e) {
        roleSel.value = curRole;
        toast(String(e?.message || e));
      }
    };
  }
  const scopeText = currentScopePathsText(cur);
  if (scopeTa) {
    if (document.activeElement !== scopeTa) {
      scopeTa.value = scopeText;
    }
    scopeTa.disabled = locked;
    scopeTa.onchange = async () => {
      if (
        !cur ||
        !ctx.taskEditable ||
        hasActiveRun() ||
        ctx.vm.getSnapshot().editing
      ) {
        scopeTa.value = scopeText;
        return;
      }
      const nextPaths = String(scopeTa.value || "")
        .split(/[\n,]+/)
        .map((s) => s.trim())
        .filter(Boolean);
      const prevPaths = scopeText
        .split(/[\n,]+/)
        .map((s) => s.trim())
        .filter(Boolean);
      if (
        nextPaths.length === prevPaths.length &&
        nextPaths.every((p, i) => p === prevPaths[i])
      ) {
        return;
      }
      try {
        await ctx.vm.setScopePaths(cur.id, nextPaths);
        ctx.pushSelection();
        toast(
          nextPaths.length
            ? `已设「${cur.title || cur.id}」范围 ${nextPaths.length} 条`
            : `已清除「${cur.title || cur.id}」范围`
        );
        ctx.afterMutate();
        ctx.render();
      } catch (e) {
        scopeTa.value = scopeText;
        toast(String(e?.message || e));
      }
    };
  }
}

export function paintChrome(vm, job, runLocked) {
  const s = vm.getSnapshot();
  const paused = isRunPaused();
  const editing = !!s.editing;
  const err = $("confirm-error");
  if (err && !s.lastError) err.hidden = true;
  if (err && s.lastError) {
    err.hidden = false;
    err.textContent = s.lastError;
  }
  // shell-chrome A2：主按钮固定「执行规划」；验收 stub 用 title/黄条，不改成长句 label
  const startBtn = $("btn-confirm-start");
  if (startBtn) {
    startBtn.disabled = !!runLocked || editing || !!s.busy;
    const accStub =
      job &&
      (job.acceptance_is_stub === true ||
        job.acceptanceIsStub === true ||
        String(job.acceptance_is_stub || job.acceptanceIsStub || "") === "true");
    let label = "执行规划";
    if (runLocked) label = "运行中…";
    else if (paused) label = "继续运行";
    startBtn.textContent = label;
    startBtn.classList.toggle("is-acceptance-stub", !!accStub && !runLocked);
    if (runLocked) {
      startBtn.title = "本轮还在执行";
    } else if (accStub) {
      startBtn.title =
        "计划验收未写清：仍可执行规划，建议先补「怎样算做完」（见上方黄条）";
    } else if (paused) {
      startBtn.title = "从暂停处继续执行";
    } else {
      startBtn.title = "核对后开始执行（走确认开跑）";
    }
  }
  const replanBtn = $("btn-replan");
  if (replanBtn) {
    // Keep clickable while runLocked so the handler can toast「先停止」
    replanBtn.disabled = !!editing;
    replanBtn.textContent = "重新规划";
    replanBtn.title = runLocked
      ? "本轮还在执行：请先停止，再重新规划"
      : "按当前计划再拆一次，尽量保留你在拆分台上的修改";
  }
  // 调整… 退出第一屏（能力 DOM 保留，handler 仍绑）
  const more = $("split-more-actions");
  if (more) more.hidden = true;
}

export { toast, hasActiveRun, isRunPaused, canEditTask, $ };
