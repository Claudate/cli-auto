# 拆分软校验 + SQLite 存储

> 日期：2026-07-21  
> 产品意图：拆分结果给 **后续 AI 跑**；软件侧只要 **顺序 · 并发波次 · 是否执行** 显示完整合理，**不要拆分过严**导致整图丢弃。  
> 状态：波次 1 = **过渡**（soften + PlanIR 双写索引）· **终态 SoT 已由 C1–C7 落地**  
> **用户完整意图 / 独立 cco 格式 SoT**：见 [`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)（`cco_split_*` 为拆分真源；plan_tasks 仅为兼容索引）

---

## 0. 产品规则（真源）

| 要 | 不要 |
|----|------|
| 任务标题、依赖（顺序）、波次（并发）、optional/include（是否执行） | 因 scope 重叠 / 缺 scope 整包 LLM 图作废 |
| 拆分台能展示 → 确认后能开跑 | 校验当「代码协作门禁」卡主路径 |
| 数据可查询、可恢复（SQLite） | 一次用 SQLite 替换全部 JSON（先 dual-write） |

严格 collab 规则仍可在 **多 provider / 高级路径** 用警告或执行前再收紧；**规划 accept 路径默认 soften**。

---

## 1. 波次 1（已做）

### 1.1 Soften：LLM 出图软接受

- 文件：[`src/domain/plan/soften.rs`](../src/domain/plan/soften.rs)
- 调用：[`src/plan/planner/llm.rs`](../src/plan/planner/llm.rs) `validate` 前 `soften_plan_for_accept`
- 自动修：
  1. `codex` + `bg` → `print`
  2. `role=implement` 且无 `scope.paths` → 私有 `.cco-out/wp/{id}/`
  3. 多 provider → `worktree=true`
  4. **并行 implement scope 重叠 → 按任务序加 `depends_on` 串行**（修你日志里 t2∩t8 的问题）
  5. 剪掉非法 depends_on
- 仍 `validate` 失败才 fallback heuristic
- 测：`soften_serializes_parallel_scope_overlap` · `soften_fills_empty_implement_scope`

### 1.2 SQLite dual-write

- 依赖：`rusqlite`（bundled）[`Cargo.toml`](../Cargo.toml)
- 文件：[`src/state/sqlite.rs`](../src/state/sqlite.rs)
- DB：`{state_root}/cco.db`（默认 `~/.cco/cco.db`）
- 表：
  - **plan_jobs**：job 元数据 / status / cost / critic 摘要
  - **plan_tasks**：`ord` · `title` · `optional` · `include` · `depends_on` · `wave` · role/provider · prompt_preview
- 挂钩：
  - `PlanJob::save` → `try_upsert_plan_job`
  - `write_proposed` → `try_replace_plan_tasks`
- **JSON 仍为真源**；SQLite 失败只 warn，不挡规划
- 测：`dual_write_job_and_tasks`

---

## 2. 后续波次（未做 · 勾选）

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **S2** | 桌面/API 读 SQLite 列表 job/tasks（可选） | 查询比扫盘快；失败回落 JSON | ☐ |
| **S3** | 僵尸 planning 心跳/PID 收尸写 SQLite status | 无永久 planning | ☐ |
| **S4** | 默认 critic LLM 关（配置） | 少一轮 Claude | ☐ |
| **S5** | 规划两段式 / 轻量 API（中长期） | 不必等完整 CLI JSON | ☐ |
| **S6** | 可选：runs/task_state 进 SQLite | 与 plan_jobs 同库 | ☐ |

---

## 3. 与硬规则关系

- Domain 仍纯：`soften` 无 IO
- SQLite 在 `state/` 适配层，**不**进 domain
- 不旁路 `confirm_start`
- 不新开 A0–A5 阶段表

---

## 4. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 首版：soften + rusqlite dual-write plan_jobs/plan_tasks |
