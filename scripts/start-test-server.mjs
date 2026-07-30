#!/usr/bin/env node
/**
 * Test Server Starter for W1-6 Automation
 *
 * Auto-detects available HTTP server tool (python3 or npx http-server)
 * Starts static file server on port 3456 serving web/ directory
 * Manages lifecycle with proper cleanup on exit
 */

import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { existsSync } from 'node:fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');
const webDir = join(projectRoot, 'web');

// Configuration
const PORT = 3456;

// Verify web directory exists
if (!existsSync(webDir)) {
  console.error(`[TEST-SERVER] ❌ Web directory not found: ${webDir}`);
  process.exit(1);
}

// Detect preferred HTTP server tool
async function detectServer() {
  // Check for python3
  if (await checkCommand('python3')) {
    return { cmd: 'python3', args: ['-m', 'http.server', String(PORT)] };
  }

  // Check for npx http-server
  if (await checkCommand('npx')) {
    return { cmd: 'npx', args: ['-y', 'http-server', webDir, '-p', String(PORT)] };
  }

  return null;
}

// Check if command is available
function checkCommand(cmd) {
  return new Promise((resolve) => {
    const proc = spawn('which', [cmd], { stdio: 'pipe' });
    proc.on('close', (code) => resolve(code === 0));
  });
}

// Start HTTP server
async function startServer() {
  const serverConfig = await detectServer();

  if (!serverConfig) {
    console.error('[TEST-SERVER] ❌ No HTTP server available');
    console.error('[TEST-SERVER]   Please install one of:');
    console.error('     • Python 3: brew install python');
    console.error('     • Or use global npm: npm install -g http-server');
    process.exit(1);
  }

  console.log(`[TEST-SERVER] 🚀 Starting HTTP server: ${serverConfig.cmd} ${serverConfig.args.join(' ')}`);

  try {
    const proc = spawn(serverConfig.cmd, serverConfig.args, {
      cwd: webDir,
      detached: true,
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env, PORT: String(PORT) }
    });

    proc.unref(); // Don't block parent process

    // Wait for server to be ready
    await waitForServer(PORT);

    const url = `http://localhost:${PORT}/index.html`;
    console.log(`[TEST-SERVER] ✅ Running at ${url}`);
    console.log(`[TEST-SERVER] 📦 PID: ${proc.pid}`);
    console.log(`[TEST-SERVER] 🛡️  Directory: ${webDir}`);

    // Register cleanup on exit
    process.on('exit', () => {
      cleanup(proc.pid);
    });

    process.on('SIGTERM', () => {
      cleanup(proc.pid);
      process.exit(0);
    });

    process.on('SIGINT', () => {
      cleanup(proc.pid);
      process.exit(0);
    });

  } catch (err) {
    console.error(`[TEST-SERVER] ❌ Failed to start ${serverConfig.cmd}:`, err.message);
    process.exit(1);
  }
}

// Wait for server to respond
function waitForServer(port, maxAttempts = 30) {
  return new Promise((resolve, reject) => {
    let attempts = 0;

    const timer = setInterval(async () => {
      attempts++;

      try {
        const response = await fetch(`http://localhost:${port}/index.html`);
        if (response.ok) {
          clearInterval(timer);
          resolve();
          return;
        }
      } catch (err) {
        // Continue polling
      }

      if (attempts >= maxAttempts) {
        clearInterval(timer);
        reject(new Error(`Server failed to start after ${maxAttempts} attempts`));
      }
    }, 500);
  });
}

// Cleanup helper
function cleanup(pid) {
  if (pid) {
    try {
      process.kill(-pid, 'SIGTERM'); // Kill process group
    } catch (err) {
      // Process may have already exited
    }
  }
}

// Execute startup
startServer().catch(err => {
  console.error('[TEST-SERVER] Fatal:', err.stack);
  process.exit(1);
});
