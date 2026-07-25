# src/runtime/provider/
> L2 | 父级: /src/runtime/CLAUDE.md

成员清单
mod.rs: **A1-4** ProviderRegistry · bin 解析 · **worker_path_env / apply_worker_process_env**（GUI/.app PATH 补 Homebrew）· re-export `ports::WorkerPort` DTO；`WorkerProvider` 别名；**P2-7** opt-in `sdk`；**shell_print** 循环注册 codex/gemini/qwen/kimi/deepseek(=**CodeWhale** `codewhale exec --auto`)/copilot/codebuddy；re-export **exit_status**
exit_status.rs: 共享 exit→TaskStatus/WorkerStatus（`-1`/SIGKILL=Stopped · prefer `.done=130`）
claude/: Claude CLI print/bg · spawn（默认 **bypassPermissions** + allow 旗）· poll_bg · parse_result（WorkerPort；scope via append-system-prompt；**permission_denials>0 不得 Done**）
shell_print/: **多 CLI 共享 print 骨架** — scope 前缀 · stream_child · ShellPrintProvider · profiles（含 install_hint；**禁止** spawn 时 npm install）
codex.rs: 薄封装 → ShellPrintProvider + CODEX profile（`codex exec` · 兼容 re-export scope helpers）
fake.rs: 测试/演示 provider（CCO_DONE / hang / FAIL_ONCE / with_name 别名）
sdk.rs / sdk_http.rs / sdk_tool_loop/: **P2-7** 非 CLI 路径（默认 enabled=false）

注: chat/planner 复用固定 task_dir；start 必须 remove `.done`  
策略（soft-fill / failover_order / isolation FailClosed）在 **domain/worker**，不在本目录 if-provider 业务分支。  
设计：[`docs/archive/p2-7-sdk-provider-2026-07-21.md`](../../../docs/archive/p2-7-sdk-provider-2026-07-21.md)

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
