#!/usr/bin/env node
/**
 * P4-4 执行台视觉重构 —— 一次性目视冒烟（不入 CI）
 * 本地起静态服务加载 web/index.html（dist/ 已构建），stub invoke 造 live DTO，
 * 渲染执行台 → 断言任务流程卡 dsh 语言（StateDot · .is-running 蓝追光 · 失败卡执行方式 ·
 * 自动提交状态）· 右次级列 Terminal/Diff/Read + wait/stall 琥珀条 + 日志折叠 ·
 * 详情列可折叠 · 停/续/重跑仍经 ccoRun → runApi → gateway 1:1 · 明暗截图。
 * 用法：node scripts/p44-visual-smoke.mjs
 */
import { chromium } from "playwright";
import { existsSync, createReadStream } from "node:fs";
import http from "node:http";
import { extname, join, normalize } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const WEB_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "web");
if (!existsSync(join(WEB_DIR, "dist", "app.js"))) {
  console.error("BUILD_FAIL: web/dist/app.js 缺失（先 node build.mjs）");
  process.exit(2);
}

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json", ".ico": "image/x-icon", ".webmanifest": "application/manifest+json" };
const server = http.createServer((req, res) => {
  const urlPath = decodeURIComponent((req.url || "/").split("?")[0]);
  const file = join(WEB_DIR, normalize(urlPath === "/" ? "/index.html" : urlPath));
  if (!file.startsWith(WEB_DIR)) { res.writeHead(403); res.end(); return; }
  const ext = extname(file);
  if (!existsSync(file)) { res.writeHead(404); res.end("404"); return; }
  res.writeHead(200, { "Content-Type": MIME[ext] || "application/octet-stream" });
  createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(0, r));
const port = server.address().port;
const BASE = `http://localhost:${port}`;

/** 任务级 live DTO（与 src/services/live.rs 字段对齐；仅展示用，不写策略）。 */
const live = {
  run_id: "run-p44",
  runId: "run-p44",
  run_status: "running",
  runStatus: "running",
  project_path: "/tmp/demo",
  status_one_liner: "正在执行 5 个步骤中的第 2 步",
  current_wave: 1,
  layers: [["t1", "t2"], ["t3"], ["t4"], ["t5"]],
  has_checkpoint: true,
  started_at: "2026-08-15T09:00:00Z",
  auto_commits: [
    { ok: true, commit_hash: "a1b2c3d4", files: ["src/main.rs"], pushed: true },
  ],
  tasks: [
    {
      task_id: "t1",
      title: "构建产物",
      status: "running",
      provider: "claude",
      role: "implement",
      started_at: "2026-08-15T09:01:00Z",
      cost_usd: 0.0123,
      log_tail: "npm run build\n构建成功",
      log_bytes: 64,
      log_events: [
        { id: 1, kind: "message", title: "思考", summary: "先构建验证", level: "info" },
        { id: 2, kind: "tool_use", title: "Bash", summary: "npm run build", detail: '{"command":"npm run build"}' },
        { id: 3, kind: "tool_result", title: "结果·2", summary: "构建成功，dist/app.js", detail: "构建成功，产物 dist/app.js（长文本…）" },
      ],
    },
    {
      task_id: "t2",
      title: "跑单元测试",
      status: "pending",
      provider: "codex",
      role: "implement",
      depends_on: ["t1"],
      waiting_on: ["t1"],
      started_at: null,
      cost_usd: null,
      log_tail: "",
      log_bytes: 0,
      log_events: [],
    },
    {
      task_id: "t3",
      title: "打包上传",
      status: "failed",
      provider: "claude",
      role: "implement",
      route_label: "claude",
      attempt: 1,
      error_summary: "ERR 依赖解析失败：无法解析 crate semver",
      error: "ERR 依赖解析失败：无法解析 crate semver",
      log_tail: "ERR 依赖解析失败：无法解析 crate semver",
      log_bytes: 128,
      started_at: "2026-08-15T09:02:00Z",
      finished_at: "2026-08-15T09:03:30Z",
      cost_usd: 0.045,
      auto_commit: { ok: true, commit_hash: "b0b1b2b3", files: ["src/main.rs", "Cargo.toml"], pushed: true },
      log_events: [
        { id: 4, kind: "tool_use", title: "Write", summary: "编辑 Cargo.toml", detail: '{"file_path":"Cargo.toml"}' },
        { id: 5, kind: "tool_use", title: "Read", summary: "查看 main.rs", detail: '{"file_path":"src/main.rs"}' },
        { id: 6, kind: "tool_result", title: "结果·5", summary: "编译失败", detail: "error: failed to parse Cargo.toml" },
      ],
    },
    {
      task_id: "t4",
      title: "更新说明文档",
      status: "completed",
      provider: "deepseek",
      role: "implement",
      started_at: "2026-08-15T09:00:30Z",
      finished_at: "2026-08-15T09:01:10Z",
      cost_usd: 0.021,
      auto_commit: { ok: true, commit_hash: "9f8e7d6c", files: ["web/index.html", "web/css/app.css"], pushed: false },
      log_tail: "README 已更新",
      log_bytes: 32,
      log_events: [
        { id: 7, kind: "tool_use", title: "Edit", summary: "更新 index.html", detail: '{"file_path":"web/index.html"}' },
        { id: 8, kind: "tool_result", title: "结果·7", summary: "ok", detail: "done" },
      ],
    },
    {
      task_id: "t5",
      title: "上线巡检",
      status: "running",
      provider: "claude",
      role: "inspect",
      stall_idle_secs: 90,
      stall_threshold_secs: 120,
      started_at: "2026-08-15T09:04:00Z",
      cost_usd: null,
      log_tail: "正在检查线上页面…",
      log_bytes: 16,
      log_events: [],
    },
  ],
};

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond });
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? "  — " + detail : ""}`);
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1400, height: 860 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });

try {
  await page.addInitScript((l) => {
    const called = [];
    window.__calledCmds = called;
    window.__stubLive = l;
    const stub = (cmd, args = {}) => {
      called.push(cmd);
      if (cmd === "meta") return Promise.resolve({ version: "0.0.0-test" });
      if (cmd === "get_projects") return Promise.resolve([]);
      if (cmd === "get_settings") return Promise.resolve({ permission_mode: "bypassPermissions" });
      if (cmd === "doctor_status_cmd" || cmd === "doctor") return Promise.resolve({ ok: true });
      if (cmd === "latest_plan_job_cmd") return Promise.resolve(null);
      if (cmd === "get_project_live" || cmd === "getProjectLive") return Promise.resolve(window.__stubLive);
      return Promise.resolve(null);
    };
    window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
    window.__TAURI_INTERNALS__.invoke = stub;
    window.__stubInvoke = stub;
    window.invoke = stub;
  }, live);

  await page.goto(`${BASE}/index.html`, { waitUntil: "load" });
  await page.waitForFunction(() => window.ccoRun && window.state, null, { timeout: 8000 });
  await page.evaluate(() => { window.invoke = window.__stubInvoke; });

  // 渲染执行台（running live；单真源 window.__stubLive，避免轮询竞态）
  await page.evaluate(() => {
    window.state.page = "workspace";
    window.state.phase = "running";
    window.state.selectedPath = "/tmp/demo";
    window.state.selectedTaskId = "t1";
    window.state.live = window.__stubLive;
    if (typeof window.showPage === "function") window.showPage("workspace");
    if (typeof window.renderWorkspaceShell === "function") window.renderWorkspaceShell();
    window.ccoRun.renderProgress();
  });
  await page.waitForSelector('#cli-board .cli-window[data-task="t1"]', { timeout: 8000 });
  await page.waitForTimeout(300);

  // ── 执行台壳 ──────────────────────────────────────────────────
  check("run-flow-row 可见", await page.locator("#run-flow-row").isVisible());
  check("flow-row 内含 #monitor + 右次级列", (await page.locator("#run-flow-row > #monitor.monitor").count()) === 1 && (await page.locator("#run-flow-row > #run-detail-column").count()) === 1);
  check("看板工具条有详情开关", (await page.locator("#log-toolbar-side #btn-run-detail-toggle").count()) === 1);

  // ── 任务流程卡 dsh 语言 ───────────────────────────────────────
  const nCards = await page.locator("#cli-board .cli-window").count();
  check("流程卡 5 张", nCards === 5, `count=${nCards}`);
  const gridCols = await page.locator("#cli-board").evaluate((n) => getComputedStyle(n).gridTemplateColumns);
  check("cli-board 保持两列", (gridCols.trim().split(/\s+/).length) === 2, gridCols.slice(0, 80));
  check("每卡 StateDot", (await page.locator("#cli-board .cli-window .dot").count()) === 5);

  const runningCls = await page.locator('#cli-board .cli-window[data-task="t1"].is-running').count();
  check("t1 运行中 → is-running", runningCls === 1);
  const chaseName = await page.locator('#cli-board .cli-window[data-task="t1"]').evaluate((c) => getComputedStyle(c.querySelector(".cli-window-head"), "::after").animationName);
  check("is-running 蓝追光动画", chaseName === "cco-run-chase", chaseName);
  const runShadow = await page.locator('#cli-board .cli-window[data-task="t1"]').evaluate((c) => getComputedStyle(c).boxShadow);
  check("运行中卡片有描边辉光", runShadow && runShadow !== "none", runShadow.slice(0, 60));

  const human3 = await page.locator('#cli-board .cli-window[data-task="t3"] .cli-window-human').textContent().catch(() => "");
  check("t3 失败卡执行方式", /claude/.test(human3 || ""), human3);
  const git4 = await page.locator('#cli-board .cli-window[data-task="t4"] .cli-window-git').textContent().catch(() => "");
  check("t4 完成卡自动提交状态", /自动提交 9f8e7d6c/.test(git4 || ""), git4);
  const stallTxt = await page.locator('#cli-board .cli-window[data-task="t5"] .cli-window-stall').textContent().catch(() => "");
  check("t5 卡住条", /没有新进展/.test(stallTxt || ""), stallTxt);

  // ── 右次级列：Terminal/Diff/Read ─────────────────────────────
  check("详情列可见", await page.locator("#run-detail-column").isVisible());
  const colW = await page.locator("#run-detail-column").evaluate((n) => n.getBoundingClientRect().width);
  check("详情列约 320px", colW >= 310 && colW <= 330, `w=${Math.round(colW)}`);
  const dTitle = await page.locator("#run-detail-title").textContent();
  check("详情标题 = 选中任务", (dTitle || "").includes("构建产物"), dTitle);
  const termCmd = await page.locator(".run-detail-term-cmd").textContent().catch(() => "");
  check("TerminalBlock 命令", (termCmd || "").includes("npm run build"), termCmd);
  const termOut = await page.locator(".run-detail-term-out").textContent().catch(() => "");
  check("TerminalBlock 输出", /构建成功/.test(termOut || ""), termOut);
  check("复制按钮", (await page.locator("[data-copy-term]").count()) === 1);
  check("日志折叠 默认收起", !(await page.locator("[data-log-disclosure]").evaluate((d) => d.open)));

  // 选中 t2（排队）→ 琥珀条（只用既有 wait 语义，不造假「等待审批」）
  await page.evaluate(() => { window.state.selectedTaskId = "t2"; window.ccoRunDetail.render(window.state.live.tasks); });
  await page.waitForTimeout(150);
  const amberWait = await page.locator(".run-detail-amber").textContent().catch(() => "");
  check("t2 排队琥珀条", /排队等待 1 个前序/.test(amberWait || ""), amberWait);

  // 选中 t5（卡住）→ 琥珀条 stall 语义
  await page.evaluate(() => { window.state.selectedTaskId = "t5"; window.ccoRunDetail.render(window.state.live.tasks); });
  await page.waitForTimeout(150);
  const amberStall = await page.locator(".run-detail-amber").textContent().catch(() => "");
  check("t5 卡住琥珀条", /没有新进展/.test(amberStall || ""), amberStall);

  // 选中 t3（失败）→ 错误块 + 自动提交 DiffBlock + ReadBlock
  await page.evaluate(() => { window.state.selectedTaskId = "t3"; window.ccoRunDetail.render(window.state.live.tasks); });
  await page.waitForTimeout(150);
  const errTxt = await page.locator(".run-detail-error").textContent().catch(() => "");
  check("t3 失败错误块", /依赖解析失败/.test(errTxt || ""), errTxt);
  const diffFoot = await page.locator(".diff-foot").textContent().catch(() => "");
  check("自动提交 DiffBlock", /2 个变更文件/.test(diffFoot || ""), diffFoot);
  check("ReadBlock 读文件", (await page.locator(".read-block .run-detail-file").count()) >= 1);
  const dTitle3 = await page.locator("#run-detail-title").textContent();
  check("选中 t3 标题跟随", (dTitle3 || "").includes("打包上传"), dTitle3);

  // 卡片「聚焦」分发 → 详情列跟随
  await page.locator('#cli-board .cli-window[data-task="t3"] [data-focus="t3"]').click();
  await page.waitForTimeout(150);
  const dTitleFocus = await page.locator("#run-detail-title").textContent();
  check("聚焦分发 → 详情跟随", (dTitleFocus || "").includes("打包上传"), dTitleFocus);

  // 渲染幂等：签名未变则 body 不变
  const sig1 = await page.evaluate(() => {
    const body = document.getElementById("run-detail-body");
    const before = body.innerHTML;
    window.ccoRunDetail.render(window.state.live.tasks);
    return { same: body.innerHTML === before, len: before.length };
  });
  check("详情渲染幂等（签名未变不重绘）", sig1.same, `len=${sig1.len}`);

  // 详情列折叠：toggle 按钮 / 关闭按钮 · aria-pressed 同步
  await page.locator("#btn-run-detail-toggle").click();
  await page.waitForTimeout(120);
  check("toggle 折叠详情列", !(await page.locator("#run-detail-column").isVisible()));
  check("toggle aria-pressed=false", (await page.locator("#btn-run-detail-toggle").getAttribute("aria-pressed")) === "false");
  await page.locator("#btn-run-detail-toggle").click();
  await page.waitForTimeout(120);
  check("toggle 再展开", await page.locator("#run-detail-column").isVisible());
  await page.locator("#btn-run-detail-close").click();
  await page.waitForTimeout(120);
  check("关闭按钮折叠详情列", !(await page.locator("#run-detail-column").isVisible()));
  await page.locator("#btn-run-detail-toggle").click();
  await page.waitForTimeout(120);

  check("无页面错误（基线）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p44-run-light.png" });

  // ── 停 / 重跑 / 续：仍经 ccoRun → runApi → gateway 1:1 ───────
  const stopBtn = page.locator('#cli-board .cli-window[data-task="t1"] [data-stop="t1"]');
  if (await stopBtn.isVisible()) {
    await stopBtn.click();
    await page.waitForTimeout(250);
    const cmds = await page.evaluate(() => window.__calledCmds);
    check("停步 → stop_task_cmd", cmds.includes("stop_task_cmd"), cmds.filter((c) => c.includes("stop")).join(","));
  }

  // 失败卡「再跑一次」→ retry_task_cmd
  const rerunBtn = page.locator('#cli-board .cli-window[data-task="t3"] [data-rerun="t3"]');
  if (await rerunBtn.isVisible()) {
    await rerunBtn.click();
    await page.waitForTimeout(250);
    const cmds = await page.evaluate(() => window.__calledCmds);
    check("重跑失败步 → retry_task_cmd", cmds.includes("retry_task_cmd"), cmds.filter((c) => c.includes("retry")).join(","));
  }

  // 暂停 → 日志栏「继续」→ resume_run_cmd
  await page.evaluate(() => { window.__stubLive.run_status = "paused"; window.state.live = window.__stubLive; window.ccoRun.renderProgress(); });
  await page.waitForTimeout(300);
  const resumeVisible = await page.locator("#btn-log-resume").isVisible();
  check("暂停态显示「继续」", resumeVisible);
  if (resumeVisible) {
    await page.locator("#btn-log-resume").click();
    await page.waitForTimeout(250);
    const cmds = await page.evaluate(() => window.__calledCmds);
    check("继续 → resume_run_cmd", cmds.includes("resume_run_cmd"), cmds.filter((c) => c.includes("resume")).join(","));
  }

  // ── 暗色 ──────────────────────────────────────────────────────
  await page.evaluate(() => {
    document.body.dataset.leafTheme = "dark";
    window.__stubLive.run_status = "running";
    window.state.live = window.__stubLive;
    window.state.selectedTaskId = "t1";
    window.ccoRun.renderProgress();
  });
  await page.waitForTimeout(250);
  const colBg = await page.locator("#run-detail-column").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("暗色详情列非白", colBg !== "rgb(255, 255, 255)" && colBg !== "rgba(0, 0, 0, 0)", colBg);
  const termBg = await page.locator(".run-detail-term").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("暗色 TerminalBlock 终端深面", termBg !== "rgb(255, 255, 255)", termBg);
  check("无页面错误（暗色）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p44-run-dark.png" });
} catch (err) {
  console.error("SMOKE_ERR:", err);
  await page.screenshot({ path: "/tmp/p44-error.png" }).catch(() => {});
} finally {
  await browser.close();
  server.close();
}

const fails = results.filter((r) => !r.ok);
console.log(`\n-- summary: ${results.length - fails.length}/${results.length} pass, FAIL=${fails.length}`);
process.exit(fails.length ? 1 : 0);
