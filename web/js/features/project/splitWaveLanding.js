/**
 * [INPUT]: state.planItems/selectedPlan · features/chat 纯 wave 模块（动态导入）
 * [OUTPUT]: 拆分 tab 的「本波总览」落地 + 钻进单份时的返回口 + 认领后落到拆分 tab
 * [POS]: features/project/splitWaveLanding.js（从 confirmActions 抽出 · 守 600 行硬顶）
 * [PROTOCOL]: 复用 buildWaveOverview/renderWaveOverviewHtml/loadWaveJobsByPath；
 *   不建第二套拆分台；确认仍走 confirm_start（rule 10/25）；landWaveInSplitTab 仅选中+导航；变更后检查 web/CLAUDE.md
 */
import {
  state,
  $,
  esc,
  hasActiveRun,
  normalizePlanPath,
  showPage,
} from "./legacy.js";
import {
  getBoundPlanJob,
  setBoundPlanJob,
  stampSplitDeskProject,
} from "./projectScope.js";

/**
 * 多计划「本波」总览落到拆分 tab：当前项目没有单份绑定 job，但上下文是一波
 * bundle 时，画出可复用的本波总览（列表 + 拆下一份 + 确认本波）。best-effort · 异步。
 * 调用方已先画保底空态；本函数只在命中一波时把 #confirm-waves 升级为总览。
 * @param {string} root 调用时锁定的项目路径
 * @returns {Promise<boolean>} 是否画出了总览
 */
export async function paintWaveLanding(root) {
  if (!root) return false;
  let mods;
  try {
    const [ov, plans, batch] = await Promise.all([
      import("../chat/chatWaveOverview.js"),
      import("../chat/chatWavePlans.js"),
      import("../chat/chatWaveBatch.js"),
    ]);
    mods = { ...ov, ...plans, ...batch };
  } catch (_) {
    return false;
  }
  const {
    buildWaveOverview,
    renderWaveOverviewHtml,
    waveDirKeyFromPath,
    groupPlanItemsByWave,
    loadWaveJobsByPath,
  } = mods;

  // 当前波：优先选中/草稿；否则本项目计划列表里最新一波（含执行计划）
  const pool = Array.isArray(state.planItems) ? state.planItems : [];
  let waveKey =
    waveDirKeyFromPath(state.selectedPlan || "") ||
    waveDirKeyFromPath(state.chatDraftPlan || "");
  if (!waveKey) {
    const { waves } = groupPlanItemsByWave(pool);
    const firstWithExec = (waves || []).find((w) => (w.plans || []).length > 0);
    waveKey = firstWithExec ? firstWithExec.key : null;
  }
  if (!waveKey) return false;

  const norm = (p) =>
    typeof normalizePlanPath === "function" ? normalizePlanPath(p, root) || p : p;
  let jobsByPath = {};
  try {
    jobsByPath = await loadWaveJobsByPath(root, pool, waveKey);
  } catch (_) {}

  // 竞态守卫：加载期间切了项目 / 绑上了单份 job / 离开了 confirm 相 → 放弃写 DOM
  if (
    state.selectedPath !== root ||
    getBoundPlanJob(root) ||
    state.phase !== "confirm"
  ) {
    return false;
  }

  const ov = buildWaveOverview({
    path: `${waveKey}/INDEX.md`,
    allItems: pool,
    splitByPath: state.planSplitByPath || {},
    jobsByPath,
    norm,
  });
  if (!ov || !ov.planCount) return false;

  const runLocked = typeof hasActiveRun === "function" ? hasActiveRun() : false;
  const waves = $("#confirm-waves");
  if (waves) {
    waves.innerHTML = renderWaveOverviewHtml(ov, esc, { runLocked });
    delete waves.dataset.sig;
    delete waves.dataset.ccoAwaitSplit;
  }
  const titleEl = $("#confirm-title");
  if (titleEl) titleEl.textContent = `本波 · ${ov.label}`;
  const metaEl = $("#confirm-meta");
  if (metaEl) metaEl.textContent = ov.closeout || "";
  toggleWaveBackLink(null);
  try {
    stampSplitDeskProject(root);
  } catch (_) {}
  return true;
}

/**
 * 钻进本波某单份步骤时，在标题下给一个「← 本波总览」返回口，闭合多计划回路。
 * 元素完全由本模块拥有（不放进 ccoSplit 管辖的 #confirm-waves，避免 sig 冲突）。
 * @param {string|null} planPath 绑定计划路径；null = 隐藏
 * @param {() => void} [onBack] 点返回口的回调（confirmActions 传 renderConfirmPanel）
 */
export async function toggleWaveBackLink(planPath, onBack) {
  const meta = $("#confirm-meta");
  if (!meta || !meta.parentNode) return;
  let bar = $("#split-wave-back");
  let waveKey = null;
  if (planPath) {
    try {
      const { waveDirKeyFromPath } = await import("../chat/chatWavePlans.js");
      waveKey = waveDirKeyFromPath(planPath);
    } catch (_) {}
  }
  if (!waveKey) {
    if (bar) bar.hidden = true;
    return;
  }
  if (!bar) {
    bar = document.createElement("p");
    bar.id = "split-wave-back";
    bar.className = "split-wave-back muted";
    bar.innerHTML =
      `<button type="button" class="linkish" id="btn-split-wave-back">← 本波总览</button>`;
    meta.parentNode.insertBefore(bar, meta.nextSibling);
  }
  const btn = bar.querySelector("#btn-split-wave-back");
  if (btn && typeof onBack === "function") btn.onclick = () => onBack();
  bar.hidden = false;
}

/**
 * 认领/保存本波后落到拆分 tab（复用 paintWaveLanding 的多计划总览），而非跳
 * 孤立的「计划管理」页。仅选中 + 列表/索引加载 + 导航；绝不 confirm_start / start_run。
 * 波 = 多计划：先清掉任何单份绑定 job，让确认台走 paintWaveLanding 分支。
 * @param {{index_rel?:string, plan_rels?:string[]}} resp chatSaveWaveBundle 响应
 * @param {Record<string, any>} host chat/project host 袋（loadPlanItems/…/render*）
 * @returns {Promise<boolean>}
 */
export async function landWaveInSplitTab(resp, host) {
  const root = state.selectedPath;
  if (!root) return false;
  const primary =
    (resp && (resp.index_rel || resp.indexRel)) ||
    (resp && Array.isArray(resp.plan_rels) && resp.plan_rels[0]) ||
    (resp && Array.isArray(resp.planRels) && resp.planRels[0]) ||
    null;
  if (primary) {
    state.selectedPlan = primary;
    state.chatDraftPlan = primary;
  }
  // 波是多计划：清掉可能残留的单份绑定 job，否则确认台会走单份分支而非总览
  try {
    setBoundPlanJob(null, { projectPath: root });
  } catch (_) {}
  // 先索引后列表：paintWaveLanding 读 state.planItems / state.planSplitByPath
  try {
    if (typeof host?.loadPlanSplitIndex === "function") {
      await host.loadPlanSplitIndex(root);
    }
  } catch (_) {}
  try {
    if (typeof host?.loadPlanItems === "function") await host.loadPlanItems();
  } catch (_) {}
  if (primary && typeof host?.selectPlanRailItem === "function") {
    try {
      host.selectPlanRailItem(primary);
    } catch (_) {}
  }
  // 导航到拆分 tab：先落 legacy confirm 相（goSplit 见到 confirm 就不改），再重绘
  state.phase = "confirm";
  try {
    if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
      window.ccoApp.goSplit();
    } else if (typeof showPage === "function") {
      showPage("workspace");
    }
  } catch (_) {}
  try {
    if (typeof host?.renderPhasePanels === "function") host.renderPhasePanels();
  } catch (_) {}
  try {
    if (typeof host?.renderPlanPicker === "function") host.renderPlanPicker();
  } catch (_) {}
  return true;
}
