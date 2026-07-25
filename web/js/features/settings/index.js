/**
 * [INPUT]: settingsApi / form / doctor / boot / ui
 * [OUTPUT]: settings feature 公共出口
 * [POS]: A5-2d features/settings
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 模块图:
 *   settingsApi   → gateway（get/set settings · doctor · meta · open_monitor）
 *   settingsForm  → 设置页 load/save
 *   doctorPage    → 环境检查页 + warn bar
 *   shellBoot     → 冷启动 / 轮询 / 监视窗
 *   uiActions      → 事件表只绑意图
 *   bindUi        → 全局委托壳（resize/key/paste/change）
 *   bindUiClick   → document click 意图委托（P-ship-D 纵切）
 *   installSettings → window.ccoSettings
 */

export * as settingsApi from "./settingsApi.js";
export {
  loadSettings,
  saveSettings,
  restoreRecommendedPermission,
  paintPermissionUi,
} from "./settingsForm.js";
export {
  loadDoctor,
  ensureDoctor,
  renderDoctorWarn,
  dismissDoctorWarn,
} from "./doctorPage.js";
export {
  startPolling,
  openMonitorWindow,
  boot,
  waitTauri,
  parseCcoWindowBoot,
} from "./shellBoot.js";
export { createUiActions, backFromSubpage } from "./uiActions.js";
export { bindGlobalUI, wire } from "./bindUi.js";
export { createSettingsDesk, installSettingsHost } from "./installSettings.js";
