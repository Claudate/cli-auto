/**
 * [INPUT]: SplitViewModel · 既有 DOM ids（confirm-* / split-*）
 * [OUTPUT]: 三栏绑定 + 意图转发；只发意图
 * [POS]: A3-1/A3-2 SplitView；禁止 invoke / start_run
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { cardsHtml, timelineHtml } from "./splitRender.js";
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

    const layers = job.layers || [];
    const tasks = job.tasks || [];
    const byId = Object.fromEntries(tasks.map((t) => [t.id, t]));
    const runLocked = hasActiveRun();
    const selectedId = s.selectedTaskId;

    const tl = $("split-timeline");
    if (tl) {
      tl.innerHTML = timelineHtml(layers, byId, selectedId);
      tl.querySelectorAll(".split-tl-cell").forEach((b) => {
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
    }

    const waves = $("confirm-waves");
    if (waves) {
      waves.innerHTML = cardsHtml(job, byId, {
        runLocked,
        selectedId,
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
      if (
        !window.confirm(
          `从本轮拆分中删除「${label}」？\n依赖它的步骤会自动去掉这条边。`
        )
      ) {
        return;
      }
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
      const live = g("state")?.live;
      if (isRunPaused() && live?.run_id) {
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
