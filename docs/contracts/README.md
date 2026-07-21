# docs/contracts/

> A0 契约冻结目录（P2-17）  
> 父级：[`../architecture-redesign-2026-07-20.md`](../architecture-redesign-2026-07-20.md)

| 文件 | 内容 |
|------|------|
| [`behavior-golden.md`](./behavior-golden.md) | 行为红线 + 金样映射（confirm / stop / soft-fill / optional） |
| [`run-dir.md`](./run-dir.md) | `{state_root}/runs/{run_id}/` 布局与 run.json 字段 |
| [`plan-job.md`](./plan-job.md) | `{state_root}/plan_jobs/{job_id}/` 布局与 Mode B 状态机 |

**无产品行为变化**：本目录只锁契约；实现搬家从 A1 起。
