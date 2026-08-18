/**
 * [INPUT]: settingsApi · state.doctorCache · DOM #doctor-list / #doctor-warn
 * [OUTPUT]: loadDoctor / ensureDoctor / renderDoctorWarn / dismissDoctorWarn
 * [POS]: A5-2d features/settings；仅渲染 DTO，无环境策略
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as settingsApi from "./settingsApi.js";

function $(sel) {
  return typeof window.$ === "function"
    ? window.$(sel)
    : document.querySelector(sel);
}

function state() {
  return typeof window !== "undefined" ? window.state : null;
}

function esc(s) {
  if (typeof window.esc === "function") return window.esc(s);
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function badge(kind) {
  if (typeof window.badge === "function") return window.badge(kind);
  return kind === "ok"
    ? '<span class="badge ok">通过</span>'
    : '<span class="badge failed">失败</span>';
}

/**
 * Safe external https URL only (doctor help links).
 * @param {unknown} raw
 * @returns {string|null}
 */
function safeHelpUrl(raw) {
  const s = String(raw || "").trim();
  if (!s) return null;
  try {
    const u = new URL(s);
    if (u.protocol !== "https:" && u.protocol !== "http:") return null;
    return u.href;
  } catch {
    return null;
  }
}

/**
 * Detail cell: text + optional 「官网下载」link (opens in browser).
 * @param {{ detail?: string, help_url?: string, ok?: boolean }} line
 */
function detailCellHtml(line) {
  const detail = esc(line.detail || "");
  const url = safeHelpUrl(line.help_url);
  if (!url) {
    return `<td class="muted">${detail}</td>`;
  }
  const link = `<a class="linkish doctor-help-link" href="${esc(url)}" target="_blank" rel="noopener noreferrer">官网下载</a>`;
  return `<td class="muted doctor-detail">${detail}<span class="doctor-help"> · ${link}</span></td>`;
}

/**
 * Extract provider short name from a doctor line name like "provider:codex:auth".
 * @param {string} name
 * @returns {string|null}
 */
function extractProviderName(name) {
  const m = (name || "").match(/^provider:([^:]+):/);
  return m ? m[1] : null;
}

/**
 * Channel status cell from probe result, with optional「设为默认」button.
 * @param {{ name?: string, probe?: { probe_status?: string, hint?: string }, kind?: string }} line
 * @param {string} [currentDefault] - current default_provider from settings
 */
function channelCellHtml(line, currentDefault) {
  const p = line.probe;
  if (!p) return '<td class="muted">—</td>';
  const status = p.probe_status || "not_supported";
  const hint = esc(p.hint || "");
  const map = {
    ok: { cls: "ok", label: "通道可用" },
    auth_invalid: { cls: "failed", label: "Key 失效" },
    insufficient_funds: { cls: "warn", label: "余额不足" },
    rate_limited: { cls: "warn", label: "限流中" },
    endpoint_broken: { cls: "failed", label: "接口异常" },
    not_supported: { cls: "muted", label: "—" },
  };
  const m = map[status] || { cls: "muted", label: status };
  // "Set as default" shortcut: show when probe ok, kind=auth, and not already default.
  const provName = extractProviderName(line.name);
  const isAuth = (line.kind || "") === "auth";
  const isDefault = provName && currentDefault && provName.toLowerCase() === currentDefault.toLowerCase();
  const showSetDefault = isAuth && provName && status === "ok" && !isDefault;
  const setBtn = showSetDefault
    ? ` <button class="linkish doctor-set-default" data-provider="${esc(provName)}" title="将 ${esc(provName)} 设为默认通道">设为默认</button>`
    : "";
  const defaultTag = isAuth && isDefault ? ' <span class="badge muted" style="font-size:.75rem">当前默认</span>' : "";
  return `<td class="doctor-channel"><span class="badge ${m.cls}" title="${hint}">${m.label}</span>${defaultTag}${setBtn}</td>`;
}

/**
 * Doctor page table from doctor_cmd DTO.
 */
export async function loadDoctor() {
  const st = state();
  if (!st) return null;
  try {
    const d = await settingsApi.runDoctor(st.selectedPath || null);
    st.doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    // Read current default_provider from settings cache or DOM.
    const provSel = $("#s-default-provider");
    const currentDefault = provSel ? provSel.value : "";
    const list = $("#doctor-list");
    if (list) {
      list.innerHTML = `<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th><th>通道状态</th></tr></thead>
      <tbody>
        ${lines
          .map(
            (l) => `<tr>
          <td>${esc(l.name)}</td>
          <td>${l.ok ? badge("ok") : badge("failed")}</td>
          ${detailCellHtml(l)}
          ${channelCellHtml(l, currentDefault)}
        </tr>`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">${
      d.ok
        ? "全部检查通过"
        : "存在失败项：未安装的 CLI 可点「官网下载」；Key 失效请更换；装好后点重新检查"
    }</p>`;
      // Bind "set as default" buttons.
      list.querySelectorAll(".doctor-set-default").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const prov = btn.dataset.provider;
          if (!prov) return;
          try {
            await settingsApi.setSettings({ default_provider: prov });
            if (provSel) provSel.value = prov;
            if (typeof window.toast === "function") {
              window.toast(`已将默认通道切换为 ${prov}`);
            }
            // Reload doctor to refresh badges.
            await loadDoctor();
          } catch (e) {
            if (typeof window.toast === "function") window.toast(String(e));
          }
        });
      });
    }
    renderDoctorWarn();
    return st.doctorCache;
  } catch (e) {
    if (typeof window.toast === "function") window.toast(String(e));
    throw e;
  }
}

/**
 * Cached doctor gate (60s) for workspace warn bar / confirm precheck.
 * @param {boolean} [force]
 */
export async function ensureDoctor(force = false) {
  const st = state();
  if (!st) return null;
  const now = Date.now();
  if (!force && st.doctorCache && now - st.doctorCache.at < 60_000) {
    renderDoctorWarn();
    return st.doctorCache;
  }
  try {
    const d = await settingsApi.runDoctor(st.selectedPath || null);
    st.doctorCache = { ok: !!d.ok, at: now, lines: d.lines || [] };
  } catch (e) {
    st.doctorCache = {
      ok: false,
      at: now,
      lines: [{ name: "doctor", ok: false, detail: String(e) }],
    };
  }
  renderDoctorWarn();
  return st.doctorCache;
}

/** Soft workspace banner when doctor fails (no strategy). */
export function renderDoctorWarn() {
  const st = state();
  const bar = $("#doctor-warn");
  if (!bar || !st || st.page !== "workspace") return;
  const d = st.doctorCache;
  if (!d || d.ok) {
    bar.hidden = true;
    return;
  }
  const fails = (d.lines || []).filter((l) => !l.ok);
  const key = fails.map((l) => l.name + ":" + l.detail).join("|");
  if (st.doctorDismissedKey && st.doctorDismissedKey === key) {
    bar.hidden = true;
    return;
  }
  const live = st.live;
  const runSt = String(live?.run_status || "").toLowerCase();
  const historyOk = live && ["completed", "done"].includes(runSt);
  const liveFn =
    typeof window.isLiveStatus === "function" ? window.isLiveStatus : () => false;
  if (historyOk && !liveFn(runSt)) {
    bar.hidden = true;
    return;
  }
  const detail = fails
    .map((l) => {
      const short = String(l.detail || "").split("·")[0].trim();
      return `${l.name}: ${short}`;
    })
    .slice(0, 2)
    .join(" · ");
  bar.classList.add("soft");
  const textEl = $("#doctor-warn-text");
  if (textEl) {
    textEl.textContent =
      (detail ? detail + "。" : "") +
      "可到「环境检查」点「官网下载」安装缺失 CLI，装好后重新检查。";
  }
  bar.hidden = false;
}

/** User dismisses current fail set. */
export function dismissDoctorWarn() {
  const st = state();
  if (!st) return;
  const d = st.doctorCache;
  const fails = (d?.lines || []).filter((l) => !l.ok);
  st.doctorDismissedKey =
    fails.map((l) => l.name + ":" + l.detail).join("|") || "dismissed";
  renderDoctorWarn();
  if (typeof window.toast === "function") {
    window.toast("已暂时忽略环境提示");
  }
}
