---
context: "轻叶项目防逆向保护方案 —— 前端打包+混淆 + Cargo release 硬化"
date: "2026-08-13"
status: "实施中"
---

# 轻叶项目防逆向保护方案（防逆向为主）

## 背景
当前产物保护基线几乎为零：
- 二进制 56MB arm64，**未 strip 符号**（nm 可见内部符号），无 `[profile.release]`（无 LTO/strip/panic=abort）
- 前端 ~120 个 ESM 模块明文打包进 `CCO.app/Contents/{MacOS/,}web/js/`，无打包器无混淆
- 无 codesign / notarization / entitlements，无任何授权机制

用户目标：**防逆向为主（混淆+签名）**，无 Apple Developer 账号（先不管签名），前端可接受 esbuild 打包 + terser 混淆。

## 实施步骤

### Step 1: Cargo release 硬化 ✅
- 根 `Cargo.toml` 加 `[profile.release]`：lto=fat / codegen-units=1 / strip=true / debug=false / overflow-checks=false
- **不用 `panic="abort"`**（2026-08-13 决策）：本 crate 有 1100+ `.unwrap()` 与 7 处 `tokio::task::spawn_blocking`，abort 会把单条 Tauri 命令的 panic 放大成整个 app 无提示闪退，且会吞掉 IPC 错误返回；unwind 对逆向难度影响可忽略（仅多 unwind tables）。防逆向主力 = strip + LTO fat + codegen-units=1。
- `src-tauri/Cargo.toml` **不**写 member profile：workspace 只认根 `Cargo.toml` 的 `[profile.release]`，member 里写是死配置（2026-08-13 已删并注释根配置）。

### Step 2: 前端打包+混淆（esbuild + terser）✅
- 新建 `web/package.json`（devDeps: esbuild + terser）
- 新建 `web/build.mjs`：esbuild bundle main.js → `web/dist/app.js`，再 terser 混淆
- `web/index.html`：ESM 入口从 `js/main.js` 改为 `dist/app.js`；classic 脚本引用 `dist/classic/*.js`
- 经典脚本（state/flow/templates/plan/monitor/result/log/chat/doctor）**只 compress 不 mangle**（facade 顶层函数 = window 全局契约，见事故记录）；主 bundle `dist/app.js` 可 toplevel mangle（ESM 闭包内安全）
- **dist/ 产物 gitignored**：干净 clone / CI 必须先 `cd web && node build.mjs`，否则 index.html 全 404

### Step 3: 打包脚本集成 ✅
- `scripts/package-app.sh`：Tauri 构建前先 `cd web && node build.mjs`，产物 dist/ 随 app 打包
- 清理源 `web/js/`、`build.mjs`、`package.json`、`node_modules`、未混淆中间产物 `dist/app.bundle.js` 不进 app（只进 dist/）
- **防逆向硬闸**（2026-08-13 加）：purge 后扫描 `Contents/{MacOS/,}web/`，残留明文源码/构建物任一 → `PURGE_FAIL` + exit 1
- **UI 标记硬闸**：打包产物 `index.html + app.css + dist/ + css/` 需命中 UI 标记正则（`拆成步骤`/`确认并开始`/`result-desk` 等）→ 否则 `SANITY_FAIL` + exit 1（原检查扫已删除的 `web/js` 静默空转，已改扫 dist/）

### Step 4: 验证 ✅
- `cargo build --release -p cco-desktop` → nm 验证符号已 strip
- `npm run build` → dist/app.js 混淆不可读 + `FACADE_OK` 守卫（见事故记录）
- `./scripts/package-app.sh` → 全量构建 + PURGE_OK/SANITY_OK 双硬闸；strings 无 `/Users/` 路径；`Contents/{MacOS/,}web/` 内仅 dist/ 混淆产物

## 事故记录：classic facade 顶层混淆 = 聊天框消失（2026-08-13）

**症状**：打开项目聊天框不渲染、输入后点发送无反应。
**根因**：`build.mjs` 第 3 步对经典 facade（`state/flow/.../chat.js`）直接套用了主 bundle 的 terser 选项（`mangle.toplevel: true` + `compress.passes: 2`）。classic 脚本以 `<script>` 全局加载，顶层 `function sendChatMessage(){...}` 即 window 全局契约——`uiActions.js` 事件表（`btn-chat-send → call("sendChatMessage")`）与 `index.html` 按名调用。混淆后函数改名 + DCE 删除「未引用」函数，facade 全灭。
**修复**：classic 文件用独立选项 `classicTerserOpts`——只 `compress`，`mangle: false`。主 bundle `dist/app.js` 不受影响（ESM 闭包内 toplevel 混淆安全，`window.cco*` 显式挂载点不受 mangle 影响）。
**回归检查**：`node build.mjs` 第 4 步内置 **facade 守卫**——解析 9 个 classic 源文件的顶层 `function <name>` 声明，断言每个名字都存在于 `dist/classic/*.js`；缺任一 → `FACADE_FAIL` + exit 1（当前基线 253/253 全保留）。脚本级兜底：`package-app.sh` 的 SANITY 正则含 `result-desk`/`拆成步骤` 等 dist/ 内标记。
**后续候选**（未做）：把 facade 全局函数显式挂 `window.sendChatMessage = ...`，或事件表改走 `window.ccoChat.*`，之后才能对 classic 也开 toplevel 混淆。
