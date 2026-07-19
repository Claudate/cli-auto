# examples/
> L2 | 父级: /CLAUDE.md

成员清单
plans/hello.cco.yaml: cco-plan/v1 双任务 fake 示例
plans/raw-hello.md: raw-single 散文单任务
plans/serial-prompts-sample.md: serial-prompts/v0 多段
plans/with-acceptance.cco.yaml: 带 acceptance 命令的 v1 示例
plans/mixed-claude-codex-inspect.cco.yaml: P0-2/P0-3 混跑真源；inventory(claude)→feat-a(claude)‖feat-b(codex)→integrate→inspect；含 role/scope/outputs + 头注释 handoff 样板
plans/plan-loop-inspect-rework.md: P-loop 说明样例（规范体四波 plan_ref/severity · rework · 桌面回补）；真源 docs/plan-execute-inspect-rework

运行注意（混跑）：
- 禁止 `cco run --force-provider` 硬抹全部引擎；勿依赖 `--provider` 抹掉已声明 provider
- 必须 `defaults.worktree: true`
- 命令：`cco parse|run --project <repo> --plan examples/plans/mixed-claude-codex-inspect.cco.yaml [--yes]`
- 协议：docs/multi-cli-collaboration-2026-07-18.md §3 · §5.3 · §6 P0

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
