/**
 * [INPUT]: settingsApi · gateway pins/gitDoctor · DOM #s-* · local prefs (flowFun / chatAssignDirect)
 * [OUTPUT]: loadSettings / saveSettings / GitHub 发布状态 / 自动提交粒度表单 / pin CRUD（P2-2）
 * [POS]: A5-2d features/settings
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * paintPermissionUi 末尾同步 syncPermissionNav —— 授权分区拒写时左侧菜单出现警示点。
 */

import * as settingsApi from "./settingsApi.js";
import * as gateway from "../../shared/gateway.js";
import { wireSettingsNav, syncPermissionNav } from "./settingsNav.js";
import {
  loadWorkStyleSetting,
  saveWorkStyleSetting,
  suggestedMaxParallel,
  resolvedWorkStyle,
} from "../../shared/workStyle.js";

function $(sel) {
  return typeof window.$ === "function"
    ? window.$(sel)
    : document.querySelector(sel);
}

function state() {
  return typeof window !== "undefined" ? window.state : null;
}

function escHtml(s) {
  return String(s || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

const PERMISSION_MODES = [
  "bypassPermissions",
  "acceptEdits",
  "dontAsk",
  "default",
];

/** Modes that auto-deny tools needing confirmation (unattended write fails). */
function permissionBlocks(mode) {
  const m = String(mode || "");
  return m === "dontAsk" || m === "default";
}

/** Modes that count as "auto authorize" for the main checkbox. */
function permissionIsAuto(mode) {
  const m = String(mode || "bypassPermissions");
  return m === "bypassPermissions" || m === "acceptEdits";
}

/**
 * Paint 任务授权 section from a permission_mode string.
 * @param {string} mode
 * @param {{ skipSelect?: boolean }} [opts]
 */
export function paintPermissionUi(mode, opts = {}) {
  const m = PERMISSION_MODES.includes(String(mode || ""))
    ? String(mode)
    : "bypassPermissions";
  const blocks = permissionBlocks(m);
  const auto = permissionIsAuto(m);
  syncPermissionNav(blocks);

  const select = $("#s-permission-mode");
  if (select && !opts.skipSelect) select.value = m;

  const chk = $("#s-permission-auto");
  if (chk) chk.checked = auto;

  const note = $("#s-permission-mode-note");
  if (note) {
    if (m === "bypassPermissions") {
      note.textContent = "推荐开启 · 任务才能真正落地";
    } else if (m === "acceptEdits") {
      note.textContent = "可改文件；shell 命令仍可能被拦";
    } else if (m === "dontAsk") {
      note.textContent = "当前会拒写 → 任务易假完成";
    } else {
      note.textContent = "CLI 默认 · 无人值守时可能拒写";
    }
  }

  const status = $("#s-permission-status");
  if (status) {
    status.dataset.state = blocks ? "warn" : "ok";
    if (m === "bypassPermissions") {
      status.textContent = "任务可自动写文件与执行命令";
    } else if (m === "acceptEdits") {
      status.textContent = "可自动改文件；部分命令仍可能被拦";
    } else {
      status.textContent =
        "当前会拒绝写操作：执行规划时任务可能假完成，请改回自动授权";
    }
  }

  const restore = $("#btn-permission-restore");
  if (restore) restore.hidden = m === "bypassPermissions";
}

let _permissionUiWired = false;

/** One-time listeners: checkbox ↔ select stay in sync (save still via 保存). */
function wirePermissionUi() {
  if (_permissionUiWired) return;
  _permissionUiWired = true;

  const chk = $("#s-permission-auto");
  if (chk && !chk.dataset.ccoPermWire) {
    chk.dataset.ccoPermWire = "1";
    chk.addEventListener("change", () => {
      const next = chk.checked ? "bypassPermissions" : "dontAsk";
      paintPermissionUi(next);
    });
  }

  const select = $("#s-permission-mode");
  if (select && !select.dataset.ccoPermWire) {
    select.dataset.ccoPermWire = "1";
    select.addEventListener("change", () => {
      paintPermissionUi(select.value || "bypassPermissions");
    });
  }

  const restore = $("#btn-permission-restore");
  if (restore && !restore.dataset.ccoPermWire) {
    restore.dataset.ccoPermWire = "1";
    restore.addEventListener("click", () => {
      restoreRecommendedPermission().catch((e) => {
        const toast = typeof window.toast === "function" ? window.toast : null;
        if (toast) toast(String(e?.message || e));
      });
    });
  }
}

/**
 * Immediately set + persist bypassPermissions (one-click fix).
 * @returns {Promise<object|null>} updated settings or null
 */
export async function restoreRecommendedPermission() {
  paintPermissionUi("bypassPermissions");
  const updated = await settingsApi.setSettings({
    permission_mode: "bypassPermissions",
  });
  if (updated?.permission_mode) {
    paintPermissionUi(updated.permission_mode);
  }
  const status = $("#s-save-status");
  if (status) {
    status.className = "save-status ok";
    status.textContent = "已恢复自动授权并保存";
    status.hidden = false;
  }
  const toast = typeof window.toast === "function" ? window.toast : null;
  if (toast) toast("已恢复推荐授权（自动写文件与命令）");
  return updated;
}

function lineMap(lines) {
  const map = new Map();
  for (const line of Array.isArray(lines) ? lines : []) {
    if (line?.name) map.set(String(line.name), line);
  }
  return map;
}

function paintGithubStatus(lines, err) {
  const box = $("#s-github-status");
  if (!box) return;
  if (err) {
    box.dataset.state = "bad";
    box.innerHTML =
      `<div class="settings-status-title">GitHub 状态检查失败</div>` +
      `<p class="field-hint">${escHtml(err)}</p>`;
    return;
  }
  if (lines == null) {
    box.dataset.state = "idle";
    box.innerHTML =
      `<div class="settings-status-title">正在检查 GitHub 发布状态</div>` +
      `<p class="field-hint">正在读取 Git 仓库、远端、提交身份和 gh 登录状态。</p>`;
    return;
  }
  const st = state();
  if (!st?.selectedPath) {
    box.dataset.state = "warn";
    box.innerHTML =
      `<div class="settings-status-title">请先在左侧选中项目</div>` +
      `<p class="field-hint">选中项目后才能检查 Git 仓库、远端和 gh 登录。</p>`;
    return;
  }
  const byName = lineMap(lines);
  const repo = byName.get("git_repo");
  const branch = byName.get("git_branch");
  const remote = byName.get("git_remotes");
  const identity = byName.get("git_identity");
  const ghBin = byName.get("gh_bin");
  const ghAuth = byName.get("gh_auth");
  const blockers = [repo, remote, identity, ghBin, ghAuth].filter(
    (x) => !x || !x.ok
  );
  const ready = blockers.length === 0;
  box.dataset.state = ready ? "ok" : "warn";
  const title = ready
    ? "GitHub 发布已准备好"
    : `GitHub 发布还差 ${blockers.length} 项`;
  const branchText = branch?.detail ? `当前分支：${branch.detail}` : "当前分支：未识别";
  const remoteText = remote?.ok ? `远端：${remote.detail}` : "远端：未配置";
  const ghText = ghAuth?.ok
    ? `gh：${ghAuth.detail}`
    : ghBin?.ok
      ? "gh：未登录"
      : "gh：未安装或不在 PATH";
  const identityText = identity?.ok
    ? `提交身份：${identity.detail}`
    : "提交身份：未配置 user.name / user.email";
  box.innerHTML =
    `<div class="settings-status-title">${escHtml(title)}</div>` +
    `<p class="field-hint">${escHtml(branchText)} · ${escHtml(remoteText)} · ${escHtml(identityText)} · ${escHtml(ghText)}</p>`;
}

export async function refreshGithubStatus() {
  const st = state();
  const project = st?.selectedPath || "";
  paintGithubStatus(null, null);
  if (!project) return [];
  try {
    const lines = await settingsApi.gitDoctor(project);
    paintGithubStatus(lines, null);
    return lines;
  } catch (e) {
    paintGithubStatus(null, String(e?.message || e));
    return [];
  }
}

/**
 * Fill settings page from backend DTO (+ local-only prefs).
 */
export async function loadSettings() {
  try {
    wirePermissionUi();
    wireSettingsNav();
    const s = await settingsApi.getSettings();
    const poll = $("#s-poll-interval");
    if (poll) poll.value = s.poll_interval_secs;
    const modeIdx = { print: 0, bg: 1, auto: 2 };
    const modeEl = $("#s-default-mode");
    if (modeEl) modeEl.value = modeIdx[s.default_mode] ?? 0;
    const prov = $("#s-default-provider");
    if (prov) prov.value = s.default_provider;
    const effortEl = $("#s-effort");
    if (effortEl && s.effort) {
      const e = String(s.effort).toLowerCase();
      if (["low", "medium", "high", "xhigh", "max", "ultracode"].includes(e)) {
        effortEl.value = e;
      }
    }
    paintPermissionUi(s.permission_mode || "bypassPermissions");
    // Seed chat composer select from config default (unless user already picked)
    const chatEffort = $("#chat-effort");
    if (chatEffort) {
      let pick = null;
      try {
        pick = localStorage.getItem("cco.chatEffort");
      } catch (_) {}
      const seed = pick || s.effort || "high";
      const e = String(seed).toLowerCase();
      if (["low", "medium", "high", "xhigh", "max", "ultracode"].includes(e)) {
        chatEffort.value = e;
      }
    }
    // Seed split-page + chooser depth pickers (local split pick wins, else config)
    const seedSplit = (() => {
      let pick = null;
      try {
        pick = localStorage.getItem("cco.splitEffort");
      } catch (_) {}
      const e = String(pick || s.effort || "high").toLowerCase();
      return ["low", "medium", "high", "xhigh", "max", "ultracode"].includes(e)
        ? e
        : "high";
    })();
    for (const id of ["#split-effort", "#pp-effort"]) {
      const el = $(id);
      if (el) el.value = seedSplit;
    }
    const maxP = $("#s-max-parallel");
    if (maxP) maxP.value = s.max_parallel;
    // H3/H4: stall/retry + failover（与 scheduler 读取同源 DTO）
    if ($("#s-retry-max")) $("#s-retry-max").value = s.retry_max ?? 2;
    if ($("#s-stall-secs")) $("#s-stall-secs").value = s.stall_secs ?? 180;
    if ($("#s-failover-enabled")) {
      $("#s-failover-enabled").checked = s.failover_enabled !== false;
    }
    // keep short static #s-failover-order-note; long DTO stays for CLI/docs
    if ($("#s-cost-route-enabled")) {
      $("#s-cost-route-enabled").checked = s.cost_route_enabled !== false;
    }
    if ($("#s-cost-escalate-enabled")) {
      $("#s-cost-escalate-enabled").checked = s.cost_escalate_enabled !== false;
    }
    if ($("#s-cost-intent-enabled")) {
      $("#s-cost-intent-enabled").checked = !!s.cost_intent_enabled;
    }
    if ($("#s-cost-route-note") && s.cost_route_note) {
      $("#s-cost-route-note").textContent = String(s.cost_route_note);
    }
    if ($("#s-post-inspect")) {
      $("#s-post-inspect").checked = !!s.post_inspect_enabled;
    }
    if ($("#s-git-auto-commit-granularity")) {
      const granularity = String(
        s.git_auto_commit_granularity ||
          (s.post_git_push_enabled ? "per_plan" : "off")
      ).toLowerCase();
      $("#s-git-auto-commit-granularity").value = [
        "off",
        "per_plan",
        "per_task",
      ].includes(granularity)
        ? granularity
        : "off";
    }
    if ($("#s-git-push-after-commit")) {
      $("#s-git-push-after-commit").checked = !!s.git_push_after_commit;
    }
    if ($("#s-post-open-pr")) {
      $("#s-post-open-pr").checked = !!s.post_open_pr_enabled;
    }
    if ($("#s-planner-critic")) {
      $("#s-planner-critic").checked = !!s.planner_critic_enabled;
    }
    if ($("#s-browser-enabled")) {
      $("#s-browser-enabled").checked = !!s.browser_enabled;
    }
    if ($("#s-browser-engine") && s.browser_engine) {
      const eng = String(s.browser_engine).toLowerCase();
      $("#s-browser-engine").value =
        eng.includes("playwright") ? "playwright_mcp" : "kitewright";
    }
    if ($("#s-browser-require-preview")) {
      $("#s-browser-require-preview").checked =
        s.browser_require_preview !== false;
    }
    if ($("#s-browser-note") && s.browser_note) {
      $("#s-browser-note").textContent = String(s.browser_note);
    }
    // UI keeps short static copy under #s-post-tasks-note; do not paste long DTO note.
    if ($("#s-flow-fun") && typeof window.flowFunEnabled === "function") {
      $("#s-flow-fun").checked = window.flowFunEnabled();
    }
    // A2：勾选「先确认选项」= 关直拆（!chatAssignDirectEnabled）
    if (
      $("#s-chat-assign-direct") &&
      typeof window.chatAssignDirectEnabled === "function"
    ) {
      $("#s-chat-assign-direct").checked = !window.chatAssignDirectEnabled();
    }
    const st = state();
    const projectPath = st?.selectedPath || null;
    try {
      loadWorkStyleSetting(projectPath);
    } catch (_) {}
    const fontEl = $("#s-log-font");
    if (fontEl && st) fontEl.value = String(st.logFontSize);
    // Seed split-time concurrency: config × work-style hint when user hasn't touched pickers
    const seedMp = suggestedMaxParallel(s.max_parallel || 2, projectPath);
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(seedMp);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(seedMp);
    }
    // Soft-seed plan_mode only when unset and user never touched chooser
    const pm = $("#pp-plan-mode");
    if (pm && !pm.dataset.touched) {
      const style = resolvedWorkStyle(projectPath);
      // Product default remains AI; profiles never force fast (W4 / Q0).
      void style;
    }
    // P2-2: load pins for selected project (best-effort)
    try {
      await loadProjectPins();
    } catch (_) {}
    try {
      await refreshGithubStatus();
    } catch (_) {}
    return s;
  } catch (_) {
    /* ignore cold-load failures */
    return null;
  }
}

function escPin(s) {
  return String(s || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** P2-2: render pin list (≤3) for current project. */
export async function loadProjectPins() {
  const list = $("#s-pins-list");
  const empty = $("#s-pins-empty");
  const status = $("#s-pins-status");
  const st = state();
  const project = st?.selectedPath;
  if (!list) return [];
  if (!project) {
    list.innerHTML =
      `<p class="field-hint" id="s-pins-empty">请先在左侧选中项目</p>`;
    return [];
  }
  try {
    const pins = (await gateway.projectPinsList(project)) || [];
    if (!pins.length) {
      list.innerHTML =
        `<p class="field-hint" id="s-pins-empty">暂无 pin（最多 3 条）</p>`;
    } else {
      list.innerHTML = pins
        .map(
          (p) =>
            `<div class="pin-row" data-pin-key="${escPin(p.key)}">` +
            `<span class="pin-key"><strong>${escPin(p.key)}</strong></span>` +
            `<span class="pin-val muted">${escPin(p.value)}</span>` +
            `<button type="button" class="linkish sm" data-pin-delete="${escPin(
              p.key
            )}" title="删除">删除</button>` +
            `</div>`
        )
        .join("");
    }
    if (status) {
      status.hidden = true;
      status.textContent = "";
    }
    return pins;
  } catch (e) {
    list.innerHTML = `<p class="field-hint" id="s-pins-empty">加载 pin 失败</p>`;
    if (status) {
      status.hidden = false;
      status.textContent = String(e?.message || e);
    }
    return [];
  }
}

/** P2-2: add pin from settings form. */
export async function addProjectPin() {
  const st = state();
  const project = st?.selectedPath;
  const toast =
    typeof window.toast === "function" ? window.toast : (m) => console.log(m);
  if (!project) {
    toast("请先选择项目");
    return null;
  }
  const key = ($("#s-pin-key")?.value || "").trim();
  const value = ($("#s-pin-value")?.value || "").trim();
  if (!key || !value) {
    toast("请填写 pin 键与值");
    return null;
  }
  try {
    const pin = await gateway.projectPinUpsert(project, key, value);
    if ($("#s-pin-key")) $("#s-pin-key").value = "";
    if ($("#s-pin-value")) $("#s-pin-value").value = "";
    toast(`已保存 pin：${key}`);
    await loadProjectPins();
    return pin;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  }
}

/** P2-2: delete pin by key. */
export async function deleteProjectPin(key) {
  const st = state();
  const project = st?.selectedPath;
  const toast =
    typeof window.toast === "function" ? window.toast : (m) => console.log(m);
  if (!project || !key) return false;
  try {
    await gateway.projectPinDelete(project, key);
    toast(`已删除 pin：${key}`);
    await loadProjectPins();
    return true;
  } catch (e) {
    toast(String(e?.message || e));
    return false;
  }
}

/**
 * Validate form → setSettings → sync pickers / poll interval.
 */
export async function saveSettings() {
  if ($("#s-flow-fun") && typeof window.setFlowFunEnabled === "function") {
    window.setFlowFunEnabled(!!$("#s-flow-fun").checked);
  }
  // A2：勾选「先确认选项」→ setChatAssignDirectEnabled(false)
  if (
    $("#s-chat-assign-direct") &&
    typeof window.setChatAssignDirectEnabled === "function"
  ) {
    window.setChatAssignDirectEnabled(!$("#s-chat-assign-direct").checked);
  }
  try {
    saveWorkStyleSetting(state()?.selectedPath || null);
  } catch (_) {}
  const pollVal = parseInt($("#s-poll-interval")?.value, 10);
  const modeVal = parseInt($("#s-default-mode")?.value, 10);
  const providerVal = ($("#s-default-provider")?.value || "").trim();
  const effortVal = ($("#s-effort")?.value || "").trim().toLowerCase();
  // Prefer explicit select; if user only flipped the main checkbox, derive mode.
  let permissionModeVal = ($("#s-permission-mode")?.value || "").trim();
  const autoChk = $("#s-permission-auto");
  if (autoChk) {
    if (autoChk.checked && permissionBlocks(permissionModeVal)) {
      permissionModeVal = "bypassPermissions";
    } else if (!autoChk.checked && permissionIsAuto(permissionModeVal)) {
      permissionModeVal = "dontAsk";
    }
  }
  const maxParallelVal = parseInt($("#s-max-parallel")?.value, 10);
  const retryMaxVal = parseInt($("#s-retry-max")?.value, 10);
  const stallSecsVal = parseInt($("#s-stall-secs")?.value, 10);
  const failoverEl = $("#s-failover-enabled");
  const failoverEnabled = failoverEl ? !!failoverEl.checked : undefined;
  const postInspectEl = $("#s-post-inspect");
  const gitAutoCommitGranularityEl = $("#s-git-auto-commit-granularity");
  const gitPushAfterCommitEl = $("#s-git-push-after-commit");
  const postOpenPrEl = $("#s-post-open-pr");
  const postInspectEnabled = postInspectEl ? !!postInspectEl.checked : undefined;
  const gitAutoCommitGranularity = gitAutoCommitGranularityEl
    ? String(gitAutoCommitGranularityEl.value || "").trim()
    : undefined;
  const gitPushAfterCommit = gitPushAfterCommitEl
    ? !!gitPushAfterCommitEl.checked
    : undefined;
  const postOpenPrEnabled = postOpenPrEl ? !!postOpenPrEl.checked : undefined;
  const plannerCriticEl = $("#s-planner-critic");
  const plannerCriticEnabled = plannerCriticEl
    ? !!plannerCriticEl.checked
    : undefined;
  const browserEl = $("#s-browser-enabled");
  const browserEnabled = browserEl ? !!browserEl.checked : undefined;
  const browserEngineEl = $("#s-browser-engine");
  const browserEngine = browserEngineEl
    ? String(browserEngineEl.value || "").trim()
    : undefined;
  const browserReqPrevEl = $("#s-browser-require-preview");
  const browserRequirePreview = browserReqPrevEl
    ? !!browserReqPrevEl.checked
    : undefined;
  const costRouteEl = $("#s-cost-route-enabled");
  const costRouteEnabled = costRouteEl ? !!costRouteEl.checked : undefined;
  const costEscEl = $("#s-cost-escalate-enabled");
  const costEscalateEnabled = costEscEl ? !!costEscEl.checked : undefined;
  const costIntentEl = $("#s-cost-intent-enabled");
  const costIntentEnabled = costIntentEl ? !!costIntentEl.checked : undefined;
  const fontVal = parseInt($("#s-log-font")?.value, 10) || 14;
  const status = $("#s-save-status");
  if (!pollVal || pollVal < 1 || pollVal > 60) {
    if (status) {
      status.className = "save-status err";
      status.textContent = "刷新间隔需在 1–60 秒之间";
      status.hidden = false;
    }
    return;
  }
  if (Number.isFinite(retryMaxVal) && (retryMaxVal < 0 || retryMaxVal > 10)) {
    if (status) {
      status.className = "save-status err";
      status.textContent = "同 CLI 再试次数需在 0–10 之间";
      status.hidden = false;
    }
    return;
  }
  if (
    Number.isFinite(stallSecsVal) &&
    (stallSecsVal < 30 || stallSecsVal > 7200)
  ) {
    if (status) {
      status.className = "save-status err";
      status.textContent = "卡死秒数需在 30–7200 之间（多久没新日志算卡死）";
      status.hidden = false;
    }
    return;
  }
  try {
    const update = {
      poll_interval_secs: pollVal,
      default_mode: modeVal,
      default_provider: providerVal,
      max_parallel: maxParallelVal || 2,
      retry_max: Number.isFinite(retryMaxVal) ? retryMaxVal : 2,
      stall_secs: Number.isFinite(stallSecsVal) ? stallSecsVal : 180,
    };
    if (
      ["low", "medium", "high", "xhigh", "max", "ultracode"].includes(effortVal)
    ) {
      update.effort = effortVal;
    }
    if (
      [
        "bypassPermissions",
        "acceptEdits",
        "dontAsk",
        "default",
      ].includes(permissionModeVal)
    ) {
      update.permission_mode = permissionModeVal;
    }
    if (failoverEnabled !== undefined) {
      update.failover_enabled = failoverEnabled;
    }
    if (postInspectEnabled !== undefined) {
      update.post_inspect_enabled = postInspectEnabled;
    }
    if (["off", "per_plan", "per_task"].includes(gitAutoCommitGranularity)) {
      update.git_auto_commit_granularity = gitAutoCommitGranularity;
    }
    if (gitPushAfterCommit !== undefined) {
      update.git_push_after_commit = gitPushAfterCommit;
    }
    if (postOpenPrEnabled !== undefined) {
      update.post_open_pr_enabled = postOpenPrEnabled;
    }
    if (plannerCriticEnabled !== undefined) {
      update.planner_critic_enabled = plannerCriticEnabled;
    }
    if (browserEnabled !== undefined) {
      update.browser_enabled = browserEnabled;
    }
    if (browserEngine === "kitewright" || browserEngine === "playwright_mcp") {
      update.browser_engine = browserEngine;
    }
    if (browserRequirePreview !== undefined) {
      update.browser_require_preview = browserRequirePreview;
    }
    if (costRouteEnabled !== undefined) {
      update.cost_route_enabled = costRouteEnabled;
    }
    if (costEscalateEnabled !== undefined) {
      update.cost_escalate_enabled = costEscalateEnabled;
    }
    if (costIntentEnabled !== undefined) {
      update.cost_intent_enabled = costIntentEnabled;
    }
    const updated = await settingsApi.setSettings(update);
    if (typeof window.applyLogFontSize === "function") {
      window.applyLogFontSize(fontVal);
    }
    // Sync chat composer depth when settings effort changed (keep local pick if set)
    if (updated?.effort && $("#chat-effort")) {
      let localPick = null;
      try {
        localPick = localStorage.getItem("cco.chatEffort");
      } catch (_) {}
      if (!localPick) $("#chat-effort").value = updated.effort;
    }
    // Sync split-page depth when settings change (keep local split pick if set)
    if (updated?.effort) {
      let splitPick = null;
      try {
        splitPick = localStorage.getItem("cco.splitEffort");
      } catch (_) {}
      if (!splitPick) {
        for (const id of ["#split-effort", "#pp-effort"]) {
          const el = $(id);
          if (el) el.value = updated.effort;
        }
      }
    }
    if ($("#s-effort") && updated?.effort) {
      $("#s-effort").value = updated.effort;
    }
    if (updated?.permission_mode) {
      paintPermissionUi(updated.permission_mode);
    }
    if ($("#pp-provider")) $("#pp-provider").value = providerVal;
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(maxParallelVal || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(maxParallelVal || 2);
    }
    // keep short static #s-failover-order-note
    if ($("#s-failover-enabled") && typeof updated.failover_enabled === "boolean") {
      $("#s-failover-enabled").checked = updated.failover_enabled;
    }
    if ($("#s-post-inspect") && typeof updated.post_inspect_enabled === "boolean") {
      $("#s-post-inspect").checked = updated.post_inspect_enabled;
    }
    if (
      $("#s-planner-critic") &&
      typeof updated.planner_critic_enabled === "boolean"
    ) {
      $("#s-planner-critic").checked = updated.planner_critic_enabled;
    }
    if ($("#s-browser-enabled") && typeof updated.browser_enabled === "boolean") {
      $("#s-browser-enabled").checked = updated.browser_enabled;
    }
    if ($("#s-browser-engine") && updated?.browser_engine) {
      const eng = String(updated.browser_engine).toLowerCase();
      $("#s-browser-engine").value =
        eng.includes("playwright") ? "playwright_mcp" : "kitewright";
    }
    if (
      $("#s-browser-require-preview") &&
      typeof updated.browser_require_preview === "boolean"
    ) {
      $("#s-browser-require-preview").checked = updated.browser_require_preview;
    }
    if ($("#s-cost-route-enabled") && typeof updated.cost_route_enabled === "boolean") {
      $("#s-cost-route-enabled").checked = updated.cost_route_enabled;
    }
    if (
      $("#s-cost-escalate-enabled") &&
      typeof updated.cost_escalate_enabled === "boolean"
    ) {
      $("#s-cost-escalate-enabled").checked = updated.cost_escalate_enabled;
    }
    if (
      $("#s-cost-intent-enabled") &&
      typeof updated.cost_intent_enabled === "boolean"
    ) {
      $("#s-cost-intent-enabled").checked = updated.cost_intent_enabled;
    }
    if ($("#s-cost-route-note") && updated?.cost_route_note) {
      $("#s-cost-route-note").textContent = String(updated.cost_route_note);
    }
    if ($("#s-browser-note") && updated?.browser_note) {
      $("#s-browser-note").textContent = String(updated.browser_note);
    }
    if (
      $("#s-git-auto-commit-granularity") &&
      ["off", "per_plan", "per_task"].includes(
        String(updated?.git_auto_commit_granularity || "")
      )
    ) {
      $("#s-git-auto-commit-granularity").value =
        updated.git_auto_commit_granularity;
    }
    if (
      $("#s-git-push-after-commit") &&
      typeof updated?.git_push_after_commit === "boolean"
    ) {
      $("#s-git-push-after-commit").checked = updated.git_push_after_commit;
    }
    if (
      $("#s-post-open-pr") &&
      typeof updated.post_open_pr_enabled === "boolean"
    ) {
      $("#s-post-open-pr").checked = updated.post_open_pr_enabled;
    }
    // keep short static #s-post-tasks-note; ignore long DTO note
    if (status) {
      status.className = "save-status ok";
      status.textContent = "已保存";
      status.hidden = false;
      setTimeout(() => {
        status.hidden = true;
      }, 2500);
    }
    if (typeof window.startPolling === "function") {
      window.startPolling(Math.min(updated.poll_interval_secs * 1000, 5000));
    }
    return updated;
  } catch (e) {
    if (status) {
      status.className = "save-status err";
      status.textContent = "保存失败: " + e;
      status.hidden = false;
    }
    throw e;
  }
}
