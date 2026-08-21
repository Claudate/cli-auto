/**
 * [INPUT]: splitApi · 展示状态
 * [OUTPUT]: 拆分台意图（选步 / 勾选 / 改通道 / 保存 / 确认开跑）
 * [POS]: A3 SplitViewModel；禁止 soft-fill / optional 自动策略 / start_run 旁路
 * note: setJob sig 含 task id 指纹；invalidateJobSig/force 防 DOM 清空后 no-op 空白台
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { createStore } from "../../shared/store.js";
import * as splitApi from "./splitApi.js";

/**
 * @typedef {{
 *   jobId: string|null,
 *   job: object|null,
 *   selectedTaskId: string|null,
 *   editing: boolean,
 *   busy: boolean,
 *   lastError: string|null,
 *   lastToast: string|null,
 * }} SplitSnap
 */

/**
 * @param {{
 *   onJobUpdated?: (job: object) => void,
 *   onConfirmed?: (res: { runId: string|null, job: object|null }) => void,
 *   onPhaseRun?: () => void,
 * }} [deps]
 */
export function createSplitViewModel(deps = {}) {
  /** @type {import("../../shared/store.js").createStore extends Function ? ReturnType<typeof createStore<SplitSnap>> : never} */
  const store = createStore({
    jobId: null,
    job: null,
    selectedTaskId: null,
    editing: false,
    busy: false,
    lastError: null,
    lastToast: null,
  });

  /**
   * Job-snapshot signature for setJob no-op guard.
   * Prevents the 2s poll tick (softSyncFromLegacy) from churning the store
   * with a structurally-identical planJob, which would trigger unnecessary
   * re-renders on every tick. Computed from job identity + opts.
   * Include task id fingerprint so "same count, different graph" still updates.
   */
  let lastJobSig = "";

  function snap() {
    return store.get();
  }

  function setPatch(partial) {
    store.set({ ...snap(), ...partial });
    return snap();
  }

  function notifyJob(job) {
    if (job && typeof deps.onJobUpdated === "function") {
      try {
        deps.onJobUpdated(job);
      } catch (e) {
        console.error("[SplitViewModel] onJobUpdated", e);
      }
    }
  }

  function jobIdOf(job, fallback) {
    return job?.job_id || job?.jobId || fallback || null;
  }

  function taskSigOf(job) {
    const tasks = job?.tasks;
    if (!Array.isArray(tasks) || !tasks.length) return "0";
    // id list is enough to catch empty→filled and replan graph swaps
    return `${tasks.length}:${tasks
      .map((t) => t?.id || "")
      .slice(0, 24)
      .join(",")}`;
  }

  return {
    store,
    getSnapshot: snap,
    subscribe: (fn) => store.subscribe(fn),

    /**
     * Force next setJob to apply even if the job signature matches.
     * Call after DOM-only desk wipes so softSync/rebind cannot leave a blank
     * #confirm-waves while the VM still holds the same job.
     */
    invalidateJobSig() {
      lastJobSig = "";
    },

    /**
     * Mirror legacy planJob into VM (strangler). Does not fetch.
     * @param {object|null} job
     * @param {{ jobId?: string|null, selectedTaskId?: string|null, editing?: boolean, force?: boolean }} [opts]
     */
    setJob(job, opts = {}) {
      // No-op guard: skip when the job + opts signature hasn't changed.
      // The 2s poll tick calls this every cycle; without the guard, each
      // tick replaces the store object (new reference) → triggers subscribers
      // → full SplitView.render() → innerHTML churn even when nothing moved.
      // Clearing (job==null) always applies so cross-project bleed cannot stick.
      const jobId = opts.jobId ?? jobIdOf(job, job ? snap().jobId : null);
      const sig =
        `${job?.job_id || job?.jobId || jobId || ""}|${job?.status || ""}|${taskSigOf(
          job
        )}|${opts.selectedTaskId ?? ""}|${opts.editing ?? ""}`;
      // Allow no-op for both bound job AND already-cleared null (softSync every 2s).
      // opts.force / empty lastJobSig (after invalidateJobSig) always apply.
      if (!opts.force && sig === lastJobSig && lastJobSig !== "" && !snap().busy) {
        return snap();
      }
      lastJobSig = sig;
      let selected =
        opts.selectedTaskId !== undefined
          ? opts.selectedTaskId
          : snap().selectedTaskId;
      if (job?.tasks?.length) {
        const ids = new Set(job.tasks.map((t) => t.id));
        if (!selected || !ids.has(selected)) {
          selected = job.tasks[0].id;
        }
      } else {
        selected = null;
      }
      return setPatch({
        job: job || null,
        jobId: job ? jobId || null : null,
        selectedTaskId: selected,
        editing: job
          ? opts.editing !== undefined
            ? !!opts.editing
            : snap().editing
          : false,
        lastError: null,
      });
    },

    /** Explicit clear for project switch / unbound desk. */
    clearJob() {
      const cur = snap();
      if (!cur.job && !cur.jobId && !cur.selectedTaskId && !cur.editing) {
        lastJobSig = "";
        return cur;
      }
      lastJobSig = "";
      return setPatch({
        job: null,
        jobId: null,
        selectedTaskId: null,
        editing: false,
        busy: false,
        lastError: null,
        lastToast: null,
      });
    },

    /** @param {string|null} taskId */
    selectTask(taskId) {
      if (snap().editing) {
        return setPatch({ lastToast: "请先保存或取消当前编辑" });
      }
      return setPatch({ selectedTaskId: taskId || null, lastToast: null });
    },

    beginEdit() {
      return setPatch({ editing: true, lastError: null });
    },

    cancelEdit() {
      return setPatch({ editing: false, lastError: null });
    },

    /**
     * Optional include toggle — backend owns defaults; UI only sends checkbox.
     * @param {string} taskId
     * @param {boolean} include
     */
    async setInclude(taskId, include) {
      const s = snap();
      if (!s.jobId) throw new Error("没有可编辑的拆分");
      if (s.editing) throw new Error("请先保存或取消当前编辑");
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.updateTask({
          jobId: s.jobId,
          taskId,
          include,
        });
        const next = setPatch({
          busy: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          lastToast: include
            ? `已勾选：将执行`
            : `已取消：不跑`,
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /**
     * Task-level provider (Worker 路由)。不复制 soft-fill。
     * @param {string} taskId
     * @param {string} provider
     */
    async setProvider(taskId, provider) {
      const s = snap();
      if (!s.jobId) throw new Error("没有可编辑的拆分");
      if (s.editing) throw new Error("请先保存或取消当前编辑");
      const nextProv = String(provider || "claude").toLowerCase();
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.updateTask({
          jobId: s.jobId,
          taskId,
          provider: nextProv,
        });
        const next = setPatch({
          busy: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          lastToast: `已更新执行通道`,
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /**
     * Task-level collaboration role (S-role)。空串清除。不复制 soft-fill。
     * @param {string} taskId
     * @param {string} role
     */
    async setRole(taskId, role) {
      const s = snap();
      if (!s.jobId) throw new Error("没有可编辑的拆分");
      if (s.editing) throw new Error("请先保存或取消当前编辑");
      const nextRole = String(role ?? "").trim().toLowerCase();
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.updateTask({
          jobId: s.jobId,
          taskId,
          role: nextRole,
        });
        const next = setPatch({
          busy: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          lastToast: nextRole ? `已更新角色` : `已清除角色`,
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /**
     * Writable scope paths (S-role)。空数组清除 paths。不复制 soft-fill。
     * @param {string} taskId
     * @param {string[]} paths
     */
    async setScopePaths(taskId, paths) {
      const s = snap();
      if (!s.jobId) throw new Error("没有可编辑的拆分");
      if (s.editing) throw new Error("请先保存或取消当前编辑");
      const list = Array.isArray(paths)
        ? paths.map((p) => String(p || "").trim()).filter(Boolean)
        : [];
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.updateTask({
          jobId: s.jobId,
          taskId,
          scopePaths: list,
        });
        const next = setPatch({
          busy: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          lastToast: list.length ? `已更新范围` : `已清除范围`,
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /**
     * Save title/prompt/provider/dependsOn from edit form.
     * @param {{ title: string, prompt: string, provider?: string, dependsOn?: string[] }} input
     */
    async saveEdit(input) {
      const s = snap();
      if (!s.jobId || !s.selectedTaskId) {
        throw new Error("没有可保存的任务");
      }
      const title = String(input?.title || "").trim();
      const prompt = String(input?.prompt || "").trimEnd();
      if (!title) throw new Error("标题不能为空");
      if (!prompt.trim()) throw new Error("任务说明不能为空");
      const provider = String(
        input?.provider || "claude"
      ).toLowerCase();
      const dependsOn = Array.isArray(input?.dependsOn)
        ? input.dependsOn
        : [];
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.updateTask({
          jobId: s.jobId,
          taskId: s.selectedTaskId,
          title,
          prompt,
          provider,
          dependsOn,
        });
        const keepId = s.selectedTaskId;
        const ids = new Set((view.tasks || []).map((t) => t.id));
        const next = setPatch({
          busy: false,
          editing: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          selectedTaskId: ids.has(keepId)
            ? keepId
            : view.tasks?.[0]?.id || null,
          lastToast: dependsOn.length
            ? `已保存「${title}」· 等待 ${dependsOn.length} 项`
            : `已保存「${title}」· 无依赖`,
          lastError: null,
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /** @param {string} taskId */
    async removeTask(taskId) {
      const s = snap();
      if (!s.jobId) throw new Error("没有可编辑的拆分");
      if (s.editing) throw new Error("请先保存或取消编辑");
      const tasks = s.job?.tasks || [];
      if (tasks.length <= 1) throw new Error("至少保留一个步骤");
      setPatch({ busy: true, lastError: null });
      try {
        const view = await splitApi.removeTask({
          jobId: s.jobId,
          taskId,
        });
        const next = setPatch({
          busy: false,
          editing: false,
          job: view,
          jobId: jobIdOf(view, s.jobId),
          selectedTaskId: view.tasks?.[0]?.id || null,
          lastToast: "已删除步骤",
        });
        notifyJob(view);
        return next;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /**
     * 唯一开跑：confirm_start → onPhaseRun / onConfirmed。
     * 不在此实现 optional 门禁（后端 confirm 处理 include=false）。
     * @returns {Promise<{ runId: string|null, job: object|null }>}
     */
    async confirm() {
      const s = snap();
      if (s.busy) {
        // A confirm (or another mutation) is already in flight — never allow
        // a second confirm_start from a double-click.
        throw new Error("上一步操作还在进行，请稍候…");
      }
      if (s.editing) {
        const err = "请先保存或取消编辑";
        setPatch({ lastError: err });
        throw new Error(err);
      }
      if (!s.jobId) {
        const err = "没有待确认的规划";
        setPatch({ lastError: err });
        throw new Error(err);
      }
      setPatch({ busy: true, lastError: null });
      try {
        // Execute-time depth from split desk (or chooser / localStorage seed).
        const EFFORT_OK = ["low", "medium", "high", "xhigh", "max", "ultracode"];
        let effort = null;
        try {
          const raw = (
            document.getElementById("split-effort")?.value ||
            document.getElementById("pp-effort")?.value ||
            localStorage.getItem("cco.splitEffort") ||
            ""
          )
            .trim()
            .toLowerCase();
          if (EFFORT_OK.includes(raw)) effort = raw;
        } catch (_) {}
        // Get persona chip values
        const { getChipValue } = await import("../chat/chatPersona.js");
        const chips = {
          clarify_depth: getChipValue('clarify_depth'),
          split_grain: getChipValue('split_grain'),
        };
        const res = await splitApi.confirmStart(s.jobId, effort, chips);
        const runId = res?.run_id || res?.runId || null;
        const job = s.job
          ? { ...s.job, status: "confirmed", run_id: runId }
          : null;
        const out = { runId, job };
        setPatch({
          busy: false,
          editing: false,
          job,
          lastToast: "已开始运行",
          lastError: null,
        });
        if (typeof deps.onConfirmed === "function") {
          try {
            deps.onConfirmed(out);
          } catch (e) {
            console.error("[SplitViewModel] onConfirmed", e);
          }
        }
        if (typeof deps.onPhaseRun === "function") {
          try {
            deps.onPhaseRun();
          } catch (e) {
            console.error("[SplitViewModel] onPhaseRun", e);
          }
        }
        return out;
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    /** Resume paused run (not open-run). @param {string} runId */
    async resume(runId) {
      setPatch({ busy: true, lastError: null });
      try {
        await splitApi.resumeRun(runId);
        setPatch({ busy: false, lastToast: "正在继续…" });
        if (typeof deps.onPhaseRun === "function") {
          deps.onPhaseRun();
        }
        return snap();
      } catch (e) {
        const msg = e?.message || String(e);
        setPatch({ busy: false, lastError: msg });
        throw e;
      }
    },

    clearToast() {
      return setPatch({ lastToast: null });
    },
  };
}

export default createSplitViewModel;
