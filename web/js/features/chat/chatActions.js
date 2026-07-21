/**
 * [INPUT]: legacy · chatApi · sessions · format · planDir · host
 * [OUTPUT]: render · send · save · normalize · assign · openChat
 * [POS]: A5-2a features/chat/chatActions.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  toastRunLocked,
  selectPlan,
  startExecuteFromSelection,
  openPlanChooser,
  updateChooserAssignState,
  loadPlansForPicker,
  planTemplateChatEmptyHtml,
  openDoctorPage,
  runDoctor,
  loadDoctor,
} from "./legacy.js";
import * as chatApi from "./chatApi.js";
import { host } from "./host.js";
import {
  ensureChatState,
  chatProjectName,
  stashChatSession,
  restoreChatSession,
  applyChatDraftFromSession,
  chatCacheKey,
} from "./chatState.js";
import { getPlansDir, applyPlanRailVisibility } from "./planDir.js";
import { chatEsc, chatExtractPlanFence, chatFormatBody } from "./chatFormat.js";
import {
  clearChatAttachments,
  uploadPendingAttachments,
  renderChatAttachPreview,
} from "./chatAttachments.js";
import {
  loadChatSessionList,
  loadChatSession,
  startChatWaitTicker,
  stopChatWaitTicker,
  renderChatSessionSelect,
  chatWaitLabel,
} from "./chatSessions.js";

export function fillChatExample(text) {
  const input = $("#chat-input");
  if (!input || !state.selectedPath) return;
  input.value = text;
  input.focus();
}

export function renderChatMessages() {
  const list = $("#chat-messages");
  if (!list) return;
  ensureChatState();
  const msgs = state.chatSession.messages || [];
  if (!msgs.length && !state.chatBusy) {
    // T2: empty state + 模板入口委托 templates.js（不在此堆功能）
    list.innerHTML =
      typeof planTemplateChatEmptyHtml === "function"
        ? planTemplateChatEmptyHtml()
        : `<div class="chat-empty muted"><p>用自然语言说明你要做什么，保存后再点「拆成步骤」。</p></div>`;
    return;
  }
  // Only the last assistant message's plan card gets save/execute CTAs
  let lastAssistantIdx = -1;
  for (let i = msgs.length - 1; i >= 0; i--) {
    if (msgs[i]?.role === "assistant") {
      lastAssistantIdx = i;
      break;
    }
  }
  let html = msgs
    .map((m, mi) => {
      const role = m.role === "assistant" ? "assistant" : m.role === "system" ? "system" : "user";
      const label = role === "assistant" ? "AI" : role === "system" ? "系统" : "我";
      const atts = Array.isArray(m.attachments) ? m.attachments : [];
      const attHtml = atts.length
        ? `<div class="chat-msg-atts">${atts
            .map((a) => {
              const src = a._preview || "";
              const name = chatEsc(a.name || a.path || "图");
              if (src) {
                return (
                  `<div class="chat-msg-att">` +
                  `<img class="chat-img-zoomable" src="${src}" alt="${name}" data-img-src="${chatEsc(src)}" data-img-name="${name}" title="点击放大" />` +
                  `<span>${name}</span></div>`
                );
              }
              return `<div class="chat-msg-att chat-msg-att-path" title="${chatEsc(a.path || "")}">📎 ${name}</div>`;
            })
            .join("")}</div>`
        : "";
      const activePlan = role === "assistant" && mi === lastAssistantIdx;
      return `<div class="chat-msg chat-msg-${role}">
        <div class="chat-msg-role">${label}</div>
        <div class="chat-msg-body">${chatFormatBody(m.content || "", { activePlan })}${attHtml}</div>
      </div>`;
    })
    .join("");
  // Waiting bubble: user already sent; UI must stay responsive while CLI runs.
  // C3: if stream partial arrived, show it in place of the wait label.
  if (state.chatBusy) {
    const stream = String(state.chatStreamText || "").trim();
    if (stream) {
      const shown =
        stream.length > 6000 ? "…\n" + stream.slice(-6000) : stream;
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending chat-msg-streaming">${chatEsc(
        shown
      )}<span class="chat-stream-cursor" aria-hidden="true">▍</span></div>
    </div>`;
    } else {
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending">
        <span class="chat-pending-dots" aria-hidden="true"></span>
        ${chatEsc(chatWaitLabel())}
      </div>
    </div>`;
    }
  }
  list.innerHTML = html;
  list.scrollTop = list.scrollHeight;
}

export function renderChatEnvBar() {
  const bar = $("#chat-env-bar");
  if (!bar) return;
  ensureChatState();
  const note = state.chatEnvNote;
  // forced fake 联调也可显示简短 mock 条；有 env_note 优先
  const show = !!(note && String(note).trim());
  bar.hidden = !show;
  const noteEl = $("#chat-env-note");
  if (noteEl && show) noteEl.textContent = String(note).trim();
}

export function dismissChatEnvBar() {
  state.chatEnvNote = null;
  stashChatSession(state.selectedPath || state.chatProjectPath);
  renderChatEnvBar();
}

export function openChatEnvDoctor() {
  try {
    if (typeof showPage === "function") showPage("doctor");
    else if (typeof openDoctorPage === "function") openDoctorPage();
  } catch (_) {
    toast("请从侧栏打开「环境检查」");
  }
  try {
    if (typeof runDoctor === "function") runDoctor();
    else if (typeof loadDoctor === "function") loadDoctor();
  } catch (_) {}
}

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

/**
 * Sticky ready-bar retired: save / re-save / execute live only on the plan card
 * footer inside the assistant reply (bottom of that message). Keep this function
 * so old call sites stay safe; always hide the bar and its fixed buttons.
 */
export function renderChatReadyBar() {
  const bar = $("#chat-ready-bar");
  if (bar) {
    bar.hidden = true;
    bar.classList.remove("is-fake");
  }
  const saveBtn = $("#btn-chat-save");
  const assignBtn = $("#btn-chat-assign");
  const previewBtn = $("#btn-chat-preview");
  const normalizeBtn = $("#btn-chat-normalize");
  if (saveBtn) saveBtn.hidden = true;
  if (assignBtn) assignBtn.hidden = true;
  if (previewBtn) previewBtn.hidden = true;
  if (normalizeBtn) normalizeBtn.hidden = true;
}

export function renderChatPage() {
  const projLabel = $("#chat-project-label");
  if (projLabel) {
    projLabel.textContent = state.selectedPath
      ? chatProjectName()
      : "未选择项目";
  }
  const input = $("#chat-input");
  const sendBtn = $("#btn-chat-send");
  const attachBtn = $("#btn-chat-attach");
  if (input) {
    // Keep the composer editable while waiting so the app never feels frozen;
    // only the send button is gated (double-send guard).
    input.disabled = !state.selectedPath;
    input.placeholder = !state.selectedPath
      ? "请先在左侧选择项目"
      : state.chatBusy
        ? "AI 正在回复，可先写下一条…"
        : "说清目标与约束；可附图；满意后让 AI 生成计划…";
  }
  if (sendBtn) {
    // Disabled while waiting = prevent double-send, NOT app freeze.
    // Backend chat_send runs on a worker thread so the rest of the UI stays live.
    sendBtn.disabled = !state.selectedPath || !!state.chatBusy;
    sendBtn.textContent = state.chatBusy ? "思考中…" : "发送";
    sendBtn.title = state.chatBusy
      ? "正在等待本机 Claude CLI 回复，请稍候"
      : "发送消息";
  }
  if (attachBtn) {
    attachBtn.disabled = !state.selectedPath || !!state.chatBusy;
  }
  renderChatSessionSelect();
  renderChatAttachPreview();
  renderChatMessages();
  renderChatEnvBar();
  renderChatReadyBar();
  host.renderPlanRail();
  host.renderPlanFullView();
}

export async function openChatPage() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  // Leaving another page: keep current chat in cache first.
  if (state.chatProjectPath) stashChatSession(state.chatProjectPath);
  // G0: re-read per-project rail open preference when switching projects
  const railKey = `cco.planRailOpen:${state.selectedPath}`;
  state.planRailOpen = localStorage.getItem(railKey) === "1";
  showPage("chat");
  // Restore immediately so history is never blank while disk loads.
  restoreChatSession(state.selectedPath);
  applyPlanRailVisibility();
  renderChatPage();
  await loadChatSession();
  // C3: session switcher list (best-effort)
  try {
    await loadChatSessionList();
  } catch (_) {}
  // G0/G1: only scan rail when user has opened 计划管理
  if (state.planRailOpen) {
    try {
      await host.loadPlanRail();
    } catch (_) {}
  }
}

export async function sendChatMessage() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const input = $("#chat-input");
  const text = (input?.value || "").trim();
  const hasAtt = (state.chatPendingAttachments || []).length > 0;
  if (!text && !hasAtt) return;
  if (state.chatBusy) return;

  const projectPath = state.selectedPath;
  state.chatProjectPath = projectPath;
  state.chatBusy = true;
  state.chatWaitStartedAt = Date.now();
  state.chatStreamText = "";
  if (input) input.value = "";
  const pendingSnap = (state.chatPendingAttachments || []).slice();
  // optimistic user bubble + pending AI bubble (renderChatMessages)
  const optContent =
    text ||
    (pendingSnap.length ? `（附图 ${pendingSnap.length} 张）` : "");
  state.chatSession.messages = [
    ...(state.chatSession.messages || []),
    {
      role: "user",
      content: optContent,
      attachments: pendingSnap.map((p) => ({
        name: p.name,
        mime: p.mime,
        path: "",
        _preview: p.dataUrl,
      })),
    },
  ];
  clearChatAttachments();
  stashChatSession(projectPath);
  renderChatPage();
  startChatWaitTicker();

  try {
    // G4: upload pending images first, then send with attachment meta
    let attachments = [];
    if (pendingSnap.length) {
      // restore pending temporarily for upload helper
      state.chatPendingAttachments = pendingSnap;
      try {
        attachments = await uploadPendingAttachments();
      } finally {
        state.chatPendingAttachments = [];
      }
    }
    // Non-blocking for the webview: Tauri command is async + spawn_blocking.
    // User sees "思考中…" bubble; send is disabled only to avoid double-send.
    const sendArgs = {
      project: projectPath,
      message: text || (attachments.length ? "（见附图）" : ""),
      sessionId: state.chatSession.session_id || "default",
      attachments: attachments.length ? attachments : null,
    };
    const resp = await chatApi.sendMessage(sendArgs);
    // If user switched project mid-send, still write into that project's cache.
    if (state.selectedPath !== projectPath) {
      const sid = resp.session_id || "default";
      const key = chatCacheKey(projectPath, sid);
      const snap = {
        session_id: sid,
        messages: Array.isArray(resp.messages) ? resp.messages : [],
        draft_plan: resp.draft_plan || null,
        draftPath:
          resp.draft_plan?.saved && resp.draft_plan.path
            ? resp.draft_plan.path
            : state.chatSessions[key]?.draftPath ||
              state.chatSessions[projectPath]?.draftPath ||
              null,
        fake: !!resp.fake,
        envNote: resp.env_note || null,
        busy: false,
        waitStartedAt: 0,
      };
      state.chatSessions[key] = snap;
      state.chatSessions[projectPath] = snap;
    } else {
      applyChatDraftFromSession({
        session_id: resp.session_id,
        messages: resp.messages,
        draft_plan: resp.draft_plan,
      });
      if (resp.draft_plan?.saved && resp.draft_plan.path) {
        state.chatDraftPlan = resp.draft_plan.path;
      }
      // 有 markdown 时记 fake；真实 AI 成功则清掉
      state.chatFake = !!resp.fake;
      // 生产 soft-fallback：env_note 进系统条；forced fake 无 env_note 时用简短 mock 提示
      if (resp.env_note) {
        state.chatEnvNote = String(resp.env_note);
      } else if (resp.fake) {
        state.chatEnvNote = "本地模板联调（CCO_CHAT_FAKE / provider=fake）· 非真实 AI";
      } else {
        state.chatEnvNote = null;
      }
      state.chatProjectPath = projectPath;
      stashChatSession(projectPath);
      // C3: refresh switcher counts/preview after a successful turn
      try {
        await loadChatSessionList();
      } catch (_) {}
    }
    if (resp.fake) {
      if (resp.env_note) {
        toast("本机 Claude CLI 暂不可用，请查看上方环境提示");
      } else {
        toast("当前是本地模板联调（非真实 AI）");
      }
    }
  } catch (e) {
    if (state.selectedPath === projectPath) {
      state.chatSession.messages.push({
        role: "system",
        content: `发送失败：${e?.message || e}`,
      });
      stashChatSession(projectPath);
    }
    toast(String(e?.message || e));
  } finally {
    if (state.selectedPath === projectPath) {
      state.chatBusy = false;
      state.chatWaitStartedAt = 0;
      state.chatStreamText = "";
      stopChatWaitTicker();
      stashChatSession(projectPath);
      renderChatPage();
      input?.focus();
    } else if (state.chatSessions[projectPath]) {
      state.chatSessions[projectPath].busy = false;
      state.chatSessions[projectPath].waitStartedAt = 0;
    }
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
  // Overwrite only when re-saving an already-saved draft (H1 未执行可改).
  // Unsaved new draft → planRel null → 新建 chat-*.md；asCopy 强制新建。
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
    // refresh plans list so chooser + rail see it
    try {
      await loadPlansForPicker();
    } catch (_) {}
    // 刷新列表：右栏打开或在计划管理页时
    if (state.planRailOpen || state.page === "plans") {
      try {
        await host.loadPlanRail();
      } catch (_) {}
    }
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

/** Ready-bar「打开预览」→ App 内全文 modal（不默认 open_path）. */
export async function previewChatPlan() {
  if (!state.chatDraftPlan || !state.selectedPath) return;
  await host.openPlanFullView(state.chatDraftPlan);
}

/* ══════════════════════════════════════════════
 * H1 — plan-rail list + plan-full-view modal
 * ══════════════════════════════════════════════ */
