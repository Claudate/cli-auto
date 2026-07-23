/**
 * [INPUT]: legacy · chatApi · chatState · format · planDir · host · chatRender
 * [OUTPUT]: normalize · save · assign · direct · preview（计划草稿操作）
 * [POS]: A5-2a features/chat；自 chatActions 纵切（P-ship-D）
 * note: 「直接执行」= plan_mode=direct + forceAutoStart；禁止 start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  toast,
  showPage,
  hasActiveRun,
  toastRunLocked,
  selectPlan,
  startExecuteFromSelection,
  openPlanChooser,
  updateChooserAssignState,
  loadPlansForPicker,
} from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { host } from "./host.js";
import {
  ensureChatState,
  stashChatSession,
} from "./chatState.js";
import { getPlansDir } from "./planDir.js";
import {
  chatExtractPlanFence,
  chatNormMdKey,
  chatPlanCardRaw,
} from "./chatFormat.js";
import { renderChatPage } from "./chatRender.js";

/** G0b: re-structure current draft via chat_normalize_plan_cmd. */
export async function normalizeChatDraft(hint) {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return null;
  }
  const draft = state.chatSession?.draft_plan;
  let md = draft?.markdown;
  if (!md) {
    const msgs = state.chatSession?.messages || [];
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].role === "assistant") {
        const m = String(msgs[i].content || "").match(/```plan\s*([\s\S]*?)```/i);
        if (m) {
          md = m[1].trim();
          break;
        }
      }
    }
  }
  if (!md) {
    toast("还没有可整理的计划草稿");
    return null;
  }
  state.chatBusy = true;
  renderChatPage();
  try {
    const resp = await chatApi.normalizePlan({
      project: state.selectedPath,
      markdown: md,
      hint: hint || null,
    });
    const out = resp?.markdown || md;
    const title = resp?.title || host.planTitleFromMarkdown(out);
    if (!state.chatSession.draft_plan) {
      state.chatSession.draft_plan = {
        path: "",
        saved: false,
        markdown: out,
        title,
      };
    } else {
      state.chatSession.draft_plan.markdown = out;
      state.chatSession.draft_plan.title = title;
      if (!state.chatSession.draft_plan.path) {
        state.chatSession.draft_plan.saved = false;
      }
    }
    stashChatSession(state.selectedPath);
    toast(
      resp?.used_cli
        ? "已用 CLI 整理计划结构"
        : "已整理计划结构（本地模板补全）"
    );
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  } finally {
    state.chatBusy = false;
    stashChatSession(state.selectedPath);
    renderChatPage();
  }
}

export async function saveChatPlan(opts) {
  ensureChatState();
  if (!state.selectedPath) return;
  const draft = state.chatSession?.draft_plan;
  let md = (opts && opts.markdown) || draft?.markdown;
  if (!md) {
    // try extract from last assistant message (nesting-aware; do not cut at ```text)
    const msgs = state.chatSession.messages || [];
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].role === "assistant") {
        const extracted = chatExtractPlanFence(msgs[i].content || "");
        if (extracted) {
          md = extracted;
          break;
        }
      }
    }
  }
  if (!md) {
    toast("还没有可保存的计划草稿，请先让 AI 生成计划");
    return;
  }
  // Overwrite only when re-saving the *same* already-saved draft body (H1 未执行可改).
  // New ```plan body (path cleared by chat_send) or asCopy → planRel null → 新建 chat-*.md.
  // Never pass a stale chatDraftPlan alone — that overwrote unrelated plan files.
  const overwriteRel =
    opts && opts.asCopy
      ? null
      : (opts && opts.planRel) ||
        (draft?.saved && draft?.path ? draft.path : null) ||
        null;
  const plansDir = getPlansDir();
  // G2: one path confirm before write (skip when opts.skipConfirm or asCopy from full-view)
  if (!(opts && opts.skipConfirm)) {
    const previewPath =
      overwriteRel ||
      `${plansDir}/chat-${new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "").slice(0, 13)}.md`;
    const ok = window.confirm(
      overwriteRel
        ? `将覆盖已保存计划：\n${overwriteRel}\n\n确定保存？`
        : `将保存到：\n${previewPath}\n\n确定保存？`
    );
    if (!ok) return null;
  }
  state.chatBusy = true;
  renderChatPage();
  try {
    const resp = await chatApi.savePlan({
      project: state.selectedPath,
      markdown: md,
      sessionId: state.chatSession.session_id || "default",
      title: (opts && opts.title) || draft?.title || null,
      planRel: overwriteRel || null,
      plansDir: overwriteRel ? null : plansDir,
    });
    state.chatDraftPlan = resp.plan_rel;
    // Author-phase identity: the just-saved file is what「拆成步骤」must target.
    // Do not leave selectedPlan on a prior plan (pilotdeck hang).
    state.selectedPlan = resp.plan_rel;
    state.chatProjectPath = state.selectedPath;
    if (state.chatSession.draft_plan) {
      state.chatSession.draft_plan.path = resp.plan_rel;
      state.chatSession.draft_plan.saved = true;
      state.chatSession.draft_plan.markdown = md;
    } else {
      state.chatSession.draft_plan = {
        path: resp.plan_rel,
        saved: true,
        markdown: md,
        title: draft?.title || null,
      };
    }
    stashChatSession(state.selectedPath);
    // refresh plans list so chooser + 计划管理 see it
    try {
      await loadPlansForPicker();
    } catch (_) {}
    // 刷新 meta（计划管理页用；聊天右栏已撤）
    try {
      await host.loadPlanRail();
    } catch (_) {}
    toast(`计划已保存：${resp.plan_rel}`);
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  } finally {
    state.chatBusy = false;
    stashChatSession(state.selectedPath);
    renderChatPage();
  }
}

export async function assignFromChat() {
  ensureChatState();
  if (!state.chatDraftPlan) {
    toast("请先保存计划");
    return;
  }
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(state.chatDraftPlan, {
      source: "chat",
      fakeNote: !!state.chatFake,
    });
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  if (state.chatFake) {
    toast("注意：当前计划来自本地模板（非真实 AI），确认后仍将进入执行");
  }
  try {
    await selectPlan(state.chatDraftPlan);
    showPage("workspace");
    openPlanChooser(true);
    updateChooserAssignState();
    toast("已选中计划 · 确认选项后点「拆成步骤」");
  } catch (e) {
    toast(String(e?.message || e));
  }
}

/**
 * Ensure plan-card body is on disk as the selected plan (silent save when needed).
 * Shared by「拆成步骤」and「直接执行」— never starts a run by itself.
 * @param {HTMLElement|null} btn
 * @returns {Promise<string|null>} plan relative path or null on abort/error
 */
async function ensurePlanSavedFromCard(btn) {
  ensureChatState();
  const card = btn?.closest?.(".chat-plan-card");
  // Expand view is rendered HTML; raw lives in .chat-plan-raw.
  const md = chatPlanCardRaw(card);
  if (md) {
    const prevKey = chatNormMdKey(state.chatSession?.draft_plan?.markdown || "");
    const nextKey = chatNormMdKey(md);
    if (!state.chatSession.draft_plan) {
      // Card body only — never seed path from chatDraftPlan alone (stale identity).
      state.chatSession.draft_plan = {
        path: "",
        saved: false,
        markdown: md,
        title: null,
      };
      state.chatDraftPlan = null;
    } else {
      state.chatSession.draft_plan.markdown = md;
      if (prevKey && nextKey && prevKey !== nextKey) {
        state.chatSession.draft_plan.path = "";
        state.chatSession.draft_plan.saved = false;
        state.chatDraftPlan = null;
      }
    }
  }
  const draft = state.chatSession?.draft_plan;
  // Path + saved is not enough: a polluted session may keep old plan_rel while
  // draft.markdown already changed (pre-fix). Require disk body match before skip-save.
  let alreadySaved = !!(
    state.chatDraftPlan &&
    draft?.saved &&
    draft?.path &&
    draft.path === state.chatDraftPlan &&
    chatNormMdKey(draft.markdown || "")
  );
  if (alreadySaved && state.selectedPath) {
    try {
      const disk = await chatApi.readPlanMd(state.selectedPath, draft.path);
      if (chatNormMdKey(disk) !== chatNormMdKey(draft.markdown || "")) {
        // Identity lies — force new file, do not overwrite / split the old plan.
        draft.path = "";
        draft.saved = false;
        state.chatDraftPlan = null;
        alreadySaved = false;
      }
    } catch (_) {
      draft.path = "";
      draft.saved = false;
      state.chatDraftPlan = null;
      alreadySaved = false;
    }
  }
  if (!alreadySaved) {
    const resp = await saveChatPlan({ skipConfirm: true });
    if (!resp?.plan_rel && !state.chatDraftPlan) {
      return null;
    }
  }
  return state.chatDraftPlan || state.selectedPlan || null;
}

/**
 * B2：未落盘时先静默保存，再走 startExecuteFromSelection（默认直拆，禁止 start_run）。
 * @param {HTMLElement|null} btn  plan card button (optional; seeds markdown from card)
 */
export async function assignAndSplitFromChat(btn) {
  ensureChatState();
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  const path = await ensurePlanSavedFromCard(btn);
  if (!path) return;
  await assignFromChat();
}

/**
 * Plan-card「直接执行」：整份计划 = 单任务，跳过多步智能拆分；仍走
 * start_plan_job(plan_mode=direct) → confirm_start（禁止 start_run 旁路）。
 * 业务 optional 仍会停在拆分台；系统收尾默认勾选时可直接开跑。
 * @param {HTMLElement|null} btn
 */
export async function assignAndDirectFromChat(btn) {
  ensureChatState();
  if (hasActiveRun()) {
    toastRunLocked("直接执行");
    return;
  }
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const path = await ensurePlanSavedFromCard(btn);
  if (!path) return;
  // One-shot: force plan_mode=direct + auto-confirm after planned (see jobPoll).
  state.forcePlanModeDirect = true;
  state.forceAutoStartAfterPlan = true;
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(path, {
      source: "chat-direct",
      fakeNote: !!state.chatFake,
      direct: true,
    });
    return;
  }
  toast("执行入口未就绪，请刷新窗口后重试");
}

/** Ready-bar「打开预览」→ App 内全文 modal（不默认 open_path）. */
export async function previewChatPlan() {
  if (!state.chatDraftPlan || !state.selectedPath) return;
  await host.openPlanFullView(state.chatDraftPlan);
}
