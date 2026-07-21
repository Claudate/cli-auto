#!/usr/bin/env bash
# [INPUT]: 仓库根相对路径；可选 STRICT=1 将 warn 升级为失败
# [OUTPUT]: 架构硬规则可自动检查项（行数 / 巨石增量哨兵 / IPC 散落提示）
# [POS]: scripts/ 门禁；对应 CLAUDE.md「工程硬规则」与 P2-17
# [PROTOCOL]: 变更时更新此头部，然后检查 scripts/CLAUDE.md
#
# 用法:
#   ./scripts/check-arch.sh           # 默认：超限打印 WARN，exit 0（迁移期）
#   STRICT=1 ./scripts/check-arch.sh  # 硬失败 exit 1
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

STRICT="${STRICT:-0}"
SOFT_LINES=400
HARD_LINES=600
WARN=0
FAIL=0

note() { printf '%s\n' "$*"; }
warn() { WARN=$((WARN + 1)); note "WARN: $*"; }
fail() { FAIL=$((FAIL + 1)); note "FAIL: $*"; }

# 已知迁移期巨石：只允许缩小，不允许作为「新写主战场」
# 检查：若文件存在且 > HARD，记 warn（STRICT 下 fail）
# A5-4（2026-07-21）：classic 业务 facade 均 ≤200 出 S8 榜；不再列入 GIANTS。
# 仍厚非业务策略：state.js（D9 · invoke 桥）— 用下方 LEGACY_THICK 软提醒，不当业务巨石。
GIANTS=(
  # plan/mod.rs A1-1 已瘦身出榜（真源 domain/plan）
  # runtime/scheduler.rs A1-3 已拆 runtime/scheduler/*（单文件 ≤600）出榜
  # runtime/handoff.rs A1-5 已拆 runtime/handoff/* + domain/inspect（单文件 ≤600）出榜
  # services/chat.rs A1-6 已拆 services/chat/* + domain/chat + app/chat（单文件 ≤600）出榜
  # web/js/plan.js · chat.js · log.js · doctor.js · monitor.js · result.js — A5-2 S8 facade ≤200 出榜
)

check_line_count() {
  local f="$1"
  local kind="${2:-file}"
  [[ -f "$f" ]] || return 0
  local n
  n=$(wc -l <"$f" | tr -d ' ')
  if (( n > HARD_LINES )); then
    if [[ "$kind" == "giant" ]]; then
      warn "$f has $n lines (hard $HARD_LINES) — migration giant; do not add features, only extract"
      if [[ "$STRICT" == "1" ]]; then
        # 迁移期巨石在 STRICT 仍 warn→记 fail 仅当「策略」要求零巨石；A1 完成前默认不 fail 巨石本身
        :
      fi
    else
      if [[ "$STRICT" == "1" ]]; then
        fail "$f has $n lines > hard $HARD_LINES"
      else
        warn "$f has $n lines > hard $HARD_LINES"
      fi
    fi
  elif (( n > SOFT_LINES )); then
    warn "$f has $n lines > soft $SOFT_LINES"
  fi
}

note "== cco architecture check (soft=$SOFT_LINES hard=$HARD_LINES STRICT=$STRICT) =="

# 1) 巨石哨兵（A5-4 后 GIANTS 可为空；set -u 下须防护空数组）
if ((${#GIANTS[@]:-0} > 0)); then
  for g in "${GIANTS[@]}"; do
    [[ -z "$g" || "$g" == \#* ]] && continue
    check_line_count "$g" giant
  done
else
  note "info: GIANTS empty (A5-4 S8 classic facades out of list)"
fi

# 1b) 厚遗留（非 S8 业务巨石）：state 桥 — 只软提醒，禁止当新功能堆场
# D9（P-ship-C）：state.js ~820→~503（展示 helper → shared/statusUi+markdown）；>HARD 才 warn
LEGACY_THICK=(
  "web/js/state.js"
)
for g in "${LEGACY_THICK[@]}"; do
  if [[ -f "$g" ]]; then
    n=$(wc -l <"$g" | tr -d ' ')
    if (( n > HARD_LINES )); then
      warn "$g has $n lines (legacy thick / D9; not a business giant — do not add features)"
    elif (( n > SOFT_LINES )); then
      note "info: $g has $n lines (D9 under hard $HARD_LINES; keep thinning, no new features)"
    fi
  fi
done

# 2) 新架构目录（若已存在）行数
if [[ -d src/domain || -d src/app || -d src/ports || -d src/adapters ]]; then
  while IFS= read -r -d '' f; do
    check_line_count "$f" file
  done < <(find src/domain src/app src/ports src/adapters -name '*.rs' -print0 2>/dev/null)
fi

# 3) 前端：A2 之后应有 gateway；若有 features/ 则禁止 feature 内直接 invoke
# 支持路径：web/js/features（A2 实装）· web/src/features · web/features
FEAT_ROOT=""
if [[ -d web/js/features ]]; then
  FEAT_ROOT="web/js/features"
elif [[ -d web/src/features ]]; then
  FEAT_ROOT="web/src/features"
elif [[ -d web/features ]]; then
  FEAT_ROOT="web/features"
fi

if [[ -n "$FEAT_ROOT" ]]; then
  if ! find web -type f \( -name 'gateway.js' -o -path '*/shared/gateway.js' \) 2>/dev/null | grep -q .; then
    if [[ "$STRICT" == "1" ]]; then
      fail "features/ present but no gateway.js — IPC should be centralized (L1 #20)"
    else
      warn "features/ present but no gateway.js — IPC should be centralized (L1 #20)"
    fi
  fi
  # feature 内禁止散落 invoke/__TAURI__（gateway 自身除外）
  if command -v rg >/dev/null 2>&1; then
    if rg -n --glob '*.js' 'invoke\(|__TAURI__' "$FEAT_ROOT" 2>/dev/null \
      | rg -v 'gateway' >/tmp/cco-arch-invoke.txt 2>/dev/null; then
      if [[ -s /tmp/cco-arch-invoke.txt ]]; then
        if [[ "$STRICT" == "1" ]]; then
          fail "invoke/__TAURI__ outside gateway under $FEAT_ROOT:"
          cat /tmp/cco-arch-invoke.txt
        else
          warn "invoke/__TAURI__ under $FEAT_ROOT (should go through gateway):"
          head -20 /tmp/cco-arch-invoke.txt
        fi
      fi
    fi
  fi
fi

# 3b) A2 起：gateway.js 存在时，记录迁移进度（legacy invoke 计数，仅信息）
if find web -type f -path '*/shared/gateway.js' 2>/dev/null | grep -q .; then
  note "info: gateway.js present (A2 IPC hub)"
  if command -v rg >/dev/null 2>&1; then
    # 不用 set -e 敏感管道；失败当 0
    legacy_n=0
    while IFS= read -r line; do
      case "$line" in
        *'//'*) ;;
        *) legacy_n=$((legacy_n + 1)) ;;
      esac
    done < <(rg -n --glob '*.js' --glob '!**/shared/gateway.js' --glob '!**/features/**' \
      'invoke\(' web/js 2>/dev/null || true)
    note "info: legacy invoke( count outside gateway/features ≈ $legacy_n (migrate toward 0 by A5)"
  fi
fi

# 4) UI 旁路开跑粗检：web 里 start_run 且非注释（弱信号）
if command -v rg >/dev/null 2>&1; then
  hits=$(rg -n --glob '*.js' 'start_run' web 2>/dev/null | rg -v '^\s*//' | rg -v '旁路|禁止|must not|Mode B' || true)
  if [[ -n "${hits}" ]]; then
    # 仅提示：合法委托也可能出现字符串
    warn "web references start_run — confirm not bypassing Split.confirm / confirm_start (L1 #10):"
    echo "$hits" | head -15
  fi
fi

# 5) domain 不得 use tauri / 直接依赖桌面（若 domain 已存在）
if [[ -d src/domain ]] && command -v rg >/dev/null 2>&1; then
  if rg -n 'tauri::|cco_desktop' src/domain 2>/dev/null; then
    fail "src/domain must not depend on tauri/desktop (L1 #6)"
  fi
fi

note "-- summary: WARN=$WARN FAIL=$FAIL --"
if (( FAIL > 0 )); then
  exit 1
fi
if [[ "$STRICT" == "1" ]] && (( WARN > 0 )); then
  # STRICT 且仍有非巨石策略 warn 时：巨石 warn 不计入失败；仅 FAIL 已处理
  :
fi
exit 0
