# src/cli/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: clap Commands / TermCommands 枚举 · execute dispatch（D4 已瘦身；**parse --mermaid** P2-7）
commands/: 子命令实现（doctor/init/plans/parse/plan/run/resume/status/stop/report/logs/term/tui）· common helpers
commands/run.rs: P1-7 `--provider` soft-fill vs `--force-provider` full wipe（`apply_provider_override`）
commands/status.rs: P1-8 per-provider 分栏（running/done/failed/cost）+ handoff 路径
interactive.rs: 交互选 project/plan · confirm · 非交互硬要求 --project

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
