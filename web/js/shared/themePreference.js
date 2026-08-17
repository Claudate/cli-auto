/**
 * [INPUT]: localStorage · matchMedia · 外观 radio controls
 * [OUTPUT]: 浅色/深色/跟随系统三态的解析、应用与持久化
 * [POS]: web/js/shared 的展示偏好单一真相源；不接触后端设置 DTO
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

const STORAGE_KEY = "cco.leafTheme";
const VALID_PREFERENCES = new Set(["light", "dark", "system"]);
const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");

function readPreference() {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return VALID_PREFERENCES.has(value) ? value : "system";
  } catch (_) {
    return "system";
  }
}

function resolveTheme(preference) {
  return preference === "system"
    ? (systemTheme.matches ? "dark" : "light")
    : preference;
}

function syncControls(preference) {
  document
    .querySelectorAll('input[name="leaf-theme"]')
    .forEach((control) => {
      control.checked = control.value === preference;
    });
}

export function applyThemePreference(preference) {
  const next = VALID_PREFERENCES.has(preference) ? preference : "system";
  const resolved = resolveTheme(next);
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  if (document.body) {
    document.body.dataset.leafTheme = resolved;
    document.body.dataset.theme = resolved;
  }
  syncControls(next);
  window.dispatchEvent(
    new CustomEvent("leaf-theme-change", {
      detail: { preference: next, resolved },
    })
  );
  return { preference: next, resolved };
}

export function getThemePreference() {
  return readPreference();
}

export function installThemePreference() {
  const applyStored = () => applyThemePreference(readPreference());
  document.addEventListener("change", (event) => {
    const control = event.target;
    if (!control?.matches?.('input[name="leaf-theme"]')) return;
    const preference = VALID_PREFERENCES.has(control.value)
      ? control.value
      : "system";
    try {
      localStorage.setItem(STORAGE_KEY, preference);
    } catch (_) {}
    applyThemePreference(preference);
  });
  systemTheme.addEventListener("change", () => {
    if (readPreference() === "system") applyStored();
  });
  applyStored();
  return { applyThemePreference, getThemePreference };
}
