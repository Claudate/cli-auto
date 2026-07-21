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
 * Doctor page table from doctor_cmd DTO.
 */
export async function loadDoctor() {
  const st = state();
  if (!st) return null;
  try {
    const d = await settingsApi.runDoctor(st.selectedPath || null);
    st.doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    const list = $("#doctor-list");
    if (list) {
      list.innerHTML = `<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th></tr></thead>
      <tbody>
        ${lines
          .map(
            (l) => `<tr>
          <td>${esc(l.name)}</td>
          <td>${l.ok ? badge("ok") : badge("failed")}</td>
          <td class="muted">${esc(l.detail)}</td>
        </tr>`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">${
      d.ok ? "全部检查通过" : "存在失败项，请按详情处理"
    }</p>`;
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
    .map((l) => `${l.name}: ${l.detail}`)
    .slice(0, 2)
    .join(" · ");
  bar.classList.add("soft");
  const textEl = $("#doctor-warn-text");
  if (textEl) {
    textEl.textContent =
      detail ||
      "环境检查未通过。若 Claude 已安装，点「重新检查」或设置 CCO_CLAUDE_BIN。";
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
