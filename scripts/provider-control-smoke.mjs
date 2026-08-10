#!/usr/bin/env node
/**
 * Real-browser guard: on the split confirm desk, the header「默认通道」dropdown
 * (#confirm-task-provider) must be enabled (when this job's run is not active)
 * and selecting it must persist via the VM.
 *
 * Loads web/ via local server, seeds state.planJob + phase=confirm, calls
 * ccoSplit.render(), then asserts the control is clickable and updates.
 */
import { chromium } from "playwright";

const BASE = process.env.BASE || "http://localhost:3457";
const results = [];
function check(name, cond, detail = "") {
  results.push({ name, ok: !!cond, detail: detail || "" });
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}${detail ? "  — " + detail : ""}`);
}

const job = {
  job_id: "job-test-1",
  jobId: "job-test-1",
  status: "planned",
  provider: "claude",
  plan_path: "plans/demo.md",
  project: "/tmp/demo",
  tasks: [
    {
      id: "t1",
      title: "写首页文案",
      prompt: "【做什么】写首页主标题\n【怎样算做完】文案定稿",
      provider: "claude",
      optional: false,
      include: true,
      depends_on: [],
      role: "",
      scope_paths: null,
    },
    {
      id: "t2",
      title: "更新落地页图",
      prompt: "【做什么】替换 hero 图\n【怎样算做完】图已替换",
      provider: "codex",
      optional: true,
      include: true,
      depends_on: ["t1"],
      role: "",
      scope_paths: null,
    },
  ],
};

const browser = await chromium.launch();
const page = await browser.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});

try {
  // Stub the backend before modules evaluate, so module-load-time calls
  // (e.g. settingsApi get_settings) resolve instead of throwing「invoke 不可用」.
  // Provide the Tauri-priority slot (__TAURI_INTERNALS__.invoke) that gateway's
  // getInvoke() checks first AND the legacy window.invoke (state.js re-binds the
  // latter at load, so we also re-assert after load below).
  await page.addInitScript(() => {
    const stub = (cmd, args) => {
      if (cmd === "update_plan_task_cmd") {
        const j = window.state?.planJob;
        const t = (j?.tasks || []).find((x) => x.id === args.taskId);
        if (t) t.provider = args.provider;
        return Promise.resolve(j ? { ...j } : {});
      }
      if (cmd === "get_settings") {
        return Promise.resolve({ permission_mode: "bypassPermissions" });
      }
      if (cmd === "doctor_status_cmd" || cmd === "doctor") {
        return Promise.resolve({ ok: true });
      }
      // shellBoot H0 reads state.projects on boot — return an empty array so
      // projects.find doesn't throw on a {} stub.
      if (cmd === "get_projects") {
        return Promise.resolve([]);
      }
      if (cmd === "latest_plan_job_cmd") {
        return Promise.resolve(null);
      }
      return Promise.resolve({});
    };
    window.__stubInvoke = stub;
    window.invoke = stub;
    window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
    window.__TAURI_INTERNALS__.invoke = stub;
  });

  await page.goto(`${BASE}/index.html`, { waitUntil: "load" });
  // Wait for ESM main.js globals.
  await page.waitForFunction(() => window.ccoSplit && window.state, null, {
    timeout: 8000,
  });
  // state.js binds its own classic `invoke` over the init stub at load; re-assert
  // our stub so update_plan_task_cmd round-trips instead of throwing「invoke 不可用」.
  await page.evaluate(() => {
    window.invoke = window.__stubInvoke;
  });

  // Seed the confirm desk, then switch the workspace page + confirm phase panel
  // visible so the dropdowns are actually on screen (showPage/renderPhasePanels
  // mirror what the desktop shell does on entry).
  await page.evaluate((j) => {
    window.state.phase = "confirm";
    window.state.selectedPath = "/tmp/demo";
    window.state.planJob = j;
    window.state.planJobId = j.job_id;
    window.state.live = null;
    window.state.confirmTaskId = "t1";
    window.state.page = "workspace";
    if (typeof window.showPage === "function") window.showPage("workspace");
    if (typeof window.renderPhasePanels === "function") {
      window.renderPhasePanels();
    }
    window.ccoSplit.render();
  }, job);

  // Let render + selectUi settle.
  await page.waitForTimeout(400);

  // #confirm-task-provider is enhanced → its visible trigger owns the open/select.
  const headerTrigger = page.locator("#confirm-task-provider__trigger");
  const triggerCount = await headerTrigger.count();
  check("确认台「默认通道」下拉已增强", triggerCount > 0, `triggers=${triggerCount}`);
  const headerDisabled = await headerTrigger.first().isDisabled();
  check("确认台「默认通道」下拉未禁用", !headerDisabled, `disabled=${headerDisabled}`);

  if (triggerCount > 0 && !headerDisabled) {
    // Open the enhanced menu and pick 备用通道 (index 1) via the real option button.
    await headerTrigger.first().click();
    await page.waitForTimeout(100);
    const headerMenu = page.locator("#confirm-task-provider__menu");
    const menuVisible = await headerMenu.isVisible();
    check("「默认通道」菜单可打开", menuVisible);
    const options = headerMenu.locator(".cco-select__option");
    const optCount = await options.count();
    check("菜单含 8 个通道选项", optCount === 8, `options=${optCount}`);
    await options.nth(1).click(); // 备用通道
    await page.waitForTimeout(300);
    const jobProvider = await page.evaluate(() => {
      const j = window.state.planJob;
      return (j.tasks || []).find((x) => x.id === "t1")?.provider;
    });
    check("选备用通道后 t1 通道持久化", jobProvider === "codex", `provider=${jobProvider}`);
  } else {
    check("菜单可打开 / 通道持久化", false, "下拉不可用");
  }

  // Second task is selected → header label reflects its provider (codex → 备用通道).
  await page.evaluate(() => {
    window.state.confirmTaskId = "t2";
    window.ccoSplit.render();
  });
  await page.waitForTimeout(200);
  const headerLabel2 = await headerTrigger
    .first()
    .locator(".cco-select__label")
    .textContent()
    .catch(() => null);
  check("切换步骤后下拉跟随该步通道", headerLabel2 === "备用通道", `label=${headerLabel2}`);

  // ---- 每张任务卡上的「默认通道」已是可点下拉（P2-17：用户点不动的正是它）----
  const cardCount = await page.locator(".split-provider-select").count();
  check("每张任务卡含通道下拉", cardCount === 2, `count=${cardCount}`);
  if (cardCount === 2) {
    const cardWrap = page.locator(
      ".split-provider-wrap[data-provider-for='t1']"
    );
    await cardWrap.locator(".cco-select__trigger").click();
    await page.waitForTimeout(100);
    const cardMenuOpen = await cardWrap.locator(".cco-select__menu").isVisible();
    check("卡片通道菜单可打开", cardMenuOpen);
    const cardOpts = await cardWrap.locator(".cco-select__option").count();
    check("卡片菜单含 8 个通道选项", cardOpts === 8, `options=${cardOpts}`);
    // 选回 claude（t1 现在是 codex）→ 应持久化并更新卡片当前值。
    await cardWrap.locator(".cco-select__option[data-value='claude']").click();
    await page.waitForTimeout(150);
    const cardProvider = await page.evaluate(() => {
      const j = window.state.planJob;
      return (j.tasks || []).find((x) => x.id === "t1")?.provider;
    });
    const cardVal = await page.evaluate(() => {
      const sel = document.querySelector(
        ".split-provider-wrap[data-provider-for='t1'] select"
      );
      return sel?.value;
    });
    check("卡片改通道后 t1 持久化", cardProvider === "claude", `provider=${cardProvider}`);
    check("卡片下拉当前值同步", cardVal === "claude", `value=${cardVal}`);
  }

  if (errors.length) {
    check("无页面错误", false, errors.join(" | ").slice(0, 300));
  } else {
    check("无页面错误", true);
  }
} catch (e) {
  check("脚本执行", false, String(e));
} finally {
  await browser.close();
}

const failed = results.filter((r) => !r.ok);
console.log(`\n== ${failed.length ? `${failed.length} FAILED` : "ALL PASS"} ==`);
process.exit(failed.length ? 1 : 0);
