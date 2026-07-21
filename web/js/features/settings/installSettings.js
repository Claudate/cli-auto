/**
 * [INPUT]: settings modules
 * [OUTPUT]: window.ccoSettings desk API + optional auto-boot
 * [POS]: A5-2d features/settings/installSettings.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as settingsApi from "./settingsApi.js";
import { loadSettings, saveSettings } from "./settingsForm.js";
import {
  loadDoctor,
  ensureDoctor,
  renderDoctorWarn,
  dismissDoctorWarn,
} from "./doctorPage.js";
import {
  startPolling,
  openMonitorWindow,
  boot,
  waitTauri,
  parseCcoWindowBoot,
} from "./shellBoot.js";
import { createUiActions, backFromSubpage } from "./uiActions.js";
import { bindGlobalUI, wire } from "./bindUi.js";

/**
 * Public desk for window.ccoSettings (classic doctor.js is facade).
 * IPC only via settingsApi → gateway.
 */
export function createSettingsDesk() {
  return {
    api: settingsApi,
    loadSettings,
    saveSettings,
    loadDoctor,
    ensureDoctor,
    renderDoctorWarn,
    dismissDoctorWarn,
    startPolling,
    openMonitorWindow,
    parseCcoWindowBoot,
    boot: (opts) => boot({ bindGlobalUI, ...opts }),
    waitTauri: (opts) => waitTauri({ bindGlobalUI, ...opts }),
    bindGlobalUI,
    wire,
    backFromSubpage,
    createUiActions,
    meta: () => settingsApi.meta(),
    getSettings: () => settingsApi.getSettings(),
    setSettings: (u) => settingsApi.setSettings(u),
    runDoctor: (p) => settingsApi.runDoctor(p),
  };
}

/**
 * Install globals used by classic plan.js / log.js / doctor facade.
 * Does not auto-boot unless `autoBoot` is true (doctor.js facade owns boot).
 * @param {{ autoBoot?: boolean }} [opts]
 */
export function installSettingsHost(opts = {}) {
  const desk = createSettingsDesk();
  window.ccoSettings = desk;

  // Classic global names (strangler) — overwrite any earlier stubs
  window.loadSettings = loadSettings;
  window.saveSettings = saveSettings;
  window.loadDoctor = loadDoctor;
  window.ensureDoctor = ensureDoctor;
  window.renderDoctorWarn = renderDoctorWarn;
  window.dismissDoctorWarn = dismissDoctorWarn;
  window.startPolling = startPolling;
  window.openMonitorWindow = openMonitorWindow;
  window.backFromSubpage = backFromSubpage;
  window.bindGlobalUI = bindGlobalUI;
  window.wire = wire;
  window.boot = desk.boot;
  window.waitTauri = desk.waitTauri;
  window.parseCcoWindowBoot = parseCcoWindowBoot;

  if (opts.autoBoot) {
    // Immediate bind + wait Tauri (same as classic doctor.js tail)
    bindGlobalUI();
    waitTauri({ bindGlobalUI });
  }

  return desk;
}
