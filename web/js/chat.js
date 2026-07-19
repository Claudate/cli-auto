/**
 * [INPUT]: state · invoke · selectPlan / openPlanChooser / showPage / toast
 * [OUTPUT]: 聊天建计划 UI · chat_send/save · 分配跳转（方案 A）
 * [POS]: web/js 聊天页；不 spawn worker，分配同源 analyzePlanFromPicker
 * note: chatBusy 显示「思考中」气泡；发送禁用防双发；后端 spawn_blocking 不堵 UI
 * note: chatSessions 按项目缓存，切页不丢；load 有竞态序号，busy 中不重载磁盘
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — chat plan builder */

function chatProjectName() {
  const proj = (state.projects || []).find((p) => p.path === state.selectedPath);
  if (proj?.name) return proj.name;
  if (!state.selectedPath) return "";
  const parts = String(state.selectedPath).split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || state.selectedPath;
}

function ensureChatState() {
  if (!state.chatSession) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null };
  }
  if (state.chatBusy == null) state.chatBusy = false;
  if (state.chatWaitStartedAt == null) state.chatWaitStartedAt = 0;
  if (state.chatDraftPlan === undefined) state.chatDraftPlan = null;
  if (state.chatFake == null) state.chatFake = false;
  if (state.chatEnvNote === undefined) state.chatEnvNote = null;
  if (!state.chatSessions) state.chatSessions = {};
  if (state.chatProjectPath === undefined) state.chatProjectPath = null;
  if (state._chatLoadSeq == null) state._chatLoadSeq = 0;
}

/** Snapshot current chat UI into per-project cache (survive page switches). */
function stashChatSession(path) {
  ensureChatState();
  const p = path || state.selectedPath || state.chatProjectPath;
  if (!p) return;
  state.chatSessions[p] = {
    session_id: state.chatSession?.session_id || "default",
    messages: Array.isArray(state.chatSession?.messages)
      ? state.chatSession.messages.slice()
      : [],
    draft_plan: state.chatSession?.draft_plan
      ? { ...state.chatSession.draft_plan }
      : null,
    draftPath: state.chatDraftPlan || null,
    fake: !!state.chatFake,
    envNote: state.chatEnvNote || null,
    busy: !!state.chatBusy,
    waitStartedAt: state.chatWaitStartedAt || 0,
  };
}

/** Restore cached chat UI for a project. Returns true if cache hit. */
function restoreChatSession(path) {
  ensureChatState();
  const p = path || state.selectedPath;
  if (!p || !state.chatSessions[p]) return false;
  const c = state.chatSessions[p];
  state.chatProjectPath = p;
  state.chatSession = {
    session_id: c.session_id || "default",
    messages: Array.isArray(c.messages) ? c.messages.slice() : [],
    draft_plan: c.draft_plan ? { ...c.draft_plan } : null,
  };
  state.chatDraftPlan = c.draftPath || null;
  state.chatFake = !!c.fake;
  state.chatEnvNote = c.envNote || null;
  // Do not restore busy across project switches; only same-project page hops.
  if (state.chatBusy && state.chatProjectPath === p) {
    /* keep in-flight send */
  } else {
    state.chatBusy = !!c.busy;
    state.chatWaitStartedAt = c.waitStartedAt || 0;
  }
  return true;
}

/** Elapsed wait label while Claude CLI runs in the background. */
function chatWaitLabel() {
  const started = state.chatWaitStartedAt || 0;
  if (!started) return "AI 正在思考…";
  const sec = Math.max(0, Math.floor((Date.now() - started) / 1000));
  if (sec < 5) return "AI 正在思考…";
  if (sec < 60) return `AI 正在思考…（已等 ${sec}s）`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `AI 正在思考…（已等 ${m}分${s}s，可稍候）`;
}

let _chatWaitTick = null;
function startChatWaitTicker() {
  stopChatWaitTicker();
  _chatWaitTick = setInterval(() => {
    if (!state.chatBusy) {
      stopChatWaitTicker();
      return;
    }
    // Refresh only the pending bubble + send label without full re-render of history.
    const pending = document.querySelector(".chat-msg-pending .chat-msg-body");
    if (pending) pending.textContent = chatWaitLabel();
    const sendBtn = $("#btn-chat-send");
    if (sendBtn && state.chatBusy) sendBtn.textContent = "思考中…";
  }, 1000);
}
function stopChatWaitTicker() {
  if (_chatWaitTick) {
    clearInterval(_chatWaitTick);
    _chatWaitTick = null;
  }
}

function applyChatDraftFromSession(sess) {
  ensureChatState();
  if (!sess) {
    state.chatDraftPlan = null;
    return;
  }
  const d = sess.draft_plan || null;
  state.chatSession = {
    session_id: sess.session_id || "default",
    messages: Array.isArray(sess.messages) ? sess.messages : [],
    draft_plan: d,
  };
  if (d && d.saved && d.path) {
    state.chatDraftPlan = d.path;
  } else if (d && d.path && d.saved) {
    state.chatDraftPlan = d.path;
  } else {
    // keep path if previously saved in this UI session
    if (d?.path && d.saved) state.chatDraftPlan = d.path;
    else if (!d?.saved) {
      /* unsaved draft markdown only */
      if (!state.chatDraftPlan) state.chatDraftPlan = null;
    }
  }
  // Prefer server truth for saved path
  if (d?.saved && d.path) state.chatDraftPlan = d.path;
  else if (!d?.saved) {
    // do not clear a previously saved path unless server says different project load
  }
}

/**
 * Load chat for the selected project.
 * - Prefer in-memory cache (page hop) so history never blanks.
 * - Skip disk reload while a send is in flight (would race and wipe optimistic msgs).
 * - Disk load uses a sequence token so stale responses cannot clobber newer state.
 */
async function loadChatSession(opts) {
  ensureChatState();
  const force = !!(opts && opts.force);
  if (!state.selectedPath) {
    state.chatSession = { session_id: "default", messages: [], draft_plan: null };
    state.chatDraftPlan = null;
    state.chatFake = false;
    state.chatEnvNote = null;
    state.chatProjectPath = null;
    renderChatPage();
    return;
  }
  const path = state.selectedPath;

  // Leaving a different project's chat: stash previous, then restore/load this one.
  if (state.chatProjectPath && state.chatProjectPath !== path) {
    stashChatSession(state.chatProjectPath);
  }

  // Same project + in-flight send: never re-fetch (keeps optimistic user + pending bubble).
  if (!force && state.chatBusy && state.chatProjectPath === path) {
    renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    return;
  }

  // Page hop back to same project: restore cache first so UI is never empty,
  // then optionally refresh from disk if we have no local messages yet.
  if (state.chatProjectPath === path && (state.chatSession?.messages || []).length) {
    stashChatSession(path);
    renderChatPage();
    // Background refresh only when idle and not forced skip.
    if (!force && !state.chatBusy) {
      /* keep showing cache; soft refresh below still runs for disk truth */
    } else {
      return;
    }
  } else if (restoreChatSession(path) && (state.chatSession?.messages || []).length) {
    renderChatPage();
    if (state.chatBusy) startChatWaitTicker();
    // Fall through to soft disk refresh when idle so multi-device/disk edits land.
    if (state.chatBusy) return;
  }

  const seq = ++state._chatLoadSeq;
  const sid = state.chatSession?.session_id || "default";
  try {
    const sess = await invoke("chat_session_get_cmd", {
      project: path,
      sessionId: sid,
    });
    // Stale or project switched mid-flight → drop.
    if (seq !== state._chatLoadSeq || state.selectedPath !== path) return;
    // In-flight send still owns the UI.
    if (state.chatBusy && state.chatProjectPath === path) {
      renderChatPage();
      return;
    }

    const diskMsgs = Array.isArray(sess?.messages) ? sess.messages : [];
    const memMsgs = state.chatSession?.messages || [];
    // Prefer longer history (disk after successful send, or mem if send just finished
    // and disk lag / concurrent get). Never replace a non-empty mem with empty disk
    // unless force.
    const takeDisk =
      force ||
      diskMsgs.length > memMsgs.length ||
      (diskMsgs.length === memMsgs.length && diskMsgs.length > 0) ||
      memMsgs.length === 0;

    if (takeDisk) {
      applyChatDraftFromSession(sess);
      if (sess?.draft_plan?.saved && sess.draft_plan.path) {
        state.chatDraftPlan = sess.draft_plan.path;
      } else if (!sess?.draft_plan?.saved) {
        // Keep mem draftPath if we only had unsaved markdown; clear only when empty mem.
        if (!state.chatDraftPlan || memMsgs.length === 0) {
          state.chatDraftPlan = null;
        }
      }
    }
    state.chatProjectPath = path;
    stashChatSession(path);
  } catch (e) {
    console.warn("chat_session_get failed", e);
    if (!state.chatSession?.messages?.length) {
      // Last resort: cache for this path if any.
      if (!restoreChatSession(path)) {
        state.chatSession = { session_id: "default", messages: [], draft_plan: null };
        state.chatFake = false;
      }
    }
    state.chatProjectPath = path;
  }
  renderChatPage();
  if (state.chatBusy) startChatWaitTicker();
}

function chatEsc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Parse plan markdown for card outline: first # title + up to 4 outline lines. */
function chatPlanOutline(md) {
  const lines = String(md || "").split(/\r?\n/);
  let title = "计划草稿";
  const outline = [];
  for (const line of lines) {
    const t = line.trim();
    if (!t) continue;
    if (title === "计划草稿" && /^#\s+/.test(t)) {
      title = t.replace(/^#\s+/, "").trim() || title;
      continue;
    }
    // Prefer numbered / bullet / ## headings as outline
    if (
      outline.length < 4 &&
      (/^(\d+[\.\)]\s+|[-*•]\s+|#{2,3}\s+)/.test(t) ||
        (outline.length === 0 && t.length < 80 && !t.startsWith("```")))
    ) {
      let item = t
        .replace(/^#{1,3}\s+/, "")
        .replace(/^(\d+[\.\)]\s+|[-*•]\s+)/, "")
        .trim();
      if (item && item !== title) outline.push(item);
    }
  }
  // Fallback: first non-title non-empty lines
  if (outline.length === 0) {
    for (const line of lines) {
      const t = line.trim().replace(/^#+\s+/, "");
      if (!t || t === title || t.startsWith("```")) continue;
      outline.push(t.length > 72 ? t.slice(0, 70) + "…" : t);
      if (outline.length >= 4) break;
    }
  }
  return { title, outline };
}

function chatFormatPlanCard(rawMd) {
  const md = String(rawMd || "").trim();
  const { title, outline } = chatPlanOutline(md);
  const outlineHtml =
    outline.length > 0
      ? `<ul class="chat-plan-outline">${outline
          .map((o) => `<li>${chatEsc(o)}</li>`)
          .join("")}</ul>`
      : `<p class="chat-plan-outline-empty muted">（暂无大纲条目）</p>`;
  // Full body kept in hidden pre for expand; adopt uses same markdown via saveChatPlan
  return (
    `<div class="chat-plan-card" data-plan-md="1">` +
    `<div class="chat-plan-card-label">计划草稿</div>` +
    `<div class="chat-plan-card-title">${chatEsc(title)}</div>` +
    `<div class="chat-plan-summary">` +
    outlineHtml +
    `</div>` +
    `<pre class="chat-plan-pre chat-plan-full" hidden>${chatEsc(md)}</pre>` +
    `<div class="chat-plan-card-actions">` +
    `<button type="button" class="btn ghost sm btn-chat-plan-expand">展开全文</button>` +
    `<button type="button" class="btn primary sm btn-chat-plan-adopt">采用并保存</button>` +
    `</div>` +
    `</div>`
  );
}

function chatFormatBody(text) {
  // light markdown: escape then restore fenced plan / code and bold/newlines
  let t = chatEsc(text);
  t = t.replace(/```plan\n([\s\S]*?)```/gi, (_, body) => {
    // body is already escaped (chatEsc ran on whole text); unescape for outline parse
    const raw = String(body)
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&amp;/g, "&")
      .trim();
    return chatFormatPlanCard(raw);
  });
  t = t.replace(/```[\w]*\n([\s\S]*?)```/g, (_, body) => {
    return `<pre class="chat-code-block">${body.trim()}</pre>`;
  });
  t = t.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  t = t.replace(/\n/g, "<br/>");
  return t;
}

/** Toggle plan card full markdown (expand/collapse). */
function toggleChatPlanExpand(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const summary = card.querySelector(".chat-plan-summary");
  if (!full) return;
  const open = full.hidden;
  full.hidden = !open;
  if (summary) summary.hidden = open;
  btn.textContent = open ? "收起全文" : "展开全文";
}

/** Card「采用并保存」→ same as ready-bar saveChatPlan. */
function adoptChatPlanFromCard(btn) {
  const card = btn?.closest?.(".chat-plan-card");
  if (!card) return;
  const full = card.querySelector(".chat-plan-full");
  const md = full?.textContent?.trim();
  if (!md) {
    toast("卡片中没有可保存的计划正文");
    return;
  }
  ensureChatState();
  // Seed draft_plan so saveChatPlan uses this markdown
  if (!state.chatSession.draft_plan) {
    state.chatSession.draft_plan = {
      path: "",
      saved: false,
      markdown: md,
      title: null,
    };
  } else {
    state.chatSession.draft_plan.markdown = md;
  }
  stashChatSession(state.selectedPath || state.chatProjectPath);
  return saveChatPlan();
}

function renderChatMessages() {
  const list = $("#chat-messages");
  if (!list) return;
  ensureChatState();
  const msgs = state.chatSession.messages || [];
  if (!msgs.length && !state.chatBusy) {
    list.innerHTML = `
      <div class="chat-empty muted">
        <p>用自然语言说明你要做什么。AI 会先帮你写成一份<strong>计划文档</strong>，保存后再点「分配计划」进入拆分执行。</p>
        <p class="chat-hint">例如：「给这个项目加登录页，要支持邮箱密码，写验收标准。」</p>
      </div>`;
    return;
  }
  let html = msgs
    .map((m) => {
      const role = m.role === "assistant" ? "assistant" : m.role === "system" ? "system" : "user";
      const label = role === "assistant" ? "AI" : role === "system" ? "系统" : "我";
      return `<div class="chat-msg chat-msg-${role}">
        <div class="chat-msg-role">${label}</div>
        <div class="chat-msg-body">${chatFormatBody(m.content || "")}</div>
      </div>`;
    })
    .join("");
  // Waiting bubble: user already sent; UI must stay responsive while CLI runs.
  if (state.chatBusy) {
    html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending">
        <span class="chat-pending-dots" aria-hidden="true"></span>
        ${chatEsc(chatWaitLabel())}
      </div>
    </div>`;
  }
  list.innerHTML = html;
  list.scrollTop = list.scrollHeight;
}

function renderChatEnvBar() {
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

function dismissChatEnvBar() {
  state.chatEnvNote = null;
  stashChatSession(state.selectedPath || state.chatProjectPath);
  renderChatEnvBar();
}

function openChatEnvDoctor() {
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

function renderChatReadyBar() {
  const bar = $("#chat-ready-bar");
  if (!bar) return;
  ensureChatState();
  const draft = state.chatSession?.draft_plan;
  const savedPath = state.chatDraftPlan || (draft?.saved ? draft.path : null);
  const hasMd = !!(draft?.markdown);
  const hasUnsavedMd = hasMd && !savedPath;
  const isFake = !!state.chatFake;
  const assignBtn = $("#btn-chat-assign");
  const saveBtn = $("#btn-chat-save");
  const pathEl = $("#chat-saved-path");
  const previewBtn = $("#btn-chat-preview");
  let fakeNote = $("#chat-ready-fake-note");

  // 无 draft markdown 且无已保存路径 → 隐藏就绪条
  if (!savedPath && !hasMd) {
    bar.hidden = true;
    bar.classList.remove("is-fake");
    return;
  }
  bar.hidden = false;
  bar.classList.toggle("is-fake", isFake);

  // fake 标注：本地模板 · 非真实 AI
  if (!fakeNote && bar) {
    fakeNote = document.createElement("span");
    fakeNote.id = "chat-ready-fake-note";
    fakeNote.className = "chat-ready-fake-note";
    fakeNote.textContent = "本地模板 · 非真实 AI";
    bar.insertBefore(fakeNote, bar.firstChild);
  }
  if (fakeNote) fakeNote.hidden = !isFake;

  if (pathEl) {
    if (savedPath) {
      pathEl.textContent = `已保存：${savedPath}`;
    } else if (hasUnsavedMd) {
      pathEl.textContent = isFake
        ? "本地模板草稿（尚未保存）"
        : "计划草稿已就绪（尚未保存）";
    } else {
      pathEl.textContent = "—";
    }
  }

  // CTA 层级：未保存 → primary 保存 + 隐藏分配；已保存 → ghost 重新保存 + primary 分配
  if (saveBtn) {
    const showSave = hasMd || !!savedPath;
    saveBtn.hidden = !showSave;
    saveBtn.disabled = !!state.chatBusy || !hasMd;
    if (savedPath) {
      saveBtn.textContent = "重新保存";
      saveBtn.className = "btn ghost sm";
      saveBtn.title = isFake ? "再次保存本地模板到项目" : "用当前草稿覆盖保存";
    } else {
      saveBtn.textContent = "保存为计划";
      saveBtn.className = "btn primary sm";
      saveBtn.title = isFake
        ? "保存本地模板（非真实 AI）到项目"
        : "将计划草稿保存到项目";
    }
  }
  if (previewBtn) {
    previewBtn.hidden = !savedPath;
    previewBtn.disabled = !!state.chatBusy;
  }
  if (assignBtn) {
    // 未保存：隐藏分配（勿 disabled 占位）；已保存：primary
    if (!savedPath) {
      assignBtn.hidden = true;
      assignBtn.disabled = true;
    } else {
      assignBtn.hidden = false;
      assignBtn.disabled = !!state.chatBusy;
      assignBtn.className = "btn primary sm";
      assignBtn.title = isFake
        ? "分配前请确认：当前为本地模板，非真实 AI"
        : "选中该计划并打开分配选项";
    }
  }
}

function renderChatPage() {
  const projLabel = $("#chat-project-label");
  if (projLabel) {
    projLabel.textContent = state.selectedPath
      ? chatProjectName()
      : "未选择项目";
  }
  const input = $("#chat-input");
  const sendBtn = $("#btn-chat-send");
  if (input) {
    // Keep the composer editable while waiting so the app never feels frozen;
    // only the send button is gated (double-send guard).
    input.disabled = !state.selectedPath;
    input.placeholder = !state.selectedPath
      ? "请先在左侧选择项目"
      : state.chatBusy
        ? "AI 正在回复，可先写下一条…"
        : "说清目标与约束；满意后可让 AI 生成计划文档…";
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
  renderChatMessages();
  renderChatEnvBar();
  renderChatReadyBar();
}

async function openChatPage() {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  // Leaving another page: keep current chat in cache first.
  if (state.chatProjectPath) stashChatSession(state.chatProjectPath);
  showPage("chat");
  // Restore immediately so history is never blank while disk loads.
  restoreChatSession(state.selectedPath);
  renderChatPage();
  await loadChatSession();
}

async function sendChatMessage() {
  ensureChatState();
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  const input = $("#chat-input");
  const text = (input?.value || "").trim();
  if (!text) return;
  if (state.chatBusy) return;

  const projectPath = state.selectedPath;
  state.chatProjectPath = projectPath;
  state.chatBusy = true;
  state.chatWaitStartedAt = Date.now();
  if (input) input.value = "";
  // optimistic user bubble + pending AI bubble (renderChatMessages)
  state.chatSession.messages = [
    ...(state.chatSession.messages || []),
    { role: "user", content: text },
  ];
  stashChatSession(projectPath);
  renderChatPage();
  startChatWaitTicker();

  try {
    // Non-blocking for the webview: Tauri command is async + spawn_blocking.
    // User sees "思考中…" bubble; send is disabled only to avoid double-send.
    const resp = await invoke("chat_send_cmd", {
      project: projectPath,
      message: text,
      sessionId: state.chatSession.session_id || "default",
    });
    // If user switched project mid-send, still write into that project's cache.
    if (state.selectedPath !== projectPath) {
      state.chatSessions[projectPath] = {
        session_id: resp.session_id || "default",
        messages: Array.isArray(resp.messages) ? resp.messages : [],
        draft_plan: resp.draft_plan || null,
        draftPath:
          resp.draft_plan?.saved && resp.draft_plan.path
            ? resp.draft_plan.path
            : state.chatSessions[projectPath]?.draftPath || null,
        fake: !!resp.fake,
        envNote: resp.env_note || null,
        busy: false,
        waitStartedAt: 0,
      };
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

async function saveChatPlan() {
  ensureChatState();
  if (!state.selectedPath) return;
  const draft = state.chatSession?.draft_plan;
  let md = draft?.markdown;
  if (!md) {
    // try extract from last assistant message
    const msgs = state.chatSession.messages || [];
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
    toast("还没有可保存的计划草稿，请先让 AI 生成计划");
    return;
  }
  state.chatBusy = true;
  renderChatPage();
  try {
    const resp = await invoke("chat_save_plan_cmd", {
      project: state.selectedPath,
      markdown: md,
      sessionId: state.chatSession.session_id || "default",
      title: draft?.title || null,
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
    // refresh plans list so chooser sees it
    try {
      await loadPlansForPicker();
    } catch (_) {}
    toast(`计划已保存：${resp.plan_rel}`);
  } catch (e) {
    toast(String(e?.message || e));
  } finally {
    state.chatBusy = false;
    stashChatSession(state.selectedPath);
    renderChatPage();
  }
}

async function assignFromChat() {
  ensureChatState();
  if (!state.chatDraftPlan) {
    toast("请先保存计划");
    return;
  }
  if (hasActiveRun()) {
    toastRunLocked("分配计划");
    return;
  }
  // U0：fake 草稿可保存；分配前 toast 强提示 mock
  if (state.chatFake) {
    toast("注意：当前计划来自本地模板（非真实 AI），确认后仍将进入分配");
  }
  try {
    await selectPlan(state.chatDraftPlan);
    showPage("workspace");
    openPlanChooser(true);
    updateChooserAssignState();
    toast(
      state.chatFake
        ? "已选中本地模板计划（mock），确认选项后点「分配计划」"
        : "已选中聊天生成的计划，确认选项后点「分配计划」"
    );
  } catch (e) {
    toast(String(e?.message || e));
  }
}

async function previewChatPlan() {
  if (!state.chatDraftPlan || !state.selectedPath) return;
  const abs = state.selectedPath.replace(/[/\\]$/, "") + "/" + state.chatDraftPlan;
  try {
    await invoke("open_path", { path: abs });
  } catch (e) {
    toast(String(e?.message || e));
  }
}
