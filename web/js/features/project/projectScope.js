/**
 * [INPUT]: planJob / live DTO · state.selectedPath
 * [OUTPUT]: 项目作用域闸 — 归属判定 · generation · 唯一 planJob 写入
 * [POS]: features/project/projectScope.js · 防跨项目串台真源
 * note: 所有 state.planJob 写入应走 setBoundPlanJob；渲染读 getBoundPlanJob
 * note: selectProject 必须 bumpProjectScope；异步回调带 gen，错代则丢弃
 * note: clearSplitDeskDom → invalidateJobSig；rebind 空 waves force setJob + renderConfirmPanel
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import { state } from "./legacy.js";

/** Monotonic generation; bumped on every project switch (and home). */
let _scopeGen = 0;

export function currentScopeGen() {
  return _scopeGen;
}

/**
 * Call at the start of selectProject / goHome path clear.
 * @returns {number} new generation
 */
export function bumpProjectScope() {
  _scopeGen += 1;
  return _scopeGen;
}

/** Normalize project path for ownership compare. */
export function normalizeProjectPathKey(p) {
  if (p == null || p === "") return "";
  return String(p).trim().replace(/\\/g, "/").replace(/\/+$/, "");
}

/** Project path equality (exact, then case-insensitive for macOS). */
export function pathsEqualProject(a, b) {
  const na = normalizeProjectPathKey(a);
  const nb = normalizeProjectPathKey(b);
  if (!na || !nb) return false;
  return na === nb || na.toLowerCase() === nb.toLowerCase();
}

/**
 * Does this plan job belong to the given project?
 * PlanJobView.project is SoT; fallback planSessions[path].planJobId.
 */
export function planJobBelongsToProject(job, projectPath) {
  if (!job || !projectPath) return false;
  const jp = job.project || job.project_path || job.projectPath || null;
  if (jp) return pathsEqualProject(jp, projectPath);
  const sid = job.job_id || job.jobId || null;
  if (!sid) return false;
  const mem = state.planSessions?.[projectPath];
  return !!(mem?.planJobId && String(mem.planJobId) === String(sid));
}

/**
 * Live DTO belongs to open project (project_path field).
 * Missing project_path → unknown; treat as NOT belonging when path is set
 * only if we require strict mode — default: missing path allowed (legacy).
 */
export function liveBelongsToProject(live, projectPath, { strict = false } = {}) {
  if (!live?.run_id) return false;
  if (!projectPath) return !strict;
  const lp = live.project_path || live.projectPath || live.project || null;
  if (!lp) return !strict;
  return pathsEqualProject(lp, projectPath);
}

/**
 * Stale-async guard: return false if caller’s gen is outdated.
 * @param {number|null|undefined} gen
 */
export function scopeGenStillCurrent(gen) {
  if (gen == null) return true;
  return Number(gen) === Number(_scopeGen);
}

/**
 * THE write path for state.planJob / planJobId.
 * Rejects foreign jobs and stale generation.
 *
 * @param {object|null} job
 * @param {{
 *   projectPath?: string|null,
 *   gen?: number|null,
 *   confirmTaskId?: string|null,
 *   keepConfirmTask?: boolean,
 *   allowMissingProjectField?: boolean,
 * }} [opts]
 * @returns {boolean} whether state was updated
 */
export function setBoundPlanJob(job, opts = {}) {
  const path =
    opts.projectPath !== undefined ? opts.projectPath : state.selectedPath;
  if (!scopeGenStillCurrent(opts.gen)) return false;

  if (!job) {
    state.planJob = null;
    state.planJobId = null;
    if (!opts.keepConfirmTask) {
      state.confirmTaskId = null;
      state.confirmEditing = false;
    }
    return true;
  }

  // Must have an open project to bind a job into the global UI slot
  if (!path) return false;

  const jp = job.project || job.project_path || job.projectPath || null;
  if (jp) {
    if (!pathsEqualProject(jp, path)) return false;
  } else if (!opts.allowMissingProjectField) {
    // No project field: only accept if stash for this path already points here
    if (!planJobBelongsToProject(job, path)) return false;
  }

  state.planJob = job;
  state.planJobId = job.job_id || job.jobId || null;
  if (opts.confirmTaskId !== undefined) {
    state.confirmTaskId = opts.confirmTaskId;
  } else if (
    !state.confirmTaskId &&
    Array.isArray(job.tasks) &&
    job.tasks.length
  ) {
    state.confirmTaskId = job.tasks[0].id;
  }
  return true;
}

/**
 * Clear bound job if foreign to path (or always if clearAll).
 * @returns {boolean} true if scrubbed
 */
export function scrubForeignPlanJob(projectPath = state.selectedPath) {
  if (!state.planJob && !state.planJobId) return false;
  // goHome: no open project — leave job for banner 回跳; UI cleared separately
  if (!projectPath) return false;
  if (state.planJob && planJobBelongsToProject(state.planJob, projectPath)) {
    const jid = state.planJob.job_id || state.planJob.jobId || null;
    if (jid && state.planJobId && String(state.planJobId) !== String(jid)) {
      state.planJobId = jid;
    }
    return false;
  }
  setBoundPlanJob(null, { projectPath });
  return true;
}

/**
 * Read job only if it belongs to the open project.
 * Use this in every paint path instead of raw state.planJob.
 */
export function getBoundPlanJob(projectPath = state.selectedPath) {
  const job = state.planJob;
  if (!job) return null;
  if (!projectPath) return null;
  if (!planJobBelongsToProject(job, projectPath)) return null;
  return job;
}

/**
 * Stamp desk root with bound project so residual DOM can be detected.
 * @param {string|null} projectPath
 */
export function stampSplitDeskProject(projectPath) {
  if (typeof document === "undefined") return;
  const key = normalizeProjectPathKey(projectPath);
  for (const id of [
    "plan-phase-confirm",
    "confirm-waves",
    "plan-phase-planning",
  ]) {
    const el = document.getElementById(id);
    if (!el) continue;
    if (key) el.dataset.ccoBoundProject = key;
    else delete el.dataset.ccoBoundProject;
  }
}

/**
 * True if desk DOM is stamped for a different project than open path.
 */
export function splitDeskDomIsForeign(projectPath = state.selectedPath) {
  if (typeof document === "undefined") return false;
  const waves = document.getElementById("confirm-waves");
  if (!waves?.dataset?.ccoBoundProject) return false;
  if (!projectPath) return !!waves.dataset.ccoBoundProject;
  return !pathsEqualProject(waves.dataset.ccoBoundProject, projectPath);
}

/**
 * Clear split desk DOM + optional VM. Does not touch state.planJob
 * unless scrubState is true.
 */
export function clearSplitDeskDom() {
  if (typeof document === "undefined") return;
  const waves = document.getElementById("confirm-waves");
  if (waves) {
    waves.innerHTML = "";
    delete waves.dataset.sig;
    delete waves.dataset.ccoAwaitSplit;
    delete waves.dataset.ccoBoundProject;
  }
  const titleEl = document.getElementById("confirm-title");
  if (titleEl) titleEl.textContent = "拆分结果";
  const err = document.getElementById("confirm-error");
  if (err) {
    err.hidden = true;
    err.textContent = "";
  }
  for (const id of [
    "confirm-task-title",
    "confirm-task-body",
    "confirm-detail-body",
    "confirm-acceptance",
    "confirm-task-prompt",
  ]) {
    const el = document.getElementById(id);
    if (!el) continue;
    try {
      if ("value" in el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) {
        el.value = "";
      } else {
        el.textContent = "";
        el.innerHTML = "";
      }
    } catch (_) {}
  }
  // Detail cards container
  const detail = document.querySelector(
    "#plan-phase-confirm .confirm-detail, #confirm-detail"
  );
  if (detail && detail.dataset) {
    delete detail.dataset.ccoBoundProject;
  }
  // DOM wipe without clearJob left setJob no-op → blank desk forever.
  try {
    const split = typeof window !== "undefined" ? window.ccoSplit : null;
    if (typeof split?.vm?.invalidateJobSig === "function") {
      split.vm.invalidateJobSig();
    }
  } catch (_) {}
}

/**
 * Full UI unbind: VM + DOM. Safe before ccoSplit exists.
 * @param {{ scrubState?: boolean, projectPath?: string|null }} [opts]
 */
export function clearSplitUiBinding(opts = {}) {
  if (opts.scrubState) {
    setBoundPlanJob(null, { projectPath: opts.projectPath ?? state.selectedPath });
  }
  try {
    const split = typeof window !== "undefined" ? window.ccoSplit : null;
    if (split && typeof split.clearDesk === "function") {
      split.clearDesk();
      stampSplitDeskProject(null);
      return;
    }
    if (split?.vm && typeof split.vm.clearJob === "function") {
      split.vm.clearJob();
    } else if (split?.vm && typeof split.vm.setJob === "function") {
      split.vm.setJob(null);
    }
  } catch (_) {}
  clearSplitDeskDom();
  stampSplitDeskProject(null);
}

/**
 * After project switch restore: bind VM to getBoundPlanJob or clear.
 * Call from softSync / goSplit / selectProject tail.
 */
export function rebindSplitToOpenProject() {
  const path = state.selectedPath;
  if (!path) {
    clearSplitUiBinding({ scrubState: false });
    return { bound: false, job: null };
  }
  scrubForeignPlanJob(path);
  // DOM stamped for another project → wipe even if state already cleaned
  if (splitDeskDomIsForeign(path)) {
    clearSplitDeskDom();
  }
  const job = getBoundPlanJob(path);
  try {
    const split = typeof window !== "undefined" ? window.ccoSplit : null;
    if (!split) {
      if (!job) clearSplitDeskDom();
      return { bound: !!job, job };
    }
    if (job) {
      // If waves were wiped while VM still holds this job, force setJob so
      // the next paint cannot no-op on an identical signature.
      let force = false;
      try {
        const waves = document.getElementById("confirm-waves");
        force = !!(
          waves &&
          (!waves.dataset.sig || !String(waves.innerHTML || "").trim())
        );
      } catch (_) {
        force = false;
      }
      if (typeof split.vm?.setJob === "function") {
        split.vm.setJob(job, {
          jobId: state.planJobId,
          selectedTaskId: state.confirmTaskId,
          editing: state.confirmEditing,
          force,
        });
      }
      stampSplitDeskProject(path);
      // Confirm desk visible + empty DOM → paint now (softSync used to only
      // setJob and leave a blank #confirm-waves after a DOM-only wipe).
      if (state.phase === "confirm") {
        try {
          const waves = document.getElementById("confirm-waves");
          const empty =
            !waves ||
            !waves.dataset.sig ||
            !String(waves.innerHTML || "").trim() ||
            force;
          if (empty) {
            if (typeof window.renderConfirmPanel === "function") {
              window.renderConfirmPanel();
            } else if (typeof split.render === "function") {
              split.render();
            }
          }
        } catch (_) {}
      }
    } else {
      clearSplitUiBinding({ scrubState: false });
    }
  } catch (_) {
    if (!job) clearSplitDeskDom();
  }
  return { bound: !!job, job };
}

export default {
  currentScopeGen,
  bumpProjectScope,
  normalizeProjectPathKey,
  pathsEqualProject,
  planJobBelongsToProject,
  liveBelongsToProject,
  scopeGenStillCurrent,
  setBoundPlanJob,
  scrubForeignPlanJob,
  getBoundPlanJob,
  stampSplitDeskProject,
  splitDeskDomIsForeign,
  clearSplitDeskDom,
  clearSplitUiBinding,
  rebindSplitToOpenProject,
};
