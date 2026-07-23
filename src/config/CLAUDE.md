# src/config/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: Config · AllowedProject · providers/terminal/tui 段 · retry_max/stall_secs · failover_enabled/fallback_extra_attempts(H4) · post_inspect/post_git_push/**post_open_pr**（系统收尾总开关默认关 · S-PR） · planner_critic_enabled（可选 LLM 第二跳校对，默认关） · **effort**（`low|medium|high|xhigh|max|ultracode` · 默认 high · env `CCO_EFFORT` · `normalize_effort` / `effort_cli_level` / ultracode→xhigh + `ULTRACODE_SYSTEM_HINT`） · **providers.sdk**（P2-7 非 CLI，**默认 enabled=false**；`bin=inline|messages|tools` · S1 HTTP · S2 tool loop） · load/save · state_root

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
