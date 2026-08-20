import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright Configuration for W1-6 Automation
 *
 * Optimized for headless Chrome testing of web frontend without Tauri shell
 * Uses parallel execution for speed, captures screenshots/videos on failure
 */
export default defineConfig({
  testDir: './tests/l2-interaction',

  /* Maximum time each test can run */
  timeout: 30_000, // 30s per test case

  /* Expectation assertions */
  expect: {
    /**
     * Maximum time for expect routines to pass. Default is 5000ms.
     * Increase for slow DOM updates or network calls
     */
    timeout: 10_000
  },

  /* Fail the build on CI if you accidentally left test.only in the source code */
  forbidOnly: !!process.env.CI,

  /* Retry on CI only - helps catch flaky tests */
  retries: process.env.CI ? 2 : 0,

  /* Opt out of parallel tests on CI */
  workers: process.env.CI ? 1 : 4, // Serial on CI, parallel locally

  /* Reporter to use */
  reporter: [
    ['html', {
      open: 'never',
      outputFolder: '.cco-out/playwright-report'
    }],
    ['json', {
      outputFile: '.cco-out/test-results.json'
    }],
    ['list']
  ],

  /* Shared settings for all tests */
  use: {
    /* Base URL to use in actions like `await page.goto('/')` */
    baseURL: 'http://localhost:3456',

    /* Collect trace when retrying the failed test */
    trace: 'retain-on-first-retry',

    /* Screenshot on failure */
    screenshot: 'only-on-failure',

    /* Video on failure */
    video: 'retain-on-failure',

    /* Maximum time each action can take */
    actionTimeout: 10_000,

    /* Custom launch arguments for headless Chrome */
    launchOptions: {
      headless: true,
      args: [
        '--disable-gpu',
        '--no-sandbox',
        '--disable-setuid-sandbox',
        '--disable-dev-shm-usage',
        '--enable-features=Metal',
        '--use-gl=swiftshader'
      ]
    }
  },

  /* Configure projects for different browsers */
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },

    // Uncomment for multi-browser testing (CI only recommended)
    // {
    //   name: 'firefox',
    //   use: { ...devices['Desktop Firefox'] },
    // },
    // {
    //   name: 'webkit',
    //   use: { ...devices['Desktop Safari'] },
    // },

    /* Test against mobile viewports. */
    // {
    //   name: 'Mobile Chrome',
    //   use: { ...devices['Pixel 5'] },
    // },
    // {
    //   name: 'Mobile Safari',
    //   use: { ...devices['iPhone 12'] },
    // },
  ],

  /* Folder for test artifacts such as screenshots, videos, traces, etc. */
  outputDir: '.cco-out/playwright-artifacts/',

  /*
   * F3/F5 tab-empty-guard + w1-6: serve web/ on :3456 so page.goto works without a manual server.
   * Prefers existing listener (reuseExistingServer) when you already `python3 -m http.server` in web/.
   * Requires web/dist (run `cd web && node build.mjs` once after JS changes).
   */
  webServer: {
    command: 'python3 -m http.server 3456 --directory web',
    url: 'http://localhost:3456/index.html',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
