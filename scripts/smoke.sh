#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export CCO_STATE_ROOT="${CCO_STATE_ROOT:-/tmp/cco-smoke-state}"
export CCO_DEFAULT_PROVIDER=fake
export CCO_FAST_POLL=1
rm -rf "$CCO_STATE_ROOT"
mkdir -p "$CCO_STATE_ROOT"

DEMO=/tmp/cco-smoke-proj
rm -rf "$DEMO"
mkdir -p "$DEMO/docs/plans"
cp examples/plans/hello.cco.yaml "$DEMO/docs/plans/"

cargo build -q
BIN=./target/debug/cco

"$BIN" doctor || true
"$BIN" parse --project "$DEMO" --plan docs/plans/hello.cco.yaml
"$BIN" run --project "$DEMO" --plan docs/plans/hello.cco.yaml --yes --provider fake
"$BIN" status
"$BIN" report

# terminal multi-open (embedded does not need a real GUI)
"$BIN" term open --task inventory --kind embedded
"$BIN" term list
SID=$("$BIN" term list | awk 'NR==1{print $1}')
if [[ -n "${SID:-}" && "$SID" != "(no" ]]; then
  "$BIN" term close --session "$SID"
fi

# acceptance path (should pause/fail)
cp examples/plans/with-acceptance.cco.yaml "$DEMO/docs/plans/" 2>/dev/null || \
  cat > "$DEMO/docs/plans/with-acceptance.cco.yaml" <<'EOF'
schema: cco-plan/v1
name: acc
defaults:
  provider: fake
  mode: print
  worktree: false
tasks:
  - id: a
    prompt: "x\nCCO_DONE ok"
    acceptance: "exit 1"
EOF

set +e
"$BIN" run --project "$DEMO" --plan docs/plans/with-acceptance.cco.yaml --yes --provider fake
ACC_CODE=$?
set -e
echo "acceptance run exit=$ACC_CODE (expect non-zero)"

# bg mode with fake (delayed done)
cat > "$DEMO/docs/plans/bg.cco.yaml" <<'EOF'
schema: cco-plan/v1
name: bgdemo
defaults:
  provider: fake
  mode: bg
  worktree: false
tasks:
  - id: b1
    mode: bg
    prompt: "bg\nCCO_DONE ok"
EOF
export CCO_FAKE_BG_MS=40
"$BIN" run --project "$DEMO" --plan docs/plans/bg.cco.yaml --yes --provider fake
"$BIN" status | head -20

# worktree on a temp git repo
GREPO=/tmp/cco-smoke-git
rm -rf "$GREPO"
mkdir -p "$GREPO"
git -C "$GREPO" init -q
git -C "$GREPO" config user.email cco@test
git -C "$GREPO" config user.name cco
echo hi > "$GREPO/README"
git -C "$GREPO" add README
git -C "$GREPO" commit -q -m init
mkdir -p "$GREPO/docs/plans"
cat > "$GREPO/docs/plans/wt.yaml" <<'EOF'
schema: cco-plan/v1
name: wt
defaults:
  provider: fake
  mode: print
  worktree: true
tasks:
  - id: t1
    prompt: "in worktree\nCCO_DONE ok"
EOF
"$BIN" run --project "$GREPO" --plan docs/plans/wt.yaml --yes --provider fake
test -d "$GREPO/.cco-worktrees" && echo "worktree dir present" || echo "worktree dir missing (check logs)"

echo "smoke ok"
