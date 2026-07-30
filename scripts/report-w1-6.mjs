#!/usr/bin/env node
/**
 * W1-6 Report Generator
 *
 * Reads Playwright test-results.json and generates:
 * - Markdown summary with pass/fail counts and error details
 * - HTML gallery with screenshots
 * - Output to .cco-out/w1-6-report/[date]/
 */

import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';
import os from 'os';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, '..');
const resultPath = path.join(projectRoot, '.cco-out', 'test-results.json');
const outputPath = path.join(projectRoot, '.cco-out', 'w1-6-report');

// Color codes for terminal
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  gray: '\x1b[90m'
};

async function generateReport() {
  console.log(`\n${colors.blue}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${colors.reset}`);
  console.log(`${colors.blue}📊 Generating W1-6 Verification Report${colors.reset}`);
  console.log(`${colors.blue}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${colors.reset}\n`);

  // Check if results file exists
  try {
    await fs.access(resultPath);
  } catch (err) {
    console.log(`${colors.yellow}⚠️  No test results found at ${resultPath}${colors.reset}`);
    console.log(`${colors.gray}(Run Playwright tests first: npx playwright test)${colors.reset}\n`);
    return;
  }

  // Read results
  const resultsData = JSON.parse(await fs.readFile(resultPath, 'utf-8'));
  const date = new Date().toISOString().split('T')[0];
  const reportDir = path.join(outputPath, date);

  // Create output directory
  await fs.mkdir(reportDir, { recursive: true });

  // Parse results structure
  const tests = resultsData?.projects?.[0]?.tests || [];
  const passed = tests.filter(t => t.finalStatus === 'passed');
  const failed = tests.filter(t => t.finalStatus === 'failed' || t.finalStatus === 'timedOut');
  const skipped = tests.filter(t => t.finalStatus === 'skipped');

  const total = tests.length;
  const passRate = total > 0 ? ((passed.length / total) * 100).toFixed(1) : 0;

  console.log(`${colors.green}✅ Tests executed: ${total}${colors.reset}`);
  console.log(`${colors.green}   Passed: ${passed.length}${colors.reset}`);
  console.log(`${colors.red}   Failed: ${failed.length}${colors.reset}`);
  console.log(`${colors.yellow}   Skipped: ${skipped.length}${colors.reset}`);
  console.log(`${colors.blue}   Pass rate: ${passRate}%${colors.reset}\n`);

  // Generate Markdown summary
  const mdContent = generateMarkdownSummary({
    date,
    total,
    passed,
    failed,
    skipped,
    passRate,
    tests
  });

  await fs.writeFile(path.join(reportDir, 'summary.md'), mdContent, 'utf-8');
  console.log(`${colors.green}✓ Written: summary.md${colors.reset}`);

  // Generate HTML gallery
  const htmlContent = generateHtmlGallery({
    date,
    total,
    passed,
    failed,
    skipped,
    passRate,
    tests,
    reportDir
  });

  await fs.writeFile(path.join(reportDir, 'index.html'), htmlContent, 'utf-8');
  console.log(`${colors.green}✓ Written: index.html${colors.reset}`);

  // Copy any attached screenshots
  await copyArtifacts(reportDir);

  // Print final summary
  printFinalSummary({ total, passed, failed, skipped, passRate, reportDir });
}

function generateMarkdownSummary(data) {
  const { date, total, passed, failed, skipped, passRate, tests } = data;

  let content = `# W1-6 自动化驗報告 (${date})\n\n`;

  // Overview section
  content += `## 📊 總覽\n\n`;
  content += `| 指標 | 數量 |\n`;
  content += `|------|------|\n`;
  content += `| 測試總數 | ${total} |\n`;
  content += `| ✅ 通過 | ${passed.length} |\n`;
  content += `| ❌ 失敗 | ${failed.length} |\n`;
  content += `| ⏭️ 跳過 | ${skipped.length} |\n`;
  content += `| 📈 通过率 | ${passRate}% |\n\n`;

  // Passed tests
  if (passed.length > 0) {
    content += `## ✅ 通過的測試 (${passed.length})\n\n`;
    for (const test of passed.slice(0, 10)) { // Limit to first 10
      content += `- **${test.title}**\n`;
    }
    if (passed.length > 10) {
      content += `\n*...和 ${passed.length - 10} 個其他通過測試*\n`;
    }
    content += '\n';
  }

  // Failed tests
  if (failed.length > 0) {
    content += `## ❌ 失敗項詳情\n\n`;

    for (const test of failed) {
      content += `### ${test.title}\n\n`;

      if (test.errors && test.errors.length > 0) {
        const error = test.errors[0];
        content += `**錯誤**:\n\`\`\`\n${truncate(error.message, 200)}\n\`\`\`\n\n`;
      }

      if (test.attachments && test.attachments.length > 0) {
        const screenshot = test.attachments.find(a => a.name === 'screenshot');
        if (screenshot) {
          content += `![Screenshot](../artifacts/${screenshot.name})\n\n`;
        }
      }

      content += `---\n\n`;
    }
  }

  // Append full test list for reference
  content += `## 📋 完整測試列表\n\n`;
  content += `\`\`\`\n`;

  for (const test of tests) {
    const status = test.finalStatus === 'passed' ? '✅' :
                   test.finalStatus === 'failed' ? '❌' :
                   test.finalStatus === 'skipped' ? '⏭️' : '⏳';
    const duration = Math.round(test.duration / 1000);
    content += `${status} ${test.title} (${duration}s)\n`;
  }

  content += `\`\`\`\n`;

  return content;
}

function generateHtmlGallery(data) {
  const { date, total, passed, failed, skipped, passRate, tests, reportDir } = data;

  const timestamp = new Date().toLocaleString('zh-TW');

  return `<!DOCTYPE html>
<html lang="zh-TW">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>W1-6 自動化驗報告 - ${date}</title>
  <style>
    :root {
      --green: #22c55e;
      --red: #ef4444;
      --yellow: #f59e0b;
      --blue: #3b82f6;
      --gray: #6b7280;
      --bg: #f9fafb;
      --card-bg: #ffffff;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: #111827;
      line-height: 1.6;
      padding: 2rem;
    }

    .container {
      max-width: 1200px;
      margin: 0 auto;
    }

    header {
      text-align: center;
      margin-bottom: 2rem;
    }

    h1 {
      font-size: 2rem;
      margin-bottom: 0.5rem;
      color: #111827;
    }

    .timestamp {
      color: var(--gray);
      font-size: 0.875rem;
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 1rem;
      margin-bottom: 2rem;
    }

    .stat-card {
      background: var(--card-bg);
      border-radius: 0.5rem;
      padding: 1.5rem;
      text-align: center;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }

    .stat-number {
      font-size: 2.5rem;
      font-weight: 700;
      margin-bottom: 0.25rem;
    }

    .stat-label {
      color: var(--gray);
      font-size: 0.875rem;
    }

    .stat-pass .stat-number { color: var(--green); }
    .stat-fail .stat-number { color: var(--red); }
    .stat-skip .stat-number { color: var(--yellow); }
    .stat-rate .stat-number { color: var(--blue); }

    .section {
      background: var(--card-bg);
      border-radius: 0.5rem;
      padding: 1.5rem;
      margin-bottom: 1.5rem;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }

    h2 {
      font-size: 1.25rem;
      margin-bottom: 1rem;
      padding-bottom: 0.5rem;
      border-bottom: 2px solid var(--bg);
    }

    .test-list {
      list-style: none;
    }

    .test-item {
      padding: 0.75rem;
      margin-bottom: 0.5rem;
      border-radius: 0.375rem;
      background: var(--bg);
    }

    .test-item.passed { border-left: 4px solid var(--green); }
    .test-item.failed { border-left: 4px solid var(--red); }
    .test-item.skipped { border-left: 4px solid var(--yellow); }

    .test-title {
      font-weight: 500;
      margin-bottom: 0.25rem;
    }

    .test-duration {
      font-size: 0.875rem;
      color: var(--gray);
    }

    .error-details {
      margin-top: 0.5rem;
      padding: 0.75rem;
      background: #fef2f2;
      border-radius: 0.25rem;
      font-size: 0.875rem;
      white-space: pre-wrap;
      word-break: break-word;
    }

    footer {
      text-align: center;
      margin-top: 2rem;
      color: var(--gray);
      font-size: 0.875rem;
    }
  </style>
</head>
<body>
  <div class="container">
    <header>
      <h1>🎯 W1-6 桌面 UI 自化驗報告</h1>
      <p class="timestamp">生成時間：${timestamp}</p>
    </header>

    <div class="stats-grid">
      <div class="stat-card stat-pass">
        <div class="stat-number">${passed.length}</div>
        <div class="stat-label">✅ 通過</div>
      </div>

      <div class="stat-card stat-fail">
        <div class="stat-number">${failed.length}</div>
        <div class="stat-label">❌ 失敗</div>
      </div>

      <div class="stat-card stat-skip">
        <div class="stat-number">${skipped.length}</div>
        <div class="stat-label">⏭️ 跳過</div>
      </div>

      <div class="stat-card stat-rate">
        <div class="stat-number">${passRate}%</div>
        <div class="stat-label">📈 通过率</div>
      </div>
    </div>

    ${failed.length > 0 ? `
    <div class="section">
      <h2>❌ 失敗項詳情</h2>
      <ul class="test-list">
        ${failed.map(test => `
          <li class="test-item failed">
            <div class="test-title">${escapeHtml(test.title)}</div>
            ${test.errors && test.errors.length > 0 ? `
              <div class="error-details">${escapeHtml(truncate(test.errors[0].message, 500))}</div>
            ` : ''}
          </li>
        `).join('')}
      </ul>
    </div>
    ` : ''}

    <div class="section">
      <h2>✅ 通過的測試</h2>
      <ul class="test-list">
        ${passed.slice(0, 20).map(test => `
          <li class="test-item passed">
            <div class="test-title">${escapeHtml(test.title)}</div>
            <div class="test-duration">${Math.round(test.duration / 1000)}s</div>
          </li>
        `).join('')}
        ${passed.length > 20 ? `
          <li class="test-item passed">
            <div class="test-title">... 和 ${passed.length - 20} 個其他通過測試</div>
          </li>
        ` : ''}
      </ul>
    </div>

    <footer>
      <p>Generated by W1-6 Report Generator • Project: cco/claude-auto</p>
    </footer>
  </div>
</body>
</html>`;
}

async function copyArtifacts(reportDir) {
  const artifactsDir = path.join(projectRoot, '.cco-out', 'playwright-artifacts');

  try {
    await fs.access(artifactsDir);

    // Copy all screenshots and videos
    const videoDir = path.join(reportDir, 'artifacts');
    await fs.cp(artifactsDir, videoDir, { recursive: true });

    console.log(`${colors.green}✓ Copied artifacts to: artifacts/${colors.reset}`);
  } catch (err) {
    // Artifacts directory doesn't exist, skip
  }
}

function printFinalSummary(data) {
  const { total, passed, failed, skipped, passRate, reportDir } = data;

  console.log(`\n${colors.blue}╔═══════════════════════════════════════════════════════════╗${colors.reset}`);
  console.log(`${colors.blue}║                                                           ║${colors.reset}`);

  if (failed.length === 0 && total > 0) {
    console.log(`${colors.green}║   🎉 All Tests Completed Successfully!                    ║${colors.reset}`);
  } else {
    console.log(`${colors.yellow}║   ⚠️  Some Tests Failed - Review Details Above             ║${colors.reset}`);
  }

  console.log(`${colors.blue}║                                                           ║${colors.reset}`);
  console.log(`${colors.blue}║   Results saved to:                                       ║${colors.reset}`);
  console.log(`${colors.blue}║   • summary.md                                            ║${colors.reset}`);
  console.log(`${colors.blue}║   • index.html                                            ║${colors.reset}`);
  console.log(`${colors.blue}║   • ${reportDir}${' '.repeat(Math.max(0, 40 - reportDir.length))}║${colors.reset}`);
  console.log(`${colors.blue}║                                                           ║${colors.reset}`);
  console.log(`${colors.blue}╚═══════════════════════════════════════════════════════════╝${colors.reset}\n`);
}

// Helper functions
function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function truncate(str, length) {
  if (!str) return '';
  if (str.length <= length) return str;
  return str.substring(0, length) + '...';
}

// Execute
generateReport().catch(err => {
  console.error(`${colors.red}❌ Report generation failed:${colors.reset}`, err.stack);
  process.exit(1);
});
