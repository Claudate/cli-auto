/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: doctor UI 片段 · settings 读写（含 H3 stall/retry · H4 failover_enabled）
 * [POS]: web/js D4 自 app.js 纵切；无构建器，顺序 script 共享全局
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — doctor */

function startPolling(intervalMs = 2000) {
  clearInterval(state.pollTimer);
  state.pollTimer = setInterval(() => {
    state.now = Date.now();
    // 规划轮询不绑死 workspace：切到设置/帮助/环境检查也继续
    if (state.planJobId && state.phase === "planning") {
      refreshPlanJob().catch(() => {});
    }
    if (state.page === "workspace" && state.selectedPath) {
      loadProjects().catch(() => {});
      loadLive().catch(() => {});
    } else if (state.page === "welcome") {
      loadProjects().catch(() => {});
    }
    updateBgPlanBanner();
  }, intervalMs);
}

/* ── Settings ── */
async function loadSettings() {
  try {
    const s = await invoke("get_settings_cmd");
    $("#s-poll-interval").value = s.poll_interval_secs;
    const modeIdx = { print: 0, bg: 1, auto: 2 };
    $("#s-default-mode").value = modeIdx[s.default_mode] ?? 0;
    $("#s-default-provider").value = s.default_provider;
    $("#s-max-parallel").value = s.max_parallel;
    // H3/H4: stall/retry 人话字段 + failover 开关（与 scheduler 读取同源）
    if ($("#s-retry-max")) $("#s-retry-max").value = s.retry_max ?? 2;
    if ($("#s-stall-secs")) $("#s-stall-secs").value = s.stall_secs ?? 180;
    if ($("#s-failover-enabled")) {
      // 缺省 true；仅当后端明确 false 时关（不覆盖用户 config 显式值由后端 serde 负责）
      $("#s-failover-enabled").checked = s.failover_enabled !== false;
    }
    if ($("#s-failover-order-note") && s.failover_order_note) {
      $("#s-failover-order-note").textContent = s.failover_order_note;
    }
    if ($("#s-post-inspect")) {
      $("#s-post-inspect").checked = !!s.post_inspect_enabled;
    }
    if ($("#s-post-git-push")) {
      $("#s-post-git-push").checked = !!s.post_git_push_enabled;
    }
    if ($("#s-planner-critic")) {
      $("#s-planner-critic").checked = !!s.planner_critic_enabled;
    }
    if ($("#s-post-tasks-note") && s.post_tasks_note) {
      $("#s-post-tasks-note").textContent = s.post_tasks_note;
    }
    // Flow fun blurbs: local only (not backend config)
    if ($("#s-flow-fun") && typeof flowFunEnabled === "function") {
      $("#s-flow-fun").checked = flowFunEnabled();
    }
    // C3 方案 B: local only
    if ($("#s-chat-assign-direct") && typeof chatAssignDirectEnabled === "function") {
      $("#s-chat-assign-direct").checked = chatAssignDirectEnabled();
    }
    $("#s-log-font").value = String(state.logFontSize);
    // Seed split-time concurrency from settings when user hasn't touched it.
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(s.max_parallel || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(s.max_parallel || 2);
    }
  } catch (_) {
    /* ignore */
  }
}

async function saveSettings() {
  // Local UI preference (no backend field)
  if ($("#s-flow-fun") && typeof setFlowFunEnabled === "function") {
    setFlowFunEnabled(!!$("#s-flow-fun").checked);
  }
  if ($("#s-chat-assign-direct") && typeof setChatAssignDirectEnabled === "function") {
    setChatAssignDirectEnabled(!!$("#s-chat-assign-direct").checked);
  }
  const pollVal = parseInt($("#s-poll-interval").value, 10);
  const modeVal = parseInt($("#s-default-mode").value, 10);
  const providerVal = $("#s-default-provider").value.trim();
  const maxParallelVal = parseInt($("#s-max-parallel").value, 10);
  const retryMaxVal = parseInt($("#s-retry-max")?.value, 10);
  const stallSecsVal = parseInt($("#s-stall-secs")?.value, 10);
  const failoverEl = $("#s-failover-enabled");
  // 有控件则读写；无控件不传，避免误写回默认
  const failoverEnabled = failoverEl ? !!failoverEl.checked : undefined;
  const postInspectEl = $("#s-post-inspect");
  const postGitPushEl = $("#s-post-git-push");
  const postInspectEnabled = postInspectEl ? !!postInspectEl.checked : undefined;
  const postGitPushEnabled = postGitPushEl ? !!postGitPushEl.checked : undefined;
  const plannerCriticEl = $("#s-planner-critic");
  const plannerCriticEnabled = plannerCriticEl
    ? !!plannerCriticEl.checked
    : undefined;
  const fontVal = parseInt($("#s-log-font").value, 10) || 14;
  const status = $("#s-save-status");
  if (!pollVal || pollVal < 1 || pollVal > 60) {
    status.className = "save-status err";
    status.textContent = "刷新间隔需在 1–60 秒之间";
    status.hidden = false;
    return;
  }
  if (Number.isFinite(retryMaxVal) && (retryMaxVal < 0 || retryMaxVal > 10)) {
    status.className = "save-status err";
    status.textContent = "同 CLI 再试次数需在 0–10 之间";
    status.hidden = false;
    return;
  }
  if (Number.isFinite(stallSecsVal) && (stallSecsVal < 30 || stallSecsVal > 7200)) {
    status.className = "save-status err";
    status.textContent = "卡死秒数需在 30–7200 之间（多久没新日志算卡死）";
    status.hidden = false;
    return;
  }
  try {
    const update = {
      poll_interval_secs: pollVal,
      default_mode: modeVal,
      default_provider: providerVal,
      max_parallel: maxParallelVal || 2,
      retry_max: Number.isFinite(retryMaxVal) ? retryMaxVal : 2,
      stall_secs: Number.isFinite(stallSecsVal) ? stallSecsVal : 180,
    };
    if (failoverEnabled !== undefined) {
      update.failover_enabled = failoverEnabled;
    }
    if (postInspectEnabled !== undefined) {
      update.post_inspect_enabled = postInspectEnabled;
    }
    if (postGitPushEnabled !== undefined) {
      update.post_git_push_enabled = postGitPushEnabled;
    }
    if (plannerCriticEnabled !== undefined) {
      update.planner_critic_enabled = plannerCriticEnabled;
    }
    const updated = await invoke("set_settings_cmd", { update });
    applyLogFontSize(fontVal);
    // sync picker defaults
    if ($("#pp-provider")) $("#pp-provider").value = providerVal;
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(maxParallelVal || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(maxParallelVal || 2);
    }
    // 回填后端只读说明（若服务端文案有变）
    if ($("#s-failover-order-note") && updated.failover_order_note) {
      $("#s-failover-order-note").textContent = updated.failover_order_note;
    }
    if ($("#s-failover-enabled") && typeof updated.failover_enabled === "boolean") {
      $("#s-failover-enabled").checked = updated.failover_enabled;
    }
    if ($("#s-post-inspect") && typeof updated.post_inspect_enabled === "boolean") {
      $("#s-post-inspect").checked = updated.post_inspect_enabled;
    }
    if ($("#s-planner-critic") && typeof updated.planner_critic_enabled === "boolean") {
      $("#s-planner-critic").checked = updated.planner_critic_enabled;
    }
    if ($("#s-post-git-push") && typeof updated.post_git_push_enabled === "boolean") {
      $("#s-post-git-push").checked = updated.post_git_push_enabled;
    }
    if ($("#s-post-tasks-note") && updated.post_tasks_note) {
      $("#s-post-tasks-note").textContent = updated.post_tasks_note;
    }
    status.className = "save-status ok";
    status.textContent = "已保存";
    status.hidden = false;
    setTimeout(() => {
      status.hidden = true;
    }, 2500);
    startPolling(Math.min(updated.poll_interval_secs * 1000, 5000));
  } catch (e) {
    status.className = "save-status err";
    status.textContent = "保存失败: " + e;
    status.hidden = false;
  }
}

function backFromSubpage() {
  if (state.selectedPath) {
    // 与「监控计划」同源：回 workspace 看规划/运行
    goToPlanMonitor();
  } else {
    goHome();
  }
}

/* ── Wire ── */

/* ═══════════════════════════════════════════════
 * 全局事件委托：按钮失效的根治方案
 * - 不依赖 wire 时序
 * - 不依赖 Tauri 是否已就绪（先响应 UI）
 * - 动态生成的按钮也能点（按 id / data-action）
 * - 每次点击 try/catch，失败 toast，绝不静默
 * ═══════════════════════════════════════════════ */
const UI_ACTIONS = {
  "btn-add-plus": () => openModal(),
  "btn-welcome-add": () => openModal(),
  "btn-welcome-add2": () => openModal(),
  "btn-welcome-help": () => showPage("help"),
  "btn-refresh": async () => {
    if (state.page === "workspace" && state.selectedPath) {
      await loadProjects();
      if (state.phase === "planning" && state.planJobId) {
        await refreshPlanJob().catch(() => {});
      }
      await loadLive();
      await loadPlansForPicker().catch(() => {});
      if (!isPlanSessionActive()) {
        const proj = state.projects.find((p) => p.path === state.selectedPath);
        const raw =
          state.live?.plan_path || proj?.default_plan || proj?.last_plan || state.selectedPlan;
        const cand = normalizePlanPath(raw) || raw;
        if (cand) await selectPlan(cand, { keepSession: true }).catch(() => {});
        else updateTopPlanInfo();
      } else {
        renderPhasePanels();
        updateTopPlanInfo();
      }
    } else {
      if (state.planJobId && state.phase === "planning") {
        await refreshPlanJob().catch(() => {});
      }
      await loadProjects();
    }
    toast("已刷新");
  },
  "modal-close": () => closeModal(),
  "modal-backdrop": () => closeModal(),
  "m-pick-folder": () => pickFolderToModal(),
  "m-confirm-project": () => addProjectFromModal(),
  "m-cancel-project": () => closeModal(),
  "btn-ws-stop-all": () => stopAll(),
  "btn-open-monitor-window": () =>
    typeof openMonitorWindow === "function"
      ? openMonitorWindow()
      : toast("独立监视窗不可用"),
  "btn-ws-resume": () => resumeRun(),
  "btn-ws-rework": () => startReworkWave(),
  "btn-ws-accept-residual": () => acceptRunResidual(),
  "btn-export-log-md": () =>
    typeof exportBoardLogsMd === "function"
      ? exportBoardLogsMd()
      : toast("导出不可用"),
  "btn-open-handoff": () =>
    typeof openHandoffLedger === "function"
      ? openHandoffLedger()
      : toast("打开账本不可用"),
  "btn-ws-back-chat": () =>
    typeof openChatPage === "function" ? openChatPage() : showPage("chat"),
  "btn-remove-project": () => removeSelectedProject(),
  "btn-ws-dismiss-run": () => dismissRun(),
  "btn-task-dash-toggle": () => {
    state.taskDashCollapsed = !state.taskDashCollapsed;
    localStorage.setItem("cco.taskDashCollapsed", state.taskDashCollapsed ? "1" : "0");
    const tasks = state.live?.tasks || [];
    renderTaskStrip(state.live, tasks, {
      hasRun: !!state.live?.run_id,
      active: isLiveStatus(state.live?.run_status),
      finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
      runStatus: state.live?.run_status,
    });
  },
  "btn-chooser-assign": () => analyzePlanFromPicker(),
  "btn-stop-task": () => cancelTask(),
  "btn-pp-scan": async () => {
    await loadPlansForPicker();
    renderPlanChooser();
  },
  "btn-pp-pick": () => pickPlanFileForPicker(),
  "btn-pp-pick-empty": () => pickPlanFileForPicker(),
  "btn-chooser-scan": async () => {
    await loadPlansForPicker();
    renderPlanChooser();
  },
  "btn-chooser-pick": () => pickPlanFileForPicker(),
  "btn-chooser-close": () => openPlanChooser(false),
  "btn-plan-choose": async () => {
    // 先打开面板（展开列表换文件），再扫计划——避免 invoke 失败导致「按钮像死了」
    openPlanChooser(true, { expandList: true });
    try {
      await loadPlansForPicker();
      renderPlanChooser();
      updateChooserAssignState();
    } catch (e) {
      toast(String(e));
      renderPlanChooser();
      updateChooserAssignState();
    }
  },
  "btn-pp-analyze": async () => {
    // 顶栏「执行此计划」：有选中则统一入口；无选中再打开选项层选文件
    if (state.selectedPlan && typeof startExecuteFromSelection === "function") {
      return startExecuteFromSelection(state.selectedPlan, { source: "topbar" });
    }
    openPlanChooser(true, { expandList: true });
    try {
      await loadPlansForPicker();
      renderPlanChooser();
      updateChooserAssignState();
    } catch (e) {
      toast(String(e));
      renderPlanChooser();
      updateChooserAssignState();
    }
  },
  "btn-chooser-toggle-list": () => {
    if (typeof setChooserListExpanded === "function") {
      setChooserListExpanded(!state.chooserListExpanded);
    } else {
      state.chooserListExpanded = !state.chooserListExpanded;
      if (typeof renderPlanChooser === "function") renderPlanChooser();
    }
  },
  "btn-pp-set-default": () => setDefaultPlan(),
  "btn-confirm-start": () => confirmAndStart(),
  "btn-sanitize-deps": () =>
    typeof sanitizeDepsFromConfirm === "function"
      ? sanitizeDepsFromConfirm()
      : null,
  "btn-enable-post-inspect": () =>
    typeof enablePostInspectAndResplit === "function"
      ? enablePostInspectAndResplit()
      : null,
  "btn-enable-planner-critic": () =>
    typeof enablePlannerCriticAndResplit === "function"
      ? enablePlannerCriticAndResplit()
      : null,
  "btn-replan": () => replanFromConfirm(),
  "btn-confirm-back": () => backFromConfirmToMonitor(),
  "btn-confirm-edit": () => beginConfirmEdit(),
  "btn-confirm-delete": () =>
    typeof deleteConfirmTask === "function"
      ? deleteConfirmTask()
      : toast("删除不可用"),
  "btn-confirm-edit-cancel": () => cancelConfirmEdit(),
  "btn-confirm-edit-save": () => saveConfirmEdit(),
  "split-plan-chip": () => showSplitPlanConfirm(),
  "btn-edit-plan": () => openEditPlan(),
  "btn-cancel-planning": () => cancelPlanning(),
  "btn-plan-expand": () => openPlanChooser(true),
  "btn-restore-panels": () => {
    state.closedPanels = {};
    renderCliBoard(state.live?.tasks || []);
  },
  "btn-doctor-dismiss": () => {
    const d = state.doctorCache;
    const fails = (d?.lines || []).filter((l) => !l.ok);
    state.doctorDismissedKey =
      fails.map((l) => l.name + ":" + l.detail).join("|") || "dismissed";
    renderDoctorWarn();
    toast("已暂时忽略环境提示");
  },
  "btn-task-expand": () => {
    state.taskStripExpanded = !state.taskStripExpanded;
    localStorage.setItem("cco.taskStripExpanded", state.taskStripExpanded ? "1" : "0");
    const tasks = state.live?.tasks || [];
    renderTaskStrip(state.live, tasks, {
      hasRun: !!state.live?.run_id,
      active: isLiveStatus(state.live?.run_status),
      finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
      runStatus: state.live?.run_status,
    });
  },
  "btn-cli-h-auto": () => {
    applyCliBodyHeight("auto");
    const tasks = state.live?.tasks || [];
    if (tasks.length) renderCliBoard(tasks);
    toast("CLI 高度已恢复自适应");
  },
  "btn-copy-log": async () => {
    const t =
      (state.live?.tasks || []).find((x) => x.task_id === state.selectedTaskId) ||
      (state.live?.tasks || [])[0];
    const text = aiLogPlainText(t);
    await navigator.clipboard.writeText(text || "");
    toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
  },
  "btn-rerun": () => {
    state.phase = "pick";
    state.planJobId = null;
    state.planJob = null;
    state.closedPanels = {};
    state.taskDashCollapsed = false;
    localStorage.setItem("cco.taskDashCollapsed", "0");
    renderPhasePanels();
    renderPlanPicker();
    if (state.selectedPlan) return analyzePlanFromPicker();
    openPlanChooser(true);
    toast("请先选择计划");
  },
  "btn-change-plan": () => {
    // 已移除「换计划」入口；保留 id 防旧调用
  },
  "btn-doctor-recheck": async () => {
    await ensureDoctor(true);
    toast(state.doctorCache?.ok ? "环境正常" : "仍有问题，请查看详情");
  },
  "btn-doctor-open": async () => {
    showPage("doctor");
    await loadDoctor();
  },
  "btn-open-doctor": async () => {
    showPage("doctor");
    await loadDoctor();
  },
  "btn-doctor": () => loadDoctor(),
  "btn-doctor-back": () => backFromSubpage(),
  "btn-open-settings": async () => {
    showPage("settings");
    await loadSettings();
  },
  "btn-settings-save": () => saveSettings(),
  "btn-settings-back": () => backFromSubpage(),
  "btn-open-help": () => showPage("help"),
  "btn-help-back": () => backFromSubpage(),
  "brand-home": () => goHome(),
  "btn-open-chat": () => openChatPage(),
  "btn-monitor-plan": () => goToPlanMonitor(),
  // 计划管理 = 独立 page-plans（选中后管理）；聊天右栏用 icon 展开
  "btn-plan-mgmt": () =>
    typeof openPlanManagement === "function" ? openPlanManagement() : showPage("plans"),
  "btn-chat-rail-toggle": () =>
    typeof toggleChatPlanRail === "function" ? toggleChatPlanRail() : null,
  "btn-plans-refresh": () =>
    typeof loadPlanRail === "function" ? loadPlanRail() : null,
  "btn-plans-to-chat": () =>
    typeof openChatPage === "function" ? openChatPage() : showPage("chat"),
  "btn-plans-set-dir": () =>
    typeof promptPlansDir === "function" ? promptPlansDir() : null,
  "btn-plans-open-dir": () =>
    typeof openPlansDirInFinder === "function" ? openPlansDirInFinder() : null,
  "btn-plans-pick-file": () =>
    typeof pickPlanFileForMgmt === "function" ? pickPlanFileForMgmt() : null,
  // 空态 / 提示条按钮（与 list 内动态 id 共用）
  "btn-plans-empty-show-other": () =>
    typeof showOtherPlansLocations === "function"
      ? showOtherPlansLocations()
      : null,
  "btn-plans-hint-show-other": () =>
    typeof showOtherPlansLocations === "function"
      ? showOtherPlansLocations()
      : null,
  "btn-plans-empty-pick": () =>
    typeof pickPlanFileForMgmt === "function" ? pickPlanFileForMgmt() : null,
  "btn-plans-empty-open-dir": () =>
    typeof openPlansDirInFinder === "function" ? openPlansDirInFinder() : null,
  "btn-plans-empty-to-chat": () =>
    typeof openChatPage === "function" ? openChatPage() : showPage("chat"),
  "btn-plans-preview": () => {
    const p =
      $("#btn-plans-preview")?.dataset?.plan ||
      state.planRailSelected ||
      state.selectedPlan;
    if (!p) return toast("请先选中计划");
    return typeof openPlansMgmtItem === "function"
      ? openPlansMgmtItem(p)
      : openPlanFullView(p);
  },
  "btn-plans-assign": () =>
    typeof assignFromPlansMgmt === "function"
      ? assignFromPlansMgmt()
      : null,
  "btn-empty-to-chat": () => openChatPage(),
  "btn-chooser-to-chat": () => {
    openPlanChooser(false);
    return openChatPage();
  },
  "btn-chat-send": () => sendChatMessage(),
  "btn-chat-save": () => saveChatPlan(),
  "btn-chat-assign": () =>
    typeof assignFromChat === "function" ? assignFromChat() : null,
  "btn-chat-attach": () =>
    typeof pickChatAttachments === "function" ? pickChatAttachments() : null,
  "btn-chat-env-doctor": () => openChatEnvDoctor(),
  "btn-chat-env-dismiss": () => dismissChatEnvBar(),
  // C3 multi-session
  "btn-chat-session-new": () =>
    typeof newChatSession === "function" ? newChatSession() : null,
  "btn-chat-session-del": () =>
    typeof deleteChatSession === "function" ? deleteChatSession() : null,
  "btn-img-lightbox-close": () =>
    typeof closeImageLightbox === "function" ? closeImageLightbox() : null,
  "img-lightbox-backdrop": () =>
    typeof closeImageLightbox === "function" ? closeImageLightbox() : null,
  // H1 plan-rail + full-view modal
  "btn-plan-rail-refresh": () => loadPlanRail(),
  "btn-plan-rail-close": () => {
    if (typeof setPlanRailOpen === "function") {
      setPlanRailOpen(false);
      if (typeof renderPlanRail === "function") renderPlanRail();
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
      btn.textContent = open ? "更多选项 ▾" : "更多选项 ▸";
    }
  },
  "btn-plan-full-close": () => closePlanFullView(),
  "btn-plan-full-close2": () => closePlanFullView(),
  "plan-full-backdrop": () => closePlanFullView(),
  "btn-plan-full-edit": () => beginPlanFullEdit(),
  "btn-plan-full-diff": () => openPlanFullDiff(),
  "btn-plan-full-diff-close": () => closePlanFullDiff(),
  "btn-plan-full-diff-left": () => adoptPlanDiffSide("left"),
  "btn-plan-full-diff-right": () => adoptPlanDiffSide("right"),
  "btn-plan-full-save": () => savePlanFullView({ asCopy: false }),
  "btn-plan-full-save-as": () => savePlanFullView({ asCopy: true }),
  "btn-plan-full-cancel-edit": () => cancelPlanFullEdit(),
  "btn-plan-full-assign": () => assignFromPlanFullView(),
};

function bindGlobalUI() {
  if (window.__ccoUiBound) return;
  window.__ccoUiBound = true;
  if (!window.__ccoCliFitBound) {
    window.__ccoCliFitBound = true;
    let t = null;
    window.addEventListener("resize", () => {
      clearTimeout(t);
      t = setTimeout(() => {
        if (state.cliBodyHeight === "auto") fitCliBodyHeight();
      }, 80);
    });
  }

  // chat: Enter 发送（Shift+Enter 换行）；Esc 关图片灯箱
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && typeof closeImageLightbox === "function") {
      const lb = document.getElementById("img-lightbox");
      if (lb && !lb.hidden) {
        e.preventDefault();
        closeImageLightbox();
        return;
      }
    }
    if (e.target?.id === "chat-input" && e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!state.chatBusy) sendChatMessage();
    }
  });

  // Ctrl/Cmd+V 粘贴图片 → 聊天附件（捕获阶段，避免被 textarea 吃掉）
  document.addEventListener(
    "paste",
    (e) => {
      if (state.page !== "chat") return;
      // allow paste into any chat surface (input / messages / composer)
      const t = e.target;
      const inChat =
        t?.id === "chat-input" ||
        t?.closest?.("#page-chat") ||
        t?.closest?.(".chat-composer");
      if (!inChat) return;
      if (typeof handleChatPaste === "function") {
        Promise.resolve(handleChatPaste(e)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    },
    true
  );

  // 双击：右栏 / 计划管理 → 全文编辑
  document.addEventListener("dblclick", (e) => {
    const railItem = e.target?.closest?.(".plan-rail-item[data-plan-rail]");
    if (railItem) {
      e.preventDefault();
      const p = railItem.dataset.planRail;
      if (typeof openPlanRailItem === "function") {
        Promise.resolve(openPlanRailItem(p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(openPlanFullView(p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
      return;
    }
    const mgmtItem = e.target?.closest?.(".plans-mgmt-item[data-plans-mgmt]");
    if (mgmtItem) {
      e.preventDefault();
      const p = mgmtItem.dataset.plansMgmt;
      if (typeof openPlansMgmtItem === "function") {
        Promise.resolve(openPlansMgmtItem(p)).catch((err) =>
          toast(String(err?.message || err))
        );
      } else {
        Promise.resolve(openPlanFullView(p)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    }
  });

  // C3: session select change
  const sessionSel = document.getElementById("chat-session-select");
  if (sessionSel && !sessionSel.dataset.bound) {
    sessionSel.dataset.bound = "1";
    sessionSel.addEventListener("change", () => {
      const sid = sessionSel.value || "default";
      if (typeof switchChatSession === "function") {
        Promise.resolve(switchChatSession(sid)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    });
  }

  // G4: file input change + drag-drop on composer
  const fileInput = document.getElementById("chat-file-input");
  if (fileInput && !fileInput.dataset.bound) {
    fileInput.dataset.bound = "1";
    fileInput.addEventListener("change", () => {
      if (typeof addChatAttachments === "function") {
        Promise.resolve(addChatAttachments(fileInput.files)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    });
  }
  // 拖放附图：composer + 整个聊天页消息区
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
      if (e.dataTransfer?.files?.length && typeof addChatAttachments === "function") {
        Promise.resolve(addChatAttachments(e.dataTransfer.files)).catch((err) =>
          toast(String(err?.message || err))
        );
      }
    });
  }

  document.addEventListener(
    "click",
    (e) => {
      // plan chooser backdrop
      if (e.target?.id === "plan-chooser") {
        openPlanChooser(false);
        return;
      }
      // H1 plan full-view backdrop
      if (e.target?.id === "plan-full-backdrop") {
        closePlanFullView();
        return;
      }

      // G5: 空聊示例 chip → 填入输入框
      const exampleChip = e.target?.closest?.(".chat-example-chip[data-chat-example]");
      if (exampleChip) {
        e.preventDefault();
        if (typeof fillChatExample === "function") {
          fillChatExample(exampleChip.dataset.chatExample || exampleChip.textContent);
        }
        return;
      }

      // G4: remove pending attachment thumb
      const attRm = e.target?.closest?.("[data-att-remove]");
      if (attRm) {
        e.preventDefault();
        e.stopPropagation();
        const idx = Number(attRm.getAttribute("data-att-remove"));
        if (typeof removeChatAttachment === "function") removeChatAttachment(idx);
        return;
      }

      // 点击图片放大
      const zoomImg = e.target?.closest?.(".chat-img-zoomable[data-img-src]");
      if (zoomImg) {
        e.preventDefault();
        if (typeof openImageLightbox === "function") {
          openImageLightbox(
            zoomImg.getAttribute("data-img-src") || zoomImg.src,
            zoomImg.getAttribute("data-img-name") || zoomImg.alt || ""
          );
        }
        return;
      }

      // 聊天计划卡：展开全文 / 采用并保存（动态按钮，无固定 id）
      const planExpand = e.target?.closest?.(".btn-chat-plan-expand");
      if (planExpand) {
        e.preventDefault();
        toggleChatPlanExpand(planExpand);
        return;
      }
      const planAdopt = e.target?.closest?.(".btn-chat-plan-adopt");
      if (planAdopt) {
        e.preventDefault();
        Promise.resolve(adoptChatPlanFromCard(planAdopt)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const planAssign = e.target?.closest?.(".btn-chat-plan-assign");
      if (planAssign) {
        e.preventDefault();
        // Seed draft path from card if needed, then same entry as sticky assign.
        const card = planAssign.closest?.(".chat-plan-card");
        const full = card?.querySelector?.(".chat-plan-full");
        const md = full?.textContent?.trim();
        if (md && typeof ensureChatState === "function") {
          ensureChatState();
          if (state.chatSession) {
            if (!state.chatSession.draft_plan) {
              state.chatSession.draft_plan = {
                path: state.chatDraftPlan || "",
                saved: !!state.chatDraftPlan,
                markdown: md,
                title: null,
              };
            } else if (!state.chatSession.draft_plan.markdown) {
              state.chatSession.draft_plan.markdown = md;
            }
          }
        }
        Promise.resolve(
          typeof assignFromChat === "function" ? assignFromChat() : null
        ).catch((err) => toast(String(err?.message || err)));
        return;
      }

      // 聊天右栏单击 = 选中
      const railItem = e.target?.closest?.(".plan-rail-item[data-plan-rail]");
      if (railItem) {
        e.preventDefault();
        const p = railItem.dataset.planRail;
        if (typeof selectPlanRailItem === "function") {
          selectPlanRailItem(p);
        } else {
          Promise.resolve(openPlanFullView(p)).catch((err) =>
            toast(String(err?.message || err))
          );
        }
        return;
      }

      // 计划管理页单击 = 选中 + 详情
      const mgmtItem = e.target?.closest?.(".plans-mgmt-item[data-plans-mgmt]");
      if (mgmtItem) {
        e.preventDefault();
        const p = mgmtItem.dataset.plansMgmt;
        if (typeof selectPlansMgmtItem === "function") {
          selectPlansMgmtItem(p);
        } else if (typeof selectPlanRailItem === "function") {
          selectPlanRailItem(p);
        }
        return;
      }

      // 动态列表：项目 / 计划 / 任务条
      const proj = e.target?.closest?.(".project-item[data-path]");
      if (proj) {
        e.preventDefault();
        Promise.resolve(selectProject(proj.dataset.path)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }
      const planItem = e.target?.closest?.(".plan-item[data-plan]");
      if (planItem) {
        e.preventDefault();
        Promise.resolve(selectPlan(planItem.dataset.plan))
          .then(() => {
            if (state.planChooserOpen) {
              renderPlanChooser();
              updateChooserAssignState();
            }
          })
          .catch((err) => toast(String(err?.message || err)));
        return;
      }
      const rerunBtn = e.target?.closest?.("[data-rerun]");
      if (rerunBtn?.dataset?.rerun) {
        e.preventDefault();
        e.stopPropagation();
        state.selectedTaskId = rerunBtn.dataset.rerun;
        const fn = UI_ACTIONS["btn-rerun"];
        if (fn) Promise.resolve(fn()).catch((err) => toast(String(err?.message || err)));
        return;
      }
      const taskChip = e.target?.closest?.(".task-tile[data-task], .task-chip[data-task]");
      if (taskChip) {
        e.preventDefault();
        state.selectedTaskId = taskChip.dataset.task;
        if (state.closedPanels[taskChip.dataset.task]) {
          delete state.closedPanels[taskChip.dataset.task];
        }
        const tasks = state.live?.tasks || [];
        renderCliBoard(tasks);
        renderTaskStrip(state.live, tasks, {
          hasRun: !!state.live?.run_id,
          active: isLiveStatus(state.live?.run_status),
          finished: !!state.live?.run_id && !isLiveStatus(state.live?.run_status),
          runStatus: state.live?.run_status,
        });
        return;
      }
      // CLI 窗口内动态按钮
      const closeBtn = e.target?.closest?.("[data-close]");
      if (closeBtn?.dataset?.close) {
        e.preventDefault();
        e.stopPropagation();
        state.closedPanels[closeBtn.dataset.close] = true;
        renderCliBoard(state.live?.tasks || []);
        return;
      }
      const copyBtn = e.target?.closest?.("[data-copy]");
      if (copyBtn?.dataset?.copy) {
        e.preventDefault();
        e.stopPropagation();
        const t = (state.live?.tasks || []).find((x) => x.task_id === copyBtn.dataset.copy);
        {
          const text = aiLogPlainText(t);
          Promise.resolve(navigator.clipboard.writeText(text || ""))
            .then(() => toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制"))
            .catch(() => toast("复制失败"));
        }
        return;
      }
      const stopBtn = e.target?.closest?.("[data-stop]");
      if (stopBtn?.dataset?.stop) {
        e.preventDefault();
        e.stopPropagation();
        state.selectedTaskId = stopBtn.dataset.stop;
        Promise.resolve(cancelTask()).catch((err) => toast(String(err?.message || err)));
        return;
      }
      const extBtn = e.target?.closest?.("[data-extterm]");
      if (extBtn?.dataset?.extterm) {
        e.preventDefault();
        e.stopPropagation();
        Promise.resolve(openExternalTerminal(extBtn.dataset.extterm)).catch((err) =>
          toast(String(err?.message || err))
        );
        return;
      }

      const el = e.target?.closest?.(
        "button[id], [id].linkish, [id].icon-btn, [id].filter-chip, #brand-home, #split-plan-chip, [data-action]"
      );
      if (!el) return;

      // log mode / font size segments (no stable single action id on parent)
      if (el.closest?.("#log-view-mode") && el.dataset?.mode) {
        state.logViewMode = el.dataset.mode || "term";
        localStorage.setItem("cco.logViewMode", state.logViewMode);
        $$("#log-view-mode button").forEach((b) =>
          b.classList.toggle("active", b.dataset.mode === state.logViewMode)
        );
        // 视图切换：强制 log body 重绘
        state.logPanelSig = {};
        const board = $("#cli-board");
        if (board) delete board.dataset.visKey;
        const tasks = state.live?.tasks || [];
        if (tasks.length) renderCliBoard(tasks);
        if (state.phase === "planning" && state.planJob) {
          const pl = $("#planner-log");
          if (pl) delete pl.dataset.sig;
          fillPlannerLog(state.planJob);
        }
        return;
      }
      // P2-3: event type filter chips
      if (el.closest?.("#log-event-filter") && el.dataset?.evFilter) {
        state.logEventFilter = el.dataset.evFilter || "all";
        localStorage.setItem("cco.logEventFilter", state.logEventFilter);
        $$("#log-event-filter [data-ev-filter]").forEach((b) =>
          b.classList.toggle(
            "active",
            (b.dataset.evFilter || "all") === state.logEventFilter
          )
        );
        state.logPanelSig = {};
        const board = $("#cli-board");
        if (board) delete board.dataset.visKey;
        const tasks = state.live?.tasks || [];
        if (tasks.length) renderCliBoard(tasks);
        return;
      }
      if (el.closest?.("#log-font-group") && el.dataset?.size) {
        applyLogFontSize(Number(el.dataset.size));
        return;
      }

      const action = el.dataset?.action || el.id;
      if (!action) return;
      const fn = UI_ACTIONS[action];
      if (!fn) return;

      // disabled / aria-disabled
      if (el.disabled || el.getAttribute("aria-disabled") === "true") return;

      e.preventDefault();
      Promise.resolve()
        .then(() => fn(e))
        .catch((err) => {
          console.error("UI action failed", action, err);
          toast(`${action}: ${err?.message || err}`);
        });
    },
    true // capture：不被子层 stopPropagation 吃掉
  );

  document.addEventListener("change", (e) => {
    const t = e.target;
    if (!t) return;
    if (t.id === "pp-provider") {
      t.dataset.touched = "1";
    }
    if (t.id === "pp-max-parallel" || t.id === "chooser-max-parallel") {
      commitSplitMaxParallel(t);
    }
    // D1：规划后暂停确认 ↔ autoStartAfterPlan 取反
    if (t.id === "pp-pause-confirm") {
      state.autoStartAfterPlan = !t.checked;
      localStorage.setItem(PAUSE_CONFIRM_KEY, t.checked ? "1" : "0");
      toast(
        t.checked
          ? "已开启：规划后停在确认屏"
          : "已关闭：执行后自动开跑"
      );
    }
    // H2：显示已执行 — chooser / plan-rail / 计划管理 共用
    if (
      t.id === "chooser-show-executed" ||
      t.id === "plan-rail-show-executed" ||
      t.id === "plans-mgmt-show-executed"
    ) {
      if (typeof setShowExecutedPlans === "function") {
        setShowExecutedPlans(!!t.checked);
      } else {
        state.showExecutedPlans = !!t.checked;
        try {
          localStorage.setItem("cco.showExecutedPlans", t.checked ? "1" : "0");
        } catch (_) {}
        if (state.planChooserOpen && typeof renderPlanChooser === "function") {
          renderPlanChooser();
        }
        if (typeof renderPlanRail === "function") renderPlanRail();
        if (state.page === "plans" && typeof renderPlansMgmtPage === "function") {
          renderPlansMgmtPage();
        }
      }
    }
    // E4：计划管理「显示其它位置」（非 plans_dir）
    if (t.id === "plans-mgmt-show-other") {
      if (typeof renderPlansMgmtPage === "function") renderPlansMgmtPage();
    }
  });

  // Concurrency: allow empty while typing; clamp only on blur / Enter.
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
      commitSplitMaxParallel(t);
    }
  });
  document.addEventListener("input", (e) => {
    const t = e.target;
    // H1: plan full-view editor dirty tracking
    if (t?.id === "plan-full-editor") {
      if (typeof onPlanFullEditorInput === "function") onPlanFullEditorInput();
      return;
    }
    if (t?.id !== "chooser-max-parallel" && t?.id !== "pp-max-parallel") return;
    t.dataset.touched = "1";
    t.dataset.editing = "1";
    // Mirror valid partial number into hidden without rewriting the focused field.
    const typed = parseInt(t.value, 10);
    const hidden = $("#pp-max-parallel");
    if (Number.isFinite(typed) && typed > 0 && hidden && t !== hidden) {
      hidden.value = String(Math.max(1, Math.min(32, typed)));
    }
  });

  // CLI 状态过滤 chips
  document.addEventListener(
    "click",
    (e) => {
      const chip = e.target?.closest?.("#cli-status-filters [data-cli-filter]");
      if (!chip) return;
      e.preventDefault();
      const f = chip.getAttribute("data-cli-filter") || "all";
      state.cliStatusFilter = f;
      state.filterFailedOnly = f === "fail";
      renderCliBoard(state.live?.tasks || []);
    },
    true
  );

  // 初始高度 + 拖动手柄
  try {
    applyCliBodyHeight(state.cliBodyHeight === "auto" ? "auto" : state.cliBodyHeight || "auto");
    bindCliHeightGrip();
  } catch (_) {}
}

/** 兼容旧名：wire 只做委托注册，永不抛致命错 */
function wire() {
  try {
    applyLogFontSize(state.logFontSize);
  } catch (_) {}
  bindGlobalUI();
}

/** P2-4: URL query for detached system window (`?cco_window=monitor`). */
function parseCcoWindowBoot() {
  try {
    const q = new URLSearchParams(window.location.search || "");
    const role = (q.get("cco_window") || "").trim().toLowerCase();
    let project = q.get("project");
    if (project) {
      try {
        project = decodeURIComponent(project);
      } catch (_) {
        /* keep raw */
      }
    }
    return {
      isMonitor: role === "monitor",
      project: project && project.trim() ? project.trim() : null,
    };
  } catch (_) {
    return { isMonitor: false, project: null };
  }
}

/** Open/focus the system-level monitor window (Tauri only). */
async function openMonitorWindow() {
  if (!isTauriReady()) {
    toast("请在 CCO.app 内使用独立监视窗");
    return;
  }
  try {
    const res = await invoke("open_monitor_window_cmd", {
      project: state.selectedPath || null,
    });
    if (res?.created) toast("已打开独立监视窗（可拖到另一显示器）");
    else toast("已聚焦独立监视窗");
  } catch (e) {
    toast(String(e?.message || e));
  }
}

async function boot() {
  bindGlobalUI();
  // 等 invoke 就绪（最多 ~5s），期间 UI 按钮已可点
  let ready = isTauriReady();
  for (let i = 0; !ready && i < 100; i++) {
    await new Promise((r) => setTimeout(r, 50));
    ready = isTauriReady();
  }
  if (!ready) {
    const cs = $("#conn-status");
    if (cs) cs.textContent = "需要通过 CCO.app 启动";
    // 仍不阻断本地 UI
    return;
  }
  try {
    const meta = await invoke("meta");
    const cs = $("#conn-status");
    if (cs) cs.textContent = `桌面应用 · v${meta.version}`;
    await loadProjects();

    // P2-4: detached monitor window boots straight into workspace for one project.
    const bootWin = parseCcoWindowBoot();
    state.isMonitorWindow = !!bootWin.isMonitor;
    if (bootWin.isMonitor) {
      document.body.classList.add("cco-window-monitor");
      if (cs) cs.textContent = `监视窗 · v${meta.version}`;
      let path = bootWin.project;
      if (path && !(state.projects || []).some((p) => p.path === path)) {
        // Project may not be in list yet (path still valid on disk for live).
        path = bootWin.project;
      }
      if (!path && (state.projects || []).length === 1) {
        path = state.projects[0].path;
      }
      if (!path) {
        const active = (state.projects || []).find(
          (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
        );
        if (active) path = active.path;
      }
      if (path) {
        await selectProject(path);
        showPage("workspace");
        state.phase = state.phase === "pick" ? "running" : state.phase;
        try {
          await loadLive();
        } catch (_) {}
      } else {
        showPage("welcome");
        toast("监视窗：请先在主窗选择项目");
      }
      startPolling(1500);
      return;
    }

    // H0 冷启动：仅「有活动 run」的项目自动进执行；
    // 单项目无跑 → selectProject → chat 主窗（不再因历史 planJob 进计划页）
    const active = state.projects.find(
      (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
    );
    if (active) await selectProject(active.path);
    else if (state.projects.length === 1) await selectProject(state.projects[0].path);
    else if (state.projects.length > 0) goHome();
    else showPage("welcome");
    // 冷启动双保险：无活动 run 却落在 workspace → 强制 chat
    if (
      state.selectedPath &&
      state.page === "workspace" &&
      typeof hasActiveRun === "function" &&
      !hasActiveRun()
    ) {
      const st = String(state.planJob?.status || "").toLowerCase();
      if (state.phase !== "planning" && st !== "planning") {
        if (typeof openChatPage === "function") await openChatPage();
        else showPage("chat");
      }
    }
    startPolling();
  } catch (e) {
    console.error(e);
    const cs = $("#conn-status");
    if (cs) cs.textContent = "后端连接异常";
    toast(String(e?.message || e));
  }
}

function waitTauri() {
  bindGlobalUI();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => boot().catch(console.error));
  } else {
    boot().catch(console.error);
  }
}

// 立即绑定（脚本在 body 末尾，DOM 已有按钮）
bindGlobalUI();
waitTauri();
