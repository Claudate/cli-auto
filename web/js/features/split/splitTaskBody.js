/**
 * [INPUT]: task prompt / body string
 * [OUTPUT]: markdown for desk「本步说明」
 * [POS]: features/split pure helper
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/**
 * Turn worker/task body into readable plan detail under 怎样算做完.
 * Prefers 【做什么】/【改哪里】/… blocks; drops CCO_DONE / worker scaffold lines.
 */
export function formatTaskDetailBody(raw) {
  let t = String(raw || "").trim();
  if (!t) return "";
  t = t
    .replace(/\n*完成后输出一行:\s*CCO_DONE[^\n]*/gi, "")
    .replace(/\n*CCO_DONE[^\n]*/gi, "")
    .trim();
  const re =
    /【\s*(做什么|改哪里|怎样算做完|先等谁|不要做什么|自测|范围|要点)\s*】/g;
  const hits = [];
  let m;
  while ((m = re.exec(t)) !== null) {
    hits.push({ label: m[1], index: m.index, end: m.index + m[0].length });
  }
  if (!hits.length) return t;
  const labels = {
    做什么: "做什么",
    改哪里: "改哪里",
    怎样算做完: "怎样算做完",
    先等谁: "先等谁",
    不要做什么: "不要做什么",
    自测: "自测",
    范围: "范围",
    要点: "要点",
  };
  const chunks = [];
  const pre = t.slice(0, hits[0].index).trim();
  if (pre && pre.length > 8) chunks.push(pre);
  for (let i = 0; i < hits.length; i++) {
    const h = hits[i];
    const next = hits[i + 1];
    const body = t.slice(h.end, next ? next.index : t.length).trim();
    if (body) chunks.push(`**${labels[h.label] || h.label}**  \n${body}`);
  }
  return chunks.join("\n\n");
}
