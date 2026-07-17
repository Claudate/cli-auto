#!/usr/bin/env bash
# Build release binary and assemble dist/CCO.app (macOS)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build -p cco --release
cargo build -p cco-desktop --release

DIST="$ROOT/dist"
APP="$DIST/CCO.app"
BIN="$ROOT/target/release/cco-desktop"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/CCO"
chmod +x "$APP/Contents/MacOS/CCO"
cp -f "$ROOT/target/release/cco" "$DIST/cco"

# 复制 web/ 进 bundle（Tauri custom-protocol 在 release 模式下从 binary 同级 web/ 加载）
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
echo "ZIP: $DIST/CCO-macos-arm64.zip"
echo "CLI: $DIST/cco"
