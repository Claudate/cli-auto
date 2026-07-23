#!/usr/bin/env bash
# [INPUT]: 可选站点根路径；环境 STRICT=1 · SCAN_DIST=1 · SCAN_MD=1 · SKIP_G1=1
# [OUTPUT]: 落地页假资产 / 页脚主 CTA 等门禁；默认 WARN 不失败，STRICT 升级
# [POS]: scripts/ 与 docs/runtime-prompts/landing-gates.md
# [PROTOCOL]: 变更时更新 docs/runtime-prompts/landing-gates.md 与 scripts/CLAUDE.md
# 兼容 macOS bash 3.2（不用 mapfile）
#
# 用法:
#   ./scripts/check-landing-gates.sh
#   ./scripts/check-landing-gates.sh /path/to/site
#   STRICT=1 ./scripts/check-landing-gates.sh ./web

set -euo pipefail

ROOT_SCRIPT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${1:-}"
if [[ -z "$TARGET" ]]; then
  if [[ -d "$ROOT_SCRIPT/web" ]]; then
    TARGET="$ROOT_SCRIPT/web"
  else
    TARGET="$ROOT_SCRIPT"
  fi
fi
if [[ -d "$TARGET" ]]; then
  TARGET="$(cd "$TARGET" && pwd)"
fi

STRICT="${STRICT:-0}"
SCAN_DIST="${SCAN_DIST:-0}"
SCAN_MD="${SCAN_MD:-0}"
SKIP_G1="${SKIP_G1:-0}"

WARN=0
FAIL=0

note() { printf '%s\n' "$*"; }
warn() { WARN=$((WARN + 1)); note "WARN: $*"; }
fail() { FAIL=$((FAIL + 1)); note "FAIL: $*"; }

note "check-landing-gates: target=$TARGET strict=$STRICT"

if [[ ! -d "$TARGET" ]]; then
  fail "target not a directory: $TARGET"
  exit 1
fi

FIND_PRUNE='( -path */node_modules/* -o -path */.git/* -o -path */target/*'
if [[ "$SCAN_DIST" != "1" ]]; then
  FIND_PRUNE+=' -o -path */dist/* -o -path */.vercel/* -o -path */.astro/*'
fi
FIND_PRUNE+=' -o -path */docs/runtime-prompts/* -o -path */examples/marketing-landing-reference/* )'

NAME_ARGS=( \( -name '*.html' -o -name '*.astro' -o -name '*.vue' -o -name '*.svelte' \
  -o -name '*.jsx' -o -name '*.tsx' -o -name '*.js' -o -name '*.ts' )
if [[ "$SCAN_MD" == "1" ]]; then
  NAME_ARGS+=( -o -name '*.md' -o -name '*.mdx' )
fi
NAME_ARGS+=( \) )

# shellcheck disable=SC2086
FILE_LIST="$(find "$TARGET" $FIND_PRUNE -prune -o -type f "${NAME_ARGS[@]}" -print 2>/dev/null | head -n 4000 || true)"
FILE_COUNT=0
if [[ -n "$FILE_LIST" ]]; then
  FILE_COUNT="$(printf '%s\n' "$FILE_LIST" | grep -c . || true)"
fi

if [[ "$FILE_COUNT" -eq 0 ]]; then
  warn "no frontend source files matched under $TARGET"
else
  note "scanning $FILE_COUNT files"
fi

# --- G1 / G2 ---
if [[ "$SKIP_G1" != "1" && "$FILE_COUNT" -gt 0 ]]; then
  hits="$(printf '%s\n' "$FILE_LIST" | tr '\n' '\0' | xargs -0 grep -n -E 'https?://([a-zA-Z0-9.-]*\.)?example\.com|app\.example\.com' 2>/dev/null | head -n 40 || true)"
  hits2="$(printf '%s\n' "$FILE_LIST" | tr '\n' '\0' | xargs -0 grep -n -E 'hello@example\.com|[a-zA-Z0-9._%+-]+@example\.com' 2>/dev/null | head -n 40 || true)"
  if [[ -n "${hits:-}" ]]; then
    fail "G1 example.com / app.example.com in user-facing sources (SKIP_G1=1 only for explicit demos)"
    note "$hits" | head -n 15
  fi
  if [[ -n "${hits2:-}" ]]; then
    fail "G2 placeholder contact email (@example.com)"
    note "$hits2" | head -n 10
  fi
else
  note "G1/G2 skipped (SKIP_G1=1 or no files)"
fi

# --- G5: h1 on index-like files (or hero child when index only composes) ---
g5_ok=0
for idx in "$TARGET/index.html" "$TARGET/src/pages/index.astro" "$TARGET/src/pages/index.tsx" \
  "$TARGET/pages/index.astro" "$TARGET/app/page.tsx"; do
  if [[ -f "$idx" ]]; then
    if grep -q -i '<h1' "$idx" || grep -q -E 'h1[>\s]|#hero-title|hero-title|Hero' "$idx"; then
      note "G5 ok: $idx has heading/hero marker"
      g5_ok=1
    else
      # composed pages: search nearby home/Hero components once
      base="$(dirname "$idx")"
      if find "$(dirname "$base")" -type f \( -name 'Hero.*' -o -name 'hero.*' -o -name 'index.html' \) 2>/dev/null \
        | head -n 20 | tr '\n' '\0' | xargs -0 grep -l -i -E '<h1|hero-title' 2>/dev/null | head -n 1 | grep -q .; then
        note "G5 ok: h1 found in hero/related under $(dirname "$base")"
        g5_ok=1
      else
        fail "G5 index-like file missing h1: $idx"
      fi
    fi
    break
  fi
done

# --- G3 / G4 / G6 on markup ---
MARKUP="$(printf '%s\n' "$FILE_LIST" | grep -E '\.(html|astro)$' || true)"
if [[ -n "$MARKUP" ]]; then
  while IFS= read -r f; do
    [[ -z "$f" || ! -f "$f" ]] && continue
    if grep -q -i -E 'footer|site-footer' "$f" 2>/dev/null; then
      if grep -q -E 'btn-primary|button-primary' "$f" 2>/dev/null \
        && grep -q -E '注册领取|领取免费|立即注册|Sign up|Get started' "$f" 2>/dev/null; then
        if awk 'BEGIN{IGNORECASE=1} /site-footer|<footer/{p=1} p && /btn-primary|button-primary/ && /注册|领取|Sign up|Get started/{c=1} END{exit !c}' "$f" 2>/dev/null; then
          warn "G3 footer may contain primary register CTA: $f"
        fi
      fi
    fi
    c="$(grep -o -E '注册领取免费额度|立即免费注册|Get started for free' "$f" 2>/dev/null | wc -l | tr -d ' ' || true)"
    c="${c:-0}"
    if [[ "$c" -ge 5 ]]; then
      warn "G4 main CTA phrase appears ${c} times in $f"
    fi
  done <<EOF
$MARKUP
EOF

  g6="$(printf '%s\n' "$MARKUP" | tr '\n' '\0' | xargs -0 grep -n -E 'rel="canonical"[^>]*example\.com|og:url[^>]*example\.com|content="https?://example\.com' 2>/dev/null | head -n 8 || true)"
  if [[ -n "${g6:-}" ]]; then
    warn "G6 canonical/og:url contains example.com"
    printf '%s\n' "$g6" | head -n 5
  fi
fi

note "summary: FAIL=$FAIL WARN=$WARN"

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
if [[ "$STRICT" == "1" && "$WARN" -gt 0 ]]; then
  note "STRICT=1: treating WARN as failure"
  exit 1
fi
exit 0
