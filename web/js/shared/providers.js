/**
 * Known worker providers (desktop UI). Keep ids in sync with Rust ProviderId / shell_print.
 * Product labels for PM path; engine names not first sentence on main path.
 */

export const KNOWN_PROVIDERS = [
  { id: "claude", label: "Claude", product: "Claude" },
  { id: "codex", label: "Codex", product: "Codex" },
  { id: "gemini", label: "Gemini", product: "Gemini" },
  { id: "qwen", label: "通义 Qwen", product: "通义 Qwen" },
  { id: "kimi", label: "Kimi", product: "Kimi" },
  { id: "deepseek", label: "CodeWhale", product: "CodeWhale" },
  { id: "copilot", label: "Copilot", product: "Copilot" },
  { id: "codebuddy", label: "CodeBuddy", product: "CodeBuddy" },
];

/**
 * @param {string} provider
 * @returns {string}
 */
export function engineLabel(provider) {
  const p = String(provider || "").trim().toLowerCase();
  if (!p) return "";
  const hit = KNOWN_PROVIDERS.find((x) => x.id === p);
  if (hit) return hit.label;
  if (p === "fake" || p === "mock") return "演练";
  if (p === "sdk") return "SDK";
  return provider;
}

/**
 * @param {string} provider
 * @returns {string}
 */
export function productLabel(provider) {
  const p = String(provider || "").trim().toLowerCase();
  const hit = KNOWN_PROVIDERS.find((x) => x.id === p);
  if (hit) return hit.product;
  return engineLabel(provider);
}

/**
 * Fill a <select> with known providers. Preserves current value when possible.
 * @param {HTMLSelectElement|null} el
 * @param {{ selected?: string }} [opts]
 */
export function fillProviderSelect(el, opts = {}) {
  if (!el) return;
  const selected = String(opts.selected || el.value || "claude").toLowerCase();
  const optsHtml = KNOWN_PROVIDERS.map(
    (p) => `<option value="${p.id}">${p.label}</option>`
  ).join("");
  el.innerHTML = optsHtml;
  const allowed = new Set(KNOWN_PROVIDERS.map((p) => p.id));
  el.value = allowed.has(selected) ? selected : "claude";
}

/**
 * Options HTML fragment for static embeds.
 */
export function providerOptionsHtml() {
  return KNOWN_PROVIDERS.map(
    (p) => `<option value="${p.id}">${p.label}</option>`
  ).join("");
}
