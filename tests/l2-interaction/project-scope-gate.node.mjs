/**
 * [INPUT]: node:test · features/project/projectScope (window.state mock)
 * [OUTPUT]: cross-project planJob ownership + generation gate 回归
 * [POS]: tests/l2-interaction · 防串台结构闸（不依赖完整桌面壳）
 * note: 纯 gate 行为；DOM/VM 串台见 cross-project-split-isolation.spec.js
 * [PROTOCOL]: 变更时更新此头部，然后检查 tests/CLAUDE.md
 */

import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const scopeUrl = pathToFileURL(
  path.join(root, "web/js/features/project/projectScope.js")
).href;

/** Minimal browser globals so legacy state Proxy works under node. */
function installWindowState(initial = {}) {
  const state = {
    selectedPath: null,
    planJob: null,
    planJobId: null,
    confirmTaskId: null,
    confirmEditing: false,
    planSessions: {},
    ...initial,
  };
  globalThis.window = { state };
  globalThis.document = {
    getElementById: () => null,
    querySelector: () => null,
  };
  return state;
}

describe("projectScope ownership gate", () => {
  /** @type {any} */
  let scope;
  /** @type {any} */
  let state;

  beforeEach(async () => {
    state = installWindowState();
    // Module is cached; _scopeGen is process-global. Tests assert relative bump.
    scope = await import(scopeUrl);
  });

  it("pathsEqualProject normalizes trailing slash + case", () => {
    assert.equal(
      scope.pathsEqualProject("/Users/A/proj/", "/Users/A/proj"),
      true
    );
    assert.equal(
      scope.pathsEqualProject("/Users/A/Proj", "/Users/a/proj"),
      true
    );
    assert.equal(
      scope.pathsEqualProject("/Users/A/proj", "/Users/A/other"),
      false
    );
  });

  it("planJobBelongsToProject uses job.project SoT", () => {
    const job = {
      job_id: "j1",
      project: "/p/a",
      tasks: [{ id: "t1", title: "A-only" }],
    };
    assert.equal(scope.planJobBelongsToProject(job, "/p/a"), true);
    assert.equal(scope.planJobBelongsToProject(job, "/p/b"), false);
  });

  it("setBoundPlanJob rejects foreign project field", () => {
    state.selectedPath = "/p/b";
    const foreign = {
      job_id: "ja",
      project: "/p/a",
      tasks: [{ id: "t1", title: "from-A" }],
    };
    const ok = scope.setBoundPlanJob(foreign, { projectPath: "/p/b" });
    assert.equal(ok, false);
    assert.equal(state.planJob, null);
  });

  it("setBoundPlanJob accepts matching project", () => {
    state.selectedPath = "/p/b";
    const job = {
      job_id: "jb",
      project: "/p/b",
      tasks: [{ id: "t1", title: "from-B" }],
    };
    const ok = scope.setBoundPlanJob(job, { projectPath: "/p/b" });
    assert.equal(ok, true);
    assert.equal(state.planJobId, "jb");
    assert.equal(state.planJob.tasks[0].title, "from-B");
  });

  it("stale generation drops write after bumpProjectScope", () => {
    state.selectedPath = "/p/b";
    // Clear any previous job
    scope.setBoundPlanJob(null, { projectPath: "/p/b" });
    const gen = scope.currentScopeGen();
    const job = {
      job_id: "j-old",
      project: "/p/b",
      tasks: [{ id: "t1", title: "stale" }],
    };
    scope.bumpProjectScope();
    const ok = scope.setBoundPlanJob(job, { projectPath: "/p/b", gen });
    assert.equal(ok, false);
    assert.equal(state.planJob, null);
  });

  it("getBoundPlanJob returns null for foreign residual state", () => {
    state.selectedPath = "/p/b";
    // Simulate leak: raw assign bypass (should not happen in app code)
    state.planJob = {
      job_id: "ja",
      project: "/p/a",
      tasks: [{ id: "t1", title: "from-A" }],
    };
    state.planJobId = "ja";
    assert.equal(scope.getBoundPlanJob("/p/b"), null);
    assert.equal(scope.scrubForeignPlanJob("/p/b"), true);
    assert.equal(state.planJob, null);
  });

  it("scrubForeignPlanJob leaves state when no project open (goHome banner)", () => {
    state.selectedPath = null;
    state.planJob = {
      job_id: "ja",
      project: "/p/a",
      tasks: [{ id: "t1", title: "from-A" }],
    };
    state.planJobId = "ja";
    assert.equal(scope.scrubForeignPlanJob(null), false);
    assert.equal(state.planJob?.job_id, "ja");
  });
});
