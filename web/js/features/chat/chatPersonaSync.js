/**
 * [INPUT]: gateway.getProjectPersona / setProjectPersona · chatPersona chips
 * [OUTPUT]: restorePersonaForProject / savePersonaForProject / chipGrainHintLine
 * [POS]: features/chat — P0-B 同项目 persona/芯片恢复（best-effort；无项目不调用不报错）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * IPC 只经 gateway；不开跑、不动 chatClarify。存储 SoT 在 Rust persona_store（project_pins）。
 */

import * as gateway from "../../shared/gateway.js";
import {
  getPersonaId,
  setPersonaId,
  getChipValue,
  setChipValue,
  PERSONA_PROFILES,
} from "./chatPersona.js";

const CHIP_CLARIFY = "clarify_depth";
const CHIP_GRAIN = "split_grain";

/**
 * Map chip split_grain → one grain line for the split user prompt.
 * Only used as default when user has no explicit work-style grain (jobPoll).
 * @param {string|null|undefined} grain fine|balanced|coarse
 * @returns {string} empty = omit / keep existing default
 */
export function chipGrainHintLine(grain) {
  switch (String(grain || "").trim()) {
    case "coarse":
      return "偏粗：合并可同批的小改动，步骤宜少而清";
    case "fine":
      return "偏细：按文件/模块拆开，scope_paths 尽量具体";
    default:
      // balanced / unknown → 不注入，保留现有默认
      return "";
  }
}

/** Still on the same project? (guards stale async apply after switch) */
function stillSelected(project) {
  try {
    const s = typeof window !== "undefined" ? window.state : null;
    if (s && "selectedPath" in s) return s.selectedPath === project;
  } catch (_) {}
  return true;
}

/**
 * Restore persona + chips for a project (best-effort).
 * No project → no call, no error. Stored chips win over persona defaults.
 * @param {string|null|undefined} project
 */
export async function restorePersonaForProject(project) {
  const p = String(project || "").trim();
  if (!p) return;
  let stored = null;
  try {
    stored = await gateway.getProjectPersona(p);
  } catch (_) {
    return; // storage unavailable → keep current in-memory values
  }
  if (!stored || !stillSelected(project)) return;
  try {
    if (stored.persona_id && PERSONA_PROFILES[stored.persona_id]) {
      // setPersonaId also seeds chip defaults from the profile…
      setPersonaId(stored.persona_id);
    }
    // …then explicit stored chips override those defaults.
    if (stored.clarify_depth) setChipValue(CHIP_CLARIFY, stored.clarify_depth);
    if (stored.split_grain) setChipValue(CHIP_GRAIN, stored.split_grain);
  } catch (_) {}
}

/**
 * Save current persona + chips for a project (best-effort, fire-and-forget).
 * No project → no call, no error.
 * @param {string|null|undefined} project
 */
export function savePersonaForProject(project) {
  const p = String(project || "").trim();
  if (!p) return;
  try {
    gateway
      .setProjectPersona(p, {
        // Tauri 2 命令参数走 camelCase（与 jobId/runId 惯例一致）
        personaId: getPersonaId() || null,
        clarifyDepth: getChipValue(CHIP_CLARIFY) || null,
        splitGrain: getChipValue(CHIP_GRAIN) || null,
      })
      .catch((err) => {
        console.warn("[P0-B] set_project_persona failed", err);
      });
  } catch (_) {}
}
