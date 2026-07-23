# plan job 目录契约（A0 冻结）

> 状态：**A0 契约冻结**（2026-07-20）  
> 实现锚点：`src/plan/planner/job.rs` · `src/plan/planner/view.rs` · `src/services/runs.rs::confirm_start`  
> Mode B 业务规则参考：[`product-mode-b-ai-planner.md`](../product-mode-b-ai-planner.md)（实现搬家听架构计划）

## 1. 根路径

| 项 | 规则 |
|----|------|
| 根 | `{config.state_root}/plan_jobs/`（`plan_jobs_dir`） |
| job 目录 | `{plan_jobs}/{job_id}/`（`job_dir`） |
| job_id | UUID 风格字符串（planner 分配） |

## 2. 目录树（v1）

```text
{state_root}/plan_jobs/{job_id}/
  job.json                 # PlanJob 元数据与状态机
  plan.proposed.json       # 拆分提案 PlanIR（确认前可编辑）
  plan.user_edits.json     # 人工编辑增量（P2-1/P2-2 replan 保编辑）
  plan.resolved.json       # confirm 后冻结图（可回落作再开跑）
  planner_cost.json        # 可选；规划 LLM 花费
  planner.log / tail…      # 规划过程日志（若有）
```

## 3. `job.json` 关键字段

| 字段 | 说明 |
|------|------|
| `job_id` | 与目录名一致 |
| `status` | `planning` · `planned` · `plan_failed` · `confirmed` · `cancelled` |
| `project` | 项目根绝对路径 |
| `plan_path` | 源计划路径 |
| `plan_mode` | `parse` · `fake` · `ai` · `fast` · `direct`（整份计划单任务；仍走 confirm） |
| `provider` / `exec_mode` | **confirm 后 worker 软默认**（soft-fill 来源） |
| `run_id` | confirm 后写入 |
| `task_count` / `max_parallel` / `adapter` | planned 时填充 |
| `planner_cost_usd` · critic_* | 规划/校对元数据（可选） |

## 4. 状态机（业务）

```text
start_plan_job → planning
       │
       ├─ success → planned  (+ plan.proposed.json)
       └─ error   → plan_failed

planned|confirmed
       │  load_proposed_for_exec
       │  · apply_worker_defaults(job.provider)   # soft
       │  · materialize_selected_tasks            # drop !include optional
       ▼
confirm_start → start_run_from_plan → mark_confirmed
       │
       └─ status=confirmed, run_id set, plan.resolved.json, copy cost → run_dir
```

**唯一业务开跑入口（红线 A0-R1）**：`confirm_start`（目标名 `SplitUseCase.confirm`）。  
ParseOnly 结构化计划仍创建 plan job，再 confirm——**不是**第二套旁路。

## 5. 编辑与 optional

| 操作 | 文件 / API |
|------|------------|
| 改 title/prompt/include/provider/depends_on | `update_proposed_task` → 写 proposed + user_edits |
| 删任务 | `remove_proposed_task` |
| 依赖清理 | `sanitize_proposed_deps` |
| replan 保编辑 | `preserve_from_job_id` + `plan.user_edits.json` |
| optional 勾选 | `optional: true` 时 `include` 可由确认屏改；默认 adapter 常 `include: !optional` |

**红线 A0-R4**：`include: false` 的 optional 不得在 confirm 时静默改 true 并进 run。

## 6. soft-fill（红线 A0-R3）

`apply_worker_defaults(ir, job.provider, job.exec_mode)`：

- 总是写 `ir.default_*` 与各 task `mode`  
- provider **仅**当 task 为 empty / `"default"` / 仍等于旧 plan default 时改写  
- 显式引擎（如 `codex`）**保留**

与 tags 路由 `apply_tag_routing` 同构：不覆盖已声明 route。

## 7. A1 搬家约束

- 目标：`domain/split` + `app/split` + `ports::PlanJobStore`  
- 文件名与 status 枚举字符串（snake_case JSON）冻结；UI DTO 可人话化但不改盘上枚举  
- 禁止 UI 直写 `plan_jobs/`；只经 app 用例

[PROTOCOL]: 改 job 路径/状态枚举/confirm 入口 → 先改本文件 + behavior-golden + 测，再改代码。
