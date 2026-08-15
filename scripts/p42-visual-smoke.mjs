#!/usr/bin/env node
/**
 * P4-2 两栏壳 + 侧栏 —— 一次性目视冒烟（不入 CI）
 * 本地起静态服务加载 web/index.html（dist/ 已构建），stub invoke 造项目数据，
 * 选项目 → 断言 view-ring 出现 + 无页面错误；截图 light/dark + rail 折叠。
 * 用法：node scripts/p42-visual-smoke.mjs
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

const projects = [
  { path: "/Users/demo/overseas-site", name: "出海落地页站点", active_status: "running", running_tasks: 3, total_tasks: 8, last_status: "running", exists: true },
  { path: "/Users/demo/docs-kit", name: "文档套件重构", active_status: "completed", running_tasks: 0, total_tasks: 5, last_status: "completed", exists: true },
  { path: "/Users/demo/legacy-app", name: "旧系统迁移", active_status: "", last_status: "failed", exists: true },
];

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
  await page.addInitScript(() => {
    const projects = [
      { path: "/Users/demo/overseas-site", name: "出海落地页站点", active_status: "running", running_tasks: 3, total_tasks: 8, last_status: "running", exists: true },
      { path: "/Users/demo/docs-kit", name: "文档套件重构", active_status: "completed", running_tasks: 0, total_tasks: 5, last_status: "completed", exists: true },
      { path: "/Users/demo/legacy-app", name: "旧系统迁移", active_status: "", last_status: "failed", exists: true },
    ];
    const stub = (cmd, args = {}) => {
      if (cmd === "meta") return Promise.resolve({ version: "0.0.0-test" });
      if (cmd === "get_projects") return Promise.resolve(projects);
      return Promise.resolve(null);
    };
    window.__TAURI_INTERNALS__ = { invoke: stub };
    window.invoke = stub;
  });

  await page.goto(`${BASE}/index.html`, { waitUntil: "load" });
  await page.waitForTimeout(900); // main boot + softSync

  check("boot no pageerror", errors.length === 0, errors.slice(0, 3).join(" | "));
  check("project rows rendered", (await page.locator("#project-list .project-item").count()) === 3);
  check("sidebar-count=3", (await page.locator("#sidebar-count").textContent()) === "3");
  check("conn-status not 就绪中…", (await page.locator("#conn-status").textContent()) !== "就绪中…", await page.locator("#conn-status").textContent());

  // 选项目 → data-cco-project + view-ring 出现
  await page.evaluate(() => window.selectProject("/Users/demo/overseas-site"));
  await page.waitForTimeout(500);
  check("view-ring visible with project", await page.locator("#view-ring").isVisible());
  check("view-ring active 聊天(author)", (await page.locator('#view-ring .view-ring-item[data-ring="chat"]').evaluate((n) => getComputedStyle(n).fontWeight)) === "600");
  await page.screenshot({ path: "/tmp/p42-split-light.png" });

  // 点执行段 → phase=run 高亮
  await page.locator('#view-ring .view-ring-item[data-ring="run"]').click();
  await page.waitForTimeout(400);
  check("run segment active", await page.locator('#view-ring .view-ring-item[data-ring="run"]').evaluate((n) => getComputedStyle(n).fontWeight) === "600");

  // 搜索过滤
  await page.locator("#btn-sidebar-search-toggle").click();
  await page.locator("#sidebar-search-input").fill("docs");
  await page.waitForTimeout(400);
  check("search filters to 1", (await page.locator("#project-list .project-item").count()) === 1);
  check("search count 1/3", (await page.locator("#sidebar-count").textContent()) === "1/3");
  await page.locator("#sidebar-search-clear").click();
  await page.waitForTimeout(100);
  check("search cleared", (await page.locator("#project-list .project-item").count()) === 3);

  // 折叠 rail（宽窗）
  await page.locator("#btn-sidebar-collapse").click();
  await page.waitForTimeout(450); // 300ms slide
  check("rail applied", await page.evaluate(() => document.body.classList.contains("cco-sidebar-collapsed")));
  check("rail sidebar width 56px", (await page.locator(".sidebar").boundingBox()).width < 70);
  await page.screenshot({ path: "/tmp/p42-rail-light.png" });

  // 暗色
  await page.locator("#btn-sidebar-collapse").click(); // 展开回
  await page.evaluate(() => { document.body.dataset.leafTheme = "dark"; });
  await page.waitForTimeout(200);
  await page.screenshot({ path: "/tmp/p42-split-dark.png" });
  const sbBg = await page.locator(".sidebar").evaluate((n) => getComputedStyle(n).backgroundColor);
  check("dark sidebar non-white", sbBg !== "rgb(255, 255, 255)", sbBg);
  check("dark no pageerror", errors.length === 0, errors.slice(0, 3).join(" | "));
} catch (err) {
  console.error("SMOKE_ERR:", err);
  await page.screenshot({ path: "/tmp/p42-error.png" }).catch(() => {});
} finally {
  await browser.close();
  server.close();
}

const fails = results.filter((r) => !r.ok);
console.log(`\n-- summary: ${results.length - fails.length}/${results.length} pass, FAIL=${fails.length}`);
process.exit(fails.length ? 1 : 0);
