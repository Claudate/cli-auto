/**
 * [INPUT]: optional projectPath seed
 * [OUTPUT]: thin project selection snapshot (no business policy)
 * [POS]: A5-2b-fin features/project/ProjectViewModel.js
 * note: IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

export function createProjectViewModel(opts = {}) {
  let projectPath = opts.projectPath || null;
  let phase = null;
  return {
    setProject(path) {
      projectPath = path || null;
    },
    setPhase(p) {
      phase = p || null;
    },
    getSnapshot() {
      return { projectPath, phase };
    },
  };
}
