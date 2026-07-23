# src/runtime/provider/
> L2 | 父级: /src/runtime/CLAUDE.md

成员清单
mod.rs: **A1-4** ProviderRegistry · bin 解析 · **worker_path_env / apply_worker_process_env**（GUI/.app PATH 补 Homebrew，修 codex shebang `env: node`）· re-export `ports::WorkerPort` DTO；`WorkerProvider` = `WorkerPort` 历史别名；**P2-7** opt-in 注册 `sdk`（S0/S1/S2 backend 选择）；re-export **exit_status**
exit_status.rs: 共享 exit→TaskStatus/WorkerStatus（`-1`/SIGKILL=Stopped · prefer `.done=130` · stream 不盖 stop 标记）
claude/: Claude CLI print/bg · spawn · poll_bg · parse_result（实现 WorkerPort；start 清残留 .done；max_turns/budget null 省略 flag；P2-1 build_append_system_prompt 拼 scope + role=inspect 段；collect 走 exit_status；spawn apply_env 注入 worker PATH）
codex.rs: Codex CLI 第二真实 provider（实现 WorkerPort；start 清残留 .done；P1-6 build_scope_prefix 注入 cwd/scope 文案；collect 走 exit_status；apply_env + preflight 注入 worker PATH）
fake.rs: 测试/演示 provider（实现 WorkerPort；CCO_DONE / CCO_FAKE_HANG / HANG_UNTIL_FAILOVER / FAIL_ONCE / STOP；start 清残留 .done；with_name 别名）
sdk.rs: **P2-7** 非 CLI WorkerPort（`SdkProvider` + `SdkBackend`/`InlineSdkBackend`；**无** `Command` 拉 agent；默认 config `enabled=false`）
sdk_http.rs: **P2-7 S1** Anthropic Messages HTTP one-shot（`AnthropicMessagesBackend` · mockable `MessagesHttpClient`；`bin=messages`；需 API key）
sdk_tool_loop/: **P2-7 S2** Messages tool loop（`AnthropicToolLoopBackend` · cwd-scoped read/list/write · `bin=tools` · 默认关）

注: chat/planner 复用固定 task_dir；start 必须 remove `.done`，否则 poll 立即 Done + 空 stdout → 本地模板  
策略（soft-fill / failover 目标 / isolation FailClosed）在 **domain/worker**，不在本目录 if-provider 业务分支。  
设计：[`docs/archive/p2-7-sdk-provider-2026-07-21.md`](../../../docs/archive/p2-7-sdk-provider-2026-07-21.md)

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
