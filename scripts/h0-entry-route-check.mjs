/**
 * H0 entry-route pure logic check (no DOM / Tauri).
 * Mirrors resolveEntryRoute priority in web/js/plan.js §4.1:
 *   1. hasActiveRun → workspace + running
 *   2. planJob planning / planned|confirmed (confirm phase) → workspace
 *   3. else (done / no live) → chat
 */
function isLiveStatus(s) {
  return ["running", "starting", "queued", "validated", "init", "resuming"].includes(
    String(s || "").toLowerCase()
  );
}

function hasActiveRun(live) {
  return !!(live?.run_id && isLiveStatus(live?.run_status));
}

function isPlanSessionActive(phase) {
  return phase === "planning" || phase === "confirm";
}

function resolveEntryRoute({ live, phase, planJobId, planJob }) {
  if (hasActiveRun(live)) {
    return { page: "workspace", phaseHint: "running" };
  }
  if (isPlanSessionActive(phase) && planJobId) {
    return {
      page: "workspace",
      phaseHint: phase === "planning" ? "planning" : "confirm",
    };
  }
  const st = String(planJob?.status || "").toLowerCase();
  if (planJobId && (st === "planning" || st === "planned" || st === "confirmed")) {
    return {
      page: "workspace",
      phaseHint: st === "planning" ? "planning" : "confirm",
    };
  }
  return { page: "chat", phaseHint: null };
}

function assert(name, cond) {
  if (!cond) {
    console.error("FAIL:", name);
    process.exitCode = 1;
  } else {
    console.log("ok:", name);
  }
}

// 无活动 run / 无 planJob → chat
assert(
  "idle → chat",
  resolveEntryRoute({ live: null, phase: "pick", planJobId: null, planJob: null }).page === "chat"
);

// done live → chat
assert(
  "done run → chat",
  resolveEntryRoute({
    live: { run_id: "r1", run_status: "completed" },
    phase: "done",
    planJobId: null,
    planJob: null,
  }).page === "chat"
);

// 活动 run → workspace running
{
  const r = resolveEntryRoute({
    live: { run_id: "r1", run_status: "running" },
    phase: "pick",
    planJobId: null,
    planJob: null,
  });
  assert("active run → workspace", r.page === "workspace" && r.phaseHint === "running");
}

// planJob planning → workspace planning
{
  const r = resolveEntryRoute({
    live: null,
    phase: "planning",
    planJobId: "j1",
    planJob: { status: "planning" },
  });
  assert("planning job → workspace planning", r.page === "workspace" && r.phaseHint === "planning");
}

// planJob planned (confirm) → workspace confirm
{
  const r = resolveEntryRoute({
    live: null,
    phase: "confirm",
    planJobId: "j1",
    planJob: { status: "planned" },
  });
  assert("planned job → workspace confirm", r.page === "workspace" && r.phaseHint === "confirm");
}

// 活动 run 优先于 planJob
{
  const r = resolveEntryRoute({
    live: { run_id: "r1", run_status: "starting" },
    phase: "confirm",
    planJobId: "j1",
    planJob: { status: "planned" },
  });
  assert("active run beats planJob", r.page === "workspace" && r.phaseHint === "running");
}

// status-only planned without phase session (disk restore edge)
{
  const r = resolveEntryRoute({
    live: null,
    phase: "pick",
    planJobId: "j2",
    planJob: { status: "planned" },
  });
  assert("status planned without phase → workspace", r.page === "workspace" && r.phaseHint === "confirm");
}

if (process.exitCode) {
  console.error("\nH0 entry route check FAILED");
  process.exit(1);
}
console.log("\nH0 entry route check passed");
