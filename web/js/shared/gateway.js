/**
 * [INPUT]: Tauri invoke surface (A1-7 command names, 1:1)
 * [OUTPUT]: typed async API for ViewModels / features
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
export const updatePlanTask = (args) => raw("update_plan_task_cmd", args);
export const removePlanTask = (args) => raw("remove_plan_task_cmd", args);
/** 唯一业务开跑入口（Split 确认）；禁止 UI 旁路 start_run */
export const confirmStart = (jobId) => raw("confirm_start_cmd", { jobId });

/* ── Run ── */
export const stopRun = (runId) => raw("stop_run_cmd", { runId });
export const stopTask = (runId, taskId) =>
  raw("stop_task_cmd", { runId, taskId });
export const resumeRun = (runId) => raw("resume_run_cmd", { runId });
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
export const openTaskTerminal = (args) => raw("open_task_terminal_cmd", args);

/* ── Chat (author) ── */
export const chatListSessions = (project) =>
  raw("chat_list_sessions_cmd", { project });
export const chatNewSession = (project, title) =>
  raw("chat_new_session_cmd", { project, title: title ?? null });
export const chatDeleteSession = (project, sessionId) =>
  raw("chat_delete_session_cmd", { project, sessionId });
export const chatSessionGet = (project, sessionId) =>
  raw("chat_session_get_cmd", { project, sessionId });
export const chatSend = (args) => raw("chat_send_cmd", args);
export const chatStreamPartial = (args) => raw("chat_stream_partial_cmd", args);
export const chatSavePlan = (args) => raw("chat_save_plan_cmd", args);
export const chatNormalizePlan = (args) => raw("chat_normalize_plan_cmd", args);
export const chatSaveAttachment = (args) =>
  raw("chat_save_attachment_cmd", args);

/* ── Settings / doctor / shell ── */
export const getSettings = () => raw("get_settings_cmd");
export const setSettings = (update) => raw("set_settings_cmd", { update });
export const doctor = (project) =>
  raw("doctor_cmd", { project: project || null });
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
  updatePlanTask,
  removePlanTask,
  confirmStart,
  stopRun,
  stopTask,
  resumeRun,
  startRework,
  acceptResidual,
  writebackMemory,
  projectMemoryGet,
  projectMemoryLastSummary,
  projectPinsList,
  projectPinUpsert,
  projectPinDelete,
  openTaskTerminal,
  chatListSessions,
  chatNewSession,
  chatDeleteSession,
  chatSessionGet,
  chatSend,
  chatStreamPartial,
  chatSavePlan,
  chatNormalizePlan,
  chatSaveAttachment,
  getSettings,
  setSettings,
  doctor,
  meta,
  openPath,
  openMonitorWindow,
  dialogOpen,
};

export default gateway;
