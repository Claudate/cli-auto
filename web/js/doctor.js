/**
 * [INPUT]: window.ccoSettings（main.js ESM 安装 features/settings）
 * [OUTPUT]: 经典全局函数名兼容（plan/log/monitor 调用）
 * [POS]: A5-2d doctor.js facade — 逻辑在 features/settings/*
 * note: 禁止堆新功能；禁止 invoke/confirm_start/start_run；IPC 只经 gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — doctor/settings classic facade (A5-2d strangler) */

function _ccoSettings() {
  return typeof window !== "undefined" ? window.ccoSettings : null;
}

function _settingsCall(name, ...args) {
  const d = _ccoSettings();
  if (d && typeof d[name] === "function") return d[name](...args);
  console.warn("[doctor facade] ccoSettings." + name + " not ready");
  return undefined;
}

function startPolling(...a) {
  return _settingsCall("startPolling", ...a);
}
function loadSettings(...a) {
  return _settingsCall("loadSettings", ...a);
}
function saveSettings(...a) {
  return _settingsCall("saveSettings", ...a);
}
function loadDoctor(...a) {
  return _settingsCall("loadDoctor", ...a);
}
function ensureDoctor(...a) {
  return _settingsCall("ensureDoctor", ...a);
}
function renderDoctorWarn(...a) {
  return _settingsCall("renderDoctorWarn", ...a);
}
function dismissDoctorWarn(...a) {
  return _settingsCall("dismissDoctorWarn", ...a);
}
function openMonitorWindow(...a) {
  return _settingsCall("openMonitorWindow", ...a);
}
function backFromSubpage(...a) {
  return _settingsCall("backFromSubpage", ...a);
}
function bindGlobalUI(...a) {
  return _settingsCall("bindGlobalUI", ...a);
}
function wire(...a) {
  return _settingsCall("wire", ...a);
}
function boot(...a) {
  return _settingsCall("boot", ...a);
}
function waitTauri(...a) {
  return _settingsCall("waitTauri", ...a);
}
function parseCcoWindowBoot(...a) {
  return _settingsCall("parseCcoWindowBoot", ...a);
}

// Boot owned by main.js → installSettingsHost({ autoBoot: true })
// Facade only provides classic names for plan.js / log.js / DevTools.
