/**
 * [INPUT]: live.inspect_loop DTO（app/handoff 已算字段）
 * [OUTPUT]: 人话文案片段；主路径第一句无裸 VERDICT
 * [POS]: A4-4 · P0-4 features/result；用语与 report `## 对照计划`（fallback.rs）同构
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 叙事锁（P0-4）：结论句与 src/report/fallback.rs headline 保持同词，
 * 避免 UI 与 report 同轮写不同「对照计划」结论句。
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
 * Canonical plan-compare phrases — mirror report PlanCompareSection headlines.
 * Keep in sync with `src/report/fallback.rs` fill_plan_compare.
 */
export const PLAN_COMPARE_COPY = Object.freeze({
  pass: "巡检对照计划：通过",
  fail: "巡检对照计划：有遗漏需处理",
  pending: "本轮未产出巡检结论",
  disabledHeadline: "未开启对照计划巡检",
  /** Honest footer when inspect was never enabled (report Disabled body). */
  disabledHonest:
    "本轮未开启对照计划巡检：步骤跑完 ≠ 已按计划验收。可在设置里打开「拆分后附加：任务巡检」，或回聊天补充后再拆。",
  /** Honest footer when require_inspect but no usable verdict (report Pending body). */
  pendingHonest:
    "本轮未产出巡检结论：已要求对照计划巡检，但本轮尚无可用的通过/未通过结论。步骤跑完 ≠ 已按计划验收。",
  /** Unclear / partial inspect noise without PASS|FAIL. */
  unclearHonest: "本轮未产出巡检结论：巡检结果不完整，不能当作已对照计划验收。",
});

/**
 * Classify inspect_loop into the same kind family as report PlanCompareKind.
 * @param {InspectLoopDto|null|undefined} loop
 * @returns {"pass"|"fail"|"pending"|"disabled"|"unclear"|"empty"}
 */
export function planCompareKind(loop) {
  if (!loop) return "empty";
  const v = String(loop.verdict || "").toUpperCase();
  const blocking = Number(loop.blocking_count) || 0;
  const residual = Number(loop.residual_count) || 0;
  const hasPreview = !!(loop.issue_preview && loop.issue_preview.length);
  const hasAny =
    !!loop.verdict ||
    blocking > 0 ||
    residual > 0 ||
    !!loop.require_inspect ||
    !!loop.can_rework ||
    hasPreview ||
    !!loop.accepted_residual;

  if (v === "PASS" && blocking === 0) return "pass";
  if (v === "FAIL" || blocking > 0) return "fail";
  if (!hasAny) return "empty";
  if (loop.require_inspect && !v) return "pending";
  if (!v && blocking === 0 && !hasPreview && !loop.require_inspect) {
    // can_rework / residual-only noise without a real product → treat as unclear
    if (residual > 0 || loop.can_rework || loop.accepted_residual) return "unclear";
    return "disabled";
  }
  if (loop.require_inspect) return "pending";
  if (!v) return "unclear";
  return "unclear";
}

/**
 * Human strip bits from inspect_loop DTO (no raw VERDICT as first word).
 * Lead bit matches report `## 对照计划` headline when a conclusion exists.
 * @param {InspectLoopDto|null|undefined} loop
 * @returns {{ bits: string[], kind: "ok"|"bad"|"neutral"|"empty", preview: string }}
 */
export function inspectStripParts(loop) {
  const cmp = planCompareKind(loop);
  if (cmp === "empty" || cmp === "disabled") {
    // Disabled is carried by honest footer, not the strip (same as empty strip + honest).
    return { bits: [], kind: "empty", preview: "" };
  }

  const bits = [];
  const v = String(loop?.verdict || "").toUpperCase();
  if (cmp === "pass") {
    bits.push(PLAN_COMPARE_COPY.pass);
  } else if (cmp === "fail") {
    bits.push(PLAN_COMPARE_COPY.fail);
  } else {
    // pending | unclear — same headline as report fallback
    bits.push(PLAN_COMPARE_COPY.pending);
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
  if (cmp === "fail" || loop.blocking_count > 0) kind = "bad";
  else if (cmp === "pass") kind = "ok";

  return { bits, kind, preview };
}

/**
 * Honest footer copy for result desk (S7 / P0-4).
 * Always visible when we can state a plan-compare conclusion or its absence;
 * wording stays non-contradictory with report.md of the same round.
 * @param {InspectLoopDto|null|undefined} loop
 * @returns {{ text: string, hidden: boolean }}
 */
export function honestInspectCopy(loop) {
  const cmp = planCompareKind(loop);

  if (cmp === "empty" || cmp === "disabled") {
    return {
      hidden: false,
      text: PLAN_COMPARE_COPY.disabledHonest,
    };
  }

  if (cmp === "pass") {
    const residual = Number(loop?.residual_count) || 0;
    return {
      hidden: false,
      text:
        residual > 0
          ? `${PLAN_COMPARE_COPY.pass}，仍有 ${residual} 条非阻塞残留（可接受遗漏或回补）。`
          : `${PLAN_COMPARE_COPY.pass}。`,
    };
  }

  if (cmp === "fail") {
    const blocking = Number(loop?.blocking_count) || 0;
    const extra =
      blocking > 0
        ? `需优先处理 ${blocking} 项阻塞/地图遗漏，可用「回补并再巡检」。`
        : `可用「回补并再巡检」。`;
    return {
      hidden: false,
      text: `${PLAN_COMPARE_COPY.fail}。${extra}`,
    };
  }

  if (cmp === "pending") {
    return {
      hidden: false,
      text: PLAN_COMPARE_COPY.pendingHonest,
    };
  }

  // unclear
  return {
    hidden: false,
    text: PLAN_COMPARE_COPY.unclearHonest,
  };
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
