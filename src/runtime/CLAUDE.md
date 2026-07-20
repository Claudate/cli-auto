# src/runtime/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 子模块与 re-export（Scheduler · LogEvent · ProviderRegistry · handoff）
scheduler.rs: 依赖就绪调度 · 并行上限 · 预算 · acceptance · outputs · inspect VERDICT 门禁(P2-3+P-loop) · **sys-post-git-push 先巡检 PASS 硬门禁**（spawn 前 Skipped） · handoff 钩子 · [CCO_HANDOFF] · 卡死巡检/重试 · H4 failover · 状态落盘
handoff.rs: 事中账本 handoff.md/json · Board/Timeline/Fragments · outputs 检查 · VERDICT/ISSUES 分级 · **system_push_inspect_gate** · write_task_diff/CHANGED.md（P2-2） · REWORK_HOOK · build_rework_plan · accept_residual · inspect_loop_view · with_handoff_prefix
log_events.rs: worker stdout/stderr → LogEvent · compact_text_tail/floor_char_boundary（CJK 安全）
provider/: WorkerProvider 总线 + claude/（spawn·poll_bg·parse_result）+ codex/fake
worktree.rs: git worktree 隔离创建/清理 · 混跑 WorktreeOnFail::FailClosed
acceptance.rs: 任务后软验收命令

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
