import { test, expect } from "@playwright/test";
import fs from "fs";
import path from "path";

/**
 * Cross-project split desk isolation (structural gate)
 *
 * Regression: tab 拆分 / softSync / residual DOM must never paint project A's
 * tasks under project B. Uses mock-tauri + direct state injection.
 */

const mockTauriContent = fs.readFileSync(
  path.join(__dirname, "../..", "web", "mock-tauri-ipc.js"),
  "utf-8"
);

async function waitAppReady(page) {
  await page.goto("/index.html");
  await page.waitForLoadState("networkidle");
  await page.waitForFunction(
    () =>
      !!(
        window.state &&
        window.ccoSplit &&
        typeof window.setBoundPlanJob === "function" &&
        typeof window.rebindSplitToOpenProject === "function"
      ),
    null,
    { timeout: 15000 }
  );
}

async function listProjectPaths(page) {
  return page.evaluate(() =>
    (window.state?.projects || []).map((p) => p.path).filter(Boolean)
  );
}

/** Ensure at least two distinct project paths exist in state (mock or inject). */
async function ensureTwoProjects(page) {
  const paths = await listProjectPaths(page);
  if (paths.length >= 2) return paths.slice(0, 2);
  // Inject two synthetic projects when mock has fewer
  await page.evaluate(() => {
    const s = window.state;
    if (!s) return;
    const a = "/tmp/cco-iso-project-a";
    const b = "/tmp/cco-iso-project-b";
    s.projects = [
      { path: a, name: "iso-A" },
      { path: b, name: "iso-B" },
      ...(s.projects || []).filter((p) => p.path !== a && p.path !== b),
    ];
    if (typeof window.renderProjectList === "function") {
      try {
        window.renderProjectList();
      } catch (_) {}
    }
  });
  return ["/tmp/cco-iso-project-a", "/tmp/cco-iso-project-b"];
}

test.describe("P0 · 跨项目拆分台隔离", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript((content) => {
      // eslint-disable-next-line no-eval
      eval(content);
    }, mockTauriContent);
  });

  test("A 的 planJob 注入后切 B → 拆分 tab 不得出现 A 的任务标题", async ({
    page,
  }) => {
    await waitAppReady(page);
    const [pathA, pathB] = await ensureTwoProjects(page);

    // Select A and bind a foreign-looking job owned by A
    await page.evaluate(
      ({ pathA, pathB }) => {
        const s = window.state;
        s.selectedPath = pathA;
        if (typeof window.bumpProjectScope === "function") {
          window.bumpProjectScope();
        }
        const jobA = {
          job_id: "job-iso-a",
          project: pathA,
          status: "planned",
          tasks: [
            { id: "t1", title: "ISO-TASK-FROM-PROJECT-A", include: true },
            { id: "t2", title: "ISO-A-SECOND", include: true },
          ],
        };
        window.setBoundPlanJob(jobA, {
          projectPath: pathA,
          allowMissingProjectField: false,
        });
        s.phase = "confirm";
        // Paint desk under A
        if (window.ccoSplit?.vm?.setJob) {
          window.ccoSplit.vm.setJob(jobA, { jobId: "job-iso-a" });
        }
        if (typeof window.stampSplitDeskProject === "function") {
          window.stampSplitDeskProject(pathA);
        }
        const waves = document.getElementById("confirm-waves");
        if (waves) {
          waves.innerHTML =
            '<div class="wave-task" data-id="t1">ISO-TASK-FROM-PROJECT-A</div>';
          waves.dataset.ccoBoundProject = pathA;
        }
      },
      { pathA, pathB }
    );

    // Switch to B the same way selectProject does (bump + clear + rebind)
    await page.evaluate(
      ({ pathA, pathB }) => {
        const s = window.state;
        if (typeof window.stashPlanSession === "function") {
          try {
            window.stashPlanSession(pathA);
          } catch (_) {}
        }
        window.bumpProjectScope();
        const gen = window.currentScopeGen();
        s.selectedPath = pathB;
        s.live = null;
        window.setBoundPlanJob(null, { projectPath: pathB, gen });
        s.phase = "pick";
        window.clearSplitUiBinding({ scrubState: false, projectPath: pathB });
        window.rebindSplitToOpenProject();
      },
      { pathA, pathB }
    );

    // Ring → 拆分 (same as user click path: bind then goSplit then paint)
    await page.evaluate(() => {
      if (typeof window.rebindSplitToOpenProject === "function") {
        window.rebindSplitToOpenProject();
      }
      if (window.ccoApp?.goSplit) window.ccoApp.goSplit();
      if (typeof window.renderPhasePanels === "function") {
        window.renderPhasePanels();
      }
      if (typeof window.renderConfirmPanel === "function") {
        window.renderConfirmPanel();
      }
      if (window.ccoSplit?.render) window.ccoSplit.render();
    });

    await page.waitForTimeout(300);

    const deskText = await page.evaluate(() => {
      const waves = document.getElementById("confirm-waves");
      return (waves?.innerText || waves?.textContent || "").trim();
    });
    const boundJob = await page.evaluate(() => {
      const j =
        typeof window.getBoundPlanJob === "function"
          ? window.getBoundPlanJob()
          : window.state?.planJob;
      return j
        ? {
            id: j.job_id || j.jobId,
            project: j.project || j.project_path,
            titles: (j.tasks || []).map((t) => t.title),
          }
        : null;
    });
    const vmSnap = await page.evaluate(() => {
      const snap = window.ccoSplit?.vm?.getSnapshot?.();
      return snap
        ? {
            jobId: snap.jobId,
            titles: (snap.job?.tasks || []).map((t) => t.title),
          }
        : null;
    });

    expect(deskText).not.toContain("ISO-TASK-FROM-PROJECT-A");
    expect(deskText).not.toContain("ISO-A-SECOND");
    if (boundJob) {
      expect(boundJob.titles || []).not.toContain("ISO-TASK-FROM-PROJECT-A");
      if (boundJob.project) {
        expect(String(boundJob.project)).not.toBe(pathA);
      }
    }
    if (vmSnap?.titles?.length) {
      expect(vmSnap.titles).not.toContain("ISO-TASK-FROM-PROJECT-A");
    }
  });

  test("softSync 不会把 A 的 residual VM 留在 B", async ({ page }) => {
    await waitAppReady(page);
    const [pathA, pathB] = await ensureTwoProjects(page);

    await page.evaluate(
      ({ pathA, pathB }) => {
        const s = window.state;
        // Poison VM as if A was open
        const jobA = {
          job_id: "job-poison-a",
          project: pathA,
          status: "planned",
          tasks: [{ id: "t1", title: "POISON-A-TASK", include: true }],
        };
        s.selectedPath = pathB;
        s.planJob = null;
        s.planJobId = null;
        if (window.ccoSplit?.vm?.setJob) {
          window.ccoSplit.vm.setJob(jobA, { jobId: "job-poison-a" });
        }
        const waves = document.getElementById("confirm-waves");
        if (waves) {
          waves.innerHTML = "<div>POISON-A-TASK</div>";
          waves.dataset.ccoBoundProject = pathA;
        }
        // softSync path
        window.rebindSplitToOpenProject();
        if (window.ccoSplit?.render) window.ccoSplit.render();
      },
      { pathA, pathB }
    );

    const text = await page.locator("#confirm-waves").innerText().catch(() => "");
    expect(text).not.toContain("POISON-A-TASK");
    const vmTitles = await page.evaluate(() => {
      const snap = window.ccoSplit?.vm?.getSnapshot?.();
      return (snap?.job?.tasks || []).map((t) => t.title);
    });
    expect(vmTitles).not.toContain("POISON-A-TASK");
  });

  test("setBoundPlanJob 拒绝跨项目 job 写入 selectedPath", async ({ page }) => {
    await waitAppReady(page);
    const [pathA, pathB] = await ensureTwoProjects(page);
    const result = await page.evaluate(
      ({ pathA, pathB }) => {
        window.state.selectedPath = pathB;
        const ok = window.setBoundPlanJob(
          {
            job_id: "x",
            project: pathA,
            tasks: [{ id: "t1", title: "nope" }],
          },
          { projectPath: pathB }
        );
        return {
          ok,
          planJobId: window.state.planJobId,
          bound: window.getBoundPlanJob(pathB),
        };
      },
      { pathA, pathB }
    );
    expect(result.ok).toBe(false);
    expect(result.planJobId).toBeFalsy();
    expect(result.bound).toBeFalsy();
  });
});
