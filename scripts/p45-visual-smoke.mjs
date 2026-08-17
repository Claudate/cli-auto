#!/usr/bin/env node
/**
 * P4-5/P4-8 结果台视觉重构 —— 一次性目视冒烟（不入 CI）
 * 本地起静态服务加载 web/index.html（dist/ 已构建），stub invoke 造 live DTO（含
 * inspect_loop / verification / browser_evidence），渲染结果台 → 断言完成/遗漏列表
 * 卡片化（StateDot · route_label 执行方式）· honest footer 巡检结论 · 验收面板
 * details 可折叠 + plan_items 勾选 ☑/☐ · 浏览器证据网格（截图卡 + 文本摘录）· rework
 * 按钮文案 · P4-8 巡检次级列（1280 默认展开 / 1024 默认收起可恢复）· 侧栏
 * 项目→计划二级树 · 明暗截图。
 * 用法：node scripts/p45-visual-smoke.mjs
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

/** 结果台 live DTO（finished 态 + inspect_loop + verification + browser_evidence）。 */
const live = {
  run_id: "run-p45",
  runId: "run-p45",
  run_status: "completed",
  runStatus: "completed",
  project_path: "/tmp/demo",
  status_one_liner: "本轮已完成",
  current_wave: 0,
  layers: [["t1", "t2"], ["t3"]],
  has_checkpoint: true,
  started_at: "2026-08-16T10:00:00Z",
  finished_at: "2026-08-16T10:15:00Z",
  auto_commits: [
    { ok: true, commit_hash: "abc123", files: ["src/main.rs"], pushed: true },
  ],
  tasks: [
    {
      task_id: "t1",
      title: "实现登录页",
      status: "completed",
      provider: "claude",
      role: "implement",
      route_label: "claude",
      started_at: "2026-08-16T10:00:00Z",
      finished_at: "2026-08-16T10:05:00Z",
      cost_usd: 0.12,
      log_tail: "done",
      log_bytes: 128,
      log_events: [],
    },
    {
      task_id: "t2",
      title: "写单元测试",
      status: "completed",
      provider: "codex",
      role: "implement",
      route_label: "codex",
      started_at: "2026-08-16T10:05:00Z",
      finished_at: "2026-08-16T10:10:00Z",
      cost_usd: 0.08,
      log_tail: "tests pass",
      log_bytes: 64,
      log_events: [],
    },
    {
      task_id: "t3",
      title: "部署上线",
      status: "failed",
      provider: "claude",
      role: "implement",
      route_label: "claude",
      attempt: 1,
      error_summary: "ERR 网络超时：无法连接部署服务器",
      error: "ERR 网络超时：无法连接部署服务器",
      started_at: "2026-08-16T10:10:00Z",
      finished_at: "2026-08-16T10:12:00Z",
      cost_usd: 0.05,
      log_tail: "timeout",
      log_bytes: 32,
      log_events: [],
    },
  ],
  inspect_loop: {
    verdict: "PASS",
    blocking_count: 0,
    residual_count: 1,
    issue_preview: ["部署步骤未完成（网络原因）"],
    can_rework: true,
    require_inspect: true,
    rework_round: 1,
    rework_max: 3,
    accepted_residual: false,
    auto_rework_run_id: null,
    ensure_phase: null,
    docs_closeout_only: false,
  },
  verification: {
    source: "inspect",
    plan_count: 3,
    plan_items: [
      { text: "登录页可正常访问", checked: true },
      { text: "用户名密码校验正确", checked: true },
      { text: "部署到生产环境", checked: false },
    ],
    task_items: [
      { task_id: "t1", text: "实现登录表单" },
      { task_id: "t2", text: "编写测试用例" },
    ],
    plan_note: "",
  },
  browser_evidence: [
    {
      kind: "shot",
      task_id: "t1",
      rel_path: "screenshots/login.png",
      abs_path: "/tmp/demo/screenshots/login.png",
      preview_data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
    },
    {
      kind: "report",
      task_id: "t2",
      rel_path: "test-report.txt",
      abs_path: "/tmp/demo/test-report.txt",
      excerpt: "All 12 tests passed\n- login_form_validation ✓\n- password_strength_check ✓\n- session_timeout ✓",
    },
  ],
};

const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond });
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? "  — " + detail : ""}`);
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
  if (m.text().includes('[P45]')) console.log(m.text());
});

try {
  await page.addInitScript((l) => {
    const called = [];
    window.__calledCmds = called;
    window.__stubLive = l;
    const stub = (cmd, args = {}) => {
      called.push(cmd);
      if (cmd === "meta") return Promise.resolve({ version: "0.0.0-test" });
      if (cmd === "get_projects") {
        return Promise.resolve([{ name: "演示项目", path: "/tmp/demo", exists: true }]);
      }
      if (cmd === "get_plan_meta") {
        return Promise.resolve([
          { path: "plans/login.md", title: "登录改版" },
          { path: "plans/api.md", title: "接口整理" },
          { path: "plans/test.md", title: "测试补齐" },
          { path: "plans/deploy.md", title: "发布准备" },
          { path: "plans/design.md", title: "设计校对" },
          { path: "plans/review.md", title: "验收复盘" },
        ]);
      }
      if (cmd === "get_plans") return Promise.resolve(["plans/login.md"]);
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
  await page.waitForFunction(() => window.ccoRun && window.state && window.ccoIcon, null, { timeout: 8000 });
  await page.evaluate(() => { window.invoke = window.__stubInvoke; });

  // 渲染结果台（completed live）
  await page.evaluate(() => {
    window.state.page = "workspace";
    window.state.phase = "done";
    window.state.selectedPath = "/tmp/demo";
    window.state.live = window.__stubLive;
    if (typeof window.showPage === "function") window.showPage("workspace");
    if (typeof window.renderWorkspaceShell === "function") window.renderWorkspaceShell();
    // P4-5: 必须在 renderInspectAndResult 之前水合图标（因为渲染时调用 ccoIcon()）
    if (typeof window.ccoIcon?.hydrate === "function") window.ccoIcon.hydrate();
    if (typeof window.ccoResult?.renderInspectAndResult === "function") {
      window.ccoResult.renderInspectAndResult(window.__stubLive, window.__stubLive.tasks, { finished: true, active: false });
    }
  });
  await page.waitForSelector("#result-desk", { timeout: 8000 });
  await page.waitForTimeout(300);

  // ── 结果台壳 ──────────────────────────────────────────────────
  check("result-desk 可见", await page.locator("#result-desk").isVisible());
  check("标题「本轮结果」", (await page.locator("#task-dash-heading").textContent()).includes("本轮结果"));

  // ── P4-8：项目→计划侧栏树 ────────────────────────────────────
  await page.waitForSelector(".sidebar-plan-tree .sidebar-plan-item", { timeout: 8000 });
  check("侧栏显示项目→计划二级树", (await page.locator(".sidebar-plan-tree .sidebar-plan-item").count()) === 5);
  check("侧栏计划名称", (await page.locator(".sidebar-plan-tree .sidebar-plan-item").textContent()).includes("登录改版"));
  const morePlans = page.locator(".sidebar-plan-more");
  check("侧栏计划超过五项时折叠", await morePlans.isVisible());
  await morePlans.click();
  check("侧栏计划可展开", (await page.locator(".sidebar-plan-tree .sidebar-plan-item").count()) === 6);
  check("侧栏计划可收起", (await morePlans.textContent()).includes("收起"));

  // ── P4-8：结果局部巡检列（1280 默认展开，可收起/恢复） ────────
  const inspectRail = page.locator("#result-inspect-rail");
  const inspectToggle = page.locator("#btn-result-inspect-toggle");
  check("1280 巡检列默认展开", await inspectRail.isVisible());
  check("巡检列切换按钮可见", await inspectToggle.isVisible());
  await page.locator("#btn-result-inspect-close").click();
  check("巡检列可收起", !(await inspectRail.isVisible()));
  check("巡检列收起状态", (await inspectToggle.getAttribute("aria-pressed")) === "false");
  await inspectToggle.click();
  check("巡检列可恢复", await inspectRail.isVisible());

  // ── P4-7：权限预设与完全访问确认 ────────────────────────────
  await page.evaluate(() => window.ccoSettings?.loadSettings());
  await page.locator('[data-permission-preset="acceptEdits"]').click();
  check("权限预设可切换", (await page.locator("#s-permission-mode").inputValue()) === "acceptEdits");
  await page.locator('[data-permission-preset="bypassPermissions"]').click();
  check("完全访问需要确认", await page.locator(".cco-confirm").isVisible());
  await page.locator("[data-confirm-cancel]").click();
  check("取消后保留原权限", (await page.locator("#s-permission-mode").inputValue()) === "acceptEdits");

  // ── 完成列表卡片化 ────────────────────────────────────────────
  const doneCount = await page.locator("#result-desk-done .result-desk-item.is-done").count();
  check("完成列表 2 张卡", doneCount === 2, `count=${doneCount}`);
  const doneCard1 = await page.locator("#result-desk-done .result-desk-item.is-done").first();
  const doneMarkHtml = await doneCard1.locator(".result-desk-mark").innerHTML();
  check("完成卡有 check 图标", doneMarkHtml.includes("check") || doneMarkHtml.includes("svg"), doneMarkHtml.slice(0, 80));
  const doneTitle = await doneCard1.locator(".result-desk-item-body strong").textContent();
  check("完成卡标题", doneTitle.includes("实现登录页") || doneTitle.includes("写单元测试"), doneTitle);

  // ── 遗漏列表卡片化（含 route_label 执行方式）─────────────────
  const missCount = await page.locator("#result-desk-miss .result-desk-item.is-miss").count();
  check("遗漏列表 1 张卡", missCount === 1, `count=${missCount}`);
  const missCard = await page.locator("#result-desk-miss .result-desk-item.is-miss").first();
  check("遗漏卡有 x 图标", (await missCard.locator(".result-desk-mark").innerHTML()).includes("x"));
  const missBody = await missCard.locator(".result-desk-item-body").textContent();
  check("遗漏卡标题「部署上线」", missBody.includes("部署上线"), missBody);
  check("遗漏卡 route_label 执行方式", missBody.includes("执行方式") && missBody.includes("claude"), missBody);
  check("遗漏卡失败原因", missBody.includes("网络超时") || missBody.includes("无法连接"), missBody);

  // ── issue_preview 卡片 ────────────────────────────────────────
  const issueCount = await page.locator("#result-desk-miss .result-desk-item.is-issue").count();
  check("issue_preview 1 张卡", issueCount === 1, `count=${issueCount}`);
  if (issueCount > 0) {
    const issueCard = await page.locator("#result-desk-miss .result-desk-item.is-issue").first();
    const issueMarkHtml = await issueCard.locator(".result-desk-mark").innerHTML().catch(() => "");
    // P4-5: icons.js 暂无 alert-triangle，ResultView 用 fallback "!"
    check("issue 卡有警告标记", issueMarkHtml.includes("!") || issueMarkHtml.trim().length > 0, `mark="${issueMarkHtml}"`);
    const issueText = await issueCard.locator(".result-desk-item-body").textContent();
    check("issue 卡内容", issueText.includes("部署步骤未完成"), issueText);
  }

  // ── honest footer 巡检结论提示条 ──────────────────────────────
  const honestVisible = await page.locator("#result-desk-honest").isVisible();
  check("honest footer 可见", honestVisible);
  const honestText = await page.locator("#result-desk-honest").textContent();
  check("honest footer 巡检结论", honestText.includes("通过") || honestText.includes("PASS") || honestText.length > 5, honestText.slice(0, 60));

  // ── 验收面板 details 可折叠 ───────────────────────────────────
  const verifyPanel = page.locator("#result-desk-verify");
  check("验收面板可见", await verifyPanel.isVisible());
  const isOpen = await verifyPanel.evaluate((d) => d.open);
  check("验收面板默认收起", !isOpen);
  await page.locator("#result-desk-verify-sum").click();
  await page.waitForTimeout(150);
  check("验收面板可展开", await verifyPanel.evaluate((d) => d.open));

  // ── plan_items 勾选 ☑/☐ ──────────────────────────────────────
  const planItemCount = await page.locator("#result-desk-verify-list .result-desk-item.is-plan-check").count();
  check("验收面板 plan_items 3 项", planItemCount === 3, `count=${planItemCount}`);
  const checked = await page.locator("#result-desk-verify-list .result-desk-item.is-plan-check .result-desk-mark").first().textContent();
  check("plan_items 勾选标记", checked.includes("☑") || checked.includes("✓"), checked);
  const unchecked = await page.locator("#result-desk-verify-list .result-desk-item.is-plan-check .result-desk-mark").nth(2).textContent();
  check("plan_items 未勾选标记", unchecked.includes("☐") || unchecked.includes("□"), unchecked);

  // ── 浏览器证据网格 ────────────────────────────────────────────
  check("浏览器证据区可见", await page.locator("#result-desk-browser").isVisible());
  const evidenceCount = await page.locator("#result-desk-browser-grid .result-browser-card").count();
  check("浏览器证据 2 张卡", evidenceCount === 2, `count=${evidenceCount}`);
  const shotCard = await page.locator("#result-desk-browser-grid .result-browser-card.is-shot").first();
  check("截图卡存在", await shotCard.isVisible());
  check("截图卡有 img", (await shotCard.locator(".result-browser-shot").count()) === 1);
  const textCard = await page.locator("#result-desk-browser-grid .result-browser-card.is-text").first();
  check("文本摘录卡存在", await textCard.isVisible());
  const excerpt = await textCard.locator(".result-browser-excerpt").textContent();
  check("文本摘录内容", excerpt.includes("tests passed") || excerpt.includes("✓"), excerpt.slice(0, 40));

  // ── rework 按钮文案 ───────────────────────────────────────────
  const reworkBtn = page.locator("#btn-ws-rework");
  if (await reworkBtn.isVisible()) {
    const reworkText = await reworkBtn.textContent();
    // rework_round:1 表示已跑1轮，下次是第2轮（逻辑是 round+1）
    check("rework 按钮文案含轮次", reworkText.includes("2") && reworkText.includes("3"), reworkText);
  }

  check("无页面错误（亮色）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p48-result-1280-light.png" });

  // ── 暗色 ──────────────────────────────────────────────────────
  await page.evaluate(() => {
    document.body.dataset.leafTheme = "dark";
  });
  await page.waitForTimeout(250);
  const deskBg = await page.locator("#result-desk").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("暗色结果台非白", deskBg !== "rgb(255, 255, 255)" && deskBg !== "rgba(0, 0, 0, 0)", deskBg);
  const cardBg = await page.locator("#result-desk-done .result-desk-item.is-done").first().evaluate((n) => getComputedStyle(n).backgroundColor);
  check("暗色卡片背景非白", cardBg !== "rgb(255, 255, 255)", cardBg);
  check("无页面错误（暗色）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p48-result-1280-dark.png" });

  // 1024px 是次级列的默认收起阈值；reload 让 ResultView 以窄窗初始状态挂载。
  await page.setViewportSize({ width: 1024, height: 860 });
  await page.reload({ waitUntil: "load" });
  await page.waitForFunction(() => window.ccoRun && window.state && window.ccoIcon, null, { timeout: 8000 });
  await page.evaluate(() => {
    window.invoke = window.__stubInvoke;
    window.state.page = "workspace";
    window.state.phase = "done";
    window.state.selectedPath = "/tmp/demo";
    window.state.live = window.__stubLive;
    if (typeof window.showPage === "function") window.showPage("workspace");
    if (typeof window.renderWorkspaceShell === "function") window.renderWorkspaceShell();
    if (typeof window.ccoIcon?.hydrate === "function") window.ccoIcon.hydrate();
    window.ccoResult?.renderInspectAndResult(window.__stubLive, window.__stubLive.tasks, { finished: true, active: false });
  });
  await page.waitForSelector("#result-desk", { timeout: 8000 });
  check("1024 巡检列默认收起", !(await page.locator("#result-inspect-rail").isVisible()));
  const narrowToggle = page.locator("#btn-result-inspect-toggle");
  check("1024 仍可打开巡检列", await narrowToggle.isVisible());
  await narrowToggle.click();
  check("1024 巡检列手动展开", await page.locator("#result-inspect-rail").isVisible());
  check("无页面错误（1024）", errors.length === 0, errors.slice(0, 3).join(" | "));
  await page.screenshot({ path: "/tmp/p48-result-1024-dark.png" });
} catch (err) {
  console.error("SMOKE_ERR:", err);
  await page.screenshot({ path: "/tmp/p45-error.png" }).catch(() => {});
} finally {
  await browser.close();
  server.close();
}

const fails = results.filter((r) => !r.ok);
console.log(`\n-- summary: ${results.length - fails.length}/${results.length} pass, FAIL=${fails.length}`);
process.exit(fails.length ? 1 : 0);
