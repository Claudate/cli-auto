# src/runtime/provider/
> L2 | 父级: /src/runtime/CLAUDE.md

成员清单
mod.rs: WorkerProvider trait · ProviderRegistry · TaskStatus/Result · bin 解析
claude/: Claude CLI print/bg · spawn · poll_bg · parse_result（D4 目录化；start 清残留 .done；max_turns/budget null 省略 flag；P2-1 build_append_system_prompt 拼 scope + role=inspect 段）
codex.rs: Codex CLI 第二真实 provider（已实现，非 M5；start 清残留 .done；P1-6 build_scope_prefix 注入 cwd/scope 文案）
fake.rs: 测试/演示 provider（CCO_DONE 标记；start 清残留 .done）

注: chat/planner 复用固定 task_dir；start 必须 remove `.done`，否则 poll 立即 Done + 空 stdout → 本地模板

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/runtime/CLAUDE.md
