/**
 * [INPUT]: shared/gateway only
 * [OUTPUT]: project / plan / job IPC thin wrappers
 * [POS]: A5-2b-fin features/project/projectApi.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

import * as gateway from "../../shared/gateway.js";

export const getProjects = () => gateway.getProjects();
export const addProject = (path, name) => gateway.addProject(path, name);
export const removeProject = (path) => gateway.removeProject(path);
export const getProjectLive = (project, opts) =>
  gateway.getProjectLive(project, opts || {});
export const setProjectDefaultPlan = (project, plan) =>
  gateway.setProjectDefaultPlan(project, plan);
export const getPlans = (project) => gateway.getPlans(project);
export const getPlanMeta = (project) => gateway.getPlanMeta(project);
export const previewPlan = (project, plan) => gateway.previewPlan(project, plan);
export const startPlanJob = (args) => gateway.startPlanJob(args);
export const getPlanJob = (jobId) => gateway.getPlanJob(jobId);
export const latestPlanJob = (project) => gateway.latestPlanJob(project);
export const sanitizePlanDeps = (jobId) => gateway.sanitizePlanDeps(jobId);
export const doctor = (project) => gateway.doctor(project);
export const setSettings = (update) => gateway.setSettings(update);
export const dialogOpen = (options) => gateway.dialogOpen(options);
export function isTauriReady() {
  return gateway.isTauriReady();
}
