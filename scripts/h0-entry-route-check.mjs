/**
 * H0 entry-route pure logic check (no DOM / Tauri).
 * Mirrors resolveEntryRoute in web/js/features/project/sessionEntry.js:
 *   1. hasActiveRun or paused → workspace + running
 *   2. else (planning / confirm / done / idle) → chat
 */
function isLiveStatus(s) {
  return ["running", "starting", "queued", "validated", "init", "resuming"].includes(
    String(s || "").toLowerCase()
  );
}

function hasActiveRun(live) {
  return !!(live?.run_id && isLiveStatus(live?.run_status));
}

function isRunPaused(live) {
  return !!(
    live?.run_id && String(live?.run_status || "").toLowerCase() === "paused"
  );
}

function resolveEntryRoute({ live, phase, planJobId, planJob }) {
  if (hasActiveRun(live)) {
    return { page: "workspace", phaseHint: "running" };
  }
  if (isRunPaused(live)) {
    return { page: "workspace", phaseHint: "running" };
  }
  // planning / confirm / done / idle → chat (default open)
  void phase;
  void planJobId;
  void planJob;
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

// starting/queued → workspace
{
  const r = resolveEntryRoute({
    live: { run_id: "r1", run_status: "starting" },
    phase: "pick",
    planJobId: null,
    planJob: null,
  });
  assert("starting → workspace", r.page === "workspace" && r.phaseHint === "running");
}

// paused → workspace
{
  const r = resolveEntryRoute({
    live: { run_id: "r1", run_status: "paused" },
    phase: "pick",
    planJobId: null,
    planJob: null,
  });
  assert("paused → workspace", r.page === "workspace" && r.phaseHint === "running");
}

// planJob planning → chat（不默认抢入口）
{
  const r = resolveEntryRoute({
    live: null,
    phase: "planning",
    planJobId: "j1",
    planJob: { status: "planning" },
  });
  assert("planning job → chat", r.page === "chat");
}

// planJob planned (confirm) → chat
{
  const r = resolveEntryRoute({
    live: null,
    phase: "confirm",
    planJobId: "j1",
    planJob: { status: "planned" },
  });
  assert("planned job → chat", r.page === "chat");
}

// active run wins over confirm desk
{
  const r = resolveEntryRoute({
    live: { run_id: "r1", run_status: "running" },
    phase: "confirm",
    planJobId: "j1",
    planJob: { status: "planned" },
  });
  assert(
    "active run beats confirm → workspace",
    r.page === "workspace" && r.phaseHint === "running"
  );
}

if (process.exitCode) {
  console.error("\nH0 entry route check FAILED");
  process.exit(1);
}
console.log("\nH0 entry route check passed");
