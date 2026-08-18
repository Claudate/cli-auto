/**
 * [INPUT]: Tauri invoke surface (A1-7 command names, 1:1)
 * [OUTPUT]: typed async API for ViewModels / features（含 gitDoctor 发布状态）
 * [POS]: A2-1 唯一 IPC 出口；feature 内禁止散落 __TAURI__/invoke
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * Script order (legacy) → module map (A2):
 *   state.js invoke()  ──bridge──►  gateway.raw / named methods
 *   plan/chat/log/…   ──migrate──►  features/* via gateway only
 *
 * New code MUST import from this module (or use window.ccoGateway during
 * the strangler period). Do not call window.__TAURI__ from features/.
 */

/** Resolve Tauri 2 invoke across global shapes (same candidates as state.js). */
function getInvoke() {
  const w = typeof window !== "undefined" ? window : globalThis;
  const candidates = [
    w.__TAURI__?.core?.invoke && w.__TAURI__.core.invoke.bind(w.__TAURI__.core),
    w.__TAURI__?.tauri?.invoke && w.__TAURI__.tauri.invoke.bind(w.__TAURI__.tauri),
    w.__TAURI_INTERNALS__?.invoke &&
      w.__TAURI_INTERNALS__.invoke.bind(w.__TAURI_INTERNALS__),
    typeof w.__TAURI_INVOKE__ === "function" && w.__TAURI_INVOKE__,
  ];
  for (const c of candidates) {
    if (typeof c === "function") return c;
  }
  // Strangler: legacy global from state.js (classic scripts)
  if (typeof w.invoke === "function" && w.invoke !== raw) {
    // Prefer real Tauri; fall through only when state.js already bound
  }
  return null;
}

export function isTauriReady() {
  if (getInvoke()) return true;
  const w = typeof window !== "undefined" ? window : globalThis;
  return typeof w.invoke === "function";
}

/**
 * Low-level invoke. Prefer named methods below so command strings stay here.
 * @param {string} cmd
 * @param {Record<string, unknown>} [args]
 */
export async function raw(cmd, args = {}) {
  const inv = getInvoke();
  if (inv) {
    try {
      return await inv(cmd, args);
    } catch (e) {
      const msg = e?.message || e?.toString?.() || String(e);
      throw new Error(msg);
    }
  }
  // Classic-script bridge: state.js may already define global invoke
  const w = typeof window !== "undefined" ? window : globalThis;
  if (typeof w.invoke === "function") {
    return w.invoke(cmd, args);
  }
  throw new Error("请通过 CCO.app 启动（invoke 不可用）");
}

/* ── Project ── */
export const getProjects = () => raw("get_projects");
export const addProject = (path, name) => raw("add_project_cmd", { path, name });
export const removeProject = (path) => raw("remove_project_cmd", { path });
/**
 * Live snapshot for a project.
 * @param {string} project
 * @param {{ logMaxBytes?: number }} [opts]
 */
export const getProjectLive = (project, opts = {}) => {
  const args = { project };
  if (opts.logMaxBytes != null) args.log_max_bytes = opts.logMaxBytes;
  return raw("get_project_live", args);
};
export const setProjectDefaultPlan = (project, plan) =>
  raw("set_project_default_plan", { project, plan });

/* ── Plans (read / preview) ── */
export const getPlans = (project) => raw("get_plans", { project });
export const getPlanMeta = (project) => raw("get_plan_meta", { project });
export const previewPlan = (project, plan) =>
  raw("preview_plan_cmd", { project, plan });
export const readPlanMd = (project, plan) =>
  raw("read_plan_md_cmd", { project, plan });
/** Sanitize proposed deps for a plan job (jobId, not project/plan path). */
export const sanitizePlanDeps = (jobId) =>
  raw("sanitize_plan_deps_cmd", { jobId });

/* ── Mode B / Split (唯一开跑 confirm_start) ── */
export const startPlanJob = (args) => raw("start_plan_job_cmd", args);
export const getPlanJob = (jobId) => raw("get_plan_job_cmd", { jobId });
export const latestPlanJob = (project) =>
  raw("latest_plan_job_cmd", { project });
/** Plan list reopen: latest restorable split for one plan path (SQLite + disk). */
export const latestPlanJobForPlan = (project, planPath) =>
  raw("latest_plan_job_for_plan_cmd", { project, planPath });
/** Plan list badge index: restorable splits per plan_path. */
export const listPlanSplitIndex = (project) =>
  raw("list_plan_split_index_cmd", { project });
export const updatePlanTask = (args) => raw("update_plan_task_cmd", args);
export const removePlanTask = (args) => raw("remove_plan_task_cmd", args);
/** 唯一业务开跑入口 (Split 确认)；禁止 UI 旁路 start_run */
/** @param {string} jobId @param {string|null|undefined} [effort] low…max|ultracode @param {{clarify_depth?: string, split_grain?: string}} [chips] */
export const confirmStart = (jobId, effort, chips) => {
  const args = { jobId };
  if (effort) args.effort = effort;
  if (chips && chips.clarify_depth != null) args.clarify_depth = chips.clarify_depth;
  if (chips && chips.split_grain != null) args.split_grain = chips.split_grain;
  return raw("confirm_start_cmd", args);
};

/* ── Run ── */
export const stopRun = (runId) => raw("stop_run_cmd", { runId });
export const stopTask = (runId, taskId) =>
  raw("stop_task_cmd", { runId, taskId });
export const resumeRun = (runId) => raw("resume_run_cmd", { runId });
/** Manual re-run of one failed task (same run; not re-split / not confirm).
 *  @param {string} runId
 *  @param {string} taskId
 *  @param {{ provider?: string }} [opts] — optional channel override.
 */
export const retryTask = (runId, taskId, opts) =>
  raw("retry_task_cmd", { runId, taskId, provider: opts?.provider || null });
export const startRework = (runId) => raw("start_rework_cmd", { runId });
export const acceptResidual = (runId, note) =>
  raw("accept_residual_cmd", { runId, note: note || null });
/** P2-2: write last_summary from finished run (rule template). */
export const writebackMemory = (runId) =>
  raw("writeback_memory_cmd", { runId });
export const projectMemoryGet = (project) =>
  raw("project_memory_get_cmd", { project });
export const projectMemoryLastSummary = (project) =>
  raw("project_memory_last_summary_cmd", { project });
export const projectPinsList = (project) =>
  raw("project_pins_list_cmd", { project });
export const projectPinUpsert = (project, key, value) =>
  raw("project_pin_upsert_cmd", { project, key, value });
export const projectPinDelete = (project, key) =>
  raw("project_pin_delete_cmd", { project, key });
/** Guide G0: list guided sessions for a project (newest first). */
export const guideSessionsList = (project) =>
  raw("guide_sessions_list_cmd", { project });
/** Guide G0: start a guided session (mode/entry strings; role pack id). */
export const guideSessionStart = (project, mode, entry, rolePack) =>
  raw("guide_session_start_cmd", { project, mode, entry, rolePack });
/** Guide G0: get one guided session by id. */
export const guideSessionGet = (sessionId) =>
  raw("guide_session_get_cmd", { sessionId });
/** Get persona preferences (persona_id, clarify_depth, split_grain). best-effort, no project check -> null */
export const getProjectPersona = (project) => raw("get_project_persona_cmd", { project });
/** Set persona preferences (any of the three may be omitted). best-effort. */
export const setProjectPersona = (project, args) =>
  raw("set_project_persona_cmd", { project, ...args });
/** SQLite: finish round — hide this run from project_live until cleared. */
export const projectDismissRun = (project, runId) =>
  raw("project_dismiss_run_cmd", { project, runId });
export const projectClearDismissedRun = (project) =>
  raw("project_clear_dismissed_run_cmd", { project });
export const projectGetDismissedRun = (project) =>
  raw("project_get_dismissed_run_cmd", { project });
export const openTaskTerminal = (args) => raw("open_task_terminal_cmd", args);

/* ── Chat (author) ── */
export const chatListSessions = (project) =>
  raw("chat_list_sessions_cmd", { project });
export const chatNewSession = (project, title) =>
  raw("chat_new_session_cmd", { project, title: title ?? null });
export const chatDeleteSession = (project, sessionId) =>
  raw("chat_delete_session_cmd", { project, sessionId });
export const chatRenameSession = (project, sessionId, title) =>
  raw("chat_rename_session_cmd", {
    project,
    sessionId,
    title: title == null || title === "" ? null : title,
  });
export const chatSessionGet = (project, sessionId) =>
  raw("chat_session_get_cmd", { project, sessionId });
export const chatSend = (args) => raw("chat_send_cmd", args);
export const chatClisList = () => raw("chat_clis_list_cmd", {});
/** Per-CLI slash-command catalog for the composer autocomplete. */
export const chatSlashCatalog = (cli) =>
  raw("chat_slash_catalog_cmd", cli ? { cli } : {});
export const chatStreamPartial = (args) => raw("chat_stream_partial_cmd", args);
export const chatCancel = (project) => raw("chat_cancel_cmd", { project });
export const chatSavePlan = (args) => raw("chat_save_plan_cmd", args);
/** W2: wave-index + N plans; claim ≠ run. */
export const chatSaveWaveBundle = (args) =>
  raw("chat_save_wave_bundle_cmd", args);
export const chatNormalizePlan = (args) => raw("chat_normalize_plan_cmd", args);
export const chatSaveAttachment = (args) =>
  raw("chat_save_attachment_cmd", args);
/** Project-relative image → data: URL for chat thumbs / markdown. */
export const chatReadImageDataUrl = (project, path) =>
  raw("chat_read_image_data_url_cmd", { project, path });
/** Detached local preview (npm run dev…); not Mode B worker. */
export const previewStart = (project) => raw("preview_start_cmd", { project });
export const previewStop = (project) => raw("preview_stop_cmd", { project });
export const previewStatus = (project) => raw("preview_status_cmd", { project });

/* ── Settings / doctor / shell ── */
export const getSettings = () => raw("get_settings_cmd");
export const setSettings = (update) => raw("set_settings_cmd", { update });
export const doctor = (project) =>
  raw("doctor_cmd", { project: project || null });
export const gitDoctor = (project) => raw("git_doctor_cmd", { project });
/** One-click git init for the auto-commit gate (split desk confirm). */
export const gitInit = (project, opts = {}) =>
  raw("git_init_cmd", { project, ...opts });

/* ── Git (host-level: pull / fetch / branch / log / diff / stash / tag) ── */
export const gitStatus = (project) => raw("git_status_cmd", { project });
export const gitCommit = (project, message, opts = {}) =>
  raw("git_commit_cmd", {
    project,
    message,
    dryRun: opts.dryRun,
    push: opts.push,
    all: opts.all,
    paths: opts.paths,
    force: opts.force,
  });
export const gitPush = (project, opts = {}) =>
  raw("git_push_cmd", {
    project,
    remote: opts.remote,
    branch: opts.branch,
    force: opts.force,
  });
export const gitPull = (project, opts = {}) =>
  raw("git_pull_cmd", {
    project,
    remote: opts.remote,
    branch: opts.branch,
    strategy: opts.strategy,
  });
export const gitFetch = (project, opts = {}) =>
  raw("git_fetch_cmd", {
    project,
    remote: opts.remote,
    prune: opts.prune,
  });
export const gitBranchList = (project) => raw("git_branch_list_cmd", { project });
export const gitBranchCreate = (project, name, base) =>
  raw("git_branch_create_cmd", { project, name, base });
export const gitBranchSwitch = (project, name) =>
  raw("git_branch_switch_cmd", { project, name });
export const gitBranchDelete = (project, name, force) =>
  raw("git_branch_delete_cmd", { project, name, force });
export const gitLog = (project, n) => raw("git_log_cmd", { project, n });
export const gitDiff = (project, opts = {}) =>
  raw("git_diff_cmd", {
    project,
    staged: opts.staged,
    stat: opts.stat,
    nameOnly: opts.nameOnly,
  });
export const gitStashList = (project) => raw("git_stash_list_cmd", { project });
export const gitStashPush = (project, message) =>
  raw("git_stash_push_cmd", { project, message });
export const gitStashPop = (project, index) =>
  raw("git_stash_pop_cmd", { project, index });
export const gitStashApply = (project, index) =>
  raw("git_stash_apply_cmd", { project, index });
export const gitStashDrop = (project, index) =>
  raw("git_stash_drop_cmd", { project, index });
export const gitStashShow = (project, index) =>
  raw("git_stash_show_cmd", { project, index });
export const gitTagList = (project) => raw("git_tag_list_cmd", { project });
export const gitTagCreate = (project, name, opts = {}) =>
  raw("git_tag_create_cmd", {
    project,
    name,
    commit: opts.commit,
    message: opts.message,
  });
export const gitTagDelete = (project, name) =>
  raw("git_tag_delete_cmd", { project, name });
export const gitTagShow = (project, name) =>
  raw("git_tag_show_cmd", { project, name });

export const meta = () => raw("meta");
export const openPath = (path) => raw("open_path", { path });
export const openMonitorWindow = (args) =>
  raw("open_monitor_window_cmd", args || {});

/** Dialog plugin (folder picker). */
export async function dialogOpen(options) {
  const w = typeof window !== "undefined" ? window : globalThis;
  const d = w.__TAURI__?.dialog || w.__TAURI__?.plugins?.dialog || null;
  if (d?.open) return d.open(options);
  return raw("plugin:dialog|open", { options });
}

/** Namespace object for window.ccoGateway / tests. */
export const gateway = {
  raw,
  isTauriReady,
  getProjects,
  addProject,
  removeProject,
  getProjectLive,
  setProjectDefaultPlan,
  getPlans,
  getPlanMeta,
  previewPlan,
  readPlanMd,
  sanitizePlanDeps,
  startPlanJob,
  getPlanJob,
  latestPlanJob,
  latestPlanJobForPlan,
  listPlanSplitIndex,
  updatePlanTask,
  removePlanTask,
  confirmStart,
  stopRun,
  stopTask,
  resumeRun,
  retryTask,
  startRework,
  acceptResidual,
  writebackMemory,
  projectMemoryGet,
  projectMemoryLastSummary,
  projectPinsList,
  projectPinUpsert,
  projectPinDelete,
  guideSessionsList,
  guideSessionStart,
  guideSessionGet,
  getProjectPersona,
  setProjectPersona,
  projectDismissRun,
  projectClearDismissedRun,
  projectGetDismissedRun,
  openTaskTerminal,
  chatListSessions,
  chatNewSession,
  chatDeleteSession,
  chatRenameSession,
  chatSessionGet,
  chatSend,
  chatClisList,
  chatSlashCatalog,
  chatStreamPartial,
  chatCancel,
  chatSavePlan,
  chatSaveWaveBundle,
  chatNormalizePlan,
  chatSaveAttachment,
  chatReadImageDataUrl,
  previewStart,
  previewStop,
  previewStatus,
  getSettings,
  setSettings,
  doctor,
  gitDoctor,
  gitStatus,
  gitCommit,
  gitPush,
  gitPull,
  gitFetch,
  gitBranchList,
  gitBranchCreate,
  gitBranchSwitch,
  gitBranchDelete,
  gitLog,
  gitDiff,
  gitStashList,
  gitStashPush,
  gitStashPop,
  gitStashApply,
  gitStashDrop,
  gitStashShow,
  gitTagList,
  gitTagCreate,
  gitTagDelete,
  gitTagShow,
  meta,
  openPath,
  openMonitorWindow,
  dialogOpen,
};

export default gateway;
