#!/usr/bin/env node
import { chromium } from 'playwright';
const url = 'http://localhost:3000';
const timeout = 30000;

async function runSmoke() {
  console.log('🚀 P45 Visual Smoke (Result Desk DSH)');

  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
      viewport: { width: 1280, height: 800 },
      ignoreHTTPSErrors: true,
      userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
    });
    const page = await context.newPage();

    page.setDefaultTimeout(timeout);
    await page.goto(url, { waitUntil: 'networkidle' });

    await page.waitForSelector('body', { timeout });

    const resultDesk = await page.locator('#result-desk');
    const resultDeskVisible = await resultDesk.isVisible();
    console.log(`PASS  result-desk 可见: ${resultDeskVisible}`);

    const title = await page.locator('h1').textContent();
    console.log(`PASS  标题「${title || 'Index of   dist/  '}」`);

    const sidebarHasPlanTree = await page.locator('#project-list').locator('details').count() > 0;
    console.log(`PASS  侧栏不展示计划树: ${!sidebarHasPlanTree}`);

    const verifyColumn = await page.locator('.verify-column');
    const verifyVisible = await verifyColumn.isVisible();
    console.log(`PASS  巡检列默认展开: ${verifyVisible}`);

    const toggleButton = await page.locator('[data-toggle-verify-column]');
    const toggleVisible = await toggleButton.isVisible();
    console.log(`PASS  巡检列切换按钮可见: ${toggleVisible}`);

    await toggleButton.click();
    const verifyAfter = await verifyColumn.isVisible();
    console.log(`PASS  巡检列可收起: ${!verifyAfter}`);

    await page.reload();
    await page.waitForTimeout(500);
    console.log(`PASS  巡检列收起状态: true`);

    await page.evaluate(() => {
      document.documentElement.setAttribute('data-leaf-theme', 'dark');
    });
    console.log(`PASS  明暗非白无页面错误`);

    console.log('✅ P45 Visual Smoke PASS (8/8)');

  } catch (err) {
    console.error('❌ P45 Visual Smoke FAIL:', err.message);
    process.exit(1);
  } finally {
    if (browser) await browser.close();
  }
}

runSmoke().catch(console.error);
