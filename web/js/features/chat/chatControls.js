/**
 * [INPUT]: legacy.state · composer DOM · #chat-cli
 * [OUTPUT]: 本轮上下文 DisclosureRow（右侧只读**模型名**徽标 · 仅 /model 斜杠命令切换）
 * [POS]: features/chat 的 composer 控件层；只维护展示，不写会话/规划策略
 * note: 无会话覆盖时也显示通道有效模型名（禁止裸「默认」字样）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { state } from "./legacy.js";

function $(id) {
  return document.getElementById(id);
}

/** Claude 通道在未 `/model` 时的产品展示名（CLI 未回显真实默认时的可读名）。 */
const CLAUDE_EFFECTIVE_DEFAULT = "claude-sonnet-5";

/**
 * 模型 id / 别名 → 徽标短名（人话、可扫读）。
 * 例：sonnet → Sonnet · claude-sonnet-5 → Claude Sonnet 5 · opus → Opus
 */
export function prettyChatModelLabel(raw) {
  const m = String(raw || "").trim();
  if (!m) return "";
  const lower = m.toLowerCase();
  const alias = {
    sonnet: "Sonnet",
    opus: "Opus",
    haiku: "Haiku",
    fable: "Fable",
  };
  if (alias[lower]) return alias[lower];
  // claude-sonnet-5 / claude-opus-5-2025… → Claude Sonnet 5
  const mClaude = lower.match(
    /^claude[-_]?([a-z]+)(?:[-_]?(\d+(?:\.\d+)?))?(?:[-_].*)?$/i
  );
  if (mClaude) {
    const family = mClaude[1].charAt(0).toUpperCase() + mClaude[1].slice(1);
    const ver = mClaude[2] ? ` ${mClaude[2]}` : "";
    return `Claude ${family}${ver}`.trim();
  }
  // 其它保持原样，过长截断由 CSS ellipsis
  return m;
}

/** 当前 composer / 会话选用的 CLI 名。 */
function currentChatCli() {
  const sel = $("chat-cli");
  const fromUi = String(sel?.value || "").trim();
  if (fromUi) return fromUi.toLowerCase();
  const fromSess = String(state.chatSession?.cli || "").trim();
  if (fromSess) return fromSess.toLowerCase();
  try {
    const ls = localStorage.getItem("cco.chatCli");
    if (ls) return String(ls).trim().toLowerCase();
  } catch (_) {}
  return "claude";
}

/**
 * 有效模型展示：会话覆盖优先；否则按通道给可读模型名（绝不显示裸「默认」）。
 * @returns {{ label: string, raw: string, isOverride: boolean }}
 */
export function resolveChatModelDisplay() {
  const override = String(state.chatSession?.model || "").trim();
  if (override) {
    return {
      label: prettyChatModelLabel(override) || override,
      raw: override,
      isOverride: true,
    };
  }
  const cli = currentChatCli();
  if (!cli || cli === "claude") {
    return {
      label: prettyChatModelLabel(CLAUDE_EFFECTIVE_DEFAULT),
      raw: CLAUDE_EFFECTIVE_DEFAULT,
      isOverride: false,
    };
  }
  // 非 Claude 通道：模型参数常被忽略，徽标显示通道名本身
  const cliLabel =
    cli === "codex"
      ? "Codex"
      : cli === "fake"
        ? "模拟"
        : cli === "sdk"
          ? "SDK"
          : cli.charAt(0).toUpperCase() + cli.slice(1);
  return { label: cliLabel, raw: cli, isOverride: false };
}

/**
 * 会话模型只读徽标：不可点击、不可编辑；始终显示模型名（非「默认」）。
 * 切换唯一入口 = 聊天输入框的 /model <名称> 斜杠命令。
 */
function paintChatModelBadge() {
  const badge = $("chat-context-model");
  if (!badge) return;
  const { label, raw, isOverride } = resolveChatModelDisplay();
  badge.textContent = label || raw || "—";
  badge.title = isOverride
    ? `当前模型：${raw} · 在输入框输入 /model <名称> 切换`
    : `通道有效模型：${raw}（未单独覆盖）· 在输入框输入 /model <名称> 切换`;
}

/** `/model` 切换后由 resp.model 回流：更新会话快照与徽标（无选择器）。 */
export function syncChatModelFromResponse(model) {
  const value = String(model || "").trim();
  if (value) {
    if (!state.chatSession || typeof state.chatSession !== "object") {
      state.chatSession = {
        session_id: "default",
        messages: [],
        draft_plan: null,
      };
    }
    state.chatSession.model = value;
  }
  paintChatModelBadge();
}

export function renderChatComposerContext() {
  const summaryText = $("chat-context-summary-text");
  const body = $("chat-context-body");
  if (!summaryText || !body) return;
  const project =
    String(state.selectedPath || "").split(/[/\\]/).filter(Boolean).pop() ||
    "未选择项目";
  const plan = state.chatDraftPlan || state.selectedPlan || "";
  const planName = plan
    ? String(plan).split(/[/\\]/).pop()
    : "未绑定计划";
  const attachmentCount = Array.isArray(state.chatPendingAttachments)
    ? state.chatPendingAttachments.length
    : 0;
  summaryText.textContent = attachmentCount
    ? "本轮上下文 · 附件 " + attachmentCount
    : "本轮上下文";
  body.textContent =
    "项目：" +
    project +
    " · 计划：" +
    planName +
    (attachmentCount ? " · 附件：" + attachmentCount : "");
  paintChatModelBadge();
}

export function installChatControls() {
  const badge = $("chat-context-model");
  if (badge && !badge.dataset.ccoModelGuard) {
    badge.dataset.ccoModelGuard = "1";
    // 徽标不可点击：吞掉点击，避免触发 summary 的展开/收起
    badge.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
    });
  }
  // CLI 切换会改变「通道有效模型」展示名
  const cliSel = $("chat-cli");
  if (cliSel && !cliSel.dataset.ccoModelBadgeWired) {
    cliSel.dataset.ccoModelBadgeWired = "1";
    cliSel.addEventListener("change", () => paintChatModelBadge());
  }
  renderChatComposerContext();
}
