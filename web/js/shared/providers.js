/**
 * Known worker providers + **global channel catalog** helpers.
 * Catalog SoT = settings.channels (Rust config.providers); health overlay = doctorCache.
 * Keep ids in sync with Rust ProviderId / shell_print.
 * Product labels for PM path; engine names not first sentence on main path.
 *
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
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

const NON_SWITCH_PROVIDERS = new Set([
  "fake",
  "mock",
  "sdk",
  "claude-sdk",
  "claude_sdk",
]);

/**
 * @param {string} provider
 * @returns {string}
 */
export function engineLabel(provider) {
  const p = String(provider || "")
    .trim()
    .toLowerCase();
  if (!p) return "";
  const hit = KNOWN_PROVIDERS.find((x) => x.id === p);
  if (hit) return hit.label;
  if (p === "fake" || p === "mock") return "演练";
  if (p === "sdk" || p === "claude-sdk" || p === "claude_sdk") return "SDK";
  return provider;
}

/**
 * @param {string} provider
 * @returns {string}
 */
export function productLabel(provider) {
  const p = String(provider || "")
    .trim()
    .toLowerCase();
  const hit = KNOWN_PROVIDERS.find((x) => x.id === p);
  if (hit) return hit.product;
  return engineLabel(provider);
}

/**
 * Parse doctorCache lines into per-channel health.
 * Line names: `provider:{id}:binary` | `provider:{id}:auth`.
 * @param {{ lines?: Array<{ name?: string, ok?: boolean, detail?: string }> } | null | undefined} doctorCache
 * @returns {Record<string, {
 *   binaryOk: boolean|null,
 *   authOk: boolean|null,
 *   binaryDetail: string,
 *   authDetail: string,
 *   fundsIssue: boolean,
 *   authInvalid: boolean,
 *   rateLimited: boolean,
 *   hint: string
 * }>}
 */
export function channelHealthMap(doctorCache) {
  /** @type {Record<string, any>} */
  const map = {};
  const lines = Array.isArray(doctorCache?.lines) ? doctorCache.lines : [];
  for (const line of lines) {
    const name = String(line?.name || "");
    const m = name.match(/^provider:([^:]+):(binary|auth)$/i);
    if (!m) continue;
    const id = String(m[1] || "")
      .trim()
      .toLowerCase();
    if (!id) continue;
    const kind = String(m[2] || "").toLowerCase();
    const slot =
      map[id] ||
      (map[id] = {
        binaryOk: null,
        authOk: null,
        binaryDetail: "",
        authDetail: "",
        fundsIssue: false,
        authInvalid: false,
        rateLimited: false,
        hint: "",
      });
    const detail = String(line?.detail || "").trim();
    const ok = line?.ok !== false;
    if (kind === "binary") {
      slot.binaryOk = ok;
      slot.binaryDetail = detail;
    } else {
      slot.authOk = ok;
      slot.authDetail = detail;
      if (/余额不足|insufficient_funds|quota|payment|402/i.test(detail)) {
        slot.fundsIssue = true;
      }
      if (/Key 失效|auth_invalid|401|未授权/i.test(detail)) {
        slot.authInvalid = true;
      }
      if (/限流|rate_limited|429/i.test(detail)) {
        slot.rateLimited = true;
      }
      if (!ok && detail) slot.hint = detail.split("·")[0].trim();
    }
  }
  return map;
}

/**
 * Short health suffix for pickers (empty when healthy / unknown).
 * @param {any} health
 * @returns {string}
 */
export function channelHealthHint(health) {
  if (!health) return "";
  if (health.binaryOk === false) {
    return health.binaryDetail
      ? `未安装（${String(health.binaryDetail).slice(0, 40)}）`
      : "未安装";
  }
  if (health.fundsIssue) return "余额不足";
  if (health.authInvalid) return "认证异常";
  if (health.rateLimited) return "限流中";
  if (health.authOk === false && health.hint) return health.hint.slice(0, 48);
  return "";
}

/**
 * Global channel catalog for UI.
 * Prefer `settings.channels` (Rust config.providers SoT). Overlay doctor health.
 * Fallback: KNOWN_PROVIDERS when settings not loaded yet — never invent empty.
 *
 * @param {any} st app state with settings / doctorCache
 * @returns {Array<{
 *   id: string,
 *   label: string,
 *   enabled: boolean,
 *   bin: string,
 *   switchable: boolean,
 *   health: any,
 *   healthHint: string
 * }>}
 */
export function listChannels(st) {
  const settings = st?.settings || {};
  const healthMap = channelHealthMap(st?.doctorCache);
  const raw = Array.isArray(settings.channels) ? settings.channels : null;

  /** @type {Array<{id:string,label:string,enabled:boolean,bin:string,switchable:boolean}>} */
  let base;
  if (raw && raw.length) {
    base = raw.map((c) => {
      const id = String(c?.id || c?.name || "")
        .trim()
        .toLowerCase();
      return {
        id,
        label: String(c?.label || productLabel(id) || id),
        enabled: c?.enabled !== false,
        bin: String(c?.bin || ""),
        switchable:
          c?.switchable !== false && !NON_SWITCH_PROVIDERS.has(id),
      };
    });
  } else {
    base = KNOWN_PROVIDERS.map((p) => ({
      id: p.id,
      label: p.product || p.label,
      enabled: true,
      bin: "",
      switchable: true,
    }));
  }

  const seen = new Set();
  const out = [];
  for (const c of base) {
    if (!c.id || seen.has(c.id)) continue;
    seen.add(c.id);
    const health = healthMap[c.id] || null;
    out.push({
      ...c,
      health,
      healthHint: channelHealthHint(health),
    });
  }
  return out;
}

/**
 * Switch-channel candidates: enabled + switchable, exclude current.
 * Prefer binary-ok first when doctor has spoken; still list binary-fail so UI
 * never says「没有其他可用通道」while catalog has Claude/Codex/etc.
 *
 * @param {any} st
 * @param {string} [current]
 * @returns {Array<{ id: string, label: string, healthHint: string }>}
 */
export function listSwitchableChannels(st, current) {
  const cur = String(current || "")
    .trim()
    .toLowerCase();
  const all = listChannels(st).filter(
    (c) =>
      c.enabled &&
      c.switchable &&
      !NON_SWITCH_PROVIDERS.has(c.id) &&
      c.id !== cur
  );
  // Prefer healthy binary when known; keep others after so list is never empty spuriously.
  const rank = (c) => {
    if (c.health?.binaryOk === false) return 2;
    if (c.health?.fundsIssue || c.health?.authInvalid) return 1;
    return 0;
  };
  return all
    .slice()
    .sort((a, b) => rank(a) - rank(b) || a.id.localeCompare(b.id))
    .map((c) => ({
      id: c.id,
      label: c.label || productLabel(c.id),
      healthHint: c.healthHint || "",
    }));
}

/**
 * Fill a <select> with catalog channels. Prefer opts.channels / opts.state settings.
 * @param {HTMLSelectElement|null} el
 * @param {{ selected?: string, channels?: Array<{id:string,label?:string,enabled?:boolean,switchable?:boolean}>, state?: any, includeDisabled?: boolean }} [opts]
 */
export function fillProviderSelect(el, opts = {}) {
  if (!el) return;
  const selected = String(opts.selected || el.value || "claude").toLowerCase();
  let list;
  if (Array.isArray(opts.channels) && opts.channels.length) {
    list = opts.channels;
  } else if (opts.state) {
    list = listChannels(opts.state);
  } else {
    list = KNOWN_PROVIDERS.map((p) => ({
      id: p.id,
      label: p.label,
      enabled: true,
      switchable: true,
    }));
  }
  const includeDisabled = !!opts.includeDisabled;
  const rows = list.filter(
    (c) =>
      c &&
      c.id &&
      !NON_SWITCH_PROVIDERS.has(String(c.id).toLowerCase()) &&
      (includeDisabled || c.enabled !== false)
  );
  const optsHtml = rows
    .map((p) => {
      const id = String(p.id).toLowerCase();
      const label = p.label || productLabel(id) || id;
      return `<option value="${id}">${label}</option>`;
    })
    .join("");
  el.innerHTML = optsHtml || `<option value="claude">Claude</option>`;
  const allowed = new Set(
    Array.from(el.options).map((o) => String(o.value).toLowerCase())
  );
  el.value = allowed.has(selected) ? selected : el.options[0]?.value || "claude";
}

/**
 * Options HTML fragment for static embeds.
 */
export function providerOptionsHtml() {
  return KNOWN_PROVIDERS.map(
    (p) => `<option value="${p.id}">${p.label}</option>`
  ).join("");
}
