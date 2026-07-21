/**
 * One-shot: extract web/js/plan.js residual into features/project/* modules.
 * Run from repo root: node scripts/extract-plan-project.mjs
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const PLAN = path.join(ROOT, "web/js/plan.js");
const OUT = path.join(ROOT, "web/js/features/project");

const src = fs.readFileSync(PLAN, "utf8");
const lines = src.split("\n");

/** slice 1-based inclusive line numbers → text without trailing empty */
function slice(a, b) {
  return lines.slice(a - 1, b).join("\n").replace(/\n+$/, "") + "\n";
}

/** Convert top-level `function name` / `async function name` → export */
function toExports(body) {
  return body
    .replace(/^async function /gm, "export async function ")
    .replace(/^function /gm, "export function ")
    .replace(/^const ([A-Z_]+) = /gm, "export const $1 = ");
}

/**
 * Rewrite same-desk calls to host.X when the callee lives in another module.
 * Keep classic globals (toast, state, $, showPage, …) untouched.
 */
function rewriteCross(body, ownFns, allDeskFns) {
  let out = body;
  for (const name of allDeskFns) {
    if (ownFns.has(name)) continue;
    // call site: name(  but not .name( or host.name( or function name( or export ... name(
    const re = new RegExp(
      `(?<![.\\w$])(?<!function\\s)(?<!export\\sfunction\\s)(?<!export\\sasync\\sfunction\\s)${name}\\s*\\(`,
      "g"
    );
    out = out.replace(re, `host.${name}(`);
    // typeof renderX === "function" → typeof host.renderX === "function"
    const reTypeof = new RegExp(
      `typeof\\s+${name}\\s*===\\s*["']function["']`,
      "g"
    );
    out = out.replace(reTypeof, `typeof host.${name} === "function"`);
  }
  return out;
}

function listFns(body) {
  const set = new Set();
  for (const m of body.matchAll(/^export (?:async )?function (\w+)/gm)) {
    set.add(m[1]);
  }
  for (const m of body.matchAll(/^export const (\w+) =/gm)) {
    set.add(m[1]);
  }
  return set;
}

const HEADER = (file, input, output, note = "") => `/**
 * [INPUT]: ${input}
 * [OUTPUT]: ${output}
 * [POS]: A5-2b-fin features/project/${file}
 * note: ${note || "IPC only via projectApi/gateway；禁止 start_run 旁路；optional 不静默 auto-start"}
 * [PROTOCOL]: 变更时更新此头部，然后检查 web/CLAUDE.md
 */

`;

// ── module line ranges (1-based, from current plan.js) ──
// session + entry + selectProject + banner
const MODS = [
  {
    file: "sessionEntry.js",
    range: [23, 499],
    extraAfter: "",
    note: "plan session stash + H0 entry route + selectProject + bg banner",
  },
  {
    file: "shellChrome.js",
    range: [501, 645],
    // continue chips/top later via second slice appended
    extraAfterRanges: [
      [1098, 1113], // renderWorkspaceShell, setPlanCollapsed
      [1479, 1547], // chips
      [1667, 1714], // top plan info + preview
    ],
    note: "phase panels · flow strips · chips · top title",
  },
  {
    file: "projectCrud.js",
    range: [647, 737],
    note: "add/remove project · dismiss run · doctor bridge",
  },
  {
    file: "planMeta.js",
    range: [739, 966],
    note: "plan meta · executed partition · loadPlansForPicker",
  },
  {
    file: "projectPicker.js",
    range: [968, 1477],
    // 968-1096 busy+execute+shell; 1108 already in shell? 1108-1113 is shell — skip overlap
    // Actually 968-1477 includes renderWorkspaceShell 1098-1113 and full picker
    // shellChrome also has 1098-1113 — DEDUPE: projectPicker starts 968, skips 1098-1113 if in shell
    note: "assign busy · execute · chooser · renderPlanPicker · max parallel partial",
  },
  {
    file: "planSelect.js",
    // 1549–1665 (confirm open + max parallel) + 1716–1816 (select/pick/default)
    // 1667–1714 top title/preview lives in shellChrome
    range: [1549, 1665],
    extraAfterRanges: [[1716, 1816]],
    note: "confirm open · max parallel · selectPlan · pick file · default",
  },
  {
    file: "jobPoll.js",
    range: [1818, 2122],
    note: "start_plan_job · poll · optional gate · advance (no silent auto-start past optionals)",
  },
  {
    file: "confirmActions.js",
    range: [2124, 2434],
    note: "confirm desk → ccoSplit; replan/sanitize via gateway",
  },
  {
    file: "loadLiveBridge.js",
    range: [2436, 2518],
    note: "loadLive / ensureSelectedTask → ccoLoadLive",
  },
];

// Fix projectPicker range to avoid double-export of shell helpers:
// 968-1096 + 1115-1477 (skip 1098-1113 owned by shellChrome)
function bodyFor(mod) {
  if (mod.file === "projectPicker.js") {
    return slice(968, 1096) + "\n" + slice(1115, 1477);
  }
  if (mod.file === "shellChrome.js") {
    let b = slice(501, 645);
    for (const [a, c] of mod.extraAfterRanges || []) b += "\n" + slice(a, c);
    return b;
  }
  if (mod.extraAfterRanges) {
    let b = slice(...mod.range);
    for (const [a, c] of mod.extraAfterRanges) b += "\n" + slice(a, c);
    return b;
  }
  return slice(mod.range[0], mod.range[1]);
}

// First pass: collect all desk function names
const rawBodies = {};
for (const mod of MODS) {
  rawBodies[mod.file] = toExports(bodyFor(mod));
}
const allDeskFns = new Set();
for (const b of Object.values(rawBodies)) {
  for (const n of listFns(b)) allDeskFns.add(n);
}

// Known classic / external globals — never rewrite to host
const CLASSIC = new Set([
  "toast",
  "showPage",
  "hasActiveRun",
  "isRunPaused",
  "isLiveStatus",
  "isFailedStatus",
  "toastRunLocked",
  "normalizePlanPath",
  "planDisplayName",
  "fillPlannerLog",
  "canEditSelectedTask",
  "openNativeDialog",
  "loadProjects",
  "renderProjectList",
  "renderWorkspace",
  "goHome",
  "closeModal",
  "openChatPage",
  "stashChatSession",
  "restoreChatSession",
  "stopChatWaitTicker",
  "loadPlanRail",
  "renderPlanRail",
  "selectPlanRailItem",
  "renderPlansMgmtPage",
  "openPlanManagement",
  "chatAssignDirectEnabled",
  "flowModeLabel",
  "flowModeHint",
  "flowStageStripHtml",
  "flowChooserSub",
  "flowJoinSeriousFun",
  "flowPickBlurb",
  "flowPlanHowLabel",
  "flowPlanningSub",
  "flowSanitizeDepsLabel",
  "flowRunningMonitorTitle",
  "esc",
  "requireGateway",
  "Date",
  "Math",
  "Number",
  "String",
  "Array",
  "Object",
  "JSON",
  "Promise",
  "Set",
  "Map",
  "console",
  "localStorage",
  "document",
  "setInterval",
  "clearInterval",
  "setTimeout",
  "parseInt",
  "isNaN",
]);

const rewriteTargets = [...allDeskFns].filter((n) => !CLASSIC.has(n));

fs.mkdirSync(OUT, { recursive: true });

const importLegacy = `import {
  state,
  $,
  toast,
  showPage,
  hasActiveRun,
  isRunPaused,
  isLiveStatus,
  isFailedStatus,
  toastRunLocked,
  normalizePlanPath,
  planDisplayName,
  fillPlannerLog,
  canEditSelectedTask,
  openNativeDialog,
  loadProjects,
  renderProjectList,
  renderWorkspace,
  goHome,
  closeModal,
  openChatPage,
  stashChatSession,
  restoreChatSession,
  stopChatWaitTicker,
  loadPlanRail,
  renderPlanRail,
  selectPlanRailItem,
  renderPlansMgmtPage,
  chatAssignDirectEnabled,
  flowModeLabel,
  flowModeHint,
  flowStageStripHtml,
  flowChooserSub,
  flowJoinSeriousFun,
  flowPickBlurb,
  flowPlanHowLabel,
  flowPlanningSub,
  flowSanitizeDepsLabel,
  flowRunningMonitorTitle,
  esc,
  requireGateway,
} from "./legacy.js";
import { host } from "./host.js";
`;

for (const mod of MODS) {
  const own = listFns(rawBodies[mod.file]);
  let body = rewriteCross(rawBodies[mod.file], own, rewriteTargets);
  // state / $ come from legacy — already bare identifiers via import { state, $ }
  const text =
    HEADER(mod.file, "legacy host + gateway via requireGateway", mod.note, mod.note) +
    importLegacy +
    "\n" +
    body;
  fs.writeFileSync(path.join(OUT, mod.file), text);
  console.log(
    "wrote",
    mod.file,
    "lines",
    text.split("\n").length,
    "fns",
    [...own].join(",")
  );
}

// host.js
fs.writeFileSync(
  path.join(OUT, "host.js"),
  HEADER("host.js", "module registrations", "host bag for cross-calls") +
    `/** @type {Record<string, any>} */
export const host = {};

export function register(partial) {
  Object.assign(host, partial);
}
`
);

// projectApi.js
fs.writeFileSync(
  path.join(OUT, "projectApi.js"),
  HEADER(
    "projectApi.js",
    "shared/gateway only",
    "project / plan / job IPC thin wrappers"
  ) +
    `import * as gateway from "../../shared/gateway.js";

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
`
);

// ProjectViewModel.js
fs.writeFileSync(
  path.join(OUT, "ProjectViewModel.js"),
  HEADER(
    "ProjectViewModel.js",
    "optional projectPath seed",
    "thin project selection snapshot (no business policy)"
  ) +
    `export function createProjectViewModel(opts = {}) {
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
`
);

// legacy.js
fs.writeFileSync(
  path.join(OUT, "legacy.js"),
  HEADER(
    "legacy.js",
    "window globals from classic scripts",
    "state proxy + classic helpers for features/project"
  ) +
    `/** Live view of classic \`state\`. */
export const state = new Proxy(
  {},
  {
    get(_t, prop) {
      if (prop === Symbol.toStringTag) return "StateProxy";
      const s = typeof window !== "undefined" ? window.state : null;
      if (!s) return undefined;
      const v = s[prop];
      return typeof v === "function" ? v.bind(s) : v;
    },
    set(_t, prop, val) {
      const s = typeof window !== "undefined" ? window.state : null;
      if (!s) return false;
      s[prop] = val;
      return true;
    },
    has(_t, prop) {
      const s = typeof window !== "undefined" ? window.state : null;
      return !!(s && prop in s);
    },
  }
);

export function $(sel, el = document) {
  if (typeof window !== "undefined" && typeof window.$ === "function") {
    return window.$(sel, el);
  }
  return el && el.querySelector ? el.querySelector(sel) : null;
}

function call(name, ...args) {
  const fn = typeof window !== "undefined" ? window[name] : null;
  if (typeof fn === "function") return fn(...args);
  return undefined;
}

export const toast = (...a) => call("toast", ...a);
export const showPage = (...a) => call("showPage", ...a);
export const hasActiveRun = (...a) => !!call("hasActiveRun", ...a);
export const isRunPaused = (...a) => !!call("isRunPaused", ...a);
export const isLiveStatus = (...a) => !!call("isLiveStatus", ...a);
export const isFailedStatus = (...a) => !!call("isFailedStatus", ...a);
export const toastRunLocked = (...a) => call("toastRunLocked", ...a);
export const normalizePlanPath = (...a) => call("normalizePlanPath", ...a);
export const planDisplayName = (...a) => call("planDisplayName", ...a);
export const fillPlannerLog = (...a) => call("fillPlannerLog", ...a);
export const canEditSelectedTask = (...a) => call("canEditSelectedTask", ...a);
export const openNativeDialog = (...a) => call("openNativeDialog", ...a);
export const loadProjects = (...a) => call("loadProjects", ...a);
export const renderProjectList = (...a) => call("renderProjectList", ...a);
export const renderWorkspace = (...a) => call("renderWorkspace", ...a);
export const goHome = (...a) => call("goHome", ...a);
export const closeModal = (...a) => call("closeModal", ...a);
export const openChatPage = (...a) => call("openChatPage", ...a);
export const stashChatSession = (...a) => call("stashChatSession", ...a);
export const restoreChatSession = (...a) => call("restoreChatSession", ...a);
export const stopChatWaitTicker = (...a) => call("stopChatWaitTicker", ...a);
export const loadPlanRail = (...a) => call("loadPlanRail", ...a);
export const renderPlanRail = (...a) => call("renderPlanRail", ...a);
export const selectPlanRailItem = (...a) => call("selectPlanRailItem", ...a);
export const renderPlansMgmtPage = (...a) => call("renderPlansMgmtPage", ...a);
export const chatAssignDirectEnabled = (...a) => call("chatAssignDirectEnabled", ...a);
export const flowModeLabel = (...a) => call("flowModeLabel", ...a);
export const flowModeHint = (...a) => call("flowModeHint", ...a);
export const flowStageStripHtml = (...a) => call("flowStageStripHtml", ...a);
export const flowChooserSub = (...a) => call("flowChooserSub", ...a);
export const flowJoinSeriousFun = (...a) => call("flowJoinSeriousFun", ...a);
export const flowPickBlurb = (...a) => call("flowPickBlurb", ...a);
export const flowPlanHowLabel = (...a) => call("flowPlanHowLabel", ...a);
export const flowPlanningSub = (...a) => call("flowPlanningSub", ...a);
export const flowSanitizeDepsLabel = (...a) => call("flowSanitizeDepsLabel", ...a);
export const flowRunningMonitorTitle = (...a) => call("flowRunningMonitorTitle", ...a);
export const esc = (...a) =>
  typeof call("esc", ...a) !== "undefined"
    ? call("esc", ...a)
    : String(a[0] ?? "")
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");

/** Prefer gateway; throw if neither classic nor main ready. */
export function requireGateway() {
  if (typeof window !== "undefined" && typeof window.requireGateway === "function") {
    return window.requireGateway();
  }
  if (typeof window !== "undefined" && window.ccoGateway) return window.ccoGateway;
  throw new Error("gateway not ready");
}
`
);

// Collect export names per file for install
const exportMap = {};
for (const mod of MODS) {
  exportMap[mod.file] = [...listFns(rawBodies[mod.file])];
}

// installProject.js
const imports = MODS.map(
  (m) => `import * as ${m.file.replace(".js", "")} from "./${m.file}";`
).join("\n");

const registerSpreads = MODS.map(
  (m) => `    ...${m.file.replace(".js", "")},`
).join("\n");

const allNames = [...allDeskFns].sort();
const windowAssigns = allNames
  .map((n) => `  window.${n} = host.${n};`)
  .join("\n");

const deskFields = allNames.map((n) => `    ${n}: host.${n},`).join("\n");

fs.writeFileSync(
  path.join(OUT, "installProject.js"),
  HEADER(
    "installProject.js",
    "project modules",
    "window.ccoProject + classic global names (strangler)"
  ) +
    `import { register, host } from "./host.js";
import * as projectApi from "./projectApi.js";
import { createProjectViewModel } from "./ProjectViewModel.js";
${imports}

/** Wire host bag once. */
export function installProjectHost() {
  register({
${registerSpreads}
  });
  return host;
}

/**
 * Public desk for window.ccoProject (classic plan.js is facade).
 * IPC only via projectApi / requireGateway → gateway.
 * confirm/open-run still only via ccoSplit (confirmActions delegates).
 */
export function createProjectDesk(opts = {}) {
  installProjectHost();
  const vm = createProjectViewModel({
    projectPath: opts.projectPath || null,
  });
  return {
    vm,
    api: projectApi,
    host,
${deskFields}
  };
}

/**
 * Install classic global names used by bindUi / chat / templates.
 * @param {{ projectPath?: string|null }} [opts]
 */
export function installProjectHostGlobals(opts = {}) {
  const desk = createProjectDesk(opts);
  window.ccoProject = desk;
${windowAssigns}
  return desk;
}
`
);

// index.js
fs.writeFileSync(
  path.join(OUT, "index.js"),
  HEADER(
    "index.js",
    "project feature modules",
    "public barrel for features/project"
  ) +
    `export { createProjectViewModel } from "./ProjectViewModel.js";
export { createProjectDesk, installProjectHost, installProjectHostGlobals } from "./installProject.js";
export * as projectApi from "./projectApi.js";
export { host } from "./host.js";
`
);

console.log("all desk fns:", allNames.length);
console.log("done →", OUT);
