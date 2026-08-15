#!/usr/bin/env node
/**
 * P4-3 拆分台视觉重构 —— 一次性目视冒烟（不入 CI）
 * 本地起静态服务加载 web/index.html（dist/ 已构建），stub invoke 造拆分数据，
 * 渲染确认台 → 断言任务卡 dsh 语言（StateDot · route pill 簇 · 默认通道 chip ·
 * optional 徽标 · chevron）· 无旧卡片 provider 下拉复活 · 底部确认 dock 提示涂装；
 * 再注入 live 断言状态点颜色与 runLocked 提示；截图 light/dark。
 * 用法：node scripts/p43-visual-smoke.mjs
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

const job = {
  job_id: "job-p43",
  jobId: "job-p43",
  status: "planned",
  provider: "claude",
  run_id: "run-p43",
  runId: "run-p43",
  plan_path: "plans/demo.md",
  project: "/tmp/demo",
  layers: [["t1", "t2"], ["t3"], ["t4"], ["sys-post-1"]],
  tasks: [
    { id: "t1", title: "写首页文案", prompt: "【做什么】写首页主标题\n【怎样算做完】文案定稿", provider: "claude", optional: false, include: true, depends_on: [], role: "implement", risk_class: "write_local", risk_label: "改本地" },
    { id: "t2", title: "更新落地页图", prompt: "【做什么】替换 hero 图\n【怎样算做完】图已替换", provider: "codex", optional: true, include: false, depends_on: ["t1"], role: "implement", risk_class: "read", risk_label: "只读" },
    { id: "t3", title: "上线前巡检", prompt: "【做什么】检查页面\n【怎样算做完】无红屏", provider: "claude", optional: false, include: true, depends_on: ["t2"], role: "inspect", risk_class: "exec", risk_label: "跑命令" },
    { id: "t4", title: "发布到线上", prompt: "【做什么】推送并发布\n【怎样算做完】线上可访问", provider: "deepseek", optional: false, include: true, depends_on: ["t3"], role: "implement", risk_class: "external", risk_label: "会外发" },
    { id: "sys-post-1", title: "写回计划摘要", prompt: "【做什么】把步骤摘要追加到计划文末", provider: "claude", optional: true, include: true, depends_on: ["t4"], role: "", risk_class: "write_local", risk_label: "改本地" },
  ],
};

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond });
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? "  — " + detail : ""}`);
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });

try {
  await page.addInitScript((j) => {
    const stub = (cmd, args = {}) => {
      if (cmd === "meta") return Promise.resolve({ version: "0.0.0-test" });
      if (cmd === "get_projects") return Promise.resolve([]);
      if (cmd === "get_settings") return Promise.resolve({ permission_mode: "bypassPermissions" });
      if (cmd === "doctor_status_cmd" || cmd === "doctor") return Promise.resolve({ ok: true });
      if (cmd === "latest_plan_job_cmd") return Promise.resolve(null);
      if (cmd === "update_plan_task_cmd") return Promise.resolve({ ...j });
      return Promise.resolve(null);
    };
    window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
    window.__TAURI_INTERNALS__.invoke = stub;
    window.__stubInvoke = stub;
    window.invoke = stub;
  }, job);

  await page.goto(`${BASE}/index.html`, { waitUntil: "load" });
  await page.waitForFunction(() => window.ccoSplit && window.state, null, { timeout: 8000 });
  await page.evaluate(() => { window.invoke = window.__stubInvoke; });

  // 渲染确认台（无 live：runLocked=false）
  await page.evaluate((j) => {
    window.state.phase = "confirm";
    window.state.selectedPath = "/tmp/demo";
    window.state.planJob = j;
    window.state.planJobId = j.job_id;
    window.state.confirmTaskId = "t1";
    window.state.page = "workspace";
    if (typeof window.showPage === "function") window.showPage("workspace");
    if (typeof window.renderPhasePanels === "function") window.renderPhasePanels();
    window.ccoSplit.render();
  }, job);
  await page.waitForTimeout(400);

  // ── 任务卡 dsh 语言 ──────────────────────────────────────────────
  const nTasks = job.tasks.length;
  check("任务卡渲染 5 张", (await page.locator(".wave-task-row.split-card").count()) === nTasks);
  check("每卡 StateDot", (await page.locator(".wave-task-row .dot").count()) === nTasks);
  check("每卡默认通道 chip", (await page.locator(".split-channel-chip").count()) === nTasks);
  check("每卡角色 pill", (await page.locator(".split-role-pill").count()) === nTasks);
  check("每卡 chevron", (await page.locator(".split-chevron").count()) === nTasks);
  const chip1 = await page.locator('.wave-task-row[data-id="t1"] .split-channel-chip').textContent();
  const chip2 = await page.locator('.wave-task-row[data-id="t2"] .split-channel-chip').textContent();
  check("通道 chip 文案跟随 provider", chip1 === "Claude" && chip2 === "Codex", `${chip1} / ${chip2}`);
  check("optional 徽标 2 个（业务可选+系统）", (await page.locator(".opt-badge").count()) === 2);
  const optOnT2 = await page.locator('.wave-task-row[data-id="t2"] .opt-badge').count();
  check("t2 带 optional 徽标", optOnT2 === 1);
  const riskChips = await page.locator(".risk-badge").count();
  check("风险 chip 保留", riskChips === 5, `count=${riskChips}`);
  const scopePills = await page.locator(".split-scope-pill").count();
  check("范围 pill 数量", scopePills >= 0);

  // ── 旧卡片 provider 下拉不得复活（P2-17 详头下拉唯一 · 57ab9d6） ──
  check("卡片无 provider 下拉复活", (await page.locator(".split-provider-select").count()) === 0);
  check("卡片无 provider 容器复活", (await page.locator(".split-provider-wrap").count()) === 0);

  // ── 底部确认 dock ──────────────────────────────────────────────
  check("confirm dock 可见", await page.locator(".confirm-dock").isVisible());
  check("dock primary「执行规划」", (await page.locator("#btn-confirm-start").textContent()) === "执行规划");
  check("dock primary 可点（无 runLocked）", !(await page.locator("#btn-confirm-start").isDisabled()));
  const hint = await page.locator("#split-confirm-hint").textContent();
  check("dock hint 涂装（可选停住）", /未勾选/.test(hint || ""), hint);
  const hintWarn = await page.locator("#split-confirm-hint").evaluate((n) => n.classList.contains("is-warn"));
  check("dock hint 未勾选标 warn", hintWarn);

  // ── 详情栏卡片样式 ─────────────────────────────────────────────
  const detailVisible = await page.locator(".confirm-detail").isVisible();
  check("详情栏可见", detailVisible);
  const detailBg = await page.locator(".confirm-detail").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("详情栏有卡片底色", detailBg && detailBg !== "rgba(0, 0, 0, 0)", detailBg);
  check("无页面错误（基线）", errors.length === 0, errors.slice(0, 3).join(" | "));

  // 选中任务 → 详情标题跟随
  await page.evaluate(() => { window.state.confirmTaskId = "t3"; window.ccoSplit.render(); });
  await page.waitForTimeout(200);
  const title = await page.locator("#confirm-task-title").textContent();
  check("选中 t3 详情标题跟随", (title || "").includes("上线前巡检"), title);

  await page.screenshot({ path: "/tmp/p43-split-light.png" });

  // ── live：状态点颜色 + runLocked dock 提示 ─────────────────────
  await page.evaluate((j) => {
    window.state.live = {
      run_id: "run-p43",
      run_status: "running",
      tasks: [
        { task_id: "t1", status: "completed" },
        { task_id: "t2", status: "running" },
      ],
    };
    window.ccoSplit.render();
  }, job);
  await page.waitForTimeout(300);
  check("t1 已完成 → dot.ok", (await page.locator('.wave-task-row[data-id="t1"] .dot.ok').count()) === 1);
  check("t2 运行中 → dot.run", (await page.locator('.wave-task-row[data-id="t2"] .dot.run').count()) === 1);
  const runHint = await page.locator("#split-confirm-hint").textContent();
  check("runLocked dock hint「运行中」", /运行中/.test(runHint || ""), runHint);
  check("runLocked primary 禁用", await page.locator("#btn-confirm-start").isDisabled());

  // 暗色
  await page.evaluate(() => { document.body.dataset.leafTheme = "dark"; });
  await page.waitForTimeout(200);
  const dockBg = await page.locator(".confirm-dock").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("暗色 dock 非白", dockBg !== "rgb(255, 255, 255)", dockBg);
  check("无页面错误（暗色）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p43-split-dark.png" });
} catch (err) {
  console.error("SMOKE_ERR:", err);
  await page.screenshot({ path: "/tmp/p43-error.png" }).catch(() => {});
} finally {
  await browser.close();
  server.close();
}

const fails = results.filter((r) => !r.ok);
console.log(`\n-- summary: ${results.length - fails.length}/${results.length} pass, FAIL=${fails.length}`);
process.exit(fails.length ? 1 : 0);
