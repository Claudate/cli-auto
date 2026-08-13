/**
 * [INPUT]: SplitViewModel · 既有 DOM ids（confirm-* / split-*）
 * [OUTPUT]: 三栏绑定 + 意图转发；只发意图
 * [POS]: A3-1/A3-2 SplitView；禁止 invoke / start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { cardsHtml } from "./splitRender.js";
import {
  ensureAdvancedRouteDom,
  paintDetail,
  paintChrome,
  toast,
  hasActiveRun,
  isRunPaused,
  canEditTask,
  $,
} from "./splitDetail.js";
import * as gateway from "../../shared/gateway.js";
import { confirmDialog } from "../../shared/confirmDialog.js";

function g(name) {
  const w = typeof window !== "undefined" ? window : globalThis;
  return w[name];
}

/**
 * Bind three-column desk to a ViewModel. Call once; re-render via render().
 * @param {ReturnType<import("./SplitViewModel.js").createSplitViewModel>} vm
 * @param {object} [bridge]
 */
export function bindSplitView(vm, bridge = {}) {
  ensureAdvancedRouteDom();
  let wavesDirty = false;
  let confirmInFlight = false;

  function legacy() {
    return (typeof bridge.getLegacy === "function" && bridge.getLegacy()) || {};
  }

  function syncLegacy(patch) {
    if (typeof bridge.syncLegacy === "function") bridge.syncLegacy(patch);
  }

  function afterMutate() {
    if (typeof bridge.afterMutate === "function") bridge.afterMutate();
  }

  function pullFromLegacy() {
    const L = legacy();
    if (L.planJob || L.planJobId) {
      vm.setJob(L.planJob || null, {
        jobId: L.planJobId || null,
        selectedTaskId: L.confirmTaskId ?? undefined,
        editing: L.confirmEditing,
      });
    }
  }

  function pushSelection() {
    const s = vm.getSnapshot();
    syncLegacy({
      planJob: s.job,
      planJobId: s.jobId,
      confirmTaskId: s.selectedTaskId,
      confirmEditing: s.editing,
    });
  }

  function render() {
    pullFromLegacy();
    const s = vm.getSnapshot();
    const job = s.job;
    if (!job) return;

    if (typeof bridge.fillMeta === "function") {
      try {
        bridge.fillMeta(job);
      } catch (e) {
        console.error("[SplitView] fillMeta", e);
      }
    }

    const tasks = job.tasks || [];
    const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));
    // 确认台锁定只认「本 job 的 run」：残留的历史 live / 其它计划运行不锁本拆分，
    // 否则高级折叠里的执行通道 / 角色 / 范围 全部不可点。
    // 真正开跑中的重复启动仍由 confirmAndStart 里 hasActiveRun() 兜底。
    const jrid = job?.run_id || job?.runId || null;
    const live = g("state")?.live;
    const liveIsThisJob =
      !!jrid && !!live?.run_id && String(live.run_id) === String(jrid);
    const runLocked = liveIsThisJob && hasActiveRun();
    // 卡片 live 状态同样只认「本 job 的 run」：任务 id（t1/t2…）跨 job 复用，
    // 历史 run 会把新拆分的卡片标成完成/失败，并误禁用可选步骤的勾选框。
    const liveTask = (id) => {
      if (!liveIsThisJob) return null;
      const fn = g("liveTaskById");
      return typeof fn === "function" ? fn(id) : null;
    };
    const selectedId = s.selectedTaskId;

    // 波次轨已并入步骤列表（按执行顺序 + 并行外框）；左侧栏隐藏
    const tl = $("split-timeline");
    if (tl) {
      tl.hidden = true;
      tl.innerHTML = "";
    }

    const waves = $("confirm-waves");
    if (waves) {
      const rebuildWaves = true;
      if (rebuildWaves) {
        wavesDirty = false;
        waves.innerHTML = cardsHtml(job, byId, {
          runLocked,
          selectedId,
          liveTask,
          jobProvider: job.provider,
        });
        waves.querySelectorAll(".wave-task").forEach((b) => {
          b.onclick = () => {
            if (vm.getSnapshot().editing) {
              toast("请先保存或取消当前编辑");
              return;
            }
            vm.selectTask(b.dataset.id);
            pushSelection();
            render();
          };
        });
        waves.querySelectorAll(".wave-opt-check").forEach((cb) => {
          cb.onchange = async (ev) => {
            ev.stopPropagation();
            if (vm.getSnapshot().editing) {
              cb.checked = !cb.checked;
              toast("请先保存或取消当前编辑");
              return;
            }
            if (hasActiveRun()) {
              cb.checked = !cb.checked;
              toast("运行中不可改勾选");
              return;
            }
            const taskId = cb.dataset.id;
            const include = !!cb.checked;
            try {
              await vm.setInclude(taskId, include);
              pushSelection();
              const title = byId[taskId]?.title || taskId;
              toast(
                include
                  ? `已勾选：将执行「${title}」`
                  : `已取消：不跑「${title}」`
              );
              afterMutate();
              render();
            } catch (e) {
              cb.checked = !include;
              toast(String(e?.message || e));
            }
          };
          cb.onclick = (ev) => ev.stopPropagation();
        });
      } else {
        if (!wavesDirty) {
          wavesDirty = true;
          setTimeout(() => { if (wavesDirty) render(); }, 300);
        }
      }
    }

    paintDetail({
      vm,
      job,
      byId,
      runLocked,
      render,
      pushSelection,
      afterMutate,
    });
    paintChrome(vm, job, runLocked);
  }

  const actions = {
    render,
    beginEdit() {
      if (hasActiveRun()) {
        toast("运行中不可编辑，请先停止或待计划暂停");
        return;
      }
      const s = vm.getSnapshot();
      if (!s.jobId || !s.selectedTaskId) {
        toast("请先选择任务");
        return;
      }
      if (!canEditTask(s.selectedTaskId)) {
        toast("仅未执行的任务可编辑（暂停后可选左侧 pending 任务）");
        return;
      }
      vm.beginEdit();
      pushSelection();
      render();
      setTimeout(() => $("confirm-edit-title")?.focus(), 0);
    },
    cancelEdit() {
      vm.cancelEdit();
      pushSelection();
      render();
    },
    async saveEdit() {
      const err = $("confirm-error");
      if (err) err.hidden = true;
      if (hasActiveRun()) {
        toast("运行中不可保存编辑");
        return;
      }
      const s = vm.getSnapshot();
      if (!canEditTask(s.selectedTaskId)) {
        toast("仅未执行的任务可保存修改");
        return;
      }
      const title = ($("confirm-edit-title")?.value || "").trim();
      const prompt = ($("confirm-edit-prompt")?.value || "").trimEnd();
      const provider = (
        $("confirm-edit-provider")?.value ||
        $("confirm-task-provider")?.value ||
        s.job?.provider ||
        "claude"
      ).toLowerCase();
      const dependsOn = [
        ...document.querySelectorAll(
          "#confirm-edit-deps .confirm-dep-check:checked"
        ),
      ].map((el) => el.value);
      try {
        const next = await vm.saveEdit({
          title,
          prompt,
          provider,
          dependsOn,
        });
        const depsBox = $("confirm-edit-deps");
        if (depsBox) delete depsBox.dataset.forTask;
        pushSelection();
        toast(next.lastToast || "已保存");
        afterMutate();
        render();
      } catch (e) {
        const msg = e?.message || String(e);
        if (err) {
          err.textContent = msg;
          err.hidden = false;
        }
        toast(msg);
      }
    },
    async deleteTask() {
      if (hasActiveRun()) {
        toast("运行中不可删除");
        return;
      }
      const s = vm.getSnapshot();
      if (s.editing) {
        toast("请先保存或取消编辑");
        return;
      }
      if (!s.jobId || !s.selectedTaskId) {
        toast("请先选择任务");
        return;
      }
      if (!canEditTask(s.selectedTaskId)) {
        toast("仅未执行的任务可删除");
        return;
      }
      const tasks = s.job?.tasks || [];
      if (tasks.length <= 1) {
        toast("至少保留一个步骤");
        return;
      }
      const cur = tasks.find((t) => t.id === s.selectedTaskId);
      const label = cur?.title || s.selectedTaskId;
      const okDel = await confirmDialog({
        title: "删除步骤",
        body: `从本轮拆分中删除「${label}」？\n依赖它的步骤会自动去掉这条边。`,
        okLabel: "删除",
        danger: true,
      });
      if (!okDel) return;
      try {
        await vm.removeTask(s.selectedTaskId);
        const depsBox = $("confirm-edit-deps");
        if (depsBox) delete depsBox.dataset.forTask;
        pushSelection();
        toast(`已删除「${label}」`);
        afterMutate();
        render();
      } catch (e) {
        toast(String(e?.message || e));
      }
    },
    async confirmAndStart(opts = {}) {
      // Re-entrancy guard: the doctor/settings awaits below leave a window
      // where a double-click could fire confirm_start twice.
      if (confirmInFlight) return;
      confirmInFlight = true;
      const startBtn = $("btn-confirm-start");
      if (startBtn) startBtn.disabled = true;
      try {
        await actions._confirmAndStartInner(opts);
      } finally {
        confirmInFlight = false;
        if (startBtn) startBtn.disabled = false;
        // Let phase-aware paint re-disable if a run is now live.
        try {
          render();
        } catch (_) {}
      }
    },
    async _confirmAndStartInner(opts = {}) {
      const err = $("confirm-error");
      if (err) err.hidden = true;
      if (hasActiveRun()) {
        if (typeof g("toastRunLocked") === "function") {
          g("toastRunLocked")("再次启动");
        } else toast("计划运行中");
        return;
      }
      pullFromLegacy();
      const s = vm.getSnapshot();
      if (s.editing) {
        if (err) {
          err.textContent = "请先保存或取消编辑";
          err.hidden = false;
        }
        return;
      }
      // Resume only when the paused live is **this job's** run.
      // Foreign paused history (other plan / older job) must not hijack「执行规划」—
      // that restarts wrong tasks while the split desk still shows the new graph.
      const live = g("state")?.live;
      const jobRunId = s.job?.run_id || s.job?.runId || null;
      const liveIsThisJob =
        !!(live?.run_id && jobRunId && String(live.run_id) === String(jobRunId));
      if (isRunPaused() && liveIsThisJob) {
        try {
          await vm.resume(live.run_id);
          toast("正在继续…");
          afterMutate();
        } catch (e) {
          const msg = String(e?.message || e);
          if (err) {
            err.textContent = msg;
            err.hidden = false;
          }
          toast(msg);
        }
        return;
      }
      if (!s.jobId) {
        if (err) {
          err.textContent = "没有待确认的规划";
          err.hidden = false;
        }
        return;
      }
      const provider =
        s.job?.provider || $("pp-provider")?.value || "claude";
      if (typeof opts.ensureDoctor === "function") {
        try {
          const doc = await opts.ensureDoctor();
          if (doc && !doc.ok && provider !== "fake") {
            if (err) {
              err.textContent =
                "环境未就绪，请先处理警告或改用模拟运行后重新规划";
              err.hidden = false;
            }
            if (typeof g("renderDoctorWarn") === "function") {
              g("renderDoctorWarn")();
            }
            return;
          }
        } catch (_) {}
      }
      // Unattended workers cannot pop Claude permission UI. If config is
      // dontAsk/default, offer to switch to auto-authorize before starting.
      if (provider !== "fake") {
        try {
          const settings = await gateway.getSettings();
          const mode = String(settings?.permission_mode || "");
          if (mode === "dontAsk" || mode === "default") {
            const ok = await confirmDialog({
              title: "需要开启任务自动授权",
              body:
                "当前「任务授权」会拒绝写文件（执行时没有人点允许），任务将无法改代码，看起来像软件坏了。\n\n" +
                "开始执行前会把设置改为自动授权（可到 设置 → 任务授权 改回）。",
              okLabel: "开启并开始执行",
              cancelLabel: "先不开始",
            });
            if (!ok) {
              if (err) {
                err.textContent =
                  "已取消开跑：请到设置 → 任务授权，打开自动授权";
                err.hidden = false;
              }
              toast("已取消：请先开启任务自动授权");
              return;
            }
            await gateway.setSettings({
              permission_mode: "bypassPermissions",
            });
            toast("已开启任务自动授权");
          }
        } catch (_) {
          /* best-effort; spawn still defaults to bypass if opts missing */
        }
      }
      try {
        await vm.confirm();
        pushSelection();
        toast("已开始运行");
        afterMutate();
      } catch (e) {
        const msg = String(e?.message || e);
        if (err) {
          err.textContent = msg;
          err.hidden = false;
        }
        toast(msg);
      }
    },
  };

  return actions;
}

export { ensureAdvancedRouteDom } from "./splitDetail.js";
export default bindSplitView;
