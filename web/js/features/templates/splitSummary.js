/**
 * [INPUT]: planJob 形状（tasks/layers）· 既有 markdown
 * [OUTPUT]: 拆分摘要块纯函数（不写盘、不开跑）
 * [POS]: P-ship-D features/templates/splitSummary.js（S14）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export const SPLIT_SUMMARY_START = "<!-- cco-split-summary:start -->";
export const SPLIT_SUMMARY_END = "<!-- cco-split-summary:end -->";

/** Build markdown block of step titles from a plan job. */
export function buildSplitSummaryBlock(job) {
  const tasks = job?.tasks || [];
  const layers = job?.layers || [];
  const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));
  const date = new Date().toISOString().slice(0, 10);
  const lines = [
    SPLIT_SUMMARY_START,
    "",
    "## 拆分步骤摘要",
    "",
    `> 由拆分台生成 · ${date} · 可选写回 · **不替代**上方正文`,
    "",
  ];
  if (!tasks.length) {
    lines.push("_（当前无步骤）_", "");
  } else if (layers.length) {
    layers.forEach((layer, i) => {
      lines.push(`### 波次 ${i + 1}`);
      lines.push("");
      (layer || []).forEach((id) => {
        const t = byId[id] || { id, title: id };
        const opt = t.optional ? " · 可选" : "";
        const sys =
          String(t.id || "").startsWith("sys-post-") ||
          String(t.group || "") === "系统收尾"
            ? " · 系统"
            : "";
        lines.push(`- [ ] ${t.title || id}${opt}${sys}`);
      });
      lines.push("");
    });
    const seen = new Set(layers.flat());
    const rest = tasks.filter((t) => !seen.has(t.id));
    if (rest.length) {
      lines.push("### 其他");
      lines.push("");
      rest.forEach((t) => lines.push(`- [ ] ${t.title || t.id}`));
      lines.push("");
    }
  } else {
    tasks.forEach((t) => {
      const opt = t.optional ? " · 可选" : "";
      lines.push(`- [ ] ${t.title || t.id}${opt}`);
    });
    lines.push("");
  }
  lines.push(SPLIT_SUMMARY_END);
  return lines.join("\n");
}

/**
 * Merge/replace cco-split-summary block; never clobber user prose outside markers.
 * @param {string} existing
 * @param {string} block
 */
export function mergeSplitSummaryIntoMarkdown(existing, block) {
  const body = String(existing || "").replace(/\s*$/, "");
  const re =
    /<!-- cco-split-summary:start -->[\s\S]*?<!-- cco-split-summary:end -->\n?/;
  if (re.test(body)) {
    return body.replace(re, block.trim() + "\n");
  }
  return body + "\n\n" + block.trim() + "\n";
}
