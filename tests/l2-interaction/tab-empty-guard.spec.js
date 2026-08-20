import { test, expect } from "@playwright/test";
import fs from "fs";
import path from "path";

/**
 * F3 tab 空态守卫（view-ring + confirmDialog）
 * 真源：docs/chat-dual-mode-empty-guard-2026-08-20.md §5 / §6 F3 / F5 断言清单
 *
 * 覆盖：
 * - 无计划点拆分 → 确认层文案 + 取消不切页
 * - 有 draft 点拆分 → 不弹
 * - 同因第二次点不弹
 * - CTA「去聊天写计划」→ 到聊天页 (author)
 */

const mockTauriContent = fs.readFileSync(
  path.join(__dirname, "../..", "web", "mock-tauri-ipc.js"),
  "utf-8"
);

async function pickFirstProject(page) {
  const firstProject = page.locator(".project-item, [data-path]").first();
  if ((await firstProject.count()) === 0) return false;
  await firstProject.click();
  await page.waitForTimeout(400);
  return true;
}

/** Force split empty: no draft / job / plans; clear once-per-reason session flags. */
async function forceSplitEmpty(page) {
  await page.evaluate(() => {
    const s = window.state;
    if (!s) return;
    s.planJobId = null;
    s.planJob = null;
    s.plans = [];
    if (s.chatSession) s.chatSession.draft_plan = null;
    try {
      const keys = [];
      for (let i = 0; i < sessionStorage.length; i++) {
        const k = sessionStorage.key(i);
        if (k && k.startsWith("cco-tab-empty:")) keys.push(k);
      }
      keys.forEach((k) => sessionStorage.removeItem(k));
    } catch (_) {}
  });
}

test.describe("F3 · tab 空态守卫", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript((content) => {
      // eslint-disable-next-line no-eval
      eval(content);
    }, mockTauriContent);

    await page.goto("/index.html");
    await page.waitForLoadState("networkidle");
  });

  test("无计划点拆分 → 确认层文案 + 取消不切页", async ({ page }) => {
    if (!(await pickFirstProject(page))) {
      test.skip(true, "无项目可点，跳过");
      return;
    }
    await forceSplitEmpty(page);

    const phaseBefore = await page.evaluate(
      () => document.body.dataset.ccoAppPhase || ""
    );

    await page.locator('#view-ring .view-ring-item[data-ring="split"]').click();
    const confirm = page.locator(".cco-confirm:not([hidden])");
    await expect(confirm).toBeVisible({ timeout: 3000 });
    await expect(confirm.locator(".cco-confirm-body")).toContainText(
      /还没有可拆分的计划|先和小叶聊/
    );
    await expect(confirm.locator("[data-confirm-ok]")).toContainText(
      /去聊天写计划|去聊天/
    );

    await confirm.locator("button[data-confirm-cancel]").click();
    await expect(confirm).toBeHidden({ timeout: 2000 });

    const phaseAfter = await page.evaluate(
      () => document.body.dataset.ccoAppPhase || ""
    );
    // 取消应留在原 phase（不因空态误切 split）
    expect(phaseAfter).toBe(phaseBefore);
  });

  test("有 draft 点拆分 → 不弹确认层", async ({ page }) => {
    if (!(await pickFirstProject(page))) {
      test.skip(true, "无项目可点，跳过");
      return;
    }

    await page.evaluate(() => {
      const s = window.state;
      if (!s) return;
      if (!s.chatSession) s.chatSession = { session_id: "default", messages: [] };
      s.chatSession.draft_plan = {
        markdown: "# 测试计划\n",
        path: "",
        saved: false,
      };
      try {
        const keys = [];
        for (let i = 0; i < sessionStorage.length; i++) {
          const k = sessionStorage.key(i);
          if (k && k.startsWith("cco-tab-empty:")) keys.push(k);
        }
        keys.forEach((k) => sessionStorage.removeItem(k));
      } catch (_) {}
    });

    await page.locator('#view-ring .view-ring-item[data-ring="split"]').click();
    await page.waitForTimeout(500);
    const visible = await page.locator(".cco-confirm:not([hidden])").count();
    expect(visible).toBe(0);
  });

  test("同因第二次点不弹", async ({ page }) => {
    if (!(await pickFirstProject(page))) {
      test.skip(true, "无项目可点，跳过");
      return;
    }
    await forceSplitEmpty(page);

    const splitBtn = page.locator(
      '#view-ring .view-ring-item[data-ring="split"]'
    );
    await splitBtn.click();
    const confirm = page.locator(".cco-confirm:not([hidden])");
    await expect(confirm).toBeVisible({ timeout: 3000 });
    await confirm.locator("button[data-confirm-cancel]").click();
    await expect(confirm).toBeHidden({ timeout: 2000 });

    await splitBtn.click();
    await page.waitForTimeout(400);
    const again = await page.locator(".cco-confirm:not([hidden])").count();
    expect(again).toBe(0);
  });

  test("CTA 去聊天写计划 → 到聊天页", async ({ page }) => {
    if (!(await pickFirstProject(page))) {
      test.skip(true, "无项目可点，跳过");
      return;
    }
    await forceSplitEmpty(page);

    // Leave chat first so CTA navigation is observable (go to welcome-ish or stay and still assert phase=author)
    // Prefer: go somewhere else if possible; otherwise force body phase away from author after empty clear.
    await page.evaluate(() => {
      // If already on author, still OK — CTA must land/stay on author after OK.
      document.body.dataset.ccoAppPhase = "run";
    });

    await page.locator('#view-ring .view-ring-item[data-ring="split"]').click();
    const confirm = page.locator(".cco-confirm:not([hidden])");
    await expect(confirm).toBeVisible({ timeout: 3000 });
    await expect(confirm.locator("[data-confirm-ok]")).toContainText(
      /去聊天写计划|去聊天/
    );

    await confirm.locator("[data-confirm-ok]").click();
    await expect(confirm).toBeHidden({ timeout: 2000 });

    await page.waitForTimeout(400);
    const phase = await page.evaluate(
      () => document.body.dataset.ccoAppPhase || ""
    );
    expect(phase).toBe("author");

    // Chat page visible when project selected
    const chatPage = page.locator("#page-chat");
    if ((await chatPage.count()) > 0) {
      await expect(chatPage).toBeVisible({ timeout: 3000 });
    }
  });
});
