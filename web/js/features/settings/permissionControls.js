/**
 * [INPUT]: 权限表单 DOM · shared/confirmDialog 暴露的 window.ccoConfirm
 * [OUTPUT]: 权限模式映射、危险确认与 preset/select/checkbox 展示控制
 * [POS]: features/settings 的纯展示交互模块；不保存设置、不调用 gateway
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export const PERMISSION_MODES = [
  "bypassPermissions",
  "acceptEdits",
  "dontAsk",
  "default",
];

function normalize(mode) {
  return PERMISSION_MODES.includes(mode) ? mode : "bypassPermissions";
}

export function permissionBlocks(mode) {
  const value = String(mode || "");
  return value === "dontAsk" || value === "default";
}

export function permissionIsAuto(mode) {
  const value = String(mode || "bypassPermissions");
  return value === "bypassPermissions" || value === "acceptEdits";
}

export function permissionTierLabel(mode) {
  if (mode === "bypassPermissions") return "完全访问";
  if (mode === "acceptEdits") return "可读写项目文件";
  if (mode === "dontAsk" || mode === "default") return "受限只读";
  return "完全访问";
}

export function createPermissionControls({ getElement, paint }) {
  let paintedMode = "bypassPermissions";
  let wired = false;

  function sync(mode, opts) {
    paintedMode = normalize(mode);
    paint(paintedMode, opts);
  }

  async function choose(next) {
    const mode = normalize(next);
    const previous = paintedMode;
    if (mode === "bypassPermissions" && previous !== mode) {
      const confirm = typeof window.ccoConfirm === "function" ? window.ccoConfirm : null;
      if (!confirm) return false;
      const approved = await confirm({
        title: "启用完全访问？",
        body: "任务将可以自动改写文件并执行命令。请只在信任当前项目与任务来源时启用。",
        okLabel: "启用完全访问",
        cancelLabel: "保留当前权限",
        danger: true,
      });
      if (!approved) {
        sync(previous);
        return false;
      }
    }
    sync(mode);
    return true;
  }

  function wire() {
    if (wired) return;
    wired = true;

    const checkbox = getElement("s-permission-auto");
    if (checkbox) {
      checkbox.addEventListener("change", () => {
        choose(checkbox.checked ? "bypassPermissions" : "dontAsk");
      });
    }

    const select = getElement("s-permission-mode");
    if (select) {
      select.addEventListener("change", () => {
        choose(select.value || "bypassPermissions");
      });
    }

    document.querySelectorAll("[data-permission-preset]").forEach((preset) => {
      preset.addEventListener("click", () => {
        choose(preset.dataset.permissionPreset);
      });
    });
  }

  return { choose, sync, wire };
}
