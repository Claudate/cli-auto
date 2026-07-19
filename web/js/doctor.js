/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: doctor UI 片段
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
    if ($("#s-retry-max")) $("#s-retry-max").value = s.retry_max ?? 2;
    if ($("#s-stall-secs")) $("#s-stall-secs").value = s.stall_secs ?? 600;
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
  const pollVal = parseInt($("#s-poll-interval").value, 10);
  const modeVal = parseInt($("#s-default-mode").value, 10);
  const providerVal = $("#s-default-provider").value.trim();
  const maxParallelVal = parseInt($("#s-max-parallel").value, 10);
  const retryMaxVal = parseInt($("#s-retry-max")?.value, 10);
  const stallSecsVal = parseInt($("#s-stall-secs")?.value, 10);
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
    status.textContent = "自动重试次数需在 0–10 之间";
    status.hidden = false;
    return;
  }
  if (Number.isFinite(stallSecsVal) && (stallSecsVal < 30 || stallSecsVal > 7200)) {
    status.className = "save-status err";
    status.textContent = "卡死判定需在 30–7200 秒之间";
    status.hidden = false;
    return;
  }
  try {
    const updated = await invoke("set_settings_cmd", {
      update: {
        poll_interval_secs: pollVal,
        default_mode: modeVal,
        default_provider: providerVal,
        max_parallel: maxParallelVal || 2,
        retry_max: Number.isFinite(retryMaxVal) ? retryMaxVal : 2,
        stall_secs: Number.isFinite(stallSecsVal) ? stallSecsVal : 600,
      },
    });
    applyLogFontSize(fontVal);
    // sync picker defaults
    if ($("#pp-provider")) $("#pp-provider").value = providerVal;
    if ($("#pp-max-parallel") && !$("#pp-max-parallel").dataset.touched) {
      $("#pp-max-parallel").value = String(maxParallelVal || 2);
    }
    if ($("#chooser-max-parallel") && !$("#chooser-max-parallel").dataset.touched) {
      $("#chooser-max-parallel").value = String(maxParallelVal || 2);
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
  "btn-ws-resume": () => resumeRun(),
  "btn-ws-rework": () => startReworkWave(),
  "btn-ws-accept-residual": () => acceptRunResidual(),
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
    // 先打开面板，再扫计划——避免 invoke 失败导致「按钮像死了」
    openPlanChooser(true);
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
    // 弹窗化：顶栏「分配计划」打开合并弹窗，底部确认才执行
    openPlanChooser(true);
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
  "btn-pp-set-default": () => setDefaultPlan(),
  "btn-confirm-start": () => confirmAndStart(),
  "btn-replan": () => replanFromConfirm(),
  "btn-confirm-back": () => backFromConfirmToMonitor(),
  "btn-confirm-edit": () => beginConfirmEdit(),
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
  "btn-empty-to-chat": () => openChatPage(),
  "btn-chooser-to-chat": () => {
    openPlanChooser(false);
    return openChatPage();
  },
  "btn-chat-send": () => sendChatMessage(),
  "btn-chat-save": () => saveChatPlan(),
  "btn-chat-assign": () => assignFromChat(),
  "btn-chat-preview": () => previewChatPlan(),
  "btn-chat-env-doctor": () => openChatEnvDoctor(),
  "btn-chat-env-dismiss": () => dismissChatEnvBar(),
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

  // chat: Enter 发送（Shift+Enter 换行）
  document.addEventListener("keydown", (e) => {
    if (e.target?.id === "chat-input" && e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (!state.chatBusy) sendChatMessage();
    }
  });

  document.addEventListener(
    "click",
    (e) => {
      // plan chooser backdrop
      if (e.target?.id === "plan-chooser") {
        openPlanChooser(false);
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
          : "已关闭：分配后自动开跑"
      );
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
    const active = state.projects.find(
      (p) => p.running_tasks > 0 || isLiveStatus(p.active_status)
    );
    if (active) await selectProject(active.path);
    else if (state.projects.length === 1) await selectProject(state.projects[0].path);
    else if (state.projects.length > 0) goHome();
    else showPage("welcome");
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
