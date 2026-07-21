# 行为金样清单（A0 · P2-17）

> 角色：架构大改前的**行为契约锁**  
> 真源阶段：[`architecture-redesign-2026-07-20.md`](../architecture-redesign-2026-07-20.md) §7 A0 / §11 A0-1  
> 原则：搬家代码可以，**下列红线语义不能静默漂移**

## 1. 红线（A0 必绿）

| ID | 红线 | 语义 | 自动测 | 覆盖状态 |
|----|------|------|--------|----------|
| **A0-R1** | **confirm 唯一业务开跑** | Mode B：`start_plan_job` → planned → **`confirm_start`** → `run_dir`；结构化 ParseOnly 仍走 plan job + confirm，不是 UI 旁路 `start_run` 当主路径 | `tests/mode_b_golden.rs`（散文/serial/cco-v1）· `tests/a0_behavior_golden.rs::a0_confirm_start_is_mode_b_run_entry` · `src/plan/planner` unit `confirm_starts_run_dir` | ✅ |
| **A0-R2** | **stop 含 Pending** | `stop_run` 将 Running/Starting/Queued/**Pending** 标 Stopped，写 `.done`，run → **Aborted**；否则后续波次继续 spawn | `tests/a0_behavior_golden.rs::a0_stop_run_freezes_pending_tasks`（实现见 `services/runs.rs` stop_run 注释 t38） | ✅ |
| **A0-R3** | **soft-fill 不覆盖显式 route** | `apply_worker_defaults` / `apply_tag_routing` 只改 empty/`default`/仍等于计划 default 的 provider；显式 `codex`/`claude`/… 保留 | unit：`src/plan/planner/job.rs` soft_fill_* · `src/plan/mod.rs` apply_tag_routing_* · 集成：`tests/a0_behavior_golden.rs::a0_soft_fill_preserves_explicit_provider_on_confirm` · 混跑：`tests/mixed_provider_smoke.rs` | ✅ |
| **A0-R4** | **optional 不可静默 auto-start** | `optional && !include` 在 **所有写 run 入口**经 `materialize_selected_tasks` 丢弃：Mode B `confirm` / `confirm_materialize`、ParseOnly `materialize_run` / `start_run_from_plan`（D-T3-1）；全不选报错；不得在无确认勾选时塞进 run | unit：`materialize_drops_unselected_optional` · `app::run::materialize::materialize_run_drops_unselected_optional` · `a0_materialize_rejects_all_optional_unselected` · `a0_parse_only_materialize_drops_unselected_optional` · 集成：`a0_optional_unselected_not_in_run_after_confirm` | ✅ |

## 2. 既有金样矩阵（延用，A1 搬家后仍须绿）

| 区域 | 文件 / 模块 | 锁什么 |
|------|-------------|--------|
| Mode B 三路径 | `tests/mode_b_golden.rs` | 散文 fake / serial-prompts parse / cco-v1 parse → confirm → exec |
| serial 适配器 | `tests/serial_prompts_golden.rs` | 半结构化解析形状 |
| 调度 fake | `tests/scheduler_fake.rs` | fake 多任务 · report · handoff 路径 |
| 混跑 | `tests/mixed_provider_smoke.rs` | 同 run 多 provider · 非法 mix 校验 |
| 重试 / stall | `tests/retry_and_stall.rs` | 失败重试 · stall 暂停 · H4 failover |
| resume / 预算 | `tests/resume_and_budget.rs` | resume 与预算截断 |
| handoff / inspect | `tests/handoff_ledger.rs` | ledger · VERDICT · rework |
| acceptance / term | `tests/acceptance_and_term.rs` | 验收门 · 终端会话 |
| bg / worktree | `tests/bg_and_worktree.rs` | 隔离路径 |
| PlanIR / tags / optional | `src/plan/mod.rs` `#[cfg(test)]` | validate · materialize · tag routing |
| plan job | `src/plan/planner/mod.rs` `#[cfg(test)]` | job 生命周期 · confirm 写 run |
| soft-fill | `src/plan/planner/job.rs` unit | explicit provider 保留 |

## 3. A1 搬家时的验证口令

```bash
cargo test --lib -p cco
cargo test -p cco --test mode_b_golden --test a0_behavior_golden --test mixed_provider_smoke
bash scripts/check-arch.sh
```

任何「confirm 旁路」「stop 漏 Pending」「soft-fill 硬盖 provider」「optional 默认全开并静默开跑」= **红线回归，禁止合入**。

## 4. 与目录契约的关系

- run 落盘形状 → [`run-dir.md`](./run-dir.md)  
- plan job 落盘形状 → [`plan-job.md`](./plan-job.md)  

[PROTOCOL]: A0 勾选完成后本文件只增测映射，不改红线语义；语义变更须同步 architecture-redesign + 本表 + 测。
