/**
 * [INPUT]: settingsApi · DOM #s-* · local prefs (flowFun / chatAssignDirect)
 * [OUTPUT]: loadSettings / saveSettings（只读写 DTO + 表单，无策略）
 * [POS]: A5-2d features/settings
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

/**
 * Fill settings page from backend DTO (+ local-only prefs).
 */
export async function loadSettings() {
  try {
    const s = await settingsApi.getSettings();
    const poll = $("#s-poll-interval");
    if (poll) poll.value = s.poll_interval_secs;
    const modeIdx = { print: 0, bg: 1, auto: 2 };
    const modeEl = $("#s-default-mode");
    if (modeEl) modeEl.value = modeIdx[s.default_mode] ?? 0;
    const prov = $("#s-default-provider");
    if (prov) prov.value = s.default_provider;
    const maxP = $("#s-max-parallel");
    if (maxP) maxP.value = s.max_parallel;
    // H3/H4: stall/retry + failover（与 scheduler 读取同源 DTO）
    if ($("#s-retry-max")) $("#s-retry-max").value = s.retry_max ?? 2;
    if ($("#s-stall-secs")) $("#s-stall-secs").value = s.stall_secs ?? 180;
    if ($("#s-failover-enabled")) {
      $("#s-failover-enabled").checked = s.failover_enabled !== false;
    }
    if ($("#s-failover-order-note") && s.failover_order_note) {
      $("#s-failover-order-note").textContent = s.failover_order_note;
    }
    if ($("#s-post-inspect")) {
      $("#s-post-inspect").checked = !!s.post_inspect_enabled;
    }
    if ($("#s-post-git-push")) {
      $("#s-post-git-push").checked = !!s.post_git_push_enabled;
    }
    if ($("#s-post-open-pr")) {
      $("#s-post-open-pr").checked = !!s.post_open_pr_enabled;
    }
    if ($("#s-planner-critic")) {
      $("#s-planner-critic").checked = !!s.planner_critic_enabled;
    }
    if ($("#s-post-tasks-note") && s.post_tasks_note) {
      $("#s-post-tasks-note").textContent = s.post_tasks_note;
    }
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
    const fontEl = $("#s-log-font");
    const st = state();
    if (fontEl && st) fontEl.value = String(st.logFontSize);
    // Seed split-time concurrency when user hasn't touched pickers
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(s.max_parallel || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(s.max_parallel || 2);
    }
    return s;
  } catch (_) {
    /* ignore cold-load failures */
    return null;
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
  const pollVal = parseInt($("#s-poll-interval")?.value, 10);
  const modeVal = parseInt($("#s-default-mode")?.value, 10);
  const providerVal = ($("#s-default-provider")?.value || "").trim();
  const maxParallelVal = parseInt($("#s-max-parallel")?.value, 10);
  const retryMaxVal = parseInt($("#s-retry-max")?.value, 10);
  const stallSecsVal = parseInt($("#s-stall-secs")?.value, 10);
  const failoverEl = $("#s-failover-enabled");
  const failoverEnabled = failoverEl ? !!failoverEl.checked : undefined;
  const postInspectEl = $("#s-post-inspect");
  const postGitPushEl = $("#s-post-git-push");
  const postOpenPrEl = $("#s-post-open-pr");
  const postInspectEnabled = postInspectEl ? !!postInspectEl.checked : undefined;
  const postGitPushEnabled = postGitPushEl ? !!postGitPushEl.checked : undefined;
  const postOpenPrEnabled = postOpenPrEl ? !!postOpenPrEl.checked : undefined;
  const plannerCriticEl = $("#s-planner-critic");
  const plannerCriticEnabled = plannerCriticEl
    ? !!plannerCriticEl.checked
    : undefined;
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
    if (failoverEnabled !== undefined) {
      update.failover_enabled = failoverEnabled;
    }
    if (postInspectEnabled !== undefined) {
      update.post_inspect_enabled = postInspectEnabled;
    }
    if (postGitPushEnabled !== undefined) {
      update.post_git_push_enabled = postGitPushEnabled;
    }
    if (postOpenPrEnabled !== undefined) {
      update.post_open_pr_enabled = postOpenPrEnabled;
    }
    if (plannerCriticEnabled !== undefined) {
      update.planner_critic_enabled = plannerCriticEnabled;
    }
    const updated = await settingsApi.setSettings(update);
    if (typeof window.applyLogFontSize === "function") {
      window.applyLogFontSize(fontVal);
    }
    if ($("#pp-provider")) $("#pp-provider").value = providerVal;
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(maxParallelVal || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(maxParallelVal || 2);
    }
    if ($("#s-failover-order-note") && updated.failover_order_note) {
      $("#s-failover-order-note").textContent = updated.failover_order_note;
    }
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
    if (
      $("#s-post-git-push") &&
      typeof updated.post_git_push_enabled === "boolean"
    ) {
      $("#s-post-git-push").checked = updated.post_git_push_enabled;
    }
    if (
      $("#s-post-open-pr") &&
      typeof updated.post_open_pr_enabled === "boolean"
    ) {
      $("#s-post-open-pr").checked = updated.post_open_pr_enabled;
    }
    if ($("#s-post-tasks-note") && updated.post_tasks_note) {
      $("#s-post-tasks-note").textContent = updated.post_tasks_note;
    }
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
