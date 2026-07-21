/**
 * [INPUT]: live.inspect_loop DTO（app/handoff 已算字段）
 * [OUTPUT]: 人话文案片段；主路径第一句无裸 VERDICT
 * [POS]: A4-4 features/result；禁止 UI 再解析 VERDICT 正文
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/**
 * @typedef {{
 *   verdict?: string|null,
 *   blocking_count?: number,
 *   residual_count?: number,
 *   issue_preview?: string[],
 *   can_rework?: boolean,
 *   require_inspect?: boolean,
 *   rework_round?: number,
 *   rework_max?: number,
 *   accepted_residual?: boolean,
 * }} InspectLoopDto
 */

/**
 * Human strip bits from inspect_loop DTO (no raw VERDICT as first word).
 * @param {InspectLoopDto|null|undefined} loop
 * @returns {{ bits: string[], kind: "ok"|"bad"|"neutral"|"empty", preview: string }}
 */
export function inspectStripParts(loop) {
  if (
    !loop ||
    (!loop.verdict &&
      !loop.blocking_count &&
      !loop.require_inspect &&
      !loop.can_rework &&
      !(loop.issue_preview && loop.issue_preview.length) &&
      !loop.accepted_residual)
  ) {
    return { bits: [], kind: "empty", preview: "" };
  }

  const bits = [];
  const v = String(loop.verdict || "").toUpperCase();
  if (v === "PASS") {
    bits.push("巡检对照计划：通过");
  } else if (v === "FAIL") {
    bits.push("巡检对照计划：有遗漏需处理");
  } else if (loop.require_inspect) {
    bits.push("巡检报告待产出");
  } else if (loop.verdict) {
    // Non-canonical token: still humanize, never lead with bare engine jargon
    bits.push(`巡检结果已出（${loop.verdict}）`);
  }
  if (loop.blocking_count > 0) {
    bits.push(`需优先处理 ${loop.blocking_count} 项`);
  }
  if (loop.residual_count > 0) {
    bits.push(`残留 ${loop.residual_count}`);
  }
  if (loop.rework_round > 0) {
    bits.push(`回补第 ${loop.rework_round}/${loop.rework_max || 2} 轮`);
  }
  if (loop.accepted_residual) {
    bits.push("已接受遗漏");
  }
  const preview = (loop.issue_preview || []).slice(0, 2).join(" · ");
  if (preview) bits.push(preview);

  let kind = "neutral";
  if (v === "FAIL" || loop.blocking_count > 0) kind = "bad";
  else if (v === "PASS" && !(loop.blocking_count > 0)) kind = "ok";

  return { bits, kind, preview };
}

/**
 * Honest footer copy for result desk (S7).
 * @param {InspectLoopDto|null|undefined} loop
 * @returns {{ text: string, hidden: boolean }}
 */
export function honestInspectCopy(loop) {
  const hasInspect = !!(
    loop &&
    (loop.verdict ||
      loop.require_inspect ||
      loop.blocking_count > 0 ||
      loop.residual_count > 0 ||
      (loop.issue_preview && loop.issue_preview.length))
  );

  if (!hasInspect) {
    return {
      hidden: false,
      text:
        "本轮未开启对照计划巡检：步骤跑完 ≠ 已按计划验收。可在设置里打开「拆分后附加：任务巡检」，或回聊天补充后再拆。",
    };
  }
  const v = String(loop.verdict || "").toUpperCase();
  if (v === "PASS" && !(loop.blocking_count > 0)) {
    return {
      hidden: false,
      text:
        loop.residual_count > 0
          ? `巡检通过，仍有 ${loop.residual_count} 条非阻塞残留（可接受遗漏或回补）。`
          : "巡检对照计划通过。",
    };
  }
  if (v === "FAIL" || loop.blocking_count > 0) {
    return {
      hidden: false,
      text: `巡检发现需处理项${
        loop.blocking_count > 0 ? `（阻塞 ${loop.blocking_count}）` : ""
      }，可用「回补并再巡检」。`,
    };
  }
  if (loop.require_inspect) {
    return {
      hidden: false,
      text: "已要求巡检，报告尚未产出或仍在处理。",
    };
  }
  return { hidden: true, text: "" };
}

/**
 * Whether rework / accept residual buttons should show (finished run only).
 * Rules come from DTO flags — UI does not invent can_rework.
 * @param {InspectLoopDto|null|undefined} loop
 * @param {{ finished?: boolean, active?: boolean }} ctx
 */
export function inspectActionVisibility(loop, ctx) {
  const showActions = !!ctx.finished && !ctx.active;
  const canRework = !!(showActions && loop && loop.can_rework);
  const showAccept =
    showActions &&
    loop &&
    !loop.accepted_residual &&
    (loop.blocking_count > 0 ||
      String(loop.verdict || "").toUpperCase() === "FAIL" ||
      (loop.residual_count > 0 &&
        String(loop.verdict || "").toUpperCase() === "PASS"));
  return { canRework, showAccept, showActions };
}
