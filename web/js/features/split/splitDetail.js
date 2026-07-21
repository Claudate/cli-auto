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
    `<p class="muted split-route-hint" id="split-route-hint">智能建议可改；不会静默覆盖你已改过的通道。</p>` +
    `<div class="split-route-grid">` +
    `<div class="split-route-row"><span class="muted">通道</span> <strong id="split-route-provider-label">—</strong></div>` +
    `<div class="split-route-row"><span class="muted">角色</span> <span id="split-route-role">—</span></div>` +
    `<div class="split-route-row"><span class="muted">范围</span> <span id="split-route-scope">—</span></div>` +
    `</div>` +
    `<label class="field split-route-provider-field" id="split-route-provider-field">` +
    `<span>本步骤执行通道</span>` +
    `<select id="split-route-provider">` +
    `<option value="claude">默认通道</option>` +
    `<option value="codex">备用通道</option>` +
    `<option value="fake">演练</option>` +
    `</select>` +
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
      if (editProv && document.activeElement !== editProv) {
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
      if (promptEl) {
        promptEl.hidden = false;
        promptEl.classList.add("md-body");
        promptEl.innerHTML = md(displayBody || full);
        promptEl.scrollTop = 0;
      }
      if (editForm) editForm.hidden = true;
      if (promptLabel) {
        promptLabel.textContent =
          typeof g("flowPromptLabel") === "function"
            ? g("flowPromptLabel")(false)
            : "拆解后的任务内容（执行时按完整 worker 说明进行）";
      }
    }
    if (metaEl) {
      const chars = [...(displayBody || full)].length;
      metaEl.hidden = false;
      metaEl.textContent =
        typeof g("flowConfirmMetaLine") === "function"
          ? g("flowConfirmMetaLine")(chars, editing)
          : editing
            ? `编辑中 · 说明 ${chars} 字`
            : `任务内容 ${chars} 字 · 点左侧可切换步骤`;
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

  const providerField = $("confirm-provider-field");
  if (providerField) {
    providerField.hidden = !cur || editing;
  }
  if (providerSel) {
    if (document.activeElement !== providerSel) {
      providerSel.value = cur ? curProvider : "claude";
    }
    providerSel.disabled = !cur || !taskEditable || editing || !!runLocked;
    providerSel.onchange = async () => {
      if (!cur || !taskEditable || hasActiveRun() || vm.getSnapshot().editing) {
        providerSel.value = curProvider;
        return;
      }
      const next = (providerSel.value || "claude").toLowerCase();
      if (next === curProvider) return;
      try {
        await vm.setProvider(cur.id, next);
        pushSelection();
        toast(`已设「${cur.title || cur.id}」→ ${engineLabel(next)}`);
        afterMutate();
        render();
      } catch (e) {
        providerSel.value = curProvider;
        toast(String(e?.message || e));
      }
    };
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
  const roleEl = $("split-route-role");
  const scopeEl = $("split-route-scope");
  const advSel = $("split-route-provider");
  if (provLabel) provLabel.textContent = route.providerLabel;
  if (roleEl) roleEl.textContent = route.roleLabel;
  if (scopeEl) {
    scopeEl.textContent = route.scopeText;
    scopeEl.classList.toggle("muted", !route.hasExplicitScope);
  }
  if (advSel) {
    if (document.activeElement !== advSel) {
      advSel.value = route.provider;
    }
    advSel.disabled =
      !ctx.taskEditable || ctx.editing || !!ctx.runLocked;
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
