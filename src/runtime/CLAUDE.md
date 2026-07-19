# src/runtime/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 子模块与 re-export（Scheduler · LogEvent · ProviderRegistry · handoff）
scheduler.rs: 依赖就绪调度 · 并行上限 · 预算 · acceptance · outputs · inspect VERDICT 门禁(P2-3+P-loop: Unknown/blocking ISSUES) · handoff 钩子 · [CCO_HANDOFF] 注入 · 卡死巡检/自动重试 · 状态落盘
handoff.rs: 事中账本 handoff.md/json · Board/Timeline/Fragments · outputs 检查 · VERDICT/ISSUES 分级(P2-3+P-loop) · REWORK_HOOK · build_rework_plan · accept_residual · inspect_loop_view · build_prompt_prefix / with_handoff_prefix（P1-5）
log_events.rs: worker stdout/stderr → LogEvent · compact_text_tail/floor_char_boundary（CJK 安全）
provider/: WorkerProvider 总线 + claude/（spawn·poll_bg·parse_result）+ codex/fake
worktree.rs: git worktree 隔离创建/清理 · 混跑 WorktreeOnFail::FailClosed
acceptance.rs: 任务后软验收命令

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
