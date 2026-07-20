---
name: cco-run
description: >
  Thin wrapper around the cco CLI (`cco run` / status / report / stop).
  Use when the user invokes /cco-run, or asks to run a cco plan, start orchestration,
  fake-smoke a plan, or check the latest run from Claude Code. Does not reimplement
  the scheduler — only shells out to an installed or in-repo `cco` binary.
---

# /cco-run — cco CLI thin wrapper

> **Scope**: invoke existing `cco` host. **Not** a second Scheduler, not Mode B reimplementation, not desktop Tauri.

## When to use

- User types `/cco-run` or `cco-run …`
- User wants to execute a plan file under the current project from Claude Code
- User wants a quick **fake** smoke without leaving the session
- User asks for latest `status` / `report` / `stop` after a run

## Resolve the `cco` binary

Prefer the first that works (print which one you use):

```bash
# 1) on PATH
command -v cco

# 2) this monorepo release / package artifacts (when cwd is claude-auto)
test -x ./target/release/cco && echo ./target/release/cco
test -x ./dist/cco && echo ./dist/cco

# 3) dev fallback (slow)
# cargo run -q -- <subcommand> …
```

If none exist: build once with `cargo build -p cco --release`, then use `./target/release/cco`.

## Defaults

| Flag | Default | Notes |
|------|---------|--------|
| `--project` | current workspace root (`pwd -P`) | Must be a real project dir |
| `--plan` | **required** unless user names one | Relative to project or absolute |
| `--provider` | omit (config default) | Use `fake` for offline smoke |
| `--yes` | **on** for non-interactive skill runs | Avoids TTY prompts mid-agent |
| `--plan-mode` | omit (`ai` default on prose) | Structured plans auto skip-plan |

Hard rules:

1. **Always** go through `cco run` / `cco plan` / `cco resume` — never invent a parallel spawn of Claude/Codex.
2. Prose `.md` plans go through Mode B (`plan` job then confirm inside `run` when `--yes`); structured `cco-plan/v1` auto skip-plan. Do **not** invent a `start_run` bypass.
3. Do **not** commit `dist/`, `.cco-out/`, local `plans/`, or secrets.
4. Prefer **`--provider fake --yes`** when the user only wants to verify wiring.

## Recipe: run a plan

```bash
CCO_BIN="$(command -v cco || true)"
[ -z "$CCO_BIN" ] && [ -x ./target/release/cco ] && CCO_BIN=./target/release/cco
[ -z "$CCO_BIN" ] && [ -x ./dist/cco ] && CCO_BIN=./dist/cco
[ -z "$CCO_BIN" ] && { echo "cco binary not found; cargo build -p cco --release"; exit 1; }

PROJECT="${PROJECT:-$(pwd -P)}"
PLAN="${PLAN:?set PLAN=docs/plans/….md or .cco.yaml}"

# Offline smoke (no API):
"$CCO_BIN" run --project "$PROJECT" --plan "$PLAN" --yes --provider fake

# Real Claude (needs ANTHROPIC_API_KEY / doctor ok):
# "$CCO_BIN" run --project "$PROJECT" --plan "$PLAN" --yes --provider claude
```

Optional args (pass through when the user names them):

- `--dry-run` — parse + print stages only
- `--max-parallel N`
- `--max-budget USD`
- `--only task_id` / `--from-task task_id`
- `--skip-plan` — structured only; do not force on prose unless user insists
- `--tui` — attach TUI (interactive; usually skip in agent runs)
- `--auto-open-terminal` / `--terminal-kind external`

## Recipe: doctor / status / report / stop

```bash
"$CCO_BIN" doctor --project "$PROJECT"
"$CCO_BIN" status          # latest under CCO_STATE_ROOT / default state
"$CCO_BIN" report
"$CCO_BIN" stop --all      # or stop a task if user names one
"$CCO_BIN" plans --project "$PROJECT"
```

If `CCO_STATE_ROOT` is set in the environment, keep it so status/report match the run you just started.

## Args parsing for `/cco-run`

When the user provides args after `/cco-run`, map roughly:

| User says | Action |
|-----------|--------|
| `/cco-run docs/plans/foo.md` | `run --plan docs/plans/foo.md --yes` |
| `/cco-run foo.md fake` | add `--provider fake` |
| `/cco-run … dry` / `dry-run` | add `--dry-run` |
| `/cco-run status` | `cco status` only |
| `/cco-run report` | `cco report` only |
| `/cco-run stop` | `cco stop` (confirm scope if ambiguous) |
| `/cco-run` with no plan | `cco plans --project .` then ask which plan, or use the path they already have open |

If the open editor buffer is a plan under the project, prefer that path.

## Output to the user

After `run`, always surface:

1. Exit code / final status line (`Completed` / `Paused` / `Failed`)
2. `run_id` and `run_dir` if printed
3. How to continue: `cco status` · `cco report` · `cco resume` · `cco tui`

On failure: paste the relevant tail (acceptance / task error), then suggest `cco doctor` and provider flags — do not silently retry with a different engine unless the user asked for failover.

## Out of scope (do not implement here)

- Desktop Tauri UI / chat builder
- Replacing Scheduler or Mode B confirm chain
- System multi-window (P2-4), true TUI PTY (P2-5), M5 SDK/Mermaid/PR/Windows (P2-7)
- Installing global skills outside this repo (user may copy `.claude/skills/cco-run` themselves)

## Verify (maintainer)

```bash
# From claude-auto checkout:
test -f .claude/skills/cco-run/SKILL.md
./target/release/cco run --project /tmp/… --plan docs/plans/hello.cco.yaml --yes --provider fake
# or: bash scripts/smoke.sh
```
