# docs/contracts/

> A0 契约冻结目录（P2-17）  
> 父级：[`../architecture-redesign-2026-07-20.md`](../architecture-redesign-2026-07-20.md)

| 文件 | 内容 |
|------|------|
| [`behavior-golden.md`](./behavior-golden.md) | 行为红线 + 金样映射（confirm / stop / soft-fill / optional） |
| [`run-dir.md`](./run-dir.md) | `{state_root}/runs/{run_id}/` 布局与 run.json 字段 |
| [`plan-job.md`](./plan-job.md) | `{state_root}/plan_jobs/{job_id}/` 布局与 Mode B 状态机 |
| [`session-digest.md`](./session-digest.md) | **session-digest/v1** 会话语义压缩（C0 · 非 A0 行为红线；勾选听落地计划） |
| [`session-digest.example.yaml`](./session-digest.example.yaml) | session-digest 合格示例 |

**A0 三件**（behavior / run-dir / plan-job）：只锁契约；实现搬家从 A1 起，**无**借 session-digest 改产品开跑。  
**session-digest**：Agent/人工作流缓存；**不**注入聊天 LLM；**不**旁路 confirm。
