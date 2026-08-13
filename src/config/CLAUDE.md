# src/config/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: Config · AllowedProject · **BrowserConfig**（默认关 · kitewright/playwright_mcp · 见 `docs/browser-automation-cco.md`）· providers/terminal/tui 段 · retry_max/stall_secs · failover_enabled/fallback_extra_attempts/**failover_order**(H4 可配顺序，默认 claude,codex；fake/sdk 永不自动) · **cost_route_enabled / cost_escalate_enabled**（P0/P1 默认开）· **cost_intent_enabled**（P3 默认关）· 见 `docs/cost-aware-cli-router-2026-07-27.md` · post_inspect/post_git_push/**post_open_pr**（系统收尾默认关） · **auto_commit_granularity**（off/per_plan/per_task；旧 post_git_push 兼容计划级） · planner_critic_enabled · **effort** · **permission_mode**（默认 **bypassPermissions** · 无人 worker 可写；dontAsk 会假完成）· **auto_closeout / auto_rework**（Ensure 默认开）· **auto_rework_docs_only**（默认**关**·真 blocking 也自动回补；手点 residual 主机降级不触发）· normalize_permission_mode · **builtin_provider_map**（claude/codex/fake/sdk + gemini/qwen/kimi/deepseek/copilot/codebuddy；`CCO_*_BIN`） · **MemoryConfig**（P3 本地语义记忆 enabled/ttl_days/max_entries · 默认关 · 无外部服务字段；见 `docs/memory-user-guide.md`） · load/save · state_root
git.rs: GitConfig · GitRemote · GitIdentity · AutoCommitPolicy · **AutoCommitGranularity**；描述国内/国外 remote、提交身份、自动提交粒度/消息模板/push 策略，纯数据无 IO

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
