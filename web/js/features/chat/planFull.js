/**
 * [INPUT]: legacy · chatApi · planDir · host rail/ready
 * [OUTPUT]: plan full-view modal · diff · save · assign
 * [POS]: A5-2a features/chat/planFull.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  toastRunLocked,
  normalizePlanPath,
  selectPlan,
  startExecuteFromSelection,
  openPlanChooser,
  updateChooserAssignState,
  loadPlansForPicker,
} from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { host } from "./host.js";
import { ensureChatState, stashChatSession } from "./chatState.js";
import { getPlansDir } from "./planDir.js";
import { chatEsc } from "./chatFormat.js";
import { renderMarkdown } from "../../shared/markdown.js";

export function planFullState() {
  ensureChatState();
  return state.planFull;
}

export function closePlanFullView() {
  ensureChatState();
  const pf = state.planFull;
  if (pf?.dirty && (pf.editing || pf.diffing)) {
    const ok = window.confirm("有未保存改动，确定关闭？");
    if (!ok) return;
  }
  state.planFull = {
    open: false,
    path: null,
    title: null,
    markdown: "",
    original: "",
    editing: false,
    dirty: false,
    everCompleted: false,
    lastRunStatus: null,
    saving: false,
    diffing: false,
    diffLeft: "",
    diffRight: "",
  };
  renderPlanFullView();
  host.renderPlanRail();
}

/**
 * C3/P2-9: line-level LCS diff (left=disk, right=current draft).
 * Pure local; no cloud. Returns rows: {tag:'eq'|'del'|'add', text}.
 */
export function computeLineDiff(leftText, rightText) {
  const a = String(leftText || "").replace(/\r\n/g, "\n").split("\n");
  const b = String(rightText || "").replace(/\r\n/g, "\n").split("\n");
  const n = a.length;
  const m = b.length;
  // Cap for UI safety (very large plans): still usable, O(n*m) memory.
  if (n * m > 400_000) {
    // Fallback: simple prefix/suffix + middle as del/add blocks
    let i = 0;
    while (i < n && i < m && a[i] === b[i]) i += 1;
    let j = 0;
    while (j < n - i && j < m - i && a[n - 1 - j] === b[m - 1 - j]) j += 1;
    const rows = [];
    for (let k = 0; k < i; k++) rows.push({ tag: "eq", text: a[k] });
    for (let k = i; k < n - j; k++) rows.push({ tag: "del", text: a[k] });
    for (let k = i; k < m - j; k++) rows.push({ tag: "add", text: b[k] });
    for (let k = n - j; k < n; k++) rows.push({ tag: "eq", text: a[k] });
    return rows;
  }
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i] === b[j]) dp[i][j] = dp[i + 1][j + 1] + 1;
      else dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const rows = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      rows.push({ tag: "eq", text: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      rows.push({ tag: "del", text: a[i] });
      i += 1;
    } else {
      rows.push({ tag: "add", text: b[j] });
      j += 1;
    }
  }
  while (i < n) {
    rows.push({ tag: "del", text: a[i] });
    i += 1;
  }
  while (j < m) {
    rows.push({ tag: "add", text: b[j] });
    j += 1;
  }
  return rows;
}

export function renderPlanDiffHtml(rows) {
  if (!rows || !rows.length) {
    return `<div class="plan-diff-row eq empty"><span class="mark"> </span><span class="txt">（两边皆空）</span></div>`;
  }
  return rows
    .map((r) => {
      const mark = r.tag === "del" ? "−" : r.tag === "add" ? "+" : " ";
      const txt = r.text === "" ? " " : chatEsc(r.text);
      return `<div class="plan-diff-row ${r.tag}"><span class="mark">${mark}</span><span class="txt">${txt}</span></div>`;
    })
    .join("");
}

export function openPlanFullDiff() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  // Left = disk/original; Right = current editor/draft
  const editor = $("#plan-full-editor");
  const current =
    pf.editing && editor ? editor.value : pf.markdown != null ? pf.markdown : pf.original || "";
  pf.diffLeft = pf.original || "";
  pf.diffRight = current || "";
  pf.diffing = true;
  // Leave edit mode chrome but keep draft text
  if (pf.editing && editor) {
    pf.markdown = editor.value;
    pf.dirty = pf.markdown !== pf.original;
  }
  pf.editing = false;
  renderPlanFullView();
}

export function closePlanFullDiff() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  pf.diffing = false;
  renderPlanFullView();
}

/**
 * Adopt left (disk) or right (current) into draft, then enter edit mode.
 * Does NOT auto-save; user still uses chat_save_plan via Save.
 */
export function adoptPlanDiffSide(side) {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.diffing) return;
  if (pf.everCompleted && side === "left") {
    // adopting disk into edit is fine (no overwrite yet); keep as-is
  }
  const text = side === "left" ? pf.diffLeft || "" : pf.diffRight || "";
  pf.markdown = text;
  pf.dirty = text !== (pf.original || "");
  pf.diffing = false;
  pf.editing = true;
  renderPlanFullView();
  const editor = $("#plan-full-editor");
  if (editor) {
    editor.value = text;
    editor.focus();
  }
  toast(side === "left" ? "已采用磁盘稿（未保存）" : "已保留当前稿（未保存）");
}

export async function openPlanFullView(planPath, meta) {
  ensureChatState();
  if (!state.selectedPath || !planPath) return;
  const root = state.selectedPath;
  const path = normalizePlanPath(planPath, root) || planPath;
  // Resolve meta from rail if not provided
  const rail = (state.planRailItems || []).find((it) => it.path === path) || meta || {};
  const everCompleted = !!(rail.ever_completed || rail.everCompleted);
  const lastRunStatus = rail.last_run_status || rail.lastRunStatus || null;

  let markdown = "";
  try {
    markdown = await chatApi.readPlanMd(root, path);
  } catch (e) {
    toast(String(e?.message || e));
    return;
  }
  const title =
    rail.title || host.planTitleFromMarkdown(markdown) || host.planRailTitleFromPath(path);
  state.planFull = {
    open: true,
    path,
    title,
    markdown: String(markdown || ""),
    original: String(markdown || ""),
    editing: false,
    dirty: false,
    everCompleted,
    lastRunStatus,
    saving: false,
    diffing: false,
    diffLeft: "",
    diffRight: "",
  };
  renderPlanFullView();
  host.renderPlanRail();
}

export function renderPlanFullView() {
  ensureChatState();
  const modal = $("#plan-full-view");
  if (!modal) return;
  const pf = state.planFull || { open: false };
  modal.hidden = !pf.open;
  if (!pf.open) return;

  const titleEl = $("#plan-full-title");
  const pathEl = $("#plan-full-path");
  const badgeEl = $("#plan-full-status-badge");
  const dirtyEl = $("#plan-full-dirty");
  const viewBody = $("#plan-full-view-body");
  const editBody = $("#plan-full-edit-body");
  const diffBody = $("#plan-full-diff-body");
  const mdEl = $("#plan-full-md");
  const editor = $("#plan-full-editor");
  const editHint = $("#plan-full-edit-hint");
  const diffEl = $("#plan-full-diff");
  const diffStats = $("#plan-diff-stats");

  const btnEdit = $("#btn-plan-full-edit");
  const btnDiff = $("#btn-plan-full-diff");
  const btnDiffClose = $("#btn-plan-full-diff-close");
  const btnDiffLeft = $("#btn-plan-full-diff-left");
  const btnDiffRight = $("#btn-plan-full-diff-right");
  const btnSave = $("#btn-plan-full-save");
  const btnSaveAs = $("#btn-plan-full-save-as");
  const btnCancel = $("#btn-plan-full-cancel-edit");
  const btnAssign = $("#btn-plan-full-assign");

  if (titleEl) titleEl.textContent = pf.title || host.planRailTitleFromPath(pf.path) || "计划全文";
  if (pathEl) pathEl.textContent = pf.path || "—";

  const badge = host.planRailBadgeInfo({
    ever_completed: pf.everCompleted,
    last_run_status: pf.lastRunStatus,
  });
  if (badgeEl) {
    badgeEl.textContent = badge.label;
    badgeEl.className = `plan-rail-badge ${badge.cls}`;
  }
  if (dirtyEl) dirtyEl.hidden = !pf.dirty;

  if (pf.diffing) {
    if (viewBody) viewBody.hidden = true;
    if (editBody) editBody.hidden = true;
    if (diffBody) diffBody.hidden = false;
    const rows = computeLineDiff(pf.diffLeft || "", pf.diffRight || "");
    let add = 0;
    let del = 0;
    for (const r of rows) {
      if (r.tag === "add") add += 1;
      else if (r.tag === "del") del += 1;
    }
    if (diffStats) {
      diffStats.textContent =
        add === 0 && del === 0 ? "无差异" : `+${add} / −${del} 行`;
    }
    if (diffEl) diffEl.innerHTML = renderPlanDiffHtml(rows);
  } else if (pf.editing) {
    if (viewBody) viewBody.hidden = true;
    if (editBody) editBody.hidden = false;
    if (diffBody) diffBody.hidden = true;
    if (editor && document.activeElement !== editor) {
      editor.value = pf.markdown || "";
    }
    if (editHint) {
      editHint.textContent = pf.everCompleted
        ? "该计划已有完成的执行记录，禁止原地改写；请「另存副本」后再改。"
        : "未执行计划可直接覆盖保存；保存后路径与就绪条一致。";
    }
  } else {
    if (viewBody) viewBody.hidden = false;
    if (editBody) editBody.hidden = true;
    if (diffBody) diffBody.hidden = true;
    // 只读预览：按 Markdown 渲染；编辑态仍用 textarea 源文
    if (mdEl) {
      mdEl.classList.add("md-body");
      mdEl.innerHTML = renderMarkdown(pf.markdown || "");
    }
  }

  // Buttons
  if (btnEdit) {
    // In view mode: show Edit; in edit/diff mode hide (use cancel / exit)
    btnEdit.hidden = !!pf.editing || !!pf.diffing;
    btnEdit.disabled = !!pf.saving;
    btnEdit.textContent = pf.everCompleted ? "另存副本再改" : "编辑";
    btnEdit.title = pf.everCompleted
      ? "已执行计划不可原地改；将复制为新计划后编辑"
      : "在 App 内编辑计划正文";
  }
  if (btnDiff) {
    // Available in view + edit (compare disk original vs current draft)
    btnDiff.hidden = !!pf.diffing;
    btnDiff.disabled = !!pf.saving;
    btnDiff.title = "对比磁盘稿与当前草稿（本机）";
  }
  if (btnDiffClose) {
    btnDiffClose.hidden = !pf.diffing;
    btnDiffClose.disabled = !!pf.saving;
  }
  if (btnDiffLeft) {
    btnDiffLeft.hidden = !pf.diffing;
    btnDiffLeft.disabled = !!pf.saving;
  }
  if (btnDiffRight) {
    btnDiffRight.hidden = !pf.diffing;
    btnDiffRight.disabled = !!pf.saving;
  }
  if (btnSave) {
    // Save overwrite: only when editing && !everCompleted
    btnSave.hidden = !pf.editing || !!pf.everCompleted || !!pf.diffing;
    btnSave.disabled = !!pf.saving || !pf.dirty;
    btnSave.textContent = pf.saving ? "保存中…" : "保存";
  }
  if (btnSaveAs) {
    // Save-as always available in edit mode (and primary path for completed)
    btnSaveAs.hidden = !pf.editing || !!pf.diffing;
    btnSaveAs.disabled = !!pf.saving;
    btnSaveAs.textContent = pf.saving ? "保存中…" : "另存副本";
  }
  if (btnCancel) {
    btnCancel.hidden = !pf.editing || !!pf.diffing;
    btnCancel.disabled = !!pf.saving;
  }
  if (btnAssign) {
    // 未保存改动禁止执行
    const canAssign = !!pf.path && !pf.dirty && !pf.editing && !pf.diffing;
    btnAssign.disabled = !canAssign || !!pf.saving;
    btnAssign.textContent = "拆成步骤";
    btnAssign.title = pf.dirty
      ? "请先保存改动再执行"
      : pf.editing || pf.diffing
        ? "请先保存或取消编辑再执行"
        : "带上该计划进入执行选项";
  }
}

export function beginPlanFullEdit() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  pf.diffing = false;
  if (pf.everCompleted) {
    // 已执行：走另存副本路径（先进入编辑，保存只能 save-as）
    pf.editing = true;
    pf.dirty = false;
    // Seed editor with current text; user edits then 另存副本
    renderPlanFullView();
    const editor = $("#plan-full-editor");
    editor?.focus();
    toast("已执行计划不可覆盖原文件，请编辑后点「另存副本」");
    return;
  }
  pf.editing = true;
  pf.dirty = false;
  renderPlanFullView();
  $("#plan-full-editor")?.focus();
}

export function cancelPlanFullEdit() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open) return;
  if (pf.dirty) {
    const ok = window.confirm("放弃未保存改动？");
    if (!ok) return;
  }
  pf.markdown = pf.original;
  pf.editing = false;
  pf.diffing = false;
  pf.dirty = false;
  renderPlanFullView();
}

export function onPlanFullEditorInput() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.editing) return;
  const editor = $("#plan-full-editor");
  if (!editor) return;
  pf.markdown = editor.value;
  pf.dirty = pf.markdown !== pf.original;
  // Lightweight dirty badge + assign disable without full re-render (keeps caret)
  const dirtyEl = $("#plan-full-dirty");
  if (dirtyEl) dirtyEl.hidden = !pf.dirty;
  const btnSave = $("#btn-plan-full-save");
  if (btnSave && !pf.everCompleted) btnSave.disabled = !pf.dirty || !!pf.saving;
  const btnAssign = $("#btn-plan-full-assign");
  if (btnAssign) {
    btnAssign.disabled = true;
    btnAssign.title = "请先保存改动再执行";
  }
}

export async function savePlanFullView({ asCopy = false } = {}) {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !state.selectedPath) return;
  if (!pf.editing) return;
  const editor = $("#plan-full-editor");
  const md = (editor?.value ?? pf.markdown ?? "").trim();
  if (!md) {
    toast("计划内容为空，无法保存");
    return;
  }
  if (pf.everCompleted && !asCopy) {
    toast("已执行计划禁止原地覆盖，请「另存副本」");
    return;
  }
  if (!asCopy && !pf.dirty && md === (pf.original || "").trim()) {
    toast("没有改动");
    return;
  }
  pf.saving = true;
  renderPlanFullView();
  try {
    const resp = await chatApi.savePlan({
      project: state.selectedPath,
      markdown: md,
      sessionId: state.chatSession?.session_id || "default",
      title: host.planTitleFromMarkdown(md) || pf.title || null,
      planRel: asCopy ? null : pf.path,
      plansDir: asCopy ? getPlansDir() : null,
    });
    const newPath = resp.plan_rel;
    // Sync ready-bar path so CTA matches
    state.chatDraftPlan = newPath;
    if (state.chatSession) {
      if (!state.chatSession.draft_plan) {
        state.chatSession.draft_plan = {
          path: newPath,
          saved: true,
          markdown: md,
          title: host.planTitleFromMarkdown(md),
        };
      } else {
        state.chatSession.draft_plan.path = newPath;
        state.chatSession.draft_plan.saved = true;
        state.chatSession.draft_plan.markdown = md;
      }
    }
    stashChatSession(state.selectedPath);

    pf.path = newPath;
    pf.markdown = md;
    pf.original = md;
    pf.dirty = false;
    pf.editing = false;
    pf.title = host.planTitleFromMarkdown(md) || host.planRailTitleFromPath(newPath);
    // 副本视为未执行
    if (asCopy) {
      pf.everCompleted = false;
      pf.lastRunStatus = null;
    }
    try {
      await loadPlansForPicker();
    } catch (_) {}
    try {
      await host.loadPlanRail();
    } catch (_) {}
    toast(asCopy ? `已另存副本：${newPath}` : `已保存：${newPath}`);
    host.renderChatReadyBar();
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    pf.saving = false;
    renderPlanFullView();
    host.renderPlanRail();
  }
}

export async function assignFromPlanFullView() {
  ensureChatState();
  const pf = state.planFull;
  if (!pf?.open || !pf.path) {
    toast("请先打开计划");
    return;
  }
  if (pf.editing || pf.dirty) {
    toast("请先保存改动再执行");
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  state.chatDraftPlan = pf.path;
  if (state.chatSession) {
    if (!state.chatSession.draft_plan) {
      state.chatSession.draft_plan = {
        path: pf.path,
        saved: true,
        markdown: pf.markdown || null,
        title: pf.title || null,
      };
    } else {
      state.chatSession.draft_plan.path = pf.path;
      state.chatSession.draft_plan.saved = true;
    }
  }
  stashChatSession(state.selectedPath);
  closePlanFullView();
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(pf.path, { source: "full-view" });
    return;
  }
  try {
    await selectPlan(pf.path);
    showPage("workspace");
    openPlanChooser(true);
    updateChooserAssignState();
    toast("已选中计划 · 确认选项后点「拆成步骤」");
  } catch (e) {
    toast(String(e?.message || e));
  }
}
