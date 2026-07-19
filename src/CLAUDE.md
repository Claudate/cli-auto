# src/
> L2 | 父级: /CLAUDE.md

成员清单
lib.rs: 库根；re-export plan/runtime/services/state/terminal/tui
main.rs: cco 二进制入口；clap → cli::execute
services/: CLI 与桌面共用服务层（projects/runs/live/settings · D4 目录化）
cli/: clap 命令面 + commands/ 子命令（D4）
plan/: 适配器 + PlanIR/TaskIR(role/scope/outputs/require_inspect) + planner/（Mode B plan job · D4）
runtime/: scheduler · handoff(事中账本·VERDICT/ISSUES P2-3) · provider(claude/·codex/fake) · log_events · worktree · acceptance
terminal/: TerminalManager + external launcher（桌面未接 open）
tui/: ratatui 多页观察层
config/: ~/.cco/config.toml + AllowedProject
state/: run.json / events.jsonl / TaskState
doctor/: 环境门禁 DoctorReport
graph/: DAG ready_tasks / topo_layers / format_graph
report/: report.md+json · By provider 分栏 · handoff 路径（P1-8）

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
