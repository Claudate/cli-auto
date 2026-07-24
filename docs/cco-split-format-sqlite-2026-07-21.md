# cco 独立拆分格式 + SQLite 存储（存储真源）

> 日期：2026-07-21 · 修订 2026-07-22  
> 角色：**拆分数据模型与存储真源**（`CcoSplit*` · `cco_split_*` 表 · confirm 物化）  
> 状态：**C1–C7 ✅**（类型 + SQLite SoT + confirm 物化 + desk DTO + fast 默认 + 僵尸杀 pid）  
> 产品行为短规则：[`split-product-rules.md`](./split-product-rules.md)  
> ModelSplitAgent 主路径史：[`archive/openhands-style-split-agent-landing-2026-07-21.md`](./archive/openhands-style-split-agent-landing-2026-07-21.md)

[PROTOCOL]: 勾选只认本文 §3（C1–C7 ✅）与文末 §5（残余债 S2–S6）。  
**不以** [`archive/split-soft-sqlite-2026-07-21.md`](./archive/split-soft-sqlite-2026-07-21.md) 为开项真源；**不以 PlanIR dual-write 当最终方案**。S2–S6 **唯一**在本文文末。

---

## 0. 产品意图（存储侧）

1. 拆分结果 confirm 后仍给 AI Worker 跑。  
2. 拆分阶段至少保留：顺序（depends）· 并发（wave / max_parallel）· 是否执行（optional / enabled）· 标题/说明/完成标志等展示与执行字段。  
3. 校验不能把「能显示、能勾选、能开跑」的图整包扔掉（soft_accept）。  
4. **cco 独立格式** `cco-split/v1`，不是把 PlanIR / 协作 scope 规则当唯一真源。  
5. 拆分任务进 **SQLite SoT**（`cco_split_jobs` / `cco_split_tasks`），不是仅 JSON 索引。

过渡期 soft + PlanIR dual-write 已归档，见 archive soft-sqlite；当前 SoT = `cco_split_*`。

---

## 1. 目标架构

```text
计划 md
  → SplitProducer（ModelSplitAgent | heuristic | 人工）
  → CcoSplitDoc（cco-split/v1，内存）
  → SQLite cco.db  【拆分 SoT】
  → 桌面拆分台读 SQLite
  → 用户确认
  → materialize → PlanIR / Run（仅执行边界）
  → Worker 跑
```

### 1.1 `CcoSplitJob` / `CcoSplitTask`（字段）

**Job 级 `cco_split_jobs`**

| 字段 | 用途 |
|------|------|
| `job_id` | 主键 |
| `project` | 项目路径 |
| `plan_path` | 源计划 md |
| `status` | drafting / ready / confirmed / failed / cancelled |
| `title` | 展示名 |
| `max_parallel` | 并发上限 |
| `source` | heuristic \| llm \| merge \| manual |
| `error` | 失败原因人话 |
| `created_at` / `updated_at` | |
| `run_id` | 确认后关联 |

**Task 级 `cco_split_tasks`（拆分 SoT）**

| 字段 | 用途 |
|------|------|
| `job_id` + `task_id` | 主键 |
| `ord` | 列表顺序（展示） |
| `title` | 步骤标题 |
| `summary` | 一句话（卡面） |
| `body` | 完整说明（给 AI 跑的主文案） |
| `depends_on` | JSON 数组，顺序约束 |
| `wave` | 并发波（可由 depends 算，可缓存） |
| `enabled` | 是否执行（用户勾选；映射 include） |
| `optional` | 是否可选步骤 |
| `done_when` | 怎样算做完（**仅**展示 + 巡检叙述；**禁止**当 shell） |
| `verify_cmd` | 可选一行 shell（host 软验收）；落地勾选见 [`human-status-verify-dual-landing-2026-07-24.md`](./human-status-verify-dual-landing-2026-07-24.md) **H2**（**不**并入文末 S2–S6） |
| `plan_ref` | 对照计划章节/ID |
| `kind` | do / check / system |
| `status` | pending / …（确认后同步 run） |
| `provider` / `role` / `scope_paths` | **高级可选**，默认空，不挡主路径 |
| `meta_json` | 扩展袋（避免再改表） |

**原则**：主路径只依赖 title / body / depends / wave / enabled / optional；高级字段可空。  
**materialize 到 PlanIR** 只在 `confirm_start`：prompt←body，include←enabled，optional 保留；`scope_paths` → TaskScope（执行路由）；**`done_when` → 人话/叙述（不进 shell）**；**`verify_cmd` → 执行层 shell 验收**（过渡期旧 `TaskIR.acceptance` 兼容，见 human-status-verify-dual H0–H2）。

### 1.2 校验分层

| 层 | 规则 | 失败时 |
|----|------|--------|
| **Split accept（软）** | 有任务、id 不空、无环依赖、title/body 可读 | 自动剪边 / 补默认；**不整图丢弃** |
| **Run gate（硬）** | 开跑前：至少 1 个 enabled；依赖指向存在 | 拦开跑，人话提示 |
| **Collab 高级** | scope 重叠、多 provider worktree | **警告或自动串行**，默认不否决整图 |

### 1.3 与 PlanIR 关系

- **拆分台 / SQLite**：只认 `CcoSplit*`  
- **Worker / Scheduler**：confirm 时 **一次性** `CcoSplit → PlanIR`  
- 旧 `plan.proposed.json`：迁移期可导出快照，不再当唯一真源  

---

## 2. 明确不要

- ❌ 只存四个字段 ord/wave/optional/include  
- ❌ SQLite 仅 dual-write 镜像 PlanIR 就算完事（过渡已归档，非终态）  
- ❌ 以 Claude CLI 完整 JSON + 硬 validate 为唯一拆分路径还声称「不卡」  
- ❌ 在 soft archive 与本文各维护一份 S2–S6 开项表  

---

## 3. 实施勾选（C1–C7 · 已全部 ✅）

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **C0** | 本文为拆分存储真源 | 文档互链 | ✅ |
| **C1** | `CcoSplitJob` / `CcoSplitTask` Rust 类型 | 无 IO 类型 + 测 | ✅ `domain/plan/cco_split/` |
| **C2** | SQLite 表 `cco_split_*` 为 SoT 写入 | 拆完必进库；可读回 desk DTO | ✅ `state/cco_split_store` |
| **C3** | Producer → 先转 CcoSplit 再入库 | 不经硬 collab 否决整图 | ✅ `write_proposed` → SoT |
| **C4** | `confirm_start`：SQLite → materialize PlanIR → run | 唯一开跑不变 | ✅ `load_proposed` / run_gate |
| **C5** | 桌面拆分台读 SQLite DTO | 顺序/波次/是否执行/说明完整 | ✅ job_view 优先 SoT |
| **C6** | 规划不卡：快路径能力 + 僵尸收尸 | planning 不再假转圈 | ✅ kill pid；**产品默认 ai** 见短规则 |
| **C7** | 旧 plan.proposed 可导入 CcoSplit | 兼容 | ✅ load 时 import |

说明：C6 的「快路径」是能力与兜底；**产品默认 `plan_mode=ai`**（见 [`split-product-rules.md`](./split-product-rules.md)），`fast` 仅高级/显式。

---

## 4. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 独立 cco 格式 + SQLite SoT；C1–C7 落地 |
| 2026-07-22 | soft-sqlite 归档；并入残余债 S2–S6（唯一勾选落点） |
| 2026-07-22 | C1 文档合并：删过时 dual-write 差距控诉；链短规则 + archive；角色=存储真源 |
| 2026-07-24 | §1.1 增 `verify_cmd` 字段说明；双层验收落地勾选 **只认** human-status-verify-dual 计划 H0–H3 |

---

## 5. 残余债 S2–S6

> 来源：原 `docs/split-soft-sqlite-2026-07-21.md` §2（已 `git mv` → [`archive/split-soft-sqlite-2026-07-21.md`](./archive/split-soft-sqlite-2026-07-21.md)）。  
> **本表为 S2–S6 唯一勾选真源**；archive 中 soft 文不得再当开项真源。中间摘录：`.cco-out/docs-cleanup/S2-S6-EXTRACT.md`。

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **S2** | 桌面/API 读 SQLite 列表 job/tasks（可选） | 查询比扫盘快；失败回落 JSON | ☐ |
| **S3** | 僵尸 planning 心跳/PID 收尸写 SQLite status | 无永久 planning | ☐ |
| **S4** | 默认 critic LLM 关（配置） | 少一轮 Claude | ☐ |
| **S5** | 规划两段式 / 轻量 API（中长期） | 不必等完整 CLI JSON | ☐ |
| **S6** | 可选：runs/task_state 进 SQLite | 与 plan_jobs 同库 | ☐ |

说明：C1–C7 已落地 `cco_split_*` SoT + 杀僵尸 pid；上表为 soft 波次遗留的**可选/中长期**债（S3 与 C6 部分重叠、S4 与 config `planner_critic_enabled=false` 可能已满足时可对照实现再勾，**勿**在 archive 或其它文再开第二份表）。
