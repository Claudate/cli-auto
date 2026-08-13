#!/usr/bin/env node
/**
 * [INPUT]: web/js/main.js（ESM 入口）+ 经典 facade 脚本（state/flow/...）
 * [OUTPUT]: web/dist/app.js（bundle + terser 混淆）+ web/dist/classic/*.js（facade 压缩混淆）
 * [POS]: 防逆向构建器 —— esbuild 打包主模块图为单文件，再 terser 变量名混淆；经典脚本单独混淆
 * [PROTOCOL]: 变更时更新此头部；产物不进 git（dist/ 见 .gitignore）
 *
 * 用法:
 *   node build.mjs            # 生产：bundle + 混淆
 *   node build.mjs --dev      # 开发：仅 bundle（minify，不混淆），便于调试
 */
import { build } from "esbuild";
import { minify } from "terser";
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const WEB = __dirname;
const SRC = join(WEB, "js");
const OUT = join(WEB, "dist");
const isDev = process.argv.includes("--dev");

mkdirSync(join(OUT, "classic"), { recursive: true });

// --- 1) 主模块图：esbuild bundle main.js → dist/app.bundle.js ---
const mainEntry = join(SRC, "main.js");
const mainBundleOut = join(OUT, "app.bundle.js");

console.log("[1/3] esbuild bundle main.js → app.bundle.js");
await build({
  entryPoints: [mainEntry],
  bundle: true,
  format: "esm",
  platform: "browser",
  target: "es2020",
  outfile: mainBundleOut,
  minify: true,
  minifyIdentifiers: false, // 交由 terser 做更激进的标识符混淆
  minifySyntax: true,
  minifyWhitespace: true,
  sourcemap: false,
  define: { "process.env.NODE_ENV": '"production"' },
  logLevel: "warning",
});

// --- 2) terser 混淆主 bundle ---
console.log("[2/3] terser 混淆 app.bundle.js → app.js");
const mainCode = readFileSync(mainBundleOut, "utf8");
const terserOpts = isDev
  ? {}
  : {
      compress: {
        passes: 2,
        drop_console: false,
        drop_debugger: true,
        pure_funcs: [],
      },
      mangle: {
        toplevel: true,
        properties: false, // 不混淆属性，避免破坏 DOM/IPC 字符串
      },
      format: { comments: false },
      nameCache: null,
    };
// 经典 facade 经 <script> 全局加载：顶层 function 名即 window 全局契约
// （uiActions 事件表 / index.html onclick 按名调用）。toplevel 混淆会破坏该契约，
// 导致聊天框不渲染、发送无反应——故 classic 文件只压缩，不做标识符混淆。
const classicTerserOpts = isDev
  ? {}
  : {
      compress: terserOpts.compress,
      mangle: false,
      format: terserOpts.format,
    };
const mainResult = await minify(mainCode, terserOpts);
writeFileSync(join(OUT, "app.js"), mainResult.code ?? mainCode, "utf8");

// --- 3) 经典 facade 脚本单独压缩混淆（state/flow/templates/plan/monitor/result/log/chat/doctor）---
console.log("[3/3] terser 压缩经典 facade 脚本 → dist/classic/");
const CLASSIC_FILES = [
  "state.js", "flow.js", "templates.js", "plan.js", "monitor.js",
  "result.js", "log.js", "chat.js", "doctor.js",
];
for (const f of CLASSIC_FILES) {
  const srcPath = join(SRC, f);
  try {
    const code = readFileSync(srcPath, "utf8");
    const res = await minify(code, classicTerserOpts);
    writeFileSync(join(OUT, "classic", f), res.code ?? code, "utf8");
  } catch {
    // 文件可能不存在，跳过
  }
}

const outSize = statSync(join(OUT, "app.js")).size;
console.log(`OK: dist/app.js (${(outSize / 1024).toFixed(1)} KB${isDev ? " · dev mode (no mangle)" : " · mangled"})`);
console.log(`OK: dist/classic/*.js (${CLASSIC_FILES.length} facades)`);

// --- 4) 类目冒烟回滚：经典 facade 的顶层 function 名 = window 全局契约，防混淆回归 ---
// 每个 classic 文件必须保留源文件里的每个顶层 function 名（toplevel 混淆曾把它们
// 改名+DCE 删掉，导致聊天框消失）。只查 source 里「顶层 function」声明的名字。
const FACADE_FN_RE = /^function\s+([A-Za-z_$][\w$]*)\s*\(/gm;
let facadeFns = 0;
let facadeBad = 0;
for (const f of CLASSIC_FILES) {
  const srcPath = join(SRC, f);
  let code;
  try {
    code = readFileSync(srcPath, "utf8");
  } catch {
    continue;
  }
  const names = [];
  for (const m of code.matchAll(FACADE_FN_RE)) names.push(m[1]);
  if (!names.length) continue;
  let outCode;
  try {
    outCode = readFileSync(join(OUT, "classic", f), "utf8");
  } catch {
    facadeBad += names.length;
    console.error(`FACADE_FAIL: dist/classic/${f} 缺失（顶层函数 ${names.length} 个待验）`);
    continue;
  }
  for (const n of names) {
    facadeFns++;
    if (!outCode.includes(n)) {
      facadeBad++;
      console.error(`FACADE_FAIL: ${f} 丢失全局函数 ${n}（toplevel 混淆回归？）`);
    }
  }
}
if (facadeBad > 0) {
  console.error(`FACADE_FAIL: ${facadeBad}/${facadeFns} 个顶层函数名丢失或文件缺失 — 构建产物会破坏 window 全局契约`);
  process.exit(1);
}
console.log(`FACADE_OK: classic facade 顶层函数名保留 ${facadeFns - facadeBad}/${facadeFns}`);
