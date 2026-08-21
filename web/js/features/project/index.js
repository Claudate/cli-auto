/**
 * [INPUT]: project feature modules
 * [OUTPUT]: public barrel for features/project
 * [POS]: A5-2b-fin features/project/index.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export { createProjectViewModel } from "./ProjectViewModel.js";
export { createProjectDesk, installProjectHost, installProjectHostGlobals } from "./installProject.js";
export * as projectApi from "./projectApi.js";
export * as projectScope from "./projectScope.js";
export {
  setBoundPlanJob,
  getBoundPlanJob,
  bumpProjectScope,
  rebindSplitToOpenProject,
  planJobBelongsToProject,
  scrubForeignPlanJob,
  clearSplitUiBinding,
} from "./projectScope.js";
export { host } from "./host.js";
