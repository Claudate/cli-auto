# tests/
> L2 | 父级: /CLAUDE.md

成员清单
scheduler_fake.rs: FakeProvider 多任务调度主路径 · P1-8 report by_provider + handoff 路径
resume_and_budget.rs: resume / 预算截断
acceptance_and_term.rs: acceptance 门禁 + term 会话
bg_and_worktree.rs: bg 模式与 worktree 隔离
serial_prompts_golden.rs: serial-prompts 适配器金样
mode_b_golden.rs: Mode B 三套金样（散文/serial/cco-v1 → plan→confirm→exec；P1-6）
retry_and_stall.rs: 失败自动重试成功 · 卡死巡检重试耗尽暂停
handoff_ledger.rs: P1-4 handoff.md/json 更新 · outputs 缺失 → Failed · P2-3 VERDICT=FAIL pause+ISSUES · P-loop PASS+blocking FAIL / residual PASS / rework plan
fixtures/: fake-claude · serial-prompts-sample.md · stream-json

法则: 成员完整·一行一文件·父级链接·技术词前置
注: 缺桌面 E2E（可选增强）；Mode B 金样 P1-6 已闭环

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
