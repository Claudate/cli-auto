/**
 * [INPUT]: wave key · plan list · gateway.confirmStart / latestPlanJobForPlan
 * [OUTPUT]: W3 serial confirm batch · split-next · job map load
 * [POS]: features/chat — extracted from plansMgmt (line budget); no start_run
 * [PROTOCOL]: 同一 confirm_start；同仓默认一轮；对照 landing W3
 */

import { state, $, toast, showPage, normalizePlanPath, selectPlan, startExecuteFromSelection, hasActiveRun } from "./legacy.js";
import { host } from "./host.js";
import { isWaveIndexPath, waveDirKeyFromPath } from "./chatWavePlans.js";
import { buildWaveOverview } from "./chatWaveOverview.js";
import * as gateway from "../../shared/gateway.js";

/**
 * @param {string} root
 * @param {Array} pool
 * @param {string} waveKey
 * @returns {Promise<Record<string, object>>}
 */
export async function loadWaveJobsByPath(root, pool, waveKey) {
  /** @type {Record<string, object>} */
  const out = {};
  if (!root || !waveKey) return out;
  const plans = (pool || []).filter((it) => {
    const p = it?.path || "";
    return waveDirKeyFromPath(p) === waveKey && !isWaveIndexPath(p);
  });
  const gw =
    (typeof window !== "undefined" && window.ccoGateway) || gateway;
  if (!gw?.latestPlanJobForPlan) return out;
  const slice = plans.slice(0, 8);
  await Promise.all(
    slice.map(async (it) => {
      const p = it.path || "";
      const n =
        typeof normalizePlanPath === "function"
          ? normalizePlanPath(p, root) || p
          : p;
      try {
        const view = await gw.latestPlanJobForPlan(root, p);
        if (view) {
          out[n] = view;
          out[p] = view;
        }
      } catch (_) {}
    })
  );
  return out;
}

/**
 * Serially confirm each planned job (same confirm_start). One active run at a time.
 * @param {string} waveKey
 * @param {{ after?: () => Promise<void>|void }} [opts]
 */
export async function confirmWaveBatchSerial(waveKey, opts = {}) {
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  if (typeof hasActiveRun === "function" && hasActiveRun()) {
    toast("本轮还在执行，请等结束后再确认下一份");
    return;
  }
  const root = state.selectedPath;
  const pool = state.planRailItems || [];
  const jobsBy = await loadWaveJobsByPath(root, pool, waveKey);
  const ov = buildWaveOverview({
    path: `${waveKey}/INDEX.md`,
    allItems: pool,
    splitByPath: state.planSplitByPath || {},
    jobsByPath: jobsBy,
    norm: (p) =>
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(p, root) || p
        : p,
  });
  const ready = (ov?.rows || []).filter((r) => r.canConfirm && r.jobId);
  if (!ready.length) {
    toast("没有「已拆好」可确认的计划 · 请先逐份拆成步骤");
    return;
  }
  toast(`将串行确认 ${ready.length} 份（同一确认闸 · 不并行开跑）`);
  const gw =
    (typeof window !== "undefined" && window.ccoGateway) || gateway;
  for (let i = 0; i < ready.length; i++) {
    const row = ready[i];
    if (typeof hasActiveRun === "function" && hasActiveRun()) {
      toast(
        `已开跑中 · 本波剩余 ${ready.length - i} 份等本轮结束后再点「确认本波」`
      );
      break;
    }
    try {
      state.selectedPlan = row.path;
      state.chatDraftPlan = row.path;
      host.selectPlanRailItem?.(row.path);
      if (typeof host.tryRestorePlanJobForPlan === "function") {
        await host.tryRestorePlanJobForPlan(row.path);
      }
      await gw.confirmStart(row.jobId);
      toast(`已确认开跑 ${i + 1}/${ready.length}：${row.title}`);
      if (i === 0 && ready.length > 1) {
        toast(
          "已开跑第 1 份 · 默认同仓一次只跑一轮；结束后再点「确认本波」继续下一批"
        );
        break;
      }
    } catch (e) {
      toast(
        `确认失败（${row.title}）：${String(e?.message || e)} · 其余未动`
      );
      break;
    }
  }
  try {
    if (typeof opts.after === "function") await opts.after();
  } catch (_) {}
}

/** Open next unsplit plan on the split desk. */
export async function splitNextInWave(waveKey) {
  const root = state.selectedPath;
  const pool = state.planRailItems || [];
  const jobsBy = await loadWaveJobsByPath(root, pool, waveKey);
  const ov = buildWaveOverview({
    path: `${waveKey}/INDEX.md`,
    allItems: pool,
    splitByPath: state.planSplitByPath || {},
    jobsByPath: jobsBy,
    norm: (p) =>
      typeof normalizePlanPath === "function"
        ? normalizePlanPath(p, root) || p
        : p,
  });
  const next = (ov?.rows || []).find((r) => r.canSplit);
  if (!next) {
    toast("没有未拆的执行计划");
    return;
  }
  await assignFromPlansMgmtPath(next.path);
}

export async function assignFromPlansMgmtPath(path) {
  if (!path) {
    toast("请先选中一份计划");
    return;
  }
  if (isWaveIndexPath(path)) {
    toast("索引不能拆步 · 请选执行计划");
    return;
  }
  host.selectPlanRailItem(path);
  state.chatDraftPlan = path;
  if (typeof startExecuteFromSelection === "function") {
    await startExecuteFromSelection(path, { source: "plans" });
    return;
  }
  try {
    if (typeof selectPlan === "function") await selectPlan(path);
    showPage("workspace");
    toast("已选中计划 · 请拆成步骤");
  } catch (e) {
    toast(String(e?.message || e));
  }
}
