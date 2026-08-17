/**
 * [INPUT]: classic globals on window + ccoRun/ccoResult/ccoSettings intentions
 * [OUTPUT]: UI_ACTIONS 表 — id/data-action → 意图（无 invoke）
 * [POS]: A5-2d features/settings；事件表只绑意图
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

function toast(msg) {
  if (typeof window.toast === "function") window.toast(msg);
}

function g(name) {
  return typeof window !== "undefined" ? window[name] : undefined;
}

function call(name, ...args) {
  const fn = g(name);
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

function state() {
  return g("state");
}

function $(sel) {
  return typeof window.$ === "function" ? window.$(sel) : document.querySelector(sel);
}

function $$ (sel) {
  return typeof window.$$ === "function"
    ? window.$$(sel)
    : Array.from(document.querySelectorAll(sel));
}

/**
 * Intent table only — all calls stay behind the shared presentation bridge.
 * Settings/doctor/monitor go through ccoSettings / ccoRun when present.
 */
export function createUiActions() {
  return {
    "btn-add-plus": () => call("openModal"),
    "btn-welcome-add": () => call("openModal"),
    "btn-welcome-add2": () => call("openModal"),
    "btn-welcome-help": () => call("showPage", "help"),
    "btn-split-writeback": () =>
      typeof g("writeSplitSummaryToPlan") === "function"
        ? call("writeSplitSummaryToPlan")
        : toast("写回不可用"),
    // shell-chrome A2：设置高级 · 拆分台工具（与 hidden 的调整… 同 handler）
    "btn-settings-sanitize-deps": () => {
      if (typeof g("sanitizeDepsFromConfirm") !== "function") {
        return toast("当前不可用");
      }
      const st = state();
      if (!st?.planJob) {
        return toast("请先拆成步骤，再使用此工具");
      }
      if (st.page !== "workspace" || st.phase !== "confirm") {
        if (typeof g("showSplitPlanConfirm") === "function") {
          call("showSplitPlanConfirm");
        } else if (st.page !== "workspace") {
          call("showPage", "workspace");
        }
      }
      return call("sanitizeDepsFromConfirm");
    },
    "btn-settings-split-writeback": () => {
      if (typeof g("writeSplitSummaryToPlan") !== "function") {
        return toast("写回不可用");
      }
      const st = state();
      if (!st?.planJob) {
        return toast("请先拆成步骤，再写回步骤摘要");
      }
      return call("writeSplitSummaryToPlan");
    },
    "btn-refresh": async () => {
      const st = state();
      if (st?.page === "workspace" && st.selectedPath) {
        await call("loadProjects");
        if (st.phase === "planning" && st.planJobId) {
          await Promise.resolve(call("refreshPlanJob")).catch(() => {});
        }
        await call("loadLive");
        await Promise.resolve(call("loadPlansForPicker")).catch(() => {});
        if (typeof g("isPlanSessionActive") === "function" && !call("isPlanSessionActive")) {
          const proj = (st.projects || []).find((p) => p.path === st.selectedPath);
          const raw =
            st.live?.plan_path ||
            proj?.default_plan ||
            proj?.last_plan ||
            st.selectedPlan;
          const cand =
            (typeof g("normalizePlanPath") === "function"
              ? call("normalizePlanPath", raw)
              : null) || raw;
          if (cand) await Promise.resolve(call("selectPlan", cand, { keepSession: true })).catch(() => {});
          else call("updateTopPlanInfo");
        } else {
          call("renderPhasePanels");
          call("updateTopPlanInfo");
        }
      } else {
        if (st?.planJobId && st.phase === "planning") {
          await Promise.resolve(call("refreshPlanJob")).catch(() => {});
        }
        await call("loadProjects");
      }
      toast("已刷新");
    },
    "modal-close": () => call("closeModal"),
    "modal-backdrop": () => call("closeModal"),
    "m-pick-folder": () => call("pickFolderToModal"),
    "m-confirm-project": () => call("addProjectFromModal"),
    "m-cancel-project": () => call("closeModal"),
    "btn-ws-stop-all": () =>
      window.ccoRun?.stopAll ? window.ccoRun.stopAll() : call("stopAll"),
    "btn-open-monitor-window": () => {
      if (window.ccoRun?.openMonitorWindow) {
        return window.ccoRun.openMonitorWindow({});
      }
      if (window.ccoSettings?.openMonitorWindow) {
        return window.ccoSettings.openMonitorWindow();
      }
      return typeof g("openMonitorWindow") === "function"
        ? call("openMonitorWindow")
        : toast("独立监视窗不可用");
    },
    "btn-ws-resume": () =>
      window.ccoRun?.resume ? window.ccoRun.resume() : call("resumeRun"),
    // 日志栏「继续」= 原结果台继续
    "btn-log-resume": () =>
      window.ccoRun?.resume ? window.ccoRun.resume() : call("resumeRun"),
    "btn-ws-rework": () =>
      window.ccoResult?.startRework
        ? window.ccoResult.startRework()
        : call("startReworkWave"),
    "btn-ws-accept-residual": () =>
      window.ccoResult?.acceptResidual
        ? window.ccoResult.acceptResidual()
        : call("acceptRunResidual"),
    "btn-export-log-md": () =>
      typeof g("exportBoardLogsMd") === "function"
        ? call("exportBoardLogsMd")
        : toast("导出不可用"),
    "btn-open-handoff": () =>
      typeof g("openHandoffLedger") === "function"
        ? call("openHandoffLedger")
        : toast("打开账本不可用"),
    "btn-ws-back-chat": () =>
      typeof g("openChatPage") === "function"
        ? call("openChatPage")
        : call("showPage", "chat"),
    "btn-ws-finish": () =>
      window.ccoResult?.finishRound
        ? window.ccoResult.finishRound()
        : typeof g("finishRunRound") === "function"
          ? call("finishRunRound")
          : toast("结束本轮不可用"),
    // 日志栏「结束计划」= 结束本轮
    "btn-log-end-plan": () =>
      window.ccoResult?.finishRound
        ? window.ccoResult.finishRound()
        : typeof g("finishRunRound") === "function"
          ? call("finishRunRound")
          : toast("结束本轮不可用"),
    "btn-remove-project": () => call("removeSelectedProject"),
    "btn-ws-dismiss-run": () => call("dismissRun"),
    "btn-task-dash-toggle": () => {
      if (window.ccoRun?.toggleDash) {
        return window.ccoRun.toggleDash();
      }
      const st = state();
      if (!st) return;
      st.taskDashCollapsed = !st.taskDashCollapsed;
      localStorage.setItem(
        "cco.taskDashCollapsed",
        st.taskDashCollapsed ? "1" : "0"
      );
      const tasks = st.live?.tasks || [];
      call("renderTaskStrip", st.live, tasks, {
        hasRun: !!st.live?.run_id,
        active: call("isLiveStatus", st.live?.run_status),
        finished: !!st.live?.run_id && !call("isLiveStatus", st.live?.run_status),
        runStatus: st.live?.run_status,
      });
    },
    "btn-chooser-assign": () => call("analyzePlanFromPicker"),
    "btn-stop-task": () =>
      window.ccoRun?.stopTask
        ? window.ccoRun.stopTask(state()?.selectedTaskId)
        : call("cancelTask"),

    "btn-pp-scan": async () => {
      await call("loadPlansForPicker");
      call("renderPlanChooser");
    },
    "btn-pp-pick": () => call("pickPlanFileForPicker"),
    "btn-pp-pick-empty": () => call("pickPlanFileForPicker"),
    "btn-chooser-scan": async () => {
      await call("loadPlansForPicker");
      call("renderPlanChooser");
    },
    "btn-chooser-pick": () => call("pickPlanFileForPicker"),
    "btn-chooser-close": () => call("openPlanChooser", false),
    "btn-plan-choose": async () => {
      call("openPlanChooser", true, { expandList: true });
      try {
        await call("loadPlansForPicker");
        call("renderPlanChooser");
        call("updateChooserAssignState");
      } catch (e) {
        toast(String(e));
        call("renderPlanChooser");
        call("updateChooserAssignState");
      }
    },
    "btn-pp-analyze": async () => {
      const st = state();
      if (st?.selectedPlan && typeof g("startExecuteFromSelection") === "function") {
        return call("startExecuteFromSelection", st.selectedPlan, {
          source: "topbar",
        });
      }
      call("openPlanChooser", true, { expandList: true });
      try {
        await call("loadPlansForPicker");
        call("renderPlanChooser");
        call("updateChooserAssignState");
      } catch (e) {
        toast(String(e));
        call("renderPlanChooser");
        call("updateChooserAssignState");
      }
    },
    "btn-chooser-toggle-list": () => {
      const st = state();
      if (typeof g("setChooserListExpanded") === "function") {
        call("setChooserListExpanded", !st?.chooserListExpanded);
      } else if (st) {
        st.chooserListExpanded = !st.chooserListExpanded;
        if (typeof g("renderPlanChooser") === "function") call("renderPlanChooser");
      }
    },
    "btn-pp-set-default": () => call("setDefaultPlan"),
    "btn-confirm-start": () => call("confirmAndStart"),
    "btn-sanitize-deps": () =>
      typeof g("sanitizeDepsFromConfirm") === "function"
        ? call("sanitizeDepsFromConfirm")
        : null,
    "btn-enable-post-inspect": () =>
      typeof g("enablePostInspectAndResplit") === "function"
        ? call("enablePostInspectAndResplit")
        : null,
    "btn-enable-planner-critic": () =>
      typeof g("enablePlannerCriticAndResplit") === "function"
        ? call("enablePlannerCriticAndResplit")
        : null,
    "btn-replan": () => call("replanFromConfirm"),
    "btn-confirm-back": () => call("backFromConfirmToMonitor"),
    "btn-confirm-edit": () => call("beginConfirmEdit"),
    "btn-confirm-delete": () =>
      typeof g("deleteConfirmTask") === "function"
        ? call("deleteConfirmTask")
        : toast("删除不可用"),
    "btn-confirm-edit-cancel": () => call("cancelConfirmEdit"),
    "btn-confirm-edit-save": () => call("saveConfirmEdit"),
    "split-plan-chip": () => call("showSplitPlanConfirm"),
    // shell-chrome A3：顶栏编辑任务已撤；步骤编辑在拆分台详情
    "btn-cancel-planning": () => call("cancelPlanning"),
    "btn-retry-planning": () => {
      // 拆分失败后主路径：再拆一次（不进历史执行台）
      if (typeof g("analyzePlanFromPicker") === "function") {
        return call("analyzePlanFromPicker");
      }
      if (typeof g("assignFromChat") === "function") {
        return call("assignFromChat");
      }
      return null;
    },
    "btn-plan-expand": () => call("openPlanChooser", true),
    "btn-restore-panels": () => {
      const st = state();
      if (st) st.closedPanels = {};
      call("renderCliBoard", st?.live?.tasks || []);
    },
    "btn-doctor-dismiss": () => {
      if (window.ccoSettings?.dismissDoctorWarn) {
        return window.ccoSettings.dismissDoctorWarn();
      }
      return call("dismissDoctorWarn");
    },
    "btn-task-expand": () => {
      const st = state();
      if (!st) return;
      st.taskStripExpanded = !st.taskStripExpanded;
      localStorage.setItem(
        "cco.taskStripExpanded",
        st.taskStripExpanded ? "1" : "0"
      );
      const tasks = st.live?.tasks || [];
      call("renderTaskStrip", st.live, tasks, {
        hasRun: !!st.live?.run_id,
        active: call("isLiveStatus", st.live?.run_status),
        finished: !!st.live?.run_id && !call("isLiveStatus", st.live?.run_status),
        runStatus: st.live?.run_status,
      });
    },
    "btn-cli-h-auto": () => {
      call("applyCliBodyHeight", "auto");
      const tasks = state()?.live?.tasks || [];
      if (tasks.length) call("renderCliBoard", tasks);
      toast("CLI 高度已恢复自适应");
    },
    /** 日志高级工具（视图/字号/导出/自适应高度）：默认收起，点「工具」展开 */
    "btn-log-advanced-toggle": () => {
      const adv = document.getElementById("log-advanced");
      if (!adv) return;
      const show = !!adv.hidden;
      adv.hidden = !show;
      adv.setAttribute("aria-hidden", show ? "false" : "true");
      const body = adv.querySelector(".log-advanced-body");
      if (body) body.hidden = !show;
      const btn = document.getElementById("btn-log-advanced-toggle");
      if (btn) btn.classList.toggle("active", show);
    },
    "btn-copy-log": async () => {
      const st = state();
      const t =
        (st?.live?.tasks || []).find((x) => x.task_id === st.selectedTaskId) ||
        (st?.live?.tasks || [])[0];
      const text =
        typeof g("aiLogPlainText") === "function" ? call("aiLogPlainText", t) : "";
      await navigator.clipboard.writeText(text || "");
      toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
    },
    /** 顶栏 hidden #btn-rerun：整轮重选计划（非卡片 data-rerun）。 */
    "btn-rerun": () => {
      const st = state();
      if (!st) return;
      st.phase = "pick";
      st.planJobId = null;
      st.planJob = null;
      st.closedPanels = {};
      st.taskDashCollapsed = false;
      localStorage.setItem("cco.taskDashCollapsed", "0");
      call("renderPhasePanels");
      call("renderPlanPicker");
      if (st.selectedPlan) return call("analyzePlanFromPicker");
      call("openPlanChooser", true);
      toast("请先选择计划");
    },
    "btn-change-plan": () => {
      // 已移除「换计划」入口；保留 id 防旧调用
    },
    "btn-doctor-recheck": async () => {
      if (window.ccoSettings?.ensureDoctor) {
        await window.ccoSettings.ensureDoctor(true);
      } else {
        await call("ensureDoctor", true);
      }
      toast(state()?.doctorCache?.ok ? "环境正常" : "仍有问题，请查看详情");
    },
    "btn-doctor-open": async () => {
      call("showPage", "doctor");
      if (window.ccoSettings?.loadDoctor) await window.ccoSettings.loadDoctor();
      else await call("loadDoctor");
    },
    "btn-open-doctor": async () => {
      call("showPage", "doctor");
      if (window.ccoSettings?.loadDoctor) await window.ccoSettings.loadDoctor();
      else await call("loadDoctor");
    },
    "btn-doctor": () =>
      window.ccoSettings?.loadDoctor
        ? window.ccoSettings.loadDoctor()
        : call("loadDoctor"),
    "btn-doctor-back": () =>
      window.ccoSettings?.backFromSubpage
        ? window.ccoSettings.backFromSubpage()
        : call("backFromSubpage"),
    "btn-open-settings": async () => {
      call("showPage", "settings");
      if (window.ccoSettings?.loadSettings) await window.ccoSettings.loadSettings();
      else await call("loadSettings");
    },
    "btn-settings-save": () =>
      window.ccoSettings?.saveSettings
        ? window.ccoSettings.saveSettings()
        : call("saveSettings"),
    "btn-permission-restore": () =>
      window.ccoSettings?.restoreRecommendedPermission
        ? window.ccoSettings.restoreRecommendedPermission()
        : call("restoreRecommendedPermission"),
    "btn-github-status-refresh": async () => {
      if (window.ccoSettings?.refreshGithubStatus) {
        await window.ccoSettings.refreshGithubStatus();
      } else {
        await call("refreshGithubStatus");
      }
      toast("已重新检查 GitHub 发布状态");
    },
    "btn-settings-back": () =>
      window.ccoSettings?.backFromSubpage
        ? window.ccoSettings.backFromSubpage()
        : call("backFromSubpage"),
    "btn-open-help": () => call("showPage", "help"),
    "btn-help-back": () =>
      window.ccoSettings?.backFromSubpage
        ? window.ccoSettings.backFromSubpage()
        : call("backFromSubpage"),
    "brand-home": () => call("goHome"),
    "btn-open-chat": () => call("openChatPage"),
    "btn-monitor-plan": () => call("goToPlanMonitor"),
    "btn-plan-mgmt": () =>
      typeof g("openPlanManagement") === "function"
        ? call("openPlanManagement")
        : call("showPage", "plans"),
    // 聊天右栏已撤；旧 toggle 入口 → 计划管理页
    "btn-chat-rail-toggle": () =>
      typeof g("openPlanManagement") === "function"
        ? call("openPlanManagement")
        : typeof g("toggleChatPlanRail") === "function"
          ? call("toggleChatPlanRail")
          : call("showPage", "plans"),
    "btn-plans-refresh": () =>
      typeof g("loadPlanRail") === "function" ? call("loadPlanRail") : null,
    "btn-plans-to-chat": () =>
      typeof g("openChatPage") === "function"
        ? call("openChatPage")
        : call("showPage", "chat"),
    // 选中文件夹 → 加载夹内计划列表
    "btn-plans-pick-folder": () =>
      typeof g("pickPlansFolderForMgmt") === "function"
        ? call("pickPlansFolderForMgmt")
        : null,
    // 选中文件 → 加载到列表并选中
    "btn-plans-pick-file": () =>
      typeof g("pickPlanFileForMgmt") === "function"
        ? call("pickPlanFileForMgmt")
        : null,
    "btn-plans-preview": () => {
      const st = state();
      const p =
        $("#btn-plans-preview")?.dataset?.plan ||
        st?.selectedPlan;
      if (!p) return toast("请先选中计划");
      return typeof g("openPlansMgmtItem") === "function"
        ? call("openPlansMgmtItem", p)
        : call("openPlanFullView", p);
    },
    "btn-plans-assign": () =>
      typeof g("assignFromPlansMgmt") === "function"
        ? call("assignFromPlansMgmt")
        : null,
    "btn-plans-view-split": () => {
      if (typeof g("viewSplitFromPlansMgmt") === "function") {
        return call("viewSplitFromPlansMgmt");
      }
      if (window.ccoChat?.viewSplitFromPlansMgmt) {
        return window.ccoChat.viewSplitFromPlansMgmt();
      }
      if (typeof g("showSplitPlanConfirm") === "function") {
        return call("showSplitPlanConfirm");
      }
      return null;
    },
    "btn-empty-to-chat": () => call("openChatPage"),
    "btn-chooser-to-chat": () => {
      call("openPlanChooser", false);
      return call("openChatPage");
    },
    "btn-chat-send": () => {
      const el = document.getElementById("btn-chat-send");
      if (el?.dataset?.chatMode === "cancel") return call("cancelChatMessage");
      return call("sendChatMessage");
    },
    "btn-chat-save": () => call("saveChatPlan"),
    "btn-chat-assign": () =>
      typeof g("assignFromChat") === "function" ? call("assignFromChat") : null,
    "btn-chat-attach": () =>
      typeof g("pickChatAttachments") === "function"
        ? call("pickChatAttachments")
        : null,
    "btn-chat-env-doctor": () => call("openChatEnvDoctor"),
    "btn-chat-env-dismiss": () => call("dismissChatEnvBar"),
    "btn-chat-session-new": () =>
      typeof g("newChatSession") === "function" ? call("newChatSession") : null,
    "btn-chat-session-new-in-panel": () =>
      typeof g("newChatSession") === "function" ? call("newChatSession") : null,
    "btn-chat-session-del": () =>
      typeof g("deleteChatSession") === "function"
        ? call("deleteChatSession")
        : null,
    "btn-img-lightbox-close": () =>
      typeof g("closeImageLightbox") === "function"
        ? call("closeImageLightbox")
        : null,
    "img-lightbox-backdrop": () =>
      typeof g("closeImageLightbox") === "function"
        ? call("closeImageLightbox")
        : null,
    // 右栏 DOM 已撤；保留 no-op 键名防旧缓存点击
    "btn-plan-rail-refresh": () =>
      typeof g("loadPlanRail") === "function" ? call("loadPlanRail") : null,
    "btn-plan-rail-close": () => {
      if (typeof g("setPlanRailOpen") === "function") {
        call("setPlanRailOpen", false);
        if (typeof g("renderPlanRail") === "function") call("renderPlanRail");
      }
    },
    "btn-chooser-options-toggle": () => {
      const grid = $("#chooser-options-grid");
      const btn = $("#btn-chooser-options-toggle");
      if (!grid) return;
      const open = grid.hasAttribute("hidden");
      if (open) grid.removeAttribute("hidden");
      else grid.setAttribute("hidden", "");
      if (btn) {
        btn.setAttribute("aria-expanded", open ? "true" : "false");
        if (typeof window.ccoIcon === "function") {
          const chev = window.ccoIcon(open ? "chevron-down" : "chevron-right", {
            size: 14,
          });
          btn.innerHTML = `更多选项 ${chev}`;
        } else {
          btn.textContent = open ? "更多选项 ▾" : "更多选项 ▸";
        }
      }
    },
    "btn-plan-full-close": () => call("closePlanFullView"),
    "btn-plan-full-close2": () => call("closePlanFullView"),
    "plan-full-backdrop": () => call("closePlanFullView"),
    "btn-plan-full-edit": () => call("beginPlanFullEdit"),
    "btn-plan-full-diff": () => call("openPlanFullDiff"),
    "btn-plan-full-diff-close": () => call("closePlanFullDiff"),
    "btn-plan-full-diff-left": () => call("adoptPlanDiffSide", "left"),
    "btn-plan-full-diff-right": () => call("adoptPlanDiffSide", "right"),
    "btn-plan-full-save": () => call("savePlanFullView", { asCopy: false }),
    "btn-plan-full-save-as": () => call("savePlanFullView", { asCopy: true }),
    "btn-plan-full-cancel-edit": () => call("cancelPlanFullEdit"),
    "btn-plan-full-assign": () => call("assignFromPlanFullView"),
  };
}

export function backFromSubpage() {
  if (state()?.selectedPath) {
    call("goToPlanMonitor");
  } else {
    call("goHome");
  }
}

// silence unused $$ for lint-like tooling (used by bindUi callers via re-export)
export { $$, $, state, toast, call, g };
