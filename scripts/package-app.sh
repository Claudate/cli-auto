#!/usr/bin/env bash
# Build release binary and assemble dist/CCO.app (macOS)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[1/3] build cco + cco-desktop (embeds web/ via tauri frontendDist)"
cargo build -p cco --release
# touch web so tauri rebuilds asset embed (D4: include js/ css/ tree)
touch web/index.html web/app.js web/app.css
find web/js web/css -type f \( -name '*.js' -o -name '*.css' \) -exec touch {} +
cargo build -p cco-desktop --release

DIST="$ROOT/dist"
APP="$DIST/CCO.app"
BIN="$ROOT/target/release/cco-desktop"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/CCO"
chmod +x "$APP/Contents/MacOS/CCO"
cp -f "$ROOT/target/release/cco" "$DIST/cco" 2>/dev/null || true

# Place web next to binary (Contents/MacOS/web) AND Contents/web for compatibility
rm -rf "$APP/Contents/MacOS/web" "$APP/Contents/web"
cp -R "$ROOT/web" "$APP/Contents/MacOS/web"
cp -R "$ROOT/web" "$APP/Contents/web"

cp -f "$ROOT/src-tauri/icons/icon.icns" "$APP/Contents/Resources/AppIcon.icns" 2>/dev/null || true

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>CCO</string>
  <key>CFBundleIdentifier</key><string>dev.cco.orchestrator</string>
  <key>CFBundleName</key><string>CCO</string>
  <key>CFBundleDisplayName</key><string>CCO</string>
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
rm -f CCO-macos-arm64.zip
zip -r CCO-macos-arm64.zip CCO.app >/dev/null
echo "OK: $APP"
echo "WEB_MACOS: $APP/Contents/MacOS/web"
echo "WEB_CONTENTS: $APP/Contents/web"
echo "ZIP: $DIST/CCO-macos-arm64.zip"
# sanity: new UI markers must exist in packaged web
SEARCH_BIN="$(command -v rg || true)"
if [[ -z "$SEARCH_BIN" && -x /opt/homebrew/bin/rg ]]; then SEARCH_BIN=/opt/homebrew/bin/rg; fi
if [[ -z "$SEARCH_BIN" && -x "$HOME/.cargo/bin/rg" ]]; then SEARCH_BIN=$HOME/.cargo/bin/rg; fi
# D4: logic/styles live under web/js/* and web/css/*; app.js/app.css are thin entry
WEB_ROOT="$APP/Contents/MacOS/web"
if [[ -n "$SEARCH_BIN" ]]; then
  "$SEARCH_BIN" -n "btn-chooser-assign|btn-task-dash-toggle|cli-rerun-btn|分配计划|plan-chooser-foot|taskDashCollapsed|budget-chip|updateBudgetChip|btn-open-chat|page-chat|btn-chat-assign|assignFromChat|btn-plan-full-diff|chat_stream_partial|mountVirtualLog|plan-full-diff|log-virt|data-plan-template|btn-split-writeback|templates\.js|拆成步骤|确认并开始|result-desk|help-rs-checklist" \
    "$WEB_ROOT/index.html" "$WEB_ROOT/app.css" "$WEB_ROOT/app.js" \
    "$WEB_ROOT/js" "$WEB_ROOT/css" 2>/dev/null | head -80
else
  grep -rnE "btn-chooser-assign|btn-task-dash-toggle|cli-rerun-btn|分配计划|plan-chooser-foot|taskDashCollapsed|budget-chip|updateBudgetChip|btn-open-chat|page-chat|btn-chat-assign|assignFromChat|btn-plan-full-diff|chat_stream_partial|mountVirtualLog|plan-full-diff|log-virt|data-plan-template|btn-split-writeback|templates\.js|拆成步骤|确认并开始|result-desk|help-rs-checklist" \
    "$WEB_ROOT/index.html" "$WEB_ROOT/app.css" "$WEB_ROOT/app.js" \
    "$WEB_ROOT/js" "$WEB_ROOT/css" 2>/dev/null | head -80
fi
echo ""
echo "── X3 目视清单（打包后人工走一遍）──"
echo "  1. 欢迎：添加项目 · 见模板「出海落地页/通用需求大纲」"
echo "  2. 写：模板落盘或聊天写计划 → 可改 →「拆成步骤」"
echo "  3. 拆：拆分台三栏 · 可选「写回步骤摘要」· 不旁路 confirm"
echo "  4. 确认：「确认并开始」→ fake 跑"
echo "  5. 结果：结果台收口（完成/遗漏/回聊天）"
echo "  6. 帮助：#help-rs-checklist R-S10…17 可指认 · 无第三方 agent 运行时"
