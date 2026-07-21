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
      titleEl.textContent = cur.title || cur.id;
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
        return d ? `${d.title}` : id;
      });
    }
    const depsEl = $("confirm-task-deps");
    if (depsEl) {
      const line =
        typeof g("flowConfirmDepsLine") === "function"
          ? g("flowConfirmDepsLine")(kind, depTitles)
          : depTitles.length
            ? `${kind} · 等待：${depTitles.join(" · ")}`
            : `${kind} · 无依赖，可进首波`;
      depsEl.textContent = line;
      const full =
        cur.prompt || cur.prompt_preview || cur.promptPreview || "";
      let doneLine = "";
      const ol = oneLiner(cur);
      if (ol && /怎样算做完|完成标志|验收|成功标准/.test(String(full || ""))) {
        doneLine = ol;
      } else if (cur.acceptance || cur.done_when || cur.doneWhen) {
        doneLine = String(
          cur.acceptance || cur.done_when || cur.doneWhen
        ).trim();
      }
      if (doneLine) {
        depsEl.textContent =
          (depsEl.textContent ? depsEl.textContent + " · " : "") +
          `怎样算做完：${doneLine}`;
      }
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
      // B5：默认短读 — 一句话 + 怎样算做完；完整说明进 details
      if (editForm) editForm.hidden = true;
      if (promptLabel) {
        promptLabel.textContent = "这一步做什么";
      }
      const bodyText = displayBody || full || "";
      const ol = oneLiner(cur) || "";
      let doneLine = "";
      if (cur.acceptance || cur.done_when || cur.doneWhen) {
        doneLine = String(cur.acceptance || cur.done_when || cur.doneWhen).trim();
      }
      const shortBits = [];
      if (ol) shortBits.push(ol);
      if (doneLine && doneLine !== ol) shortBits.push(`怎样算做完：${doneLine}`);
      const shortHtml = shortBits.length
        ? `<p class="split-detail-short">${esc(shortBits.join(" · "))}</p>`
        : `<p class="split-detail-short muted">点「完整说明」查看给执行 AI 的正文</p>`;
      const fullHtml = md(bodyText);
      if (promptEl) {
        promptEl.hidden = false;
        promptEl.classList.add("md-body");
        promptEl.innerHTML =
          shortHtml +
          `<details class="split-detail-full">` +
          `<summary>完整说明</summary>` +
          `<div class="split-detail-full-body md-body">${fullHtml}</div>` +
          `</details>`;
        promptEl.scrollTop = 0;
      }
    }
    if (metaEl) {
      metaEl.hidden = false;
      metaEl.textContent = editing
        ? "编辑中 · 保存后生效"
        : "点左侧切换步骤 · 需要时展开完整说明";
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
      promptEl.innerHTML = "";
    }
    if (editForm) editForm.hidden = true;
    if (metaEl) {
      metaEl.hidden = true;
      metaEl.textContent = "";
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
  const st = String(job.status || "").toLowerCase();
  const paused = isRunPaused();
  const editing = !!s.editing;
  const err = $("confirm-error");
  if (err && !s.lastError) err.hidden = true;
  if (err && s.lastError) {
    err.hidden = false;
    err.textContent = s.lastError;
  }
  const startBtn = $("btn-confirm-start");
  if (startBtn) {
    startBtn.disabled = !!runLocked || editing || !!s.busy;
    startBtn.textContent = runLocked
      ? "运行中…"
      : paused
        ? "继续运行"
        : st === "confirmed"
          ? "再次确认并开始"
          : "确认并开始";
  }
  const replanBtn = $("btn-replan");
  if (replanBtn) {
    replanBtn.disabled = !!runLocked || editing;
    if (!runLocked) replanBtn.textContent = "重新拆分（保留你的修改）";
  }
}

export { toast, hasActiveRun, isRunPaused, canEditTask, $ };
