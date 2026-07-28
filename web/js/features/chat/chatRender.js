/**
 * [INPUT]: legacy · chatState · format · sessions · attachments · host
 * [OUTPUT]: renderChat* · fillChatExample · env-bar helpers
 * [POS]: A5-2a features/chat；自 chatActions 纵切（P-ship-D）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  toast,
  showPage,
  planTemplateChatEmptyHtml,
  openDoctorPage,
  runDoctor,
  loadDoctor,
} from "./legacy.js";
import { host } from "./host.js";
import {
  ensureChatState,
  chatProjectName,
  stashChatSession,
} from "./chatState.js";
import { chatEsc, chatFormatBody, chatFormatStreamBody } from "./chatFormat.js";
import { renderChatAttachPreview } from "./chatAttachments.js";
import {
  renderChatSessionSelect,
  chatWaitLabel,
} from "./chatSessions.js";
import {
  installClarifyUi,
  renderClarifyPanelHtml,
  renderClarifyInlineIfNeeded,
  ensureClarifyState,
  ensureClaimDraftMessageVisible,
  renderHollowBarHtml,
} from "./chatClarify.js";
import {
  enhanceAssistantBody,
  shouldFoldMessage,
  shouldClampBody,
  wrapClampedBody,
  renderFoldBarHtml,
  renderFoldAgainBtn,
  ensureChatMsgEnhanceStyles,
} from "./chatMsgEnhance.js";
import {
  pathModeSegmentHtml,
  pathModeCoachHtml,
  pathModeClarifyWeight,
  thinClaimSuccessHtml,
  applyPathModeHeadStep,
  getPathMode,
  setPathMode,
} from "./chatPathMode.js";

/** W0: switch path L/M/H and repaint empty / head step. */
export function setPathModeAndPaint(id) {
  const next = setPathMode(id);
  applyPathModeHeadStep(next);
  try {
    renderChatMessages();
  } catch (_) {}
  return next;
}

export function fillChatExample(text) {
  const input = $("#chat-input");
  if (!input || !state.selectedPath) return;
  input.value = text;
  input.focus();
}

/**
 * P2-2: one-line last_summary banner for author empty state.
 * @param {string|null|undefined} text
 */
export function renderLastSummaryBanner(text) {
  const list = $("#chat-messages");
  if (!list) return;
  const existing = list.querySelector(".chat-last-summary");
  if (existing) existing.remove();
  const t = String(text || "").trim();
  if (!t) return;
  // Only show on empty author state (no messages).
  ensureChatState();
  const msgs = state.chatSession.messages || [];
  if (msgs.length || state.chatBusy) return;
  const short = t.length > 120 ? t.slice(0, 119) + "…" : t;
  const bar = document.createElement("div");
  bar.className = "chat-last-summary";
  bar.setAttribute("role", "status");
  // Human resume strip — no internal run ids as first words; sit below clarify card.
  bar.innerHTML =
    `<span class="chat-last-summary-text">要接着上次的想法吗？${chatEsc(short)}</span>` +
    `<span class="chat-last-summary-actions">` +
    `<button type="button" class="linkish" data-last-summary="reuse" title="把上次内容填进输入框">接着上次</button>` +
    `<button type="button" class="linkish muted" data-last-summary="ignore" title="先开新的，不再提示">开新的</button>` +
    `</span>`;
  const empty = list.querySelector(".chat-empty");
  if (empty) {
    // Prefer after clarify panel so the main coach card stays first visual
    const clarify = empty.querySelector(".chat-clarify");
    const secondary = empty.querySelector(".chat-empty-secondary");
    if (clarify && clarify.parentNode === empty) {
      if (clarify.nextSibling) empty.insertBefore(bar, clarify.nextSibling);
      else empty.appendChild(bar);
    } else if (secondary) {
      empty.insertBefore(bar, secondary);
    } else {
      empty.appendChild(bar);
    }
  } else {
    list.appendChild(bar);
  }
}

export function renderChatMessages() {
  const list = $("#chat-messages");
  if (!list) return;
  ensureChatState();
  ensureClarifyState();
  installClarifyUi();
  // t4: after claim + reload, draft_plan may exist without a ```plan bubble
  try {
    ensureClaimDraftMessageVisible();
  } catch (_) {}
  const msgs = state.chatSession.messages || [];
  // Re-read after ensureClaimDraftMessageVisible may have injected a plan bubble
  const msgsNow = state.chatSession.messages || [];
  if (!msgsNow.length && !state.chatBusy) {
    // W0: path L/M/H + coach first; clarify demoted (L hide / M·H fold)
    applyPathModeHeadStep(getPathMode());
    const claimed =
      state.chatClarify?.phase === "claimed_to_plan" ||
      !!state.chatSession?.draft_plan?.markdown;
    const phase = String(state.chatClarify?.phase || "not_started");
    const activeClarify =
      phase === "clarifying" ||
      phase === "brief_ready" ||
      phase === "skipped_to_plan";

    let lead = "";
    let clarifyBlock = "";
    let secondary = "";

    if (claimed) {
      // Thin success only — plan card appears once messages hydrate
      clarifyBlock = thinClaimSuccessHtml();
    } else {
      lead = pathModeSegmentHtml() + pathModeCoachHtml();
      const rawClarify = renderClarifyPanelHtml({ mode: "empty" });
      const weight = pathModeClarifyWeight();
      if (activeClarify) {
        // Mid-flow: keep full panel visible
        clarifyBlock = rawClarify;
      } else if (weight === "hide") {
        clarifyBlock = "";
      } else {
        clarifyBlock =
          `<details class="chat-clarify-fold">` +
          `<summary class="chat-clarify-fold-sum">先问关键的（可跳过）</summary>` +
          rawClarify +
          `</details>`;
      }
      const legacyEmpty =
        typeof planTemplateChatEmptyHtml === "function"
          ? planTemplateChatEmptyHtml()
          : `<div class="chat-empty-legacy muted"><p>用自然语言说明你要做什么，保存后再点「拆成步骤」。</p></div>`;
      secondary =
        `<div class="chat-empty-secondary">` +
        (legacyEmpty.includes('class="chat-empty')
          ? legacyEmpty.replace(
              /<div class="chat-empty[^"]*">/,
              '<div class="chat-empty-legacy">'
            )
          : legacyEmpty) +
        `</div>`;
    }
    list.innerHTML =
      `<div class="chat-empty muted">` +
      lead +
      clarifyBlock +
      secondary +
      `</div>`;
    if (!claimed) {
      const ignored =
        state.selectedPath &&
        localStorage.getItem(`cco.ignoreLastSummary:${state.selectedPath}`) ===
          "1";
      if (!ignored && state.chatLastSummary) {
        renderLastSummaryBanner(state.chatLastSummary);
      }
    }
    return;
  }
  // Every assistant ```plan card stays actionable until saved+alreadySplit
  // (or stream partials, which force activePlan:false). Earlier "last assistant
  // only" froze unexecuted drafts after any later AI turn / preview reply.
  // t3: inline clarify strip when still in clarify flow with messages present
  applyPathModeHeadStep(getPathMode());
  ensureChatMsgEnhanceStyles();
  let clarifyInline = renderClarifyInlineIfNeeded();
  // W0: after claim, avoid dual-card (success preview + plan bubble same md)
  const claimedInline = state.chatClarify?.phase === "claimed_to_plan";
  const hasPlanBubble = msgsNow.some(
    (m) => m && /```plan\b/i.test(String(m.content || ""))
  );
  if (claimedInline && hasPlanBubble) {
    clarifyInline = thinClaimSuccessHtml();
  }
  const total = msgsNow.length;
  let html = (clarifyInline || "") + msgsNow
    .map((m, idx) => {
      const role = m.role === "assistant" ? "assistant" : m.role === "system" ? "system" : "user";
      const label = role === "assistant" ? "AI" : role === "system" ? "系统" : "我";
      const content = m.content || "";
      const atts = Array.isArray(m.attachments) ? m.attachments : [];
      const attHtml = atts.length
        ? `<div class="chat-msg-atts">${atts
            .map((a) => {
              const src = a._preview || "";
              const name = chatEsc(a.name || a.path || "附件");
              const mime = String(a.mime || "").toLowerCase();
              const isImg =
                !!src ||
                mime.startsWith("image/") ||
                /\.(png|jpe?g|webp|gif|svg)$/i.test(a.name || a.path || "");
              if (isImg && src) {
                return (
                  `<div class="chat-msg-att">` +
                  `<img class="chat-img-zoomable" src="${src}" alt="${name}" data-img-src="${chatEsc(src)}" data-img-name="${name}" title="点击放大" />` +
                  `<span>${name}</span></div>`
                );
              }
              const clip =
                typeof window.ccoIcon === "function"
                  ? window.ccoIcon(isImg ? "paperclip" : "file", { size: 12 })
                  : "📄";
              return `<div class="chat-msg-att chat-msg-att-path" title="${chatEsc(a.path || "")}">${clip} ${name}</div>`;
            })
            .join("")}</div>`
        : "";
      const activePlan = role === "assistant";
      // Assistant numbered A/B/C → clickable quiz; else normal md
      let bodyInner;
      let usedQuiz = false;
      if (role === "assistant") {
        const enh = enhanceAssistantBody(content, idx, (t) =>
          chatFormatBody(t, { activePlan })
        );
        bodyInner = enh.html;
        usedQuiz = !!enh.usedQuiz;
      } else {
        bodyInner = chatFormatBody(content, { activePlan: false });
      }
      // Long non-quiz bodies: clamp with 展开全部
      if (shouldClampBody(content, idx, { usedQuiz })) {
        bodyInner = wrapClampedBody(bodyInner, idx, true);
      }
      // Older turns: whole bubble collapsed to one-line bar (self-expand)
      const forceOpen = usedQuiz && total - 1 - idx < 2;
      if (shouldFoldMessage(idx, total, content, { forceOpen })) {
        return (
          `<div class="chat-msg chat-msg-${role} is-folded" data-msg-idx="${idx}">` +
          renderFoldBarHtml(label, content, idx) +
          `</div>`
        );
      }
      // 「收起」：非最近两条，或用户曾手动展开过
      const foldKey = `m${idx}`;
      const showFoldAgain =
        total > 2 &&
        (total - 1 - idx >= 2 ||
          (state.chatMsgFold && state.chatMsgFold[foldKey] === false));
      const foldAgain = showFoldAgain ? renderFoldAgainBtn(idx) : "";
      return (
        `<div class="chat-msg chat-msg-${role}" data-msg-idx="${idx}">` +
        `<div class="chat-msg-role">${label}</div>` +
        foldAgain +
        `<div class="chat-msg-body md-body">${bodyInner}${attHtml}</div>` +
        `</div>`
      );
    })
    .join("");
  // Waiting bubble: user already sent; UI stays responsive while CLI runs.
  // Stream partials render as markdown (same path as final bubbles), not raw source.
  if (state.chatBusy) {
    const stream = String(state.chatStreamText || "").trim();
    // t3: when clarify uiStatus=loading, prefer product loading copy
    const clarifyLoading =
      state.chatClarify?.uiStatus === "loading"
        ? "正在整理你的想法…"
        : null;
    if (stream) {
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending chat-msg-streaming md-body">${chatFormatStreamBody(
        stream
      )}<span class="chat-stream-cursor" aria-hidden="true">▍</span></div>
    </div>`;
    } else {
      html += `<div class="chat-msg chat-msg-assistant chat-msg-pending" aria-live="polite">
      <div class="chat-msg-role">AI</div>
      <div class="chat-msg-body chat-msg-body-pending chat-msg-body-wait-only">
        <span class="chat-pending-dots" aria-hidden="true"></span>
        ${chatEsc(clarifyLoading || chatWaitLabel())}
      </div>
    </div>`;
    }
  }
  list.innerHTML = html;
  // t4: hollow yellow bar near plan card — warn only; never disables 仅保存/拆成步骤
  try {
    const draftMd = state.chatSession?.draft_plan?.markdown || "";
    const hollowHtml = renderHollowBarHtml(state.chatClarify, draftMd);
    if (hollowHtml) {
      const card = list.querySelector(".chat-plan-card");
      if (card) {
        // Insert after the first plan card so assign CTAs stay above / beside warn
        const wrap = document.createElement("div");
        wrap.innerHTML = hollowHtml;
        const bar = wrap.firstElementChild;
        if (bar) {
          // Prefer after card actions (still inside message flow)
          card.insertAdjacentElement("afterend", bar);
        }
      } else if (
        state.chatClarify?.phase === "claimed_to_plan" ||
        state.chatSession?.draft_plan
      ) {
        // No card yet but draft/claimed — append bar so warn still visible
        const wrap = document.createElement("div");
        wrap.innerHTML = hollowHtml;
        const bar = wrap.firstElementChild;
        if (bar) list.appendChild(bar);
      }
    }
  } catch (_) {}
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
        : "说清目标与约束，或拖入文件…";
  }
  if (sendBtn) {
    // Disabled while waiting = prevent double-send, NOT app freeze.
    // Backend chat_send runs on a worker thread so the rest of the UI stays live.
    // Icon-only send (arrow-up); never wipe SVG with textContent.
    sendBtn.disabled = !state.selectedPath || !!state.chatBusy;
    sendBtn.title = state.chatBusy
      ? "正在等待本机 Claude CLI 回复，请稍候"
      : "发送（Enter）";
    sendBtn.setAttribute(
      "aria-label",
      state.chatBusy ? "思考中…" : "发送"
    );
  }
  // Codex-style auto-grow textarea
  if (input && typeof input.scrollHeight === "number") {
    input.style.height = "auto";
    const max = 160; // ~10rem
    input.style.height = `${Math.min(input.scrollHeight, max)}px`;
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
