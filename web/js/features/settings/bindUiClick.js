/**
 * [INPUT]: call / g / state / UI_ACTIONS（由 bindUi 注入）
 * [OUTPUT]: attachDocumentClick — 全局 click 委托（意图表 + 看板/计划芯片）
 * [POS]: A5-2d features/settings；自 bindUi 纵切（P-ship-D）
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

/**
 * Capture-phase document click: intention-only via cco* / classic globals.
 * @param {{
 *   UI_ACTIONS: Record<string, Function>,
 *   call: (name: string, ...args: unknown[]) => unknown,
 *   g: (name: string) => unknown,
 *   state: () => any,
 *   $: (sel: string) => HTMLElement | null,
 *   $$: (sel: string, root?: ParentNode) => HTMLElement[],
 *   toast: (msg: string) => void,
 * }} deps
 */
export function attachDocumentClick(deps) {
  const { UI_ACTIONS, call, g, state, $, $$, toast } = deps;

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

      // P2-2: last_summary 沿用 / 忽略
      const lastSumBtn = e.target?.closest?.("[data-last-summary]");
      if (lastSumBtn) {
        e.preventDefault();
        const act = lastSumBtn.getAttribute("data-last-summary");
        if (typeof g("handleLastSummaryAction") === "function") {
          call("handleLastSummaryAction", act);
        } else if (window.ccoChat?.handleLastSummaryAction) {
          window.ccoChat.handleLastSummaryAction(act);
        }
        return;
      }

      // P2-2: pin delete in settings
      const pinDel = e.target?.closest?.("[data-pin-delete]");
      if (pinDel) {
        e.preventDefault();
        const key = pinDel.getAttribute("data-pin-delete");
        if (typeof g("deleteProjectPin") === "function") {
          Promise.resolve(call("deleteProjectPin", key)).catch((err) =>
            toast(String(err?.message || err))
          );
        }
        return;
      }

      const pinAdd = e.target?.closest?.("#btn-pin-add");
      if (pinAdd) {
        e.preventDefault();
        if (typeof g("addProjectPin") === "function") {
          Promise.resolve(call("addProjectPin")).catch((err) =>
            toast(String(err?.message || err))
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
        // B2: save if needed then direct-assign (no start_run)
        Promise.resolve(
          typeof g("assignAndSplitFromChat") === "function"
            ? call("assignAndSplitFromChat", planAssign)
            : typeof g("assignFromChat") === "function"
              ? call("assignFromChat")
              : null
        ).catch((err) => toast(String(err?.message || err)));
        return;
      }
      const planDirect = e.target?.closest?.(".btn-chat-plan-direct");
      if (planDirect) {
        e.preventDefault();
        // 直接执行：整份计划单任务；仍经 Mode B confirm（禁止 start_run）
        Promise.resolve(
          typeof g("assignAndDirectFromChat") === "function"
            ? call("assignAndDirectFromChat", planDirect)
            : null
        ).catch((err) => toast(String(err?.message || err)));
        return;
      }

      // shell-chrome C1：rail「查看拆分结果」— 勿当选中整行
      const railView = e.target?.closest?.("[data-plan-rail-view]");
      if (railView) {
        e.preventDefault();
        e.stopPropagation();
        const p = railView.getAttribute("data-plan-rail-view");
        if (typeof g("viewSplitFromPlanRail") === "function") {
          call("viewSplitFromPlanRail", p);
        } else if (window.ccoChat?.viewSplitFromPlanRail) {
          window.ccoChat.viewSplitFromPlanRail(p);
        } else if (typeof g("showSplitPlanConfirm") === "function") {
          call("showSplitPlanConfirm");
        } else {
          toast("查看拆分结果不可用");
        }
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

      // shell-chrome B1：移除按钮由 shellUi 绑定，勿当选中
      if (e.target?.closest?.(".project-item-remove")) {
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
        const taskId = rerunBtn.dataset.rerun;
        const st = state();
        if (st) st.selectedTaskId = taskId;
        // 卡片「再跑一次」= 只重跑该失败任务，不是整轮重拆
        const cco = typeof window !== "undefined" ? window.ccoRun : null;
        if (cco && typeof cco.retryTask === "function") {
          Promise.resolve(cco.retryTask(taskId)).catch((err) =>
            toast(String(err?.message || err))
          );
          return;
        }
        toast("执行台未就绪，请稍后重试");
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
        // 点步骤卡 → 展开该步详细日志（运行端本身始终可见）
        if (!st.cliLogExpanded) st.cliLogExpanded = {};
        st.cliLogExpanded[tid] = true;
        const mon = $("#monitor");
        if (mon) mon.hidden = false;
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
}
