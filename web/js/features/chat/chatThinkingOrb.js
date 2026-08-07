/**
 * [INPUT]: chatSessions.paintChatPendingBubble（委托）· chatRender 等待气泡 DOM
 * [OUTPUT]: 等待气泡 = 思考 orb（canvas 2D 九状态点阵球 · shared/thinkingOrb 驱动）+ 等待文案
 *           （按场景映射 orb 状态/尺寸：澄清/整合 → composing；普通思考 → weaving）
 * [POS]: A5-2a features/chat/chatThinkingOrb.js（A5-2a 纵切；渲染引擎在 shared/thinkingOrb）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import { startThinkingOrb, stopThinkingOrb } from "../../shared/thinkingOrb.js";

const ORB_WAIT_SIZE = 32; // 普通思考 orb 边长（与 shared/thinkingOrb.js ORB_SIZE 同步）

/**
 * 等待态：确保 canvas orb 存在并按状态运行，更新等待文案。不做 innerHTML 整段重写。
 * @param {string} [state="weaving"]  orb 状态（参考站 nine states：weaving/composing/…）
 * @param {number} [size]             orb 边长 px（缺省 32；澄清态可调大一点）
 * 同一 canvas 状态/尺寸变化时会先 stop 再 start（rAF 防泄漏 + 场景切换真正生效）。
 */
export function paintPendingWait(body, label, state = "weaving", size = ORB_WAIT_SIZE) {
  let orb = body.querySelector(".chat-orb");
  if (!orb) {
    orb = document.createElement("span");
    orb.className = "chat-orb";
    orb.appendChild(createOrbCanvas());
    body.appendChild(orb);
  }
  let canvas = orb.querySelector("canvas.thinking-orb");
  if (!canvas) {
    canvas = createOrbCanvas();
    orb.insertBefore(canvas, orb.firstChild);
  }
  const want = `${state}|${size}`;
  if (canvas.dataset.ccoOrb === "1" && canvas.dataset.ccoOrbState !== want) {
    stopThinkingOrb(canvas); // 清 rAF + dataset.ccoOrb，避免双跑/状态不生效
  }
  if (canvas.dataset.ccoOrb !== "1") {
    startThinkingOrb(canvas, { state, size });
    canvas.dataset.ccoOrbState = want;
  }
  let labelEl = body.querySelector(".chat-wait-label");
  if (!labelEl) {
    labelEl = document.createElement("span");
    labelEl.className = "chat-wait-label";
    body.appendChild(labelEl);
  }
  labelEl.textContent = label;
}

/** 停止等待气泡内的 orb（幂等；流式态/重绘前调用，停掉 rAF 防泄漏）。 */
export function stopPendingOrb(body) {
  if (!body) return;
  const canvas = body.querySelector("canvas.thinking-orb");
  if (canvas) stopThinkingOrb(canvas);
}

function createOrbCanvas() {
  const canvas = document.createElement("canvas");
  canvas.className = "thinking-orb";
  canvas.setAttribute("aria-hidden", "true");
  return canvas;
}
