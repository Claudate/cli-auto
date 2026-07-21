/**
 * [INPUT]: createUiActions · classic globals
 * [OUTPUT]: bindGlobalUI / wire — 事件委托只绑意图表（无 invoke）
 * [POS]: A5-2d features/settings
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
import {
  createUiActions,
  $,
  $$,
  state,
  toast,
  call,
  g,
} from "./uiActions.js";

let UI_ACTIONS = null;
function actions() {
  if (!UI_ACTIONS) UI_ACTIONS = createUiActions();
  return UI_ACTIONS;
}

/** Global click/change/paste — intention-only via cco* / classic. */
export function bindGlobalUI() {
  if (window.__ccoUiBound) return;
  window.__ccoUiBound = true;
  const UI_ACTIONS = actions();
  window.UI_ACTIONS = UI_ACTIONS;

  if (!window.__ccoCliFitBound) {
    window.__ccoCliFitBound = true;
    let t = null;
    window.addEventListener("resize", () => {
      clearTimeout(t);
      t = setTimeout(() => {
        const st = state();
        if (st?.cliBodyHeight === "auto") call("fitCliBodyHeight");
      }, 80);
    });
  }

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && typeof g("closeImageLightbox") === "function") {
      const lb = document.getElementById("img-lightbox");
      if (lb && !lb.hidden) {
        e.preventDefault();
        call("closeImageLightbox");
        return;
      }
    }
    if (e.target?.id === "chat-input" && e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!state()?.chatBusy) call("sendChatMessage");
    }
  });

  document.addEventListener(
    "paste",
    (e) => {
      if (state()?.page !== "chat") return;
      const t = e.target;
      const inChat =
        t?.id === "chat-input" ||
        t?.closest?.("#page-chat") ||
        t?.closest?.(".chat-composer");
      if (!inChat) return;
      if (typeof g("handleChatPaste") === "function") {
        Promise.resolve(call("handleChatPaste", e)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    },
    true
  );

  document.addEventListener("dblclick", (e) => {
    const railItem = e.target?.closest?.(".plan-rail-item[data-plan-rail]");
    if (railItem) {
      e.preventDefault();
      const p = railItem.dataset.planRail;
      if (typeof g("openPlanRailItem") === "function") {
        Promise.resolve(call("openPlanRailItem", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(call("openPlanFullView", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
      return;
    }
    const mgmtItem = e.target?.closest?.(".plans-mgmt-item[data-plans-mgmt]");
    if (mgmtItem) {
      e.preventDefault();
      const p = mgmtItem.dataset.plansMgmt;
      if (typeof g("openPlansMgmtItem") === "function") {
        Promise.resolve(call("openPlansMgmtItem", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(call("openPlanFullView", p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    }
  });

  const sessionSel = document.getElementById("chat-session-select");
  if (sessionSel && !sessionSel.dataset.bound) {
    sessionSel.dataset.bound = "1";
    sessionSel.addEventListener("change", () => {
      const sid = sessionSel.value || "default";
      if (typeof g("switchChatSession") === "function") {
        Promise.resolve(call("switchChatSession", sid)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    });
  }

  const fileInput = document.getElementById("chat-file-input");
  if (fileInput && !fileInput.dataset.bound) {
    fileInput.dataset.bound = "1";
    fileInput.addEventListener("change", () => {
      if (typeof g("addChatAttachments") === "function") {
        Promise.resolve(call("addChatAttachments", fileInput.files)).catch(
          (err) => toast(String(err?.message || err))
        );
      }
    });
  }

  const dropZones = [
    document.querySelector(".chat-composer"),
    document.querySelector("#page-chat .chat-shell"),
    document.getElementById("chat-messages"),
  ].filter(Boolean);
  for (const zone of dropZones) {
    if (zone.dataset.dropBound) continue;
    zone.dataset.dropBound = "1";
    zone.addEventListener("dragover", (e) => {
      e.preventDefault();
      zone.classList.add("is-drop");
    });
    zone.addEventListener("dragleave", () => {
      zone.classList.remove("is-drop");
    });
    zone.addEventListener("drop", (e) => {
      e.preventDefault();
      zone.classList.remove("is-drop");
      if (
        e.dataTransfer?.files?.length &&
        typeof g("addChatAttachments") === "function"
      ) {
        Promise.resolve(call("addChatAttachments", e.dataTransfer.files)).catch(
          (err) => toast(String(err?.message || err))
        );
      }
    });
  }

  document.addEventListener(
    "click",
    (e) => {
      if (e.target?.id === "plan-chooser") {
        call("openPlanChooser", false);
        return;
      }
      if (e.target?.id === "plan-full-backdrop") {
        call("closePlanFullView");
        return;
      }

      const exampleChip = e.target?.closest?.(
        ".chat-example-chip[data-chat-example]"
      );
      if (exampleChip) {
        e.preventDefault();
        if (typeof g("fillChatExample") === "function") {
          call(
            "fillChatExample",
            exampleChip.dataset.chatExample || exampleChip.textContent
          );
        }
        return;
      }

      const tplBtn = e.target?.closest?.("[data-plan-template]");
      if (tplBtn) {
        e.preventDefault();
        const tid = tplBtn.getAttribute("data-plan-template");
        if (typeof g("applyPlanTemplate") === "function") {
          Promise.resolve(call("applyPlanTemplate", tid)).catch((err) =>
            toast(String(err?.message || err))
          );
        } else {
          toast("模板不可用");
        }
        return;
      }

      const attRm = e.target?.closest?.("[data-att-remove]");
      if (attRm) {
        e.preventDefault();
        e.stopPropagation();
        const idx = Number(attRm.getAttribute("data-att-remove"));
        if (typeof g("removeChatAttachment") === "function") {
          call("removeChatAttachment", idx);
        }
        return;
      }

      const zoomImg = e.target?.closest?.(".chat-img-zoomable[data-img-src]");
      if (zoomImg) {
        e.preventDefault();
        if (typeof g("openImageLightbox") === "function") {
          call(
            "openImageLightbox",
            zoomImg.getAttribute("data-img-src") || zoomImg.src,
            zoomImg.getAttribute("data-img-name") || zoomImg.alt || ""
          );
        }
        return;
      }

      const planExpand = e.target?.closest?.(".btn-chat-plan-expand");
      if (planExpand) {
        e.preventDefault();
        call("toggleChatPlanExpand", planExpand);
        return;
      }
      const planAdopt = e.target?.closest?.(".btn-chat-plan-adopt");
      if (planAdopt) {
        e.preventDefault();
        Promise.resolve(call("adoptChatPlanFromCard", planAdopt)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const planAssign = e.target?.closest?.(".btn-chat-plan-assign");
      if (planAssign) {
        e.preventDefault();
        const card = planAssign.closest?.(".chat-plan-card");
        const full = card?.querySelector?.(".chat-plan-full");
        const md = full?.textContent?.trim();
        const st = state();
        if (md && typeof g("ensureChatState") === "function") {
          call("ensureChatState");
          if (st?.chatSession) {
            if (!st.chatSession.draft_plan) {
              st.chatSession.draft_plan = {
                path: st.chatDraftPlan || "",
                saved: !!st.chatDraftPlan,
                markdown: md,
                title: null,
              };
            } else if (!st.chatSession.draft_plan.markdown) {
              st.chatSession.draft_plan.markdown = md;
            }
          }
        }
        Promise.resolve(
          typeof g("assignFromChat") === "function" ? call("assignFromChat") : null
        ).catch((err) => toast(String(err?.message || err)));
        return;
      }

      const railItem = e.target?.closest?.(".plan-rail-item[data-plan-rail]");
      if (railItem) {
        e.preventDefault();
        const p = railItem.dataset.planRail;
        if (typeof g("selectPlanRailItem") === "function") {
          call("selectPlanRailItem", p);
        } else {
          Promise.resolve(call("openPlanFullView", p)).catch((err) =>
            toast(String(err?.message || err))
          );
        }
        return;
      }

      const mgmtItem = e.target?.closest?.(".plans-mgmt-item[data-plans-mgmt]");
      if (mgmtItem) {
        e.preventDefault();
        const p = mgmtItem.dataset.plansMgmt;
        if (typeof g("selectPlansMgmtItem") === "function") {
          call("selectPlansMgmtItem", p);
        } else if (typeof g("selectPlanRailItem") === "function") {
          call("selectPlanRailItem", p);
        }
        return;
      }

      const proj = e.target?.closest?.(".project-item[data-path]");
      if (proj) {
        e.preventDefault();
        Promise.resolve(call("selectProject", proj.dataset.path)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const planItem = e.target?.closest?.(".plan-item[data-plan]");
      if (planItem) {
        e.preventDefault();
        Promise.resolve(call("selectPlan", planItem.dataset.plan))
          .then(() => {
            if (state()?.planChooserOpen) {
              call("renderPlanChooser");
              call("updateChooserAssignState");
            }
          })
          .catch((err) => toast(String(err?.message || err)));
        return;
      }
      const rerunBtn = e.target?.closest?.("[data-rerun]");
      if (rerunBtn?.dataset?.rerun) {
        e.preventDefault();
        e.stopPropagation();
        const st = state();
        if (st) st.selectedTaskId = rerunBtn.dataset.rerun;
        const fn = UI_ACTIONS["btn-rerun"];
        if (fn) {
          Promise.resolve(fn()).catch((err) =>
            toast(String(err?.message || err))
          );
        }
        return;
      }
      const taskChip = e.target?.closest?.(
        ".task-tile[data-task], .task-chip[data-task]"
      );
      if (taskChip) {
        e.preventDefault();
        const st = state();
        if (!st) return;
        const tid = taskChip.dataset.task;
        st.selectedTaskId = tid;
        if (st.closedPanels?.[tid]) delete st.closedPanels[tid];
        if (!st.cliLogExpanded) st.cliLogExpanded = {};
        st.cliLogExpanded[tid] = true;
        const fold = $("#monitor-logs-fold");
        if (fold) {
          fold.open = true;
          st.monitorLogsOpen = true;
          try {
            localStorage.setItem("cco.monitorLogsOpen", "1");
          } catch (_) {}
        }
        const tasks = st.live?.tasks || [];
        call("renderCliBoard", tasks);
        call("renderTaskStrip", st.live, tasks, {
          hasRun: !!st.live?.run_id,
          active: call("isLiveStatus", st.live?.run_status),
          finished:
            !!st.live?.run_id && !call("isLiveStatus", st.live?.run_status),
          runStatus: st.live?.run_status,
        });
        return;
      }

      const closeBtn = e.target?.closest?.("[data-close]");
      if (closeBtn?.dataset?.close) {
        e.preventDefault();
        e.stopPropagation();
        const st = state();
        if (st) st.closedPanels[closeBtn.dataset.close] = true;
        call("renderCliBoard", st?.live?.tasks || []);
        return;
      }
      const copyBtn = e.target?.closest?.("[data-copy]");
      if (copyBtn?.dataset?.copy) {
        e.preventDefault();
        e.stopPropagation();
        const st = state();
        const t = (st?.live?.tasks || []).find(
          (x) => x.task_id === copyBtn.dataset.copy
        );
        const text =
          typeof g("aiLogPlainText") === "function"
            ? call("aiLogPlainText", t)
            : "";
        Promise.resolve(navigator.clipboard.writeText(text || ""))
          .then(() => toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制"))
          .catch(() => toast("复制失败"));
        return;
      }
      const stopBtn = e.target?.closest?.("[data-stop]");
      if (stopBtn?.dataset?.stop) {
        e.preventDefault();
        e.stopPropagation();
        const st = state();
        if (st) st.selectedTaskId = stopBtn.dataset.stop;
        Promise.resolve(call("cancelTask")).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const extBtn = e.target?.closest?.("[data-extterm]");
      if (extBtn?.dataset?.extterm) {
        e.preventDefault();
        e.stopPropagation();
        Promise.resolve(call("openExternalTerminal", extBtn.dataset.extterm)).catch(
          (err) => toast(String(err?.message || err))
        );
        return;
      }

      const el = e.target?.closest?.(
        "button[id], [id].linkish, [id].icon-btn, [id].filter-chip, #brand-home, #split-plan-chip, [data-action]"
      );
      if (!el) return;

      if (el.closest?.("#log-view-mode") && el.dataset?.mode) {
        const st = state();
        if (!st) return;
        st.logViewMode = el.dataset.mode || "term";
        localStorage.setItem("cco.logViewMode", st.logViewMode);
        $$("#log-view-mode button").forEach((b) =>
          b.classList.toggle("active", b.dataset.mode === st.logViewMode)
        );
        st.logPanelSig = {};
        const board = $("#cli-board");
        if (board) delete board.dataset.visKey;
        const tasks = st.live?.tasks || [];
        if (tasks.length) call("renderCliBoard", tasks);
        if (st.phase === "planning" && st.planJob) {
          const pl = $("#planner-log");
          if (pl) delete pl.dataset.sig;
          call("fillPlannerLog", st.planJob);
        }
        return;
      }
      if (el.closest?.("#log-event-filter") && el.dataset?.evFilter) {
        const st = state();
        if (!st) return;
        st.logEventFilter = el.dataset.evFilter || "all";
        localStorage.setItem("cco.logEventFilter", st.logEventFilter);
        $$("#log-event-filter [data-ev-filter]").forEach((b) =>
          b.classList.toggle(
            "active",
            (b.dataset.evFilter || "all") === st.logEventFilter
          )
        );
        st.logPanelSig = {};
        const board = $("#cli-board");
        if (board) delete board.dataset.visKey;
        const tasks = st.live?.tasks || [];
        if (tasks.length) call("renderCliBoard", tasks);
        return;
      }
      if (el.closest?.("#log-font-group") && el.dataset?.size) {
        call("applyLogFontSize", Number(el.dataset.size));
        return;
      }

      const action = el.dataset?.action || el.id;
      if (!action) return;
      const fn = UI_ACTIONS[action];
      if (!fn) return;

      if (el.disabled || el.getAttribute("aria-disabled") === "true") return;

      e.preventDefault();
      Promise.resolve()
        .then(() => fn(e))
        .catch((err) => {
          console.error("UI action failed", action, err);
          toast(`${action}: ${err?.message || err}`);
        });
    },
    true
  );

  document.addEventListener("change", (e) => {
    const t = e.target;
    if (!t) return;
    if (t.id === "pp-provider") {
      t.dataset.touched = "1";
    }
    if (t.id === "pp-max-parallel" || t.id === "chooser-max-parallel") {
      call("commitSplitMaxParallel", t);
    }
    if (t.id === "pp-auto-start" || t.id === "pp-pause-confirm") {
      const st = state();
      if (!st) return;
      const on = !!t.checked;
      st.autoStartAfterPlan = on;
      const key =
        typeof g("PAUSE_CONFIRM_KEY") === "string"
          ? g("PAUSE_CONFIRM_KEY")
          : "cco.pauseConfirmAfterPlan";
      localStorage.setItem(key, on ? "0" : "1");
      toast(
        on
          ? "已开启：拆分后自动开始（跳过拆分台，有业务可选时仍会停住）"
          : "已关闭：拆分后停在拆分台，确认后再开始"
      );
    }
    if (
      t.id === "chooser-show-executed" ||
      t.id === "plan-rail-show-executed" ||
      t.id === "plans-mgmt-show-executed"
    ) {
      if (typeof g("setShowExecutedPlans") === "function") {
        call("setShowExecutedPlans", !!t.checked);
      } else {
        const st = state();
        if (st) st.showExecutedPlans = !!t.checked;
        try {
          localStorage.setItem("cco.showExecutedPlans", t.checked ? "1" : "0");
        } catch (_) {}
        if (state()?.planChooserOpen && typeof g("renderPlanChooser") === "function") {
          call("renderPlanChooser");
        }
        if (typeof g("renderPlanRail") === "function") call("renderPlanRail");
        if (
          state()?.page === "plans" &&
          typeof g("renderPlansMgmtPage") === "function"
        ) {
          call("renderPlansMgmtPage");
        }
      }
    }
    if (t.id === "plans-mgmt-show-other") {
      if (typeof g("renderPlansMgmtPage") === "function") {
        call("renderPlansMgmtPage");
      }
    }
  });

  document.addEventListener("focusin", (e) => {
    const t = e.target;
    if (t?.id === "chooser-max-parallel" || t?.id === "pp-max-parallel") {
      t.dataset.editing = "1";
      t.dataset.touched = "1";
    }
  });
  document.addEventListener("focusout", (e) => {
    const t = e.target;
    if (t?.id === "chooser-max-parallel" || t?.id === "pp-max-parallel") {
      call("commitSplitMaxParallel", t);
    }
  });
  document.addEventListener("input", (e) => {
    const t = e.target;
    if (t?.id === "plan-full-editor") {
      if (typeof g("onPlanFullEditorInput") === "function") {
        call("onPlanFullEditorInput");
      }
      return;
    }
    if (t?.id !== "chooser-max-parallel" && t?.id !== "pp-max-parallel") return;
    t.dataset.touched = "1";
    t.dataset.editing = "1";
    const typed = parseInt(t.value, 10);
    const hidden = $("#pp-max-parallel");
    if (Number.isFinite(typed) && typed > 0 && hidden && t !== hidden) {
      hidden.value = String(Math.max(1, Math.min(32, typed)));
    }
  });

  document.addEventListener(
    "click",
    (e) => {
      const chip = e.target?.closest?.("#cli-status-filters [data-cli-filter]");
      if (!chip) return;
      e.preventDefault();
      const st = state();
      if (!st) return;
      const f = chip.getAttribute("data-cli-filter") || "all";
      st.cliStatusFilter = f;
      st.filterFailedOnly = f === "fail";
      call("renderCliBoard", st.live?.tasks || []);
    },
    true
  );

  try {
    const st = state();
    call(
      "applyCliBodyHeight",
      st?.cliBodyHeight === "auto" ? "auto" : st?.cliBodyHeight || "auto"
    );
    call("bindCliHeightGrip");
  } catch (_) {}
}

/** 兼容旧名 */
export function wire() {
  try {
    if (typeof g("applyLogFontSize") === "function") {
      call("applyLogFontSize", state()?.logFontSize);
    }
  } catch (_) {}
  bindGlobalUI();
}
