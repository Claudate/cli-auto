/**
 * [INPUT]: legacy host + gateway via requireGateway
 * [OUTPUT]: start_plan_job · poll · optional gate · advance (no silent auto-start past optionals)
 * [POS]: A5-2b-fin features/project/jobPoll.js
 * note: start_plan_job · poll · optional gate · advance (no silent auto-start past optionals)
 * note: P0-A persona 芯片：clarify_depth 透传 start_plan_job；grain 芯片仅在未选工作习惯时作默认
 * note: 空拆分台护栏：ownership/bind 失败→plan_failed+toast；empty tasks≠成功；cancelled 出失败面；paintConfirmDesk 强制 paint
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  isRunPaused,
  isLiveStatus,
  isFailedStatus,
  toastRunLocked,
  normalizePlanPath,
  planDisplayName,
  fillPlannerLog,
  canEditSelectedTask,
  openNativeDialog,
  loadProjects,
  renderProjectList,
  renderWorkspace,
  goHome,
  closeModal,
  openChatPage,
  stashChatSession,
  restoreChatSession,
  stopChatWaitTicker,
  loadPlanRail,
  renderPlanRail,
  selectPlanRailItem,
  renderPlansMgmtPage,
  chatAssignDirectEnabled,
  flowModeLabel,
  flowModeHint,
  flowStageStripHtml,
  flowChooserSub,
  flowJoinSeriousFun,
  flowPickBlurb,
  flowPlanHowLabel,
  flowPlanningSub,
  flowSanitizeDepsLabel,
  flowRunningMonitorTitle,
  esc,
  requireGateway,
} from "./legacy.js";
import { host } from "./host.js";
import {
  currentScopeGen,
  scopeGenStillCurrent,
  setBoundPlanJob,
  planJobBelongsToProject,
  rebindSplitToOpenProject,
} from "./projectScope.js";
import {
  suggestedMaxParallel,
  suggestedGrainHint,
  getWorkStyleId,
  getProjectWorkStyleId,
} from "../../shared/workStyle.js";
import { getChipValue } from "../chat/chatPersona.js";
import { chipGrainHintLine } from "../chat/chatPersonaSync.js";

/** Humanize planner hard-timeout / engine errors for toast + failure panel. */
function humanPlanFail(raw) {
  const s = String(raw || "").trim();
  if (!s) return "";
  if (/hard timeout|301s|planner worker did not finish/i.test(s)) {
    return "智能拆分超时未完成。可点「再拆一次」，或在选计划页「更多选项 → 规划方式」改用「快速拆分（本地规则）」。";
  }
  if (/未返回任何任务|找不到.*JSON|empty/i.test(s)) {
    return "智能拆分没有产出可用步骤。请再拆一次，或改用「快速拆分（本地规则）」。";
  }
  if (/保留上次成功的拆分|refuse heuristic cover|未用本地残图覆盖|没有可展示的完整拆分|不展示、不覆盖/i.test(s)) {
    return "智能拆分未完整完成：只展示成功结果，残图不显示、不覆盖。可再拆一次，或改用「快速拆分（本地规则）」。";
  }
  // Keep short; full text still in pp-error / planning-fail-detail
  return s.length > 160 ? s.slice(0, 158) + "…" : s;
}

/** Mode B: analyze plan → plan job (does NOT start workers). */
/** 拆成步骤：AI 拆分后进入拆分台（可编辑）；入口文案统一为「拆成步骤」
 *  @param {string} [planPathArg]  显式计划路径（聊天/执行入口必传）。
 *    禁止只信 state.selectedPlan：旧 confirm/planning 会话可能仍绑着上一份计划，
 *    而 toast 已显示新 path → 顶栏/job 与 toast 分裂（pilotdeck vs chat-*.md）。
 */
export async function analyzePlanFromPicker(planPathArg) {
  const err = $("#pp-error");
  if (err) err.hidden = true;
  if (state.assigning) return;
  if (hasActiveRun()) {
    toastRunLocked("拆成步骤");
    return;
  }
  if (!state.selectedPath) {
    toast("请先选择项目");
    return;
  }
  // Single identity for this split: arg > selectedPlan > chatDraftPlan.
  const rawPlan =
    (typeof planPathArg === "string" && planPathArg.trim()) ||
    state.selectedPlan ||
    state.chatDraftPlan ||
    null;
  const plan =
    (typeof normalizePlanPath === "function"
      ? normalizePlanPath(rawPlan, state.selectedPath)
      : null) || rawPlan;
  if (!plan) {
    host.openPlanChooser(true);
    toast("请先选择计划");
    return;
  }
  // Bind before any async work so top bar / job / toast cannot diverge mid-flight.
  state.selectedPlan = plan;
  state.chatDraftPlan = plan;

  // Product default: AI split (ModelSplitAgent). Fast/heuristic is opt-in only —
  // defaulting to fast made "拆成步骤" feel fake (instant title scrape, no model).
  // Work-style may bias concurrency seed + grain (never forces plan_mode=fast).
  // W3 smart-resplit / one-shot force: state.forcePlanModeAi wins once then clears.
  // Chat「直接执行」: forcePlanModeDirect → whole plan as one task (no multi-split).
  let planMode = $("#pp-plan-mode")?.value || "ai";
  if (state.forcePlanModeDirect) {
    planMode = "direct";
    state.forcePlanModeDirect = false;
  } else if (state.forcePlanModeAi) {
    planMode = "ai";
    state.forcePlanModeAi = false;
    const pm = $("#pp-plan-mode");
    if (pm) {
      pm.value = "ai";
      try {
        pm.dataset.touched = "1";
      } catch (_) {
        /* ignore */
      }
    }
  }
  const provider = $("#pp-provider")?.value || "claude";
  const mode = $("#pp-mode")?.value || "print";
  // Prefer confirm-screen depth (#split-effort), then chooser (#pp-effort), else config seed.
  const EFFORT_OK = ["low", "medium", "high", "xhigh", "max", "ultracode"];
  let effortRaw = (
    $("#split-effort")?.value ||
    $("#pp-effort")?.value ||
    ""
  )
    .trim()
    .toLowerCase();
  if (!EFFORT_OK.includes(effortRaw)) {
    try {
      effortRaw = (localStorage.getItem("cco.splitEffort") || "").toLowerCase();
    } catch (_) {
      effortRaw = "";
    }
  }
  const effort = EFFORT_OK.includes(effortRaw) ? effortRaw : null;
  if (effort) {
    try {
      localStorage.setItem("cco.splitEffort", effort);
    } catch (_) {}
    // Keep both pickers in sync when present.
    const se = $("#split-effort");
    const pe = $("#pp-effort");
    if (se && se.value !== effort) se.value = effort;
    if (pe && pe.value !== effort) pe.value = effort;
  }
  // Seed concurrency from work-style if pickers untouched (sync — no race with commit).
  // W4-2: pass selected project so project override can win.
  const projectPath = state.selectedPath || null;
  try {
    const mpEl = $("#chooser-max-parallel") || $("#pp-max-parallel");
    if (mpEl && !mpEl.dataset.touched) {
      mpEl.value = String(
        suggestedMaxParallel(Number(mpEl.value) || 2, projectPath)
      );
    }
  } catch (_) {}
  // Commit any in-progress concurrency edit before reading.
  const maxParallel = host.commitSplitMaxParallel($("#chooser-max-parallel") || $("#pp-max-parallel"));
  let grainHint = "";
  let clarifyDepth; // declare before use in try/catch below
  try {
    const styleChosen =
      !!getWorkStyleId() || !!getProjectWorkStyleId(projectPath);
    if (styleChosen) {
      // 有显式选择就使用
      grainHint = suggestedGrainHint(projectPath) || "";
    } else {
      // 无显式选择，用 chip 作为默认；若无 chip 则用 project 的软默认
      const chipLine = chipGrainHintLine(getChipValue("split_grain"));
      grainHint = chipLine || suggestedGrainHint(projectPath) || "";
    }
    clarifyDepth = getChipValue("clarify_depth") || null;
  } catch (_) {
    clarifyDepth = null;
  }
  const revEl0 = $("#split-revision-notes");
  const revisionNotes =
    (revEl0 && String(revEl0.value || "").trim()) || null;

  const doc = await host.ensureDoctor(true);
  if (doc && !doc.ok && provider !== "fake" && planMode !== "fake") {
    // 不硬拦死：提示 + 允许用户忽略后重试；首次仍阻止避免必败
    if (err) {
      err.textContent = "环境未就绪。可点上方「忽略」后重试，或到环境检查配置 Claude 路径";
      err.hidden = false;
    }
    host.renderDoctorWarn();
    // 若用户已忽略同类警告，允许继续
    const fails = (doc.lines || []).filter((l) => !l.ok);
    const key = fails.map((l) => l.name + ":" + l.detail).join("|");
    if (!(state.doctorDismissedKey && state.doctorDismissedKey === key)) {
      return;
    }
  }

  // Capture prior job BEFORE clearing — preserve_from gate + restore on failure
  // (replan must not wipe a previous good split off the desk when start throws).
  const prevJob = state.planJob || null;
  const prevJobId = state.planJobId || null;
  const prevJobPlan =
    state.planJob?.plan_path || state.planJob?.planPath || null;
  let preserveFrom = state.preserveFromJobId || null;
  if (
    preserveFrom &&
    prevJobPlan &&
    normalizePlanPath(prevJobPlan, state.selectedPath) !==
      normalizePlanPath(plan, state.selectedPath)
  ) {
    // Switching plans: never re-apply edits from the other plan's job.
    preserveFrom = null;
  }
  state.preserveFromJobId = null;

  host.setAssignBusy(true);
  state.phase = "planning";
  const startGen = currentScopeGen();
  const projectAtStart = state.selectedPath;
  setBoundPlanJob(null, {
    projectPath: projectAtStart,
    gen: startGen,
    keepConfirmTask: false,
  });
  state.confirmEditing = false;
  // C2：仅「新开拆分」supersede 旧 session；非旁路静默丢
  host.clearPlanSession(state.selectedPath);
  // Re-bind after clear — clearPlanSession must never wipe the path we are about to split.
  state.selectedPlan = plan;
  state.chatDraftPlan = plan;
  stopPlanJobPoll();
  host.openPlanChooser(false);
  // 规划 UI 在 workspace；从聊天/其它页分配时先切回
  if (state.page !== "workspace") showPage("workspace");
  host.renderPhasePanels();
  host.renderPlanPicker();
  host.renderWorkspaceShell();
  host.updateTopPlanInfo?.();
  const logEl0 = $("#planner-log");
  const smartSplit = planMode === "ai";
  const directExec = planMode === "direct";
  if (logEl0) {
    logEl0.dataset.sig = "";
    logEl0.innerHTML = directExec
      ? '<div class="cli-empty-ai muted">正在按整份计划准备直接执行（不拆多步）…</div>'
      : smartSplit
        ? '<div class="cli-empty-ai muted">正在智能拆分：会想依赖、并行与文件地界…</div>'
        : planMode === "fast"
          ? '<div class="cli-empty-ai muted">正在用本地规则拆分（不调用模型）…</div>'
          : '<div class="cli-empty-ai muted">正在理解计划并拆分步骤…</div>';
  }
  const sub0 = $("#planning-sub");
  if (sub0) {
    const name = planDisplayName(plan);
    const core = directExec
      ? `正在准备直接执行「${name}」…（整份计划一个窗口）`
      : smartSplit
        ? `正在智能拆分「${name}」…（会想依赖与并行，可能要几分钟 · 同时最多 ${maxParallel} 步）`
        : planMode === "fast"
          ? `正在用本地规则拆分「${name}」…（不调用模型 · 同时最多 ${maxParallel} 步）`
          : `正在拆分「${name}」…（同时最多 ${maxParallel} 步）`;
    sub0.textContent =
      typeof flowJoinSeriousFun === "function"
        ? flowJoinSeriousFun(
            core,
            typeof flowPickBlurb === "function" ? flowPickBlurb("planning", name) : ""
          )
        : core;
  }

  try {
    // Final identity assertion — never send a different path than toast/top bar.
    state.selectedPlan = plan;
    const view = await requireGateway().startPlanJob({
      req: {
        project: state.selectedPath,
        plan,
        plan_mode: planMode,
        provider,
        mode,
        max_parallel: maxParallel,
        // P2-2: re-apply confirm-screen edits from previous job (by title).
        preserve_from_job_id: preserveFrom || null,
        // W4 grain + optional revision_notes (never opens a run).
        grain_hint: grainHint || null,
        // Persona chip: clarify depth line for split prompt (none → omit).
        clarify_depth: clarifyDepth,
        revision_notes: revisionNotes,
        effort: effort || null,
      },
    });
    if (
      !scopeGenStillCurrent(startGen) ||
      state.selectedPath !== projectAtStart
    ) {
      // User switched project mid-flight — drop the stale response instead of
      // polluting the new project's state (job stays queryable via 计划列表).
      console.warn("startPlanJob response dropped: project switched mid-flight");
      return;
    }
    const bound = setBoundPlanJob(view, {
      projectPath: projectAtStart,
      gen: startGen,
      allowMissingProjectField: true,
    });
    if (!bound) {
      // Never silent-drop: user would see empty planning desk with no error.
      console.warn("startPlanJob response dropped: ownership gate", {
        jobProject: view?.project || view?.project_path || null,
        selected: projectAtStart,
        jobId: view?.job_id || view?.jobId || null,
      });
      host.setAssignBusy(false);
      state.phase = "plan_failed";
      const msg =
        "拆分结果无法绑定到当前项目（路径不一致）。请确认打开的是同一个项目文件夹后「再拆一次」。";
      if (err) {
        err.textContent = msg;
        err.hidden = false;
      }
      toast(msg);
      try {
        if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
          window.ccoApp.goSplit();
        }
      } catch (_) {}
      host.renderPhasePanels();
      host.renderPlanPicker();
      return;
    }
    if (revEl0) revEl0.value = "";
    // Job is source of truth after start — top bar + selectedPlan follow job path.
    const jobPlan =
      normalizePlanPath(view.plan_path || view.planPath, state.selectedPath) ||
      view.plan_path ||
      view.planPath ||
      plan;
    state.selectedPlan = jobPlan;
    state.chatDraftPlan = jobPlan;
    state.planStartedAt = Date.now();
    state.planPollFails = 0;
    host.stashPlanSession(state.selectedPath);
    host.updateTopPlanInfo?.();
    fillPlannerLog(view);
    rebindSplitToOpenProject();

    const status = String(view.status || "").toLowerCase();
    if (status === "planned") {
      await advancePlannedJob(view);
    } else if (status === "plan_failed") {
      // Stay on split path — never fall through to historical run/result desk.
      state.phase = "plan_failed";
      if (err) {
        err.textContent = view.error || "拆分失败";
        err.hidden = false;
      }
      toast(humanPlanFail(view.error) || "拆分失败");
      try {
        if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
          window.ccoApp.goSplit();
        }
      } catch (_) {}
      host.renderPhasePanels();
      host.renderPlanPicker();
      host.setAssignBusy(false);
    } else {
      // async AI planning — keep busy + poll until planned/failed
      state.phase = "planning";
      host.renderPhasePanels();
      startPlanJobPoll();
      // 立即拉一次，避免只显示 started 第一行就干等
      await refreshPlanJob();
    }
  } catch (e) {
    // One-shot direct flags must not stick across failures.
    state.forcePlanModeDirect = false;
    state.forceAutoStartAfterPlan = false;
    if (
      !scopeGenStillCurrent(startGen) ||
      state.selectedPath !== projectAtStart
    ) {
      // Project switched mid-flight: leave the new project's state untouched.
      console.warn("startPlanJob failed after project switch", e);
      return;
    }
    const prevStatus = String(prevJob?.status || "").toLowerCase();
    const samePlanAsPrev =
      prevJobPlan &&
      normalizePlanPath(prevJobPlan, state.selectedPath) ===
        normalizePlanPath(plan, state.selectedPath);
    if (
      prevJob &&
      prevJobId &&
      samePlanAsPrev &&
      (prevStatus === "planned" || prevStatus === "confirmed")
    ) {
      // Replan start failed → keep the previous good split on the desk
      // instead of dropping the user to an empty pick screen.
      setBoundPlanJob(prevJob, {
        projectPath: projectAtStart,
        gen: startGen,
        allowMissingProjectField: true,
      });
      state.phase = "confirm";
      host.stashPlanSession(state.selectedPath);
      rebindSplitToOpenProject();
      toast("重新规划没有发起成功，已回到上一次的拆分结果");
    } else {
      state.phase = "pick";
      toast(String(e));
    }
    if (err) {
      err.textContent = String(e);
      err.hidden = false;
    }
    host.renderPhasePanels();
    host.renderPlanPicker();
    host.setAssignBusy(false);
  }
}

export function stopPlanJobPoll() {
  // Generation bump also invalidates a tick that is currently awaiting IPC,
  // so it cannot re-schedule after stop (clearTimeout alone can't reach it).
  state.planPollGen = (state.planPollGen || 0) + 1;
  if (state.planJobPollTimer) {
    clearTimeout(state.planJobPollTimer);
    state.planJobPollTimer = null;
  }
}

export function startPlanJobPoll() {
  stopPlanJobPoll();
  const gen = state.planPollGen;
  const tick = async () => {
    if (gen !== state.planPollGen) return;
    try {
      await refreshPlanJob();
    } catch (e) {
      console.warn("plan poll", e);
    }
    if (gen !== state.planPollGen) return;
    // Self-scheduling (not setInterval): ticks never overlap, and flaky IPC
    // backs off instead of hammering — a ~5s blip no longer kills the poll.
    const delay = (state.planPollFails || 0) >= 3 ? 2500 : 600;
    state.planJobPollTimer = setTimeout(tick, delay);
  };
  state.planJobPollTimer = setTimeout(tick, 600);
}

export function planHasOptionalTasks(view) {
  const tasks = view?.tasks || [];
  return tasks.some((t) => !!t.optional);
}

export function isSystemPostTask(t) {
  if (!t) return false;
  const id = String(t.id || "");
  if (
    id === "sys-post-inspect" ||
    id === "sys-post-git-push" ||
    id === "sys-post-open-pr"
  )
    return true;
  if (id.startsWith("sys-post-")) return true;
  return String(t.group || "") === "系统收尾";
}

export function countOptionalIncluded(view) {
  const tasks = view?.tasks || [];
  return tasks.filter((t) => t.optional && t.include !== false).length;
}

/**
 * Whether confirm screen must wait for human before auto-start.
 * - Business optionals (非系统): always block（默认不跑，须人勾选）
 * - System post only（设置开启、默认勾选）: 全部 include 则可 auto-start
 */
export function planNeedsOptionalConfirm(view) {
  const tasks = view?.tasks || [];
  const businessOpt = tasks.filter((t) => !!t.optional && !isSystemPostTask(t));
  if (businessOpt.length > 0) return true;
  const sysOpt = tasks.filter((t) => !!t.optional && isSystemPostTask(t));
  if (!sysOpt.length) return false;
  // 系统收尾有未勾选 → 仍停一下让用户看到；全勾选则不挡 auto-start
  return sysOpt.some((t) => t.include === false);
}

export async function advancePlannedJob(view) {
  // Idempotent per job: overlapping poll ticks / double entry must not advance
  // (and possibly auto-start via confirmAndStart) the same planned job twice.
  const advJobId = view?.job_id || view?.jobId || null;
  if (advJobId && state.planAdvancedJobId === advJobId) return;
  const path = state.selectedPath;
  // Never advance a foreign project's graph into the open project's desk.
  if (
    path &&
    view &&
    (view.project || view.project_path || view.projectPath) &&
    !planJobBelongsToProject(view, path)
  ) {
    console.warn("advancePlannedJob skipped: foreign project", {
      jobProject: view.project || view.project_path || view.projectPath,
      selected: path,
      jobId: advJobId,
    });
    // Do NOT mark planAdvancedJobId — a later path fix / restore can retry.
    toast("拆分完成，但结果不属于当前打开的项目，未显示在拆分台");
    return;
  }

  const taskN = Array.isArray(view?.tasks) ? view.tasks.length : 0;
  const metaN = Number(view?.task_count || view?.taskCount || 0) || 0;
  // planned with empty tasks = incomplete payload (not success). Showing confirm
  // with「暂无步骤」looks like a silent blank desk.
  if (taskN === 0) {
    stopPlanJobPoll();
    host.setAssignBusy(false);
    setBoundPlanJob(view, {
      projectPath: path,
      allowMissingProjectField: true,
      keepConfirmTask: true,
    });
    state.phase = "plan_failed";
    const msg =
      metaN > 0
        ? `拆分标记完成（${metaN} 步）但步骤明细未加载到界面。请点「再拆一次」，或稍后在计划列表回看。`
        : "拆分完成但没有可展示的步骤。请再拆一次，或改用「快速拆分（本地规则）」。";
    const err = $("#pp-error") || $("#planning-fail-detail");
    if (err) {
      err.textContent = msg;
      err.hidden = false;
    }
    toast(msg);
    try {
      if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
        window.ccoApp.goSplit();
      }
    } catch (_) {}
    host.renderPhasePanels();
    host.renderPlanPicker();
    // Only mark advanced after we surfaced failure so a retry can re-enter if needed.
    if (advJobId) state.planAdvancedJobId = advJobId;
    return;
  }

  stopPlanJobPoll();
  const ok = setBoundPlanJob(view, {
    projectPath: path,
    allowMissingProjectField: true,
    keepConfirmTask: !!state.confirmTaskId,
  });
  if (!ok) {
    // Must not mark advanced or leave busy/planning empty — user sees nothing.
    host.setAssignBusy(false);
    state.phase = "plan_failed";
    const msg =
      "拆分已完成，但无法绑定到当前项目显示。请确认项目文件夹后「再拆一次」。";
    toast(msg);
    const err = $("#pp-error");
    if (err) {
      err.textContent = msg;
      err.hidden = false;
    }
    try {
      if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
        window.ccoApp.goSplit();
      }
    } catch (_) {}
    host.renderPhasePanels();
    host.renderPlanPicker();
    return;
  }
  // Mark advanced only after a successful bind so a failed gate can retry.
  if (advJobId) state.planAdvancedJobId = advJobId;
  if (!state.confirmTaskId && view.tasks?.length) {
    state.confirmTaskId = view.tasks[0].id;
  }
  host.stashPlanSession(state.selectedPath);
  host.updateBgPlanBanner();
  // E2：拆分完成必须回到执行面，禁止只 toast「请返回确认」而人还在 chat/plans
  if (state.page !== "workspace") {
    showPage("workspace");
  }
  const n = taskN || metaN;
  const adapter = view.adapter || "";
  const mode = String(view.plan_mode || view.planMode || "").toLowerCase();
  const isDirect = mode === "direct" || adapter === "raw-single";
  const how = isDirect
    ? "直接执行"
    : typeof flowPlanHowLabel === "function"
      ? flowPlanHowLabel(adapter)
      : adapter.includes("heuristic")
        ? "本地规则拆分"
        : adapter.includes("llm")
          ? "智能拆分"
          : "拆分完成";
  // 业务可选：必须人工确认。系统收尾默认勾选时可 auto-start。
  // 一次 shot：聊天「直接执行」设 forceAutoStartAfterPlan（用完即清）。
  const forceAuto = !!state.forceAutoStartAfterPlan;
  if (forceAuto) state.forceAutoStartAfterPlan = false;
  const needsOpt = planNeedsOptionalConfirm(view);
  const hasOptional = planHasOptionalTasks(view);
  const wantAuto = (forceAuto || state.autoStartAfterPlan) && !needsOpt;

  const paintConfirmDesk = () => {
    state.phase = "confirm";
    try {
      if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
        window.ccoApp.goSplit();
      }
    } catch (_) {}
    try {
      host.renderWorkspaceShell();
    } catch (_) {}
    rebindSplitToOpenProject();
    host.renderPhasePanels();
    if (typeof host.renderConfirmPanel === "function") {
      host.renderConfirmPanel();
    }
    host.renderPlanPicker();
    host.setAssignBusy(false);
  };

  if (wantAuto) {
    toast(
      isDirect
        ? "正在按整份计划直接启动…"
        : `${how}：${n} 个任务，正在启动…`
    );
    paintConfirmDesk();
    await host.confirmAndStart();
  } else {
    const optHint = needsOpt
      ? "；含可选项，请确认勾选后再开始"
      : hasOptional
        ? "；含系统收尾（默认已勾选）"
        : "，请确认后开始";
    toast(
      isDirect
        ? `已准备直接执行（1 个主任务）${optHint}`
        : `${how}：${n} 个任务${optHint}`
    );
    paintConfirmDesk();
  }
}

export async function refreshPlanJob() {
  if (!state.planJobId) return;
  // Callers overlap (poll chain + shellBoot tick + manual refresh) — collapse.
  if (state.planPollInFlight) return;
  state.planPollInFlight = true;
  const jobId = state.planJobId;
  const gen = currentScopeGen();
  const pathAtStart = state.selectedPath;
  try {
    const view = await requireGateway().getPlanJob(jobId);
    // Stale response: job replaced/cleared (project or plan switched) mid-flight.
    if (jobId !== state.planJobId) return;
    if (!scopeGenStillCurrent(gen) || state.selectedPath !== pathAtStart) return;
    // Cross-project bleed: IPC returned a job that is not for the open project.
    if (
      state.selectedPath &&
      (view.project || view.project_path || view.projectPath) &&
      !planJobBelongsToProject(view, state.selectedPath)
    ) {
      return;
    }
    state.planPollFails = 0;
    const bound = setBoundPlanJob(view, {
      projectPath: pathAtStart,
      gen,
      allowMissingProjectField: true,
      keepConfirmTask: true,
    });
    if (!bound) {
      // Mid-poll bind fail must not leave planning spinner with a blank desk.
      if (state.phase === "planning" || state.phase === "confirm") {
        host.setAssignBusy(false);
        state.phase = "plan_failed";
        const msg =
          "拆分进度已返回，但无法绑定到当前项目显示。请确认项目后「再拆一次」。";
        toast(msg);
        const err = $("#pp-error");
        if (err) {
          err.textContent = msg;
          err.hidden = false;
        }
        try {
          if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
            window.ccoApp.goSplit();
          }
        } catch (_) {}
        host.renderPhasePanels();
        host.renderPlanPicker();
      }
      return;
    }
    const status = String(view.status || "").toLowerCase();
    fillPlannerLog(view);

    if (status === "planned") {
      await advancePlannedJob(view);
    } else if (status === "plan_failed") {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      // Only complete success is shown on desk. Failed residual must not stick.
      // Prefer prior planned/confirmed for this plan; else fail panel with no fake graph.
      const planPath =
        view.plan_path || view.planPath || state.selectedPlan || null;
      let restored = false;
      if (planPath && state.selectedPath) {
        try {
          const prior = await requireGateway().latestPlanJobForPlan(
            state.selectedPath,
            planPath
          );
          if (!scopeGenStillCurrent(gen)) return;
          const pst = String(prior?.status || "").toLowerCase();
          const priorId = prior?.job_id || prior?.jobId || null;
          const failedId = state.planJobId;
          const n = prior?.task_count || prior?.tasks?.length || 0;
          if (
            prior &&
            priorId &&
            priorId !== failedId &&
            (pst === "planned" || pst === "confirmed") &&
            n > 0
          ) {
            if (typeof host.applyRestoredPlanJob === "function") {
              restored = !!host.applyRestoredPlanJob(prior, {
                resumePoll: false,
              });
            } else if (typeof window.applyRestoredPlanJob === "function") {
              restored = !!window.applyRestoredPlanJob(prior, {
                resumePoll: false,
              });
            }
          }
        } catch (e) {
          console.warn("restore prior split after plan_failed", e);
        }
      }
      if (restored) {
        toast(
          humanPlanFail(view.error) ||
            "智能拆分未完整完成 · 已回到上次成功的拆分"
        );
      } else {
        state.phase = "plan_failed";
        setBoundPlanJob(view, {
          projectPath: pathAtStart,
          gen,
          allowMissingProjectField: true,
          keepConfirmTask: true,
        });
        const err = $("#pp-error");
        if (err) {
          err.textContent = view.error || "拆分失败";
          err.hidden = false;
        }
        toast(humanPlanFail(view.error) || "拆分失败 · 没有可展示的完整结果");
      }
      try {
        if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
          window.ccoApp.goSplit();
        }
      } catch (_) {}
      host.renderPhasePanels();
      host.renderPlanPicker();
      if (restored && typeof host.renderConfirmPanel === "function") {
        host.renderConfirmPanel();
      }
    } else if (status === "planning") {
      state.phase = "planning";
      // 超时保护：超过 12 分钟仍 planning
      if (state.planStartedAt && Date.now() - state.planStartedAt > 12 * 60 * 1000) {
        stopPlanJobPoll();
        host.setAssignBusy(false);
        state.phase = "plan_failed";
        toast(
          "拆分等了 12 分钟还没结果，先停在这里。后台可能仍在继续，稍后可在计划列表回看；也可点「再拆一次」，或改用「快速拆分（本地规则）」。"
        );
        host.renderPhasePanels();
        host.renderPlanPicker();
        return;
      }
      const sub = $("#planning-sub");
      if (sub) {
        const elapsed = state.planStartedAt
          ? Math.round((Date.now() - state.planStartedAt) / 1000)
          : 0;
        const mode =
          view.plan_mode ||
          view.planMode ||
          $("#pp-plan-mode")?.value ||
          "ai";
        sub.textContent =
          typeof flowPlanningSub === "function"
            ? flowPlanningSub(elapsed, mode)
            : `正在拆分计划步骤（已等待 ${elapsed}s）…`;
      }
      host.renderPhasePanels();
    } else if (status === "confirmed" && (view.run_id || view.runId)) {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      state.phase = "running";
      host.renderPhasePanels();
    } else if (status === "cancelled") {
      // Superseded/cancelled job must not leave planning spinner forever with empty desk.
      stopPlanJobPoll();
      host.setAssignBusy(false);
      const errText = String(view.error || "").trim();
      const superseded = /superseded/i.test(errText);
      if (superseded) {
        // Newer job should already own planJobId; if we still point here, surface fail.
        state.phase = "plan_failed";
        const msg =
          "这次拆分已被更新的一次取代。请重新点「拆成步骤」，或到计划列表打开最新结果。";
        toast(msg);
        const err = $("#pp-error");
        if (err) {
          err.textContent = msg;
          err.hidden = false;
        }
      } else {
        state.phase = "plan_failed";
        const msg = errText
          ? humanPlanFail(errText) || errText
          : "拆分已取消，没有可展示的步骤。";
        toast(msg);
        const err = $("#pp-error");
        if (err) {
          err.textContent = msg;
          err.hidden = false;
        }
      }
      try {
        if (window.ccoApp && typeof window.ccoApp.goSplit === "function") {
          window.ccoApp.goSplit();
        }
      } catch (_) {}
      host.renderPhasePanels();
      host.renderPlanPicker();
    } else {
      host.renderPhasePanels();
    }
  } catch (e) {
    // Stale failure (job replaced mid-flight) must not count or re-render.
    if (jobId !== state.planJobId) return;
    state.planPollFails = (state.planPollFails || 0) + 1;
    console.warn("refreshPlanJob", e);
    if (state.planPollFails === 1 || state.planPollFails % 5 === 0) {
      toast(`拆分进度暂时读不到，正在重试…`);
    }
    // 持续失败（含退避约 15s+）才放弃；不回 pick 空态，留在失败面可「再拆一次」
    if (state.planPollFails >= 8) {
      stopPlanJobPoll();
      host.setAssignBusy(false);
      state.phase = "plan_failed";
      toast(
        "一直读不到拆分进度，先停在这里。后台可能仍在拆：稍后可在计划列表回看，或点「再拆一次」。"
      );
      host.renderPhasePanels();
      host.renderPlanPicker();
    }
  } finally {
    state.planPollInFlight = false;
  }
}
