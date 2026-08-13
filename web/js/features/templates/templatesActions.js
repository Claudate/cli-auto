/**
 * [INPUT]: catalog · splitSummary · templatesApi · window state/host helpers
 * [OUTPUT]: applyPlanTemplate · writeSplitSummaryToPlan · refreshSplitWritebackBtn
 * [POS]: P-ship-D features/templates/templatesActions.js
 * note: 不 confirm / 不开跑；写回默认关（须点击）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { planTemplateById } from "./catalog.js";
import {
  buildSplitSummaryBlock,
  mergeSplitSummaryIntoMarkdown,
} from "./splitSummary.js";
import * as templatesApi from "./templatesApi.js";
import { confirmDialog } from "../../shared/confirmDialog.js";

function g(name) {
  return typeof window !== "undefined" ? window[name] : undefined;
}

function call(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

function toast(msg) {
  if (typeof g("toast") === "function") call("toast", msg);
  else console.warn("[templates]", msg);
}

function stateObj() {
  return typeof window !== "undefined" && window.state ? window.state : {};
}

/**
 * T1/T2: write template markdown under plans/, bind draft, open for edit.
 * Does not start plan job / confirm_start.
 */
export async function applyPlanTemplate(templateId) {
  const tpl = planTemplateById(templateId);
  if (!tpl) {
    toast("未知模板");
    return null;
  }
  const state = stateObj();
  if (!state.selectedPath) {
    // C4: stash pending template → add folder modal → resume after select
    try {
      sessionStorage.setItem("cco.pendingPlanTemplate", String(templateId));
    } catch (_) {}
    toast("先选一个工作文件夹，添加后会自动套用模板");
    call("openModal");
    return null;
  }
  if (typeof g("hasActiveRun") === "function" && call("hasActiveRun")) {
    if (typeof g("toastRunLocked") === "function") call("toastRunLocked", "使用模板");
    else toast("有任务在跑，稍后再用模板");
    return null;
  }

  const plansDir =
    typeof g("getPlansDir") === "function" ? call("getPlansDir") : "plans";
  const sessionId =
    (state.chatSession && state.chatSession.session_id) || "default";

  try {
    call("ensureChatState");
    const resp = await templatesApi.savePlan({
      project: state.selectedPath,
      markdown: tpl.markdown,
      sessionId,
      title: tpl.title,
      planRel: null,
      plansDir,
    });
    const path = resp.plan_rel;
    state.chatDraftPlan = path;
    state.chatProjectPath = state.selectedPath;
    if (state.chatSession) {
      state.chatSession.draft_plan = {
        path,
        saved: true,
        markdown: tpl.markdown,
        title: tpl.title,
      };
    }
    call("stashChatSession", state.selectedPath);
    try {
      if (typeof g("loadPlansForPicker") === "function") {
        await call("loadPlansForPicker");
      }
    } catch (_) {}
    try {
      if (typeof g("loadPlanRail") === "function") await call("loadPlanRail");
    } catch (_) {}
    if (typeof g("selectPlan") === "function") {
      await Promise.resolve(call("selectPlan", path, { keepSession: true })).catch(
        () => {}
      );
    }
    call("showPage", "chat");
    call("renderChatPage");
    if (typeof g("openPlanFullView") === "function") {
      await Promise.resolve(call("openPlanFullView", path)).catch(() => {});
    }
    toast(`已落盘：${path} · 可改后点「拆成步骤」`);
    return resp;
  } catch (e) {
    toast(String(e?.message || e));
    return null;
  }
}

/** Enable/disable optional writeback CTA on split desk (from renderConfirmPanel). */
export function refreshSplitWritebackBtn(runLocked, editing) {
  const btn = document.getElementById("btn-split-writeback");
  if (!btn) return;
  const state = stateObj();
  const hasJob = !!(state.planJob && (state.planJob.tasks || []).length);
  btn.disabled = !!runLocked || !!editing || !hasJob || !state.selectedPath;
  btn.hidden = false;
  btn.title = runLocked
    ? "运行中不可写回"
    : "把步骤标题追加到计划文末（不覆盖正文；需点击确认）";
}

/**
 * S14: optional CTA — write step titles to plan end; default off (must click).
 * Does not overwrite user prose; only replaces previous cco-split-summary block.
 */
export async function writeSplitSummaryToPlan() {
  const state = stateObj();
  const job = state.planJob;
  if (!job || !state.selectedPath) {
    toast("当前没有可写回的拆分结果");
    return;
  }
  if (typeof g("hasActiveRun") === "function" && call("hasActiveRun")) {
    if (typeof g("toastRunLocked") === "function") {
      call("toastRunLocked", "写回步骤摘要");
    } else toast("运行中不可写回");
    return;
  }
  const planPath =
    job.plan_path ||
    job.planPath ||
    state.selectedPlan ||
    state.chatDraftPlan;
  if (!planPath) {
    toast("找不到计划文件路径");
    return;
  }
  const rel =
    typeof g("normalizePlanPath") === "function"
      ? call("normalizePlanPath", planPath) || planPath
      : planPath;

  try {
    let existing = "";
    try {
      existing = await templatesApi.readPlanMd(state.selectedPath, rel);
    } catch (e) {
      toast(`读取计划失败：${e?.message || e}`);
      return;
    }
    const block = buildSplitSummaryBlock(job);
    const next = mergeSplitSummaryIntoMarkdown(existing, block);
    if (next === existing) {
      toast("摘要无变化");
      return;
    }
    const ok = await confirmDialog({
      title: "写回步骤摘要",
      body: `将把步骤标题清单写到计划文末（不覆盖正文）：\n${rel}`,
      okLabel: "写回",
    });
    if (!ok) return;

    await templatesApi.savePlan({
      project: state.selectedPath,
      markdown: next,
      sessionId:
        (state.chatSession && state.chatSession.session_id) || "default",
      title: null,
      planRel: rel,
      plansDir: null,
    });
    if (state.chatDraftPlan === rel && state.chatSession?.draft_plan) {
      state.chatSession.draft_plan.markdown = next;
    }
    toast(`已写回步骤摘要 → ${rel}`);
  } catch (e) {
    toast(String(e?.message || e));
  }
}
