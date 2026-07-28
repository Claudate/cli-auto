/**
 * [INPUT]: plan list items { path, title, … }
 * [OUTPUT]: wave groups for 计划管理 list (W2-5)
 * [POS]: features/chat — pure group/render helpers; no IPC / no confirm
 * [PROTOCOL]: 变更时对照 docs/path-depth-wave-2026-07-28/04 · landing W2-5
 */

/**
 * @param {string} path
 * @returns {string|null} e.g. plans/wave-20260728-1200
 */
export function waveDirKeyFromPath(path) {
  const parts = String(path || "")
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean);
  const i = parts.findIndex((p) => /^wave-/i.test(p));
  if (i < 0) return null;
  return parts.slice(0, i + 1).join("/");
}

/** @param {string} path */
export function isWaveIndexPath(path) {
  const base = String(path || "")
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .pop();
  return /^INDEX\.md$/i.test(base || "");
}

/**
 * @param {Array<{path?: string, title?: string}>} items
 * @returns {{ waves: Array<{ key: string, label: string, index: object|null, plans: object[] }>, flat: object[] }}
 */
export function groupPlanItemsByWave(items) {
  const list = Array.isArray(items) ? items : [];
  /** @type {Map<string, { key: string, label: string, index: object|null, plans: object[] }>} */
  const map = new Map();
  const flat = [];
  for (const it of list) {
    const p =
      typeof it === "string"
        ? it
        : it && typeof it === "object"
          ? String(it.path || "")
          : "";
    if (!p) continue;
    const row = typeof it === "string" ? { path: it } : it;
    const key = waveDirKeyFromPath(p);
    if (!key) {
      flat.push(row);
      continue;
    }
    if (!map.has(key)) {
      const label = key.split("/").filter(Boolean).pop() || key;
      map.set(key, { key, label, index: null, plans: [] });
    }
    const g = map.get(key);
    if (isWaveIndexPath(p)) g.index = row;
    else g.plans.push(row);
  }
  // Newest wave stamp first (wave-YYYYMMDD-HHMM sorts lexically desc)
  const waves = [...map.values()].sort((a, b) =>
    String(b.label).localeCompare(String(a.label))
  );
  return { waves, flat };
}

/**
 * Build list HTML fragment for one wave group.
 * @param {object} g group from groupPlanItemsByWave
 * @param {(it: object) => string} rowHtml item → button HTML
 * @param {(s: string) => string} esc
 */
export function renderWaveGroupHtml(g, rowHtml, esc) {
  const n = (g.plans?.length || 0) + (g.index ? 1 : 0);
  const rows = [];
  if (g.index) rows.push(rowHtml(g.index));
  for (const it of g.plans || []) rows.push(rowHtml(it));
  return (
    `<div class="plans-wave-group" data-wave-key="${esc(g.key)}">` +
    `<div class="plans-wave-head" title="${esc(g.key)}">` +
    `<span class="plans-wave-title">本波 · ${esc(g.label)}</span>` +
    `<span class="plans-wave-count muted">${n} 份</span>` +
    `</div>` +
    `<div class="plans-wave-items">${rows.join("")}</div>` +
    `</div>`
  );
}

/**
 * Sibling execution plans under the same wave (exclude INDEX).
 * @param {string} path
 * @param {Array} allItems
 */
export function waveSiblingPlans(path, allItems) {
  const key = waveDirKeyFromPath(path);
  if (!key) return [];
  return (allItems || []).filter((it) => {
    const p = it?.path || "";
    return waveDirKeyFromPath(p) === key && !isWaveIndexPath(p);
  });
}
