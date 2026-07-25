# tests/
> L2 | 父级: /CLAUDE.md

成员清单
scheduler_fake.rs: FakeProvider 多任务调度主路径 · P1-8 report by_provider + handoff 路径
resume_and_budget.rs: resume / **单任务 prepare_task_retry** / 预算截断
acceptance_and_term.rs: acceptance 门禁 + term 会话
bg_and_worktree.rs: bg 模式与 worktree 隔离
serial_prompts_golden.rs: serial-prompts 适配器金样
mode_b_golden.rs: Mode B 三套金样（散文/serial/cco-v1 → plan→confirm→exec；P1-6）
a0_behavior_golden.rs: **P2-17 A0 行为红线**（confirm 唯一开跑 · stop 含 Pending · soft-fill 不盖显式 route · optional 不静默 auto-start · **ParseOnly materialize 同 drop optional · D-T3-1**）；清单见 `docs/contracts/behavior-golden.md`
retry_and_stall.rs: 失败自动重试成功 · 卡死巡检重试耗尽暂停
handoff_ledger.rs: P1-4 handoff.md/json 更新 · outputs 缺失 → Failed · P2-3 VERDICT=FAIL pause+ISSUES · P-loop PASS+blocking FAIL / residual PASS / rework plan
ensure_close_loop.rs: **Ensure E1/E3** materialize 注入 `sys-closeout`（role=None 图）· docs-only FAIL → `maybe_auto_rework` 新 run · 业务 blocking / 开关关 不自动
mixed_provider_smoke.rs: 同 run 多 provider · 非法 mix 校验
fixtures/: fake-claude · serial-prompts-sample.md · stream-json

法则: 成员完整·一行一文件·父级链接·技术词前置
注: 缺桌面 E2E（可选增强）；Mode B 金样 P1-6 已闭环；A0 红线见 a0_behavior_golden

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
