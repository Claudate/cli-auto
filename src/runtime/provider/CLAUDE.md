# src/runtime/provider/
> L2 | 父级: /src/runtime/CLAUDE.md

成员清单
mod.rs: **A1-4** ProviderRegistry · bin 解析 · re-export `ports::WorkerPort` DTO；`WorkerProvider` = `WorkerPort` 历史别名；**P2-7** opt-in 注册 `sdk`
claude/: Claude CLI print/bg · spawn · poll_bg · parse_result（实现 WorkerPort；start 清残留 .done；max_turns/budget null 省略 flag；P2-1 build_append_system_prompt 拼 scope + role=inspect 段）
codex.rs: Codex CLI 第二真实 provider（实现 WorkerPort；start 清残留 .done；P1-6 build_scope_prefix 注入 cwd/scope 文案）
fake.rs: 测试/演示 provider（实现 WorkerPort；CCO_DONE / CCO_FAKE_HANG / HANG_UNTIL_FAILOVER / FAIL_ONCE / STOP；start 清残留 .done；with_name 别名）
sdk.rs: **P2-7 S0** 非 CLI WorkerPort（`SdkProvider` + `SdkBackend`/`InlineSdkBackend`；**无** `Command` 拉 agent；默认 config `enabled=false`；S1 HTTP 另立）

注: chat/planner 复用固定 task_dir；start 必须 remove `.done`，否则 poll 立即 Done + 空 stdout → 本地模板  
策略（soft-fill / failover 目标 / isolation FailClosed）在 **domain/worker**，不在本目录 if-provider 业务分支。  
设计：[`docs/p2-7-sdk-provider-2026-07-21.md`](../../../docs/p2-7-sdk-provider-2026-07-21.md)

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
