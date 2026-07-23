/**
 * [INPUT]: gateway only（禁止 __TAURI__/invoke）
 * [OUTPUT]: Split 用例薄封装（job / 编辑 / confirm）
 * [POS]: A3-1 features/split；业务规则在 Rust app/split
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 *
 * 禁止：start_run 旁路、soft-fill、optional 策略复制。
 * 开跑只经 confirmStart → confirm_start_cmd → app::split::confirm。
 */

import * as gateway from "../../shared/gateway.js";

/** @param {string} jobId */
export function getJob(jobId) {
  return gateway.getPlanJob(jobId);
}

/** @param {string} project */
export function latestJob(project) {
  return gateway.latestPlanJob(project);
}

/**
 * Start Mode B plan job (parse | fake | ai).
 * preserve_from_job_id 由调用方传入；本层不发明 replan 规则。
 * @param {Record<string, unknown>} args start_plan_job_cmd payload
 */
export function startJob(args) {
  return gateway.startPlanJob(args);
}

/**
 * Patch one proposed task — title/prompt/include/provider/dependsOn/role/scopePaths.
 * 不复制 soft-fill；role/scope 只透传用户输入，路由策略在 Rust。
 * @param {{
 *   jobId: string,
 *   taskId: string,
 *   title?: string|null,
 *   prompt?: string|null,
 *   include?: boolean|null,
 *   provider?: string|null,
 *   dependsOn?: string[]|null,
 *   role?: string|null,
 *   scopePaths?: string[]|null,
 * }} args
 */
export function updateTask(args) {
  return gateway.updatePlanTask({
    jobId: args.jobId,
    taskId: args.taskId,
    title: args.title ?? null,
    prompt: args.prompt ?? null,
    include: args.include ?? null,
    provider: args.provider ?? null,
    dependsOn: args.dependsOn ?? null,
    role: args.role ?? null,
    scopePaths: args.scopePaths ?? null,
  });
}

/** @param {{ jobId: string, taskId: string }} args */
export function removeTask(args) {
  return gateway.removePlanTask({
    jobId: args.jobId,
    taskId: args.taskId,
  });
}

/**
 * 唯一业务开跑入口。
 * @param {string} jobId
 * @param {string|null|undefined} [effort] low…max|ultracode — 执行时的推理深度
 * @returns {Promise<{ run_id?: string, runId?: string }>}
 */
export function confirmStart(jobId, effort) {
  return gateway.confirmStart(jobId, effort || null);
}

/** @param {string} jobId */
export function sanitizeDeps(jobId) {
  return gateway.sanitizePlanDeps(jobId);
}

/** Paused run resume (not a new open-run). @param {string} runId */
export function resumeRun(runId) {
  return gateway.resumeRun(runId);
}
