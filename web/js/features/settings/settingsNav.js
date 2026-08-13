/**
 * [INPUT]: #page-settings .settings-aside / .settings-content / .settings-area
 * [OUTPUT]: 左侧父级菜单 → 点击滚动定位到对应父级区域 + 滚动时高亮当前分区 +
 *           授权分区警示点（syncPermissionNav，同步 paintPermissionUi 拒写态）
 * [POS]: A5-2d features/settings
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 说明：仅在设置页 DOM 存在时接线；重复打开会保留既有监听（用 data-* 防重绑），
 * 每次调用会重算滚动高亮，保证打开时菜单高亮与服务区一致。
 */

let _wired = false;
let _updateFn = null; // scroll 监听只绑一次，回调始终指向最近一次 wire 的 update

/**
 * 授权分区「会拒写」→ 左侧菜单项加警示点（同步 paintPermissionUi 的错误态）。
 * 状态存 CSS class（is-warn），无逻辑复制。
 */
export function syncPermissionNav(blocks) {
  const page = document.getElementById("page-settings");
  if (!page) return;
  const item = page.querySelector(
    '.settings-nav-item[data-settings-anchor="set-permission"]'
  );
  if (item) item.classList.toggle("is-warn", !!blocks);
}

/**
 * 接线设置页左侧父级菜单（幂等？No —— 每次打开重置高亮，但点击监听只绑一次）。
 */
export function wireSettingsNav() {
  const page = document.getElementById("page-settings");
  if (!page) return;
  const aside = page.querySelector(".settings-aside");
  const content = page.querySelector(".settings-content");
  const items = Array.from(
    aside ? aside.querySelectorAll(".settings-nav-item[data-settings-anchor]") : []
  );
  if (!aside || !content || items.length === 0) return;

  const areas = items
    .map((item) => ({
      item,
      el: document.getElementById(item.dataset.settingsAnchor),
    }))
    .filter((a) => a.el);

  // 点击菜单 → 滚动到对应父级区域（幂等绑定一次）
  if (!_wired) {
    _wired = true;
    items.forEach((item) => {
      if (item.dataset.ccoNavWired) return;
      item.dataset.ccoNavWired = "1";
      item.addEventListener("click", () => {
        const target = document.getElementById(item.dataset.settingsAnchor);
        if (target) scrollToArea(content, target);
      });
    });
  }

  // 滚动高亮：越过高位线的最后一个分区为当前分区
  const update = () => {
    const line = content.scrollTop + 56;
    let current = areas[0] ? areas[0].item : null;
    for (const { item, el } of areas) {
      if (el.offsetTop <= line) current = item;
      else break;
    }
    // 滚到底时最后一块矮分区顶部不越过高位线，仍应高亮（macOS 设置同款预期）
    if (content.scrollTop + content.clientHeight >= content.scrollHeight - 8) {
      current = areas[areas.length - 1].item;
    }
    for (const { item } of areas) {
      item.classList.toggle("is-active", item === current);
    }
  };

  _updateFn = update;
  if (!content.dataset.ccoNavScrollWired) {
    content.dataset.ccoNavScrollWired = "1";
    let ticking = false;
    content.addEventListener("scroll", () => {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(() => {
        if (_updateFn) _updateFn();
        ticking = false;
      });
    });
  }
  update();
}

/** 平滑滚动到区域顶部，支持用户关闭平滑动画偏好。 */
function scrollToArea(content, el) {
  const reduce = window.matchMedia
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;
  const top = Math.max(el.offsetTop - 6, 0);
  content.scrollTo({ top, behavior: reduce ? "auto" : "smooth" });
}
