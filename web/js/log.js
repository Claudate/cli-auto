/**
 * [INPUT]: 依赖 window 全局（顺序加载）；Tauri invoke
 * [OUTPUT]: log UI 片段
 * [POS]: web/js D4 自 app.js 纵切；无构建器，顺序 script 共享全局
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */
/* cco desktop — log */

function isAiInteractionEvent(e) {
  if (!e) return false;
  const k = String(e.kind || "").toLowerCase();
  // 红框3：CLI 只保留 AI 对话 / 工具调用 / 结果
  if (k === "message" || k === "tool_use" || k === "tool_result" || k === "result") return true;
  // 业务级 error 可保留；stderr / meta / system 噪音一律丢弃
  if (k === "error") {
    const lvl = String(e.level || "").toLowerCase();
    if (lvl === "debug" || lvl === "trace") return false;
    const blob = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
    if (isNoiseText(blob)) return false;
    return true;
  }
  return false;
}

/** P1-1：日志面板内容签名；相同则跳过 innerHTML 重绘 */
function logPanelSignature(t) {
  if (!t) return "";
  const mode = state.logViewMode || "term";
  const events = Array.isArray(t.log_events) ? t.log_events : [];
  const last = events.length ? events[events.length - 1] : null;
  const lastId = last?.id || "";
  const lastSum = last ? `${last.kind}|${last.title}|${String(last.summary || "").slice(0, 40)}` : "";
  return [
    t.task_id || "",
    t.status || "",
    t.log_bytes ?? "",
    events.length,
    lastId,
    lastSum,
    mode,
    t.error_summary || t.error || "",
    t.cost_usd ?? "",
  ].join("\0");
}

function isNearBottom(el, px = 48) {
  if (!el) return true;
  return el.scrollHeight - el.scrollTop - el.clientHeight < px;
}

/** Planner / generic：用 LogConsole 渲染事件列表（P1-3） */
function renderLogConsoleHtml(events, { mode, emptyText, rawText } = {}) {
  const m = mode || state.logViewMode || "term";
  const list = Array.isArray(events) ? events : [];
  if (m === "raw") {
    const plain =
      rawText ||
      list
        .map((ev) => [ev.kind, ev.title, ev.summary].filter(Boolean).join("\t"))
        .join("\n");
    if (!plain) {
      return `<div class="cli-empty-ai muted">${esc(emptyText || "等待输出…")}</div>`;
    }
    return '<pre class="panel-log-pre">' + esc(plain) + "</pre>";
  }
  if (!list.length) {
    return `<div class="cli-empty-ai muted">${esc(emptyText || "等待输出…")}</div>`;
  }
  if (m === "pretty") {
    return list.slice(-60).map((e) => renderLogEvent(e)).join("") || "";
  }
  // term transcript：planner 人类行用 raw_line / meta，也允许 message/tool
  return (
    list
      .slice(-80)
      .map((e) => {
        // planner 多为 raw_line：用精简 transcript；AI 行走原逻辑
        const k = String(e.kind || "").toLowerCase();
        if (k === "raw_line" || k === "meta" || k === "stderr") {
          const body = esc(e.summary || e.title || "…");
          const role = k === "stderr" ? "stderr" : k === "meta" ? "meta" : "out";
          return `<div class="tx-line role-${role}"><div class="tx-role">${role === "out" ? "log" : role}</div><div class="tx-body">${body}</div></div>`;
        }
        return renderTranscriptLine(e);
      })
      .filter(Boolean)
      .join("") || ""
  );
}

function fillPlannerLog(view) {
  const logEl = $("#planner-log");
  if (!logEl) return;
  const events =
    view?.planner_log_events ||
    view?.plannerLogEvents ||
    [];
  const tail =
    view?.planner_log_tail ||
    view?.plannerLogTail ||
    "";
  // 无事件时用 tail 拆行伪事件，避免 raw 墙
  let evs = Array.isArray(events) ? events : [];
  if (!evs.length && tail) {
    evs = String(tail)
      .split("\n")
      .filter((l) => l.trim() && !l.startsWith("… ("))
      .slice(-80)
      .map((line, i) => ({
        id: `p${i}`,
        kind: "raw_line",
        stream: "stdout",
        title: "log",
        summary: line.replace(/^\[[^\]]+\]\s*/, ""),
        level: /fail|error/i.test(line) ? "error" : "info",
      }));
  }
  const sig = [
    evs.length,
    evs.length ? evs[evs.length - 1]?.id || evs[evs.length - 1]?.summary : "",
    tail.length,
    state.logViewMode || "term",
  ].join("|");
  if (logEl.dataset.sig === sig) return;
  const stick = isNearBottom(logEl);
  logEl.classList.add("log-console", "term-mode");
  logEl.classList.toggle("pretty-mode", (state.logViewMode || "term") === "pretty");
  logEl.classList.toggle("raw-mode", (state.logViewMode || "term") === "raw");
  logEl.innerHTML = renderLogConsoleHtml(evs, {
    mode: state.logViewMode || "term",
    emptyText: "正在分析…",
    rawText: tail,
  });
  logEl.dataset.sig = sig;
  if (stick) logEl.scrollTop = logEl.scrollHeight;
}

async function openExternalTerminal(taskId) {
  const runId = state.live?.run_id;
  if (!runId || !taskId) return toast("无运行中的任务日志可跟随");
  try {
    const session = await invoke("open_task_terminal_cmd", {
      runId,
      taskId,
      kind: "external",
    });
    const launcher = session?.launcher || "terminal";
    toast(`已打开外置终端（${launcher}）跟随 ${taskId}`);
  } catch (e) {
    toast(String(e?.message || e));
  }
}

function isNoiseText(s) {
  const t = String(s || "");
  if (!t.trim()) return true;
  if (/Ignoring\s+--allowedTools/i.test(t)) return true;
  if (/Ignoring\s+--[\w-]+/i.test(t)) return true;
  if (/^\s*stderr\b/i.test(t)) return true;
  if (/^\s*\[?(system|meta|debug|trace)\]?/i.test(t)) return true;
  if (/permission\s*prompt/i.test(t) && /allowedTools/i.test(t)) return true;
  if (/CLI\s*warning/i.test(t)) return true;
  if (/deprecated\s+flag/i.test(t)) return true;
  if (/node:?\s*warn/i.test(t)) return true;
  if (/experimental\s+feature/i.test(t)) return true;
  if (/^\s*warn(ing)?:/i.test(t)) return true;
  return false;
}

function aiLogPlainText(t) {
  const events = (Array.isArray(t?.log_events) ? t.log_events : [])
    .filter(isAiInteractionEvent)
    .filter((ev) => String(ev.kind || "").toLowerCase() !== "result");
  if (events.length) {
    return events
      .map((ev) => {
        const kind = ev.kind || "";
        const title = ev.title || "";
        const summary = ev.summary || "";
        return [kind, title, summary].filter(Boolean).join("\t");
      })
      .join("\n");
  }
  // 无结构化事件时：不回落整段 log_tail，避免系统噪音污染
  if (isLiveStatus(t?.status)) return "AI 运行中，等待交互输出…";
  if (isFailedStatus(t?.status)) return t?.error ? String(t.error) : "任务失败，无 AI 交互日志。";
  return "";
}

function panelLogHtml(t) {
  const st = String(t.status || "").toLowerCase();
  const events = (Array.isArray(t.log_events) ? t.log_events : []).filter(isAiInteractionEvent);
  const mode = state.logViewMode || "term";

  // 默认 term / pretty：只渲染 AI 事件，绝不 dump 原始 log_tail
  // result 摘要不进黑区（成功态窗外徽章已表达）
  const viewEvents = events.filter((e) => String(e.kind || "").toLowerCase() !== "result");
  if (mode !== "raw") {
    if (!viewEvents.length) {
      if (isLiveStatus(st)) {
        return '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>';
      }
      if (isFailedStatus(st)) {
        const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
        return err
          ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
          : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>';
      }
      // 完成且仅有 result 摘要：黑区留空，成功由窗外徽章表达
      return "";
    }
    if (mode === "pretty") {
      return viewEvents.slice(-40).map((e) => renderLogEvent(e)).join("") || "";
    }
    return (
      viewEvents
        .slice(-50)
        .map((e) => renderTranscriptLine(e))
        .filter(Boolean)
        .join("") || ""
    );
  }

  // raw 模式：执行交互文本；result 摘要已在 aiLogPlainText 过滤
  const plain = aiLogPlainText(t);
  if (!plain) {
    if (isLiveStatus(st)) return '<div class="cli-empty-ai muted">AI 运行中，等待交互输出…</div>';
    if (isFailedStatus(st)) {
      const err = t.error && !isNoiseText(t.error) ? esc(String(t.error).slice(0, 240)) : "";
      return err
        ? `<div class="cli-empty-ai muted">任务失败<br/>${err}</div>`
        : '<div class="cli-empty-ai muted">任务失败，无执行输出</div>';
    }
    return "";
  }
  return '<pre class="panel-log-pre">' + esc(plain) + "</pre>";
}

function renderCliBoard(tasks) {
  const shell = $("#cli-shell");
  // Preserve outer shell scroll: re-layout / height fit must not yank the user back to top.
  const shellScrollTop = shell ? shell.scrollTop : 0;
  const __fitAfter = () => {
    if (state.cliBodyHeight === "auto") {
      requestAnimationFrame(() => {
        const before = shell ? shell.scrollTop : 0;
        const prevH = document.documentElement.style.getPropertyValue("--cli-body-h");
        fitCliBodyHeight();
        const nextH = document.documentElement.style.getPropertyValue("--cli-body-h");
        // Only second pass when height actually changed (avoids thrash).
        if (prevH !== nextH) {
          requestAnimationFrame(() => fitCliBodyHeight());
        }
        if (shell && shell.scrollTop !== before) shell.scrollTop = before;
        if (shell && shellScrollTop > 0 && shell.scrollTop === 0) {
          shell.scrollTop = shellScrollTop;
        }
      });
    } else if (shell && shellScrollTop > 0) {
      // Restore even without auto-fit.
      requestAnimationFrame(() => {
        if (shell.scrollTop === 0) shell.scrollTop = shellScrollTop;
      });
    }
  };

  const board = $("#cli-board");
  if (!board) return;

  let shown = tasks;
  // 兼容旧 filterFailedOnly
  let filter = state.cliStatusFilter || "all";
  if (state.filterFailedOnly && filter === "all") filter = "fail";
  if (filter && filter !== "all") {
    const filtered = tasks.filter((t) => taskBucket(t.status) === filter);
    // 无匹配时不回退，展示空板 + 过滤态更清晰
    shown = filtered;
  }
  // 同步过滤 chip 高亮
  $$("#cli-status-filters [data-cli-filter]").forEach((btn) => {
    const f = btn.getAttribute("data-cli-filter") || "all";
    btn.classList.toggle("active", f === filter);
  });
  // 单任务时工具条更安静（字号/视图保留）
  const toolbar = document.querySelector(".board-toolbar");
  if (toolbar) toolbar.classList.toggle("quiet", tasks.length <= 1);

  const closedCount = Object.keys(state.closedPanels || {}).filter((id) =>
    tasks.some((t) => t.task_id === id)
  ).length;
  const restoreBtn = $("#btn-restore-panels");
  if (restoreBtn) {
    restoreBtn.hidden = closedCount === 0;
    restoreBtn.textContent = `恢复已关闭 (${closedCount})`;
  }

  // 可见面板：运行中最上，未运行居中，已完成/失败最底
  const visible = sortTasksByStatus(
    shown.filter((t) => !state.closedPanels[t.task_id])
  );
  // 自动布局：网格，若用户拖过则用绝对坐标
  const cols = Math.max(1, Math.min(2, visible.length));
  board.classList.toggle("single", visible.length === 1);
  board.classList.add("cols-2");
  board.dataset.cols = "2";
  // 强制布局属性，防止旧 inline / 缓存样式
  board.style.display = "grid";
  board.style.gridTemplateColumns = "calc((100% - 0.75rem) / 2) calc((100% - 0.75rem) / 2)";
  board.style.gap = "0.75rem";
  board.style.overflowX = "hidden";
  if (state.cliBodyHeight === "auto") {
    document.documentElement.setAttribute("data-cli-h", "auto");
  } else {
    document.documentElement.removeAttribute("data-cli-h");
    document.documentElement.style.setProperty(
      "--cli-body-h",
      (Number(state.cliBodyHeight) || 300) + "px"
    );
  }
  // P1-1：任务集合未变时增量更新 body，避免 2s 全量 innerHTML 闪烁
  const visKey = visible.map((t) => t.task_id).join("|") + "#" + (state.logViewMode || "term");
  const canPatch =
    board.dataset.visKey === visKey &&
    board.querySelectorAll(".cli-window").length === visible.length &&
    !board.querySelector(".cli-board-empty");

  if (!visible.length) {
    if (board.dataset.visKey !== "empty:" + filter) {
      board.innerHTML = "";
      const empty = document.createElement("div");
      empty.className = "cli-board-empty muted";
      empty.style.gridColumn = "1 / -1";
      empty.style.padding = "1.2rem";
      empty.style.textAlign = "center";
      const f = state.cliStatusFilter || "all";
      empty.textContent =
        f === "all"
          ? "暂无 CLI 窗口"
          : `当前过滤（${
              { run: "运行中", wait: "待运行", done: "已完成", fail: "失败" }[f] || f
            }）无匹配任务`;
      board.appendChild(empty);
      board.dataset.visKey = "empty:" + filter;
      state.logPanelSig = {};
    }
    __fitAfter();
    return;
  }

  if (!canPatch) {
    board.innerHTML = "";
    state.logPanelSig = {};
    board.dataset.visKey = visKey;
  }

  visible.forEach((t, idx) => {
    const st = String(t.status || "").toLowerCase();
    const failed = isFailedStatus(st);
    const title = t.title || t.task_id;
    const elapsed = formatElapsed(t.started_at, t.finished_at);
    const sum = taskErrorSummary(t);
    const pos = state.panelPos[t.task_id];
    let card = canPatch
      ? board.querySelector(`.cli-window[data-task="${CSS.escape(t.task_id)}"]`)
      : null;
    const half = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
    const usableFree =
      pos &&
      typeof pos.x === "number" &&
      typeof pos.y === "number" &&
      state.dragSession &&
      state.dragSession[t.task_id];

    if (!card) {
      card = document.createElement("div");
      card.dataset.task = t.task_id;
      board.appendChild(card);
      // force full chrome build
      card.dataset.chrome = "";
    }

    card.className = `cli-window${failed ? " failed" : ""}${
      t.task_id === state.selectedTaskId ? " selected" : ""
    }`;
    if (usableFree) {
      card.classList.add("free");
      card.style.left = pos.x + "px";
      card.style.top = pos.y + "px";
      card.style.width = half + "px";
      card.style.maxWidth = half + "px";
    } else {
      card.classList.remove("free");
      card.dataset.slot = String(idx);
      card.style.left = "";
      card.style.top = "";
      card.style.width = "";
      card.style.maxWidth = "";
      card.style.gridColumn = "";
    }

    // Do NOT include `elapsed` in chromeSig — it ticks every poll and used to
    // wipe the whole card (resetting log scroll to top). Elapsed is light-updated.
    const chromeSig = [
      t.status,
      title,
      t.cost_usd != null ? Number(t.cost_usd).toFixed(4) : "",
      t.provider || "",
      sum || "",
      failed ? 1 : 0,
      !isLiveStatus(state.live?.run_status) && state.live?.run_id ? 1 : 0,
      isLiveStatus(st) ? 1 : 0,
      t.attempt || 0,
      t.last_retry_reason || "",
    ].join("|");

    if (card.dataset.chrome !== chromeSig) {
      // Preserve log scroll across chrome rebuild (status/badge/cost changes).
      const prevBody = card.querySelector(".cli-window-body");
      const prevScroll = prevBody ? prevBody.scrollTop : 0;
      const wasNearBottom = prevBody ? isNearBottom(prevBody) : true;
      card.innerHTML = `
      <div class="cli-window-head" data-drag="${esc(t.task_id)}">
        <div class="cli-window-title">
          <span class="dot ${statusDot(st)}"></span>
          <strong title="${esc(title)}">${esc(title)}</strong>
          ${badge(t.status)}
        </div>
        <div class="cli-window-actions">
          ${
            !isLiveStatus(state.live?.run_status) && state.live?.run_id
              ? `<button type="button" class="btn primary sm cli-rerun-btn" data-rerun="${esc(t.task_id)}" title="再跑一次">再跑一次</button>`
              : ""
          }
          <button type="button" class="icon-btn sm" data-focus="${esc(t.task_id)}" title="聚焦">◉</button>
          <button type="button" class="icon-btn sm" data-close="${esc(t.task_id)}" title="关闭窗口">×</button>
        </div>
      </div>
      <div class="cli-window-meta muted">
        ${esc(t.task_id)} · ${esc(elapsed)}${
          t.cost_usd != null ? ` · $${Number(t.cost_usd).toFixed(4)}` : ""
        }${t.provider ? ` · ${esc(t.provider)}` : ""}${
          t.attempt && t.attempt > 1
            ? ` · 第 ${t.attempt} 次${t.last_retry_reason ? "·" + esc(t.last_retry_reason) : ""}`
            : ""
        }
      </div>
      ${
        sum && failed
          ? `<div class="cli-window-err" title="${esc(sum)}">${esc(sum)}</div>`
          : ""
      }
      <div class="cli-window-body log-console term-mode" data-log="${esc(t.task_id)}"></div>
      <div class="cli-window-foot">
        <button type="button" class="btn ghost sm" data-copy="${esc(t.task_id)}">复制</button>
        <button type="button" class="btn ghost sm" data-extterm="${esc(t.task_id)}" title="在系统终端 tail -f 日志">外置终端</button>
        <button type="button" class="btn danger sm" data-stop="${esc(t.task_id)}" ${
          isLiveStatus(st) ? "" : "hidden"
        }>停止</button>
      </div>`;
      card.dataset.chrome = chromeSig;
      // chrome rebuild invalidates log body sig
      delete state.logPanelSig[t.task_id];
      // stash so the body fill below can restore scroll
      card.dataset.prevScroll = String(prevScroll);
      card.dataset.wasNearBottom = wasNearBottom ? "1" : "0";
    } else {
      // light meta/badge refresh without wiping log body
      const meta = card.querySelector(".cli-window-meta");
      if (meta) {
        meta.textContent = `${t.task_id} · ${elapsed}${
          t.cost_usd != null ? ` · $${Number(t.cost_usd).toFixed(4)}` : ""
        }${t.provider ? ` · ${t.provider}` : ""}${
          t.attempt && t.attempt > 1
            ? ` · 第 ${t.attempt} 次${t.last_retry_reason ? "·" + t.last_retry_reason : ""}`
            : ""
        }`;
      }
      const stopBtn = card.querySelector("[data-stop]");
      if (stopBtn) stopBtn.hidden = !isLiveStatus(st);
    }

    const body = card.querySelector(".cli-window-body");
    if (body) {
      body.style.height = "";
      body.style.maxHeight = "";
      body.style.minHeight = "";
      const sig = logPanelSignature(t);
      if (state.logPanelSig[t.task_id] !== sig) {
        const stick =
          card.dataset.wasNearBottom === "1" ||
          (card.dataset.wasNearBottom == null && isNearBottom(body));
        const keepScroll = parseInt(card.dataset.prevScroll || "0", 10) || 0;
        body.innerHTML = panelLogHtml(t);
        state.logPanelSig[t.task_id] = sig;
        if (stick) {
          body.scrollTop = body.scrollHeight;
        } else if (keepScroll > 0) {
          body.scrollTop = keepScroll;
        }
        delete card.dataset.prevScroll;
        delete card.dataset.wasNearBottom;
      } else if (card.dataset.prevScroll != null) {
        // chrome rebuilt but log content unchanged — still restore scroll
        const stick = card.dataset.wasNearBottom === "1";
        const keepScroll = parseInt(card.dataset.prevScroll || "0", 10) || 0;
        if (stick) body.scrollTop = body.scrollHeight;
        else if (keepScroll > 0) body.scrollTop = keepScroll;
        delete card.dataset.prevScroll;
        delete card.dataset.wasNearBottom;
      }
    }
  });

  // remove stale cards + re-order to match visible sort when patching
  if (canPatch) {
    const keep = new Set(visible.map((t) => t.task_id));
    $$(".cli-window", board).forEach((el) => {
      if (!keep.has(el.dataset.task)) el.remove();
    });
    // Only reorder when order actually changed — appendChild on every poll
    // moves DOM nodes and can jump the outer .cli-shell scroll.
    const kids = $$(".cli-window", board);
    let needsReorder =
      kids.length !== visible.length ||
      kids.some((el, i) => el.dataset.task !== visible[i]?.task_id);
    if (needsReorder) {
      visible.forEach((t) => {
        const el = board.querySelector(
          `.cli-window[data-task="${CSS.escape(t.task_id)}"]`
        );
        if (el) board.appendChild(el);
      });
    }
  }
  // Pin outer shell scroll after any DOM churn.
  if (shell && shellScrollTop > 0) {
    shell.scrollTop = shellScrollTop;
  }

  // events (rebind only on full structure rebuild — capture-phase document handler covers clicks)
  $$("[data-close]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      state.closedPanels[b.dataset.close] = true;
      renderCliBoard(tasks);
    };
  });
  $$("[data-focus]", board).forEach((b) => {
    b.onclick = (e) => {
      e.stopPropagation();
      state.selectedTaskId = b.dataset.focus;
      const card = board.querySelector(`.cli-window[data-task="${CSS.escape(b.dataset.focus)}"]`);
      if (card) {
        card.style.zIndex = String(Date.now() % 100000);
        card.classList.add("selected");
      }
    };
  });
  $$("[data-copy]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      const t = tasks.find((x) => x.task_id === b.dataset.copy);
      const text = aiLogPlainText(t);
      try {
        await navigator.clipboard.writeText(text || "");
        toast(text ? "AI 日志已复制" : "暂无 AI 交互可复制");
      } catch (_) {
        toast("复制失败");
      }
    };
  });
  $$("[data-extterm]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      await openExternalTerminal(b.dataset.extterm);
    };
  });
  $$("[data-stop]", board).forEach((b) => {
    b.onclick = async (e) => {
      e.stopPropagation();
      state.selectedTaskId = b.dataset.stop;
      await cancelTask();
    };
  });

  // drag move
  $$("[data-drag]", board).forEach((head) => {
    head.onpointerdown = (ev) => {
      if (ev.button !== 0) return;
      if (ev.target.closest("button")) return;
      const id = head.dataset.drag;
      const card = head.closest(".cli-window");
      if (!card) return;
      const rect = card.getBoundingClientRect();
      const boardRect = board.getBoundingClientRect();
      card.classList.add("free");
      const x = rect.left - boardRect.left + board.scrollLeft;
      const y = rect.top - boardRect.top + board.scrollTop;
      const half = Math.max(260, Math.floor((board.clientWidth - 12) / 2));
      card.style.left = x + "px";
      card.style.top = y + "px";
      card.style.width = Math.min(rect.width || half, half * 1.15) + "px";
      card.style.zIndex = String(Date.now() % 100000);
      state.drag = {
        id,
        ox: ev.clientX - rect.left,
        oy: ev.clientY - rect.top,
      };
      head.setPointerCapture(ev.pointerId);
    };
    head.onpointermove = (ev) => {
      if (!state.drag || state.drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const boardRect = board.getBoundingClientRect();
      let x = ev.clientX - boardRect.left - state.drag.ox + board.scrollLeft;
      let y = ev.clientY - boardRect.top - state.drag.oy + board.scrollTop;
      x = Math.max(0, x);
      y = Math.max(0, y);
      card.style.left = x + "px";
      card.style.top = y + "px";
    };
    head.onpointerup = (ev) => {
      if (!state.drag || state.drag.id !== head.dataset.drag) return;
      const card = head.closest(".cli-window");
      const id = state.drag.id;
      state.drag = null;
      const halfW = Math.max(200, Math.floor((board.clientWidth - 12) / 2));
      state.panelPos[id] = {
        x: parseFloat(card.style.left) || 0,
        y: parseFloat(card.style.top) || 0,
        w: halfW,
      };
      state.dragSession = state.dragSession || {};
      state.dragSession[id] = true;
      card.style.width = halfW + "px";
      card.style.maxWidth = halfW + "px";
      savePanelPos();
      try {
        head.releasePointerCapture(ev.pointerId);
      } catch (_) {}
    };
  });
  __fitAfter();
}

function renderTaskList(tasks) {
  // 兼容旧调用：转交看板
  renderCliBoard(tasks);
}

function renderDetailLog(tasks) {
  // 紧凑多窗口模式下，日志已在各窗口内；保留隐藏 detail 同步以便复制按钮
  const t = tasks.find((x) => x.task_id === state.selectedTaskId) || tasks[0];
  if (!t) return;
  const logEl = $("#cli-detail-log");
  if (logEl) {
    logEl.textContent = t.log_tail || "";
  }
  const stop = $("#btn-stop-task");
  if (stop) stop.hidden = !isLiveStatus(t.status);
}

function transcriptRole(e) {
  const k = String(e.kind || "");
  if (k === "tool_use" || k === "tool_result") return "tool";
  if (k === "message") return "assistant";
  if (k === "result") return "result";
  if (k === "error") return "error";
  if (k === "stderr") return "stderr";
  if (k === "meta") return "meta";
  if (k === "raw_line") return "out";
  return k || "out";
}

function renderTranscriptLine(e) {
  if (!isAiInteractionEvent(e)) return "";
  const role = transcriptRole(e);
  const label =
    role === "tool"
      ? e.kind === "tool_result"
        ? "result"
        : "tool"
      : role === "assistant"
        ? "asst"
        : role === "out"
          ? "out"
          : role;
  const noiseProbe = `${e.title || ""} ${e.summary || ""} ${e.detail || ""}`;
  if (isNoiseText(noiseProbe)) return "";
  const title = e.title && e.title !== label ? esc(e.title) : "";
  const summary = esc(e.summary || "");
  // tool_result / result 默认不把超长 detail 塞进 CLI 主视图
  const detail =
    e.detail && e.kind !== "result" && e.kind !== "tool_result" ? esc(e.detail) : "";
  let body = "";
  if (title && summary) body = `<span style="opacity:.85">${title}</span>  ${summary}`;
  else body = summary || title || "…";
  // 黑区只留执行交互；result success/$cost 由窗外徽章表达
  if (e.kind === "result") return "";
  if (e.kind === "tool_result") {
    const short = (summary || title || "完成").slice(0, 280);
    return `<div class="tx-line role-result">
      <div class="tx-role">tool✓</div>
      <div class="tx-body">${short}</div>
    </div>`;
  }
  if (detail && e.kind === "tool_use" && detail.length > 160) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  if (detail && e.kind === "message" && detail.length > 220) {
    return `<div class="tx-line role-${esc(role)}">
      <div class="tx-role">${esc(label)}</div>
      <div class="tx-body">${body}
        <details class="tx-fold" style="margin-top:.15rem"><summary>…</summary><pre>${detail}</pre></details>
      </div>
    </div>`;
  }
  return `<div class="tx-line role-${esc(role)}">
    <div class="tx-role">${esc(label)}</div>
    <div class="tx-body">${body}</div>
  </div>`;
}

function renderLogEvent(e) {
  const kind = esc(e.kind || "raw_line");
  const level = esc(e.level || "info");
  const title = esc(e.title || kind);
  const summary = esc(e.summary || "");
  const detail = e.detail ? esc(e.detail) : "";
  const detailBlock = detail
    ? `<details><summary>展开详情</summary><div class="detail">${detail}</div></details>`
    : "";
  return `<div class="log-event kind-${kind} level-${level}">
    <div class="kind">${kind}</div>
    <div class="body">
      <div class="title">${title}</div>
      <div class="summary">${summary}</div>
      ${detailBlock}
    </div>
  </div>`;
}


async function cancelTask() {
  const runId = state.live?.run_id;
  const taskId = state.selectedTaskId;
  if (!runId || typeof runId !== 'string' || !runId.trim()) {
    return toast("无活动任务");
  }
  if (!taskId) return toast("请先选择任务");
  try {
    await invoke("stop_task_cmd", { runId, taskId });
    toast(`已停止 ${taskId}`);
    await loadLive();
    await loadProjects();
  } catch (e) {
    toast(String(e));
  }
}

async function stopAll() {
  const runId = state.live?.run_id;
  // 防御性检查：确保 run_id 有意义
  if (!runId || typeof runId !== 'string' || runId.trim() === '') {
    console.warn("stopAll: no valid run_id, skipping");
    return;
  }
  try {
    await invoke("stop_run_cmd", { runId });
    toast("已请求全部停止");
    await loadLive();
    await loadProjects();
    // Unlock project switch / plan choose after stop.
    if (state.phase === "running") state.phase = "done";
    renderProjectList();
    renderPlanPicker();
    updateSplitPlanChip();
    renderWorkspace();
  } catch (e) {
    toast(String(e));
  }
}

async function resumeRun() {
  const runId = state.live?.run_id;
  if (!runId || typeof runId !== 'string' || !runId.trim()) {
    return toast("无运行记录可继续");
  }
  try {
    await invoke("resume_run_cmd", { runId });
    toast("正在继续…");
    setTimeout(() => {
      loadLive().catch(() => {});
      loadProjects().catch(() => {});
    }, 800);
  } catch (e) {
    toast(String(e));
  }
}

/** P-loop L2: start rework wave from current run's inspect ISSUES. */
async function startReworkWave() {
  const runId = state.live?.run_id;
  if (!runId || typeof runId !== "string" || !runId.trim()) {
    return toast("无运行记录可回补");
  }
  try {
    const res = await invoke("start_rework_cmd", { runId });
    toast(res?.message || `回补已启动 · ${res?.run_id || ""}`);
    state.phase = "running";
    setTimeout(() => {
      loadLive().catch(() => {});
      loadProjects().catch(() => {});
    }, 600);
  } catch (e) {
    toast(String(e));
  }
}

/** P-loop L2: explicit accept residual → handoff open_risks. */
async function acceptRunResidual() {
  const runId = state.live?.run_id;
  if (!runId || typeof runId !== "string" || !runId.trim()) {
    return toast("无运行记录");
  }
  const note =
    typeof window.prompt === "function"
      ? window.prompt("接受残留说明（将写入 handoff open_risks）", "用户显式接受巡检残留")
      : "用户显式接受巡检残留";
  if (note === null) return;
  try {
    await invoke("accept_residual_cmd", { runId, note: note || null });
    toast("已记录「接受残留」");
    setTimeout(() => {
      loadLive().catch(() => {});
    }, 400);
  } catch (e) {
    toast(String(e));
  }
}

/* ── Doctor page ── */
async function loadDoctor() {
  try {
    const d = await invoke("doctor_cmd", { project: state.selectedPath || null });
    state.doctorCache = { ok: !!d.ok, at: Date.now(), lines: d.lines || [] };
    const lines = d.lines || [];
    $("#doctor-list").innerHTML = `<table>
      <thead><tr><th>检查项</th><th>结果</th><th>详情</th></tr></thead>
      <tbody>
        ${lines
          .map(
            (l) => `<tr>
          <td>${esc(l.name)}</td>
          <td>${l.ok ? badge("ok") : badge("failed")}</td>
          <td class="muted">${esc(l.detail)}</td>
        </tr>`
          )
          .join("")}
      </tbody>
    </table>
    <p class="muted" style="margin-top:.75rem">${d.ok ? "关键检查通过" : "存在失败项，请按详情处理"}</p>`;
    renderDoctorWarn();
  } catch (e) {
    toast(String(e));
  }
}

/* ── Poll ── */
