# run_dir 目录契约（A0 冻结）

> 状态：**A0 契约冻结**（2026-07-20）  
> 实现锚点：`src/state/mod.rs` · `src/services/runs.rs` · `src/runtime/scheduler.rs` · `src/runtime/handoff.rs`  
> 兼容策略（架构计划 §6.2）：读 v1；新写带 `schema_version` / 既有 `schema` 字段；A1 搬家不得静默改相对路径

## 1. 根路径

| 项 | 规则 |
|----|------|
| 根 | `{config.state_root}/runs/`（`Config::runs_dir()`） |
| run 目录 | `{runs_root}/{run_id}/` |
| `run_id` | `YYYYMMDDTHHMMSSZ-{4 hex}`（`state::new_run_id`） |
| 创建 | `state::prepare_run_dir` → 确保 `tasks/` 存在 |

## 2. 目录树（v1）

```text
{state_root}/runs/{run_id}/
  run.json                 # RunState 主体（schema: cco-run/v1）
  events.jsonl             # 追加事件流（ts, type, …）
  plan.resolved.json       # 确认/开跑时冻结的 PlanIR（resume 真源）
  planner_cost.json        # 可选；confirm 时从 plan job 复制（P1-5）
  report.md / report.json  # 终态后 report 写手
  handoff.md / handoff.json# 事中账本（若启用 handoff 路径）
  tasks/
    {task_id}/
      meta.json            # pid / session 等（provider 写；stop 可读）
      stdout.log / stderr  # 或 provider 约定名
      .done                # 终端标记（exit code 文本，如 "0" / "130"）
      …                    # provider 私有文件
```

## 3. `run.json` 关键字段

| 字段 | 说明 |
|------|------|
| `schema` | 固定 `"cco-run/v1"` |
| `run_id` | 与目录名一致 |
| `project_root` | 绝对路径 |
| `plan_path` | 源计划路径（绝对或规范化） |
| `adapter` | 如 `cco-plan/v1` / `serial-prompts/v0` / `raw-single` |
| `status` | `init` · `validated` · `running` · `paused` · `completed` · `failed` · `aborted` |
| `started_at` / `finished_at` | RFC3339 |
| `tasks.{id}` | `TaskState`：status/provider/mode/pid/attempt/failover_used/… |

**不序列化**：`RunState.run_dir`（`#[serde(skip)]`，load 时注入）。

## 4. 任务状态（stop 契约相关）

| `TaskStatus` | stop_run 是否冻住 |
|--------------|-------------------|
| `pending` | **是**（A0-R2 红线） |
| `queued` / `starting` / `running` | 是 |
| `done` / `failed` / `stopped` / `skipped` / … | 保持终态 |

`stop_run` 后：`RunStatus::Aborted`，被冻任务 → `Stopped`，写 `tasks/{id}/.done`（常见 `130`）。

## 5. 事件流（最小）

`events.jsonl` 每行一 JSON：`ts` + `type` + 载荷。  
常见 type（非穷尽）：`task_start` · `task_end` · `run_end`（含 `via: desktop|external_stop`）· stall/retry 相关。

## 6. A1 搬家约束

- 路径名与 `schema: cco-run/v1` **不得无迁移地改**；若升版须双读 + 文档本条升版。  
- Domain 不拼路径；**Store 适配器**拥有拼接（目标 `ports::RunStore`）。  
- CLI / 桌面 / TUI **只经 Application** 读写，禁止各写一套 run_dir 布局。

[PROTOCOL]: 改路径或必填字段 → 先改本文件 + behavior-golden + 测，再改代码。
