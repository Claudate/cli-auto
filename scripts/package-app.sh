#!/usr/bin/env bash
# Build release binary and assemble dist/Leaf.app (macOS)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Fail fast: a single SyntaxError in features/* kills type=module main.js
# (shell stuck on「就绪中…」, no icons / no project list).
echo "[0/3] syntax-check web/js (ESM dynamic import)"
if command -v node >/dev/null 2>&1; then
  while IFS= read -r -d '' f; do
    NODE_OPTIONS="${NODE_OPTIONS:+$NODE_OPTIONS }--no-warnings" \
      JS_FILE="$f" node --input-type=module <<'NODE' || {
const file = process.env.JS_FILE;
const url = new URL(`./${file}`, `file://${process.cwd()}/`);
try {
  await import(url.href);
} catch (e) {
  if (e?.name === "SyntaxError") {
    console.error(e.stack || e.message || String(e));
    process.exit(99);
  }
  // Browser-only modules may throw ReferenceError under Node; syntax already parsed.
}
NODE
      echo "SYNTAX_FAIL: $f" >&2
      exit 1
    }
  done < <(find web/js -type f -name '*.js' ! -name '*.bak' -print0)
  echo "SYNTAX_OK: web/js"
else
  echo "SYNTAX_WARN: node not found; skip web/js syntax gate" >&2
fi

echo "[1/3] build cco + cco-desktop (embeds web/ via tauri frontendDist)"
cargo build -p cco --release
# touch web so tauri rebuilds asset embed (D4: include js/ css/ tree)
touch web/index.html web/app.js web/app.css
find web/js web/css -type f \( -name '*.js' -o -name '*.css' \) -exec touch {} +

# 防逆向：web 前端打包+混淆（esbuild bundle main.js → dist/app.js + dist/classic/*.js）
echo "[1.5/3] build web bundle + terser mangle (web/build.mjs)"
if [[ -d "$ROOT/web/node_modules" ]]; then
  (cd "$ROOT/web" && node build.mjs)
else
  (cd "$ROOT/web" && npm install >/dev/null 2>&1 && node build.mjs)
fi
if [[ ! -f "$ROOT/web/dist/app.js" ]]; then
  echo "WEB_BUILD_FAIL: dist/app.js missing after build.mjs" >&2
  exit 1
fi

cargo build -p cco-desktop --release

DIST="$ROOT/dist"
APP="$DIST/Leaf.app"
BIN="$ROOT/target/release/cco-desktop"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Leaf"
chmod +x "$APP/Contents/MacOS/Leaf"
cp -f "$ROOT/target/release/cco" "$DIST/cco" 2>/dev/null || true

# Place web next to binary (Contents/MacOS/web) AND Contents/web for compatibility
rm -rf "$APP/Contents/MacOS/web" "$APP/Contents/web"
cp -R "$ROOT/web" "$APP/Contents/MacOS/web"
cp -R "$ROOT/web" "$APP/Contents/web"

# 防逆向：移除打包产物中的明文源码与构建依赖，只保留 dist/ 混淆产物
for W in "$APP/Contents/MacOS/web" "$APP/Contents/web"; do
  rm -rf "$W/node_modules" "$W/js" "$W/build.mjs" "$W/package.json" "$W/package-lock.json" "$W/mock-tauri-ipc.js"
  rm -f "$W/dist/app.bundle.js"
done

# 防逆向闸门：app 内不得残留明文源码 / 未混淆中间产物 / 构建依赖
LEAK=""
for W in "$APP/Contents/MacOS/web" "$APP/Contents/web"; do
  for P in js node_modules build.mjs package.json dist/app.bundle.js; do
    [[ -e "$W/$P" ]] && LEAK="$LEAK $W/$P"
  done
done
if [[ -n "$LEAK" ]]; then
  echo "PURGE_FAIL: 打包产物残留明文源码/构建物:$LEAK" >&2
  exit 1
fi
echo "PURGE_OK: app 内仅 dist/ 混淆产物"

# === App icon: CFBundleIconFile=AppIcon → Resources/AppIcon.icns ===
if [[ ! -f "$ROOT/src-tauri/icons/icon.icns" ]]; then
  echo "ICON_FAIL: missing src-tauri/icons/icon.icns" >&2
  exit 1
fi
cp -f "$ROOT/src-tauri/icons/icon.icns" "$APP/Contents/Resources/AppIcon.icns"
cp -f "$ROOT/src-tauri/icons/icon.png" "$APP/Contents/Resources/icon.png"
# 刷新 Finder/Dock 图标缓存线索（用户仍可能需重登或 `killall Dock`）
touch "$APP" "$APP/Contents/Info.plist" "$APP/Contents/Resources/AppIcon.icns"
ls -la "$APP/Contents/Resources/AppIcon.icns" "$APP/Contents/MacOS/web/favicon-32x32.png"

# runtime-prompts: disk path for MacOS/../Resources (also include_str-embedded in binary)
rm -rf "$APP/Contents/Resources/runtime-prompts"
if [[ -d "$ROOT/docs/runtime-prompts" ]]; then
  cp -R "$ROOT/docs/runtime-prompts" "$APP/Contents/Resources/runtime-prompts"
fi

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Leaf</string>
  <key>CFBundleIdentifier</key><string>dev.leaf.console</string>
  <key>CFBundleName</key><string>Leaf</string>
  <key>CFBundleDisplayName</key><string>轻叶</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cd "$DIST"
rm -f Leaf-macos-arm64.zip
zip -r Leaf-macos-arm64.zip Leaf.app >/dev/null
echo "OK: $APP"
echo "WEB_MACOS: $APP/Contents/MacOS/web"
echo "WEB_CONTENTS: $APP/Contents/web"
echo "RUNTIME_PROMPTS: $APP/Contents/Resources/runtime-prompts"
echo "ZIP: $DIST/Leaf-macos-arm64.zip"
if [[ -f "$APP/Contents/Resources/runtime-prompts/chat-plan-writing.md" ]]; then
  echo "RUNTIME_PROMPTS_OK: chat-plan-writing.md present"
else
  echo "RUNTIME_PROMPTS_WARN: missing Resources/runtime-prompts (binary still has include_str fallback)"
fi
# sanity: 新 UI 标记必须存在于打包产物（js/ 源已被净化，标记在 dist/ 混淆产物里）
SEARCH_BIN="$(command -v rg || true)"
if [[ -z "$SEARCH_BIN" && -x /opt/homebrew/bin/rg ]]; then SEARCH_BIN=/opt/homebrew/bin/rg; fi
if [[ -z "$SEARCH_BIN" && -x "$HOME/.cargo/bin/rg" ]]; then SEARCH_BIN=$HOME/.cargo/bin/rg; fi
WEB_ROOT="$APP/Contents/MacOS/web"
SANITY_RE="btn-chooser-assign|btn-task-dash-toggle|cli-rerun-btn|分配计划|plan-chooser-foot|taskDashCollapsed|budget-chip|updateBudgetChip|btn-open-chat|page-chat|btn-chat-assign|assignFromChat|btn-plan-full-diff|chat_stream_partial|mountVirtualLog|plan-full-diff|log-virt|data-plan-template|btn-split-writeback|templates|拆成步骤|确认并开始|result-desk|help-rs-checklist"
if [[ -n "$SEARCH_BIN" ]]; then
  HITS=$("$SEARCH_BIN" -l "$SANITY_RE" "$WEB_ROOT/index.html" "$WEB_ROOT/app.css" \
    "$WEB_ROOT/dist" "$WEB_ROOT/css" 2>/dev/null | head -80 || true)
else
  HITS=$(grep -rlE "$SANITY_RE" "$WEB_ROOT/index.html" "$WEB_ROOT/app.css" \
    "$WEB_ROOT/dist" "$WEB_ROOT/css" 2>/dev/null | head -80 || true)
fi
if [[ -z "$HITS" ]]; then
  echo "SANITY_FAIL: 打包产物中未找到任何 UI 标记（$WEB_ROOT/index.html + dist/ + css/ 全空）" >&2
  exit 1
fi
echo "SANITY_OK: 打包产物含 UI 标记"
echo "$HITS"
echo ""
echo "── X3 目视清单（打包后人工走一遍）──"
echo "  1. 欢迎：添加项目 · 见模板「出海落地页/通用需求大纲」"
echo "  2. 写：模板落盘或聊天写计划 → 可改 →「拆成步骤」"
echo "  3. 拆：拆分台三栏 · 可选「写回步骤摘要」· 不旁路 confirm"
echo "  4. 确认：「确认并开始」→ fake 跑"
echo "  5. 结果：结果台收口（完成/遗漏/回聊天）"
echo "  6. 帮助：#help-rs-checklist R-S10…17 可指认 · 无第三方 agent 运行时"
