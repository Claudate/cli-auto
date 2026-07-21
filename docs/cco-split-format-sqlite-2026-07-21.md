# cco 独立拆分格式 + SQLite 存储（产品真源）

> 日期：2026-07-21  
> 角色：**拆分数据模型与存储真源**（纠正「只存 ord/wave/optional」的窄理解）  
> 状态：**C1–C7 ✅**（CcoSplit 类型 + SQLite SoT + confirm 物化 + desk DTO + fast 默认 + 僵尸杀 pid）  
> 后续 **ModelSplitAgent 主路径** 见 [`openhands-style-split-agent-landing-2026-07-21.md`](./openhands-style-split-agent-landing-2026-07-21.md) · 勿把 dual-write 当终态

[PROTOCOL]: 勾选只认本文 §4。`split-soft-sqlite` 波次 1 = 过渡；**不以 PlanIR 双写当最终方案**。

---

## 0. 听进去的产品意图（完整）

用户原意（归纳，不缩水）：

1. **拆分结果后面仍给 AI 跑**（confirm 后走 Worker）。  
2. 拆分阶段要区分的信息 **至少**包括但不限于：  
   - **顺序**（谁先谁后 / depends）  
   - **并发**（谁可同波 / max_parallel）  
   - **是否执行**（optional · include / 用户勾选）  
   - 以及软件展示与后续执行所需的 **完整合理字段**（标题、说明、完成标志、状态、计划来源…）  
3. **不要拆分太严格**：校验不能把「能显示、能勾选、能开跑」的图整包扔掉。  
4. **拆分用 cco 自己的独立格式**（不是把 Claude 吐的 PlanIR / 协作 scope 规则当唯一真源）。  
5. **拆分后的任务进 SQLite**，由 **cco 存储**（SoT），不是仅给 JSON 做索引表。

---

## 1. 和已做工作的差距（诚实）

| 已做（过渡） | 用户要的 |
|--------------|----------|
| `soften_plan_for_accept`：LLM PlanIR 软修再 validate | ✅ 方向对（少整图丢弃） |
| SQLite **dual-write** `plan_jobs` / `plan_tasks` 子集字段 | ❌ 仍是 PlanIR 镜像索引 |
| JSON `job.json` + `plan.proposed.json` 仍是真源 | ❌ 要 cco 格式 + SQLite 为拆分 SoT |
| 字段偏 `ord/wave/optional/include` | ❌ 字段不够、格式不独立 |

**结论：波次 1 没有完成「独立格式 + cco SQLite 存储」。**

---

## 2. 若继续「现在这种方式」拆分，还会卡吗？

**会，软校验解决不了卡顿。**

| 原因 | 软 accept 是否解决 | 说明 |
|------|-------------------|------|
| 等 Claude CLI 出完整规划（常 3–6 分钟，上限 ~600s） | **否** | 卡在 planning 转圈的主因 |
| CLI 死掉 / 无心跳 → 僵尸 `planning` | **否** | 需收尸；supersede 只标 cancelled 不杀进程 |
| LLM 出图后 collab 硬拒 → 整图 heuristic | **部分** | soften 减少「白等再丢图」，但已等的分钟数不退回 |
| critic 第二跳 Claude | **否** | 设置开着会再加一轮 |
| 桌面绑错旧 job | **否** | 状态绑定问题 |

所以：

- **只靠 soften + dual-write** → 仍可能 **等很久 / 假转圈**。  
- 要「不卡」必须改路径：**本地/快路径出 cco 图** 或 **轻量 API 分段出图**，CLI 不当默认 planner；并 **僵尸收尸**。

---

## 3. 目标架构（cco 独立格式）

```text
计划 md
  → SplitProducer（heuristic | 轻量 LLM | 人工）
  → CcoSplitDoc（独立格式，内存）
  → SQLite cco.db  【拆分 SoT】
  → 桌面拆分台读 SQLite（顺序/波次/是否执行/说明…）
  → 用户确认
  → materialize → PlanIR / Run（仅执行边界需要时再生成）
  → Worker 跑
```

### 3.1 `CcoSplitDoc` / `CcoSplitTask`（建议字段 · 完整展示 + 可跑）

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
| `done_when` | 怎样算做完（展示 + 巡检） |
| `plan_ref` | 对照计划章节/ID |
| `kind` | do / check / system |
| `status` | pending / …（确认后同步 run） |
| `provider` / `role` / `scope_paths` | **高级可选**，默认空，不挡主路径 |
| `meta_json` | 扩展袋（避免再改表） |

**原则**：主路径只依赖 title / body / depends / wave / enabled / optional；高级字段可空。  
**materialize 到 PlanIR** 只在 `confirm_start`：prompt←body，include←enabled，optional 保留。

### 3.2 校验分层

| 层 | 规则 | 失败时 |
|----|------|--------|
| **Split accept（软）** | 有任务、id 不空、无环依赖、title/body 可读 | 自动剪边 / 补默认；**不整图丢弃** |
| **Run gate（硬）** | 开跑前：至少 1 个 enabled；依赖指向存在 | 拦开跑，人话提示 |
| **Collab 高级** | scope 重叠、多 provider worktree | **警告或自动串行**，默认不否决整图 |

### 3.3 与现 PlanIR 关系

- **拆分台 / SQLite**：只认 `CcoSplit*`  
- **Worker / Scheduler**：confirm 时 **一次性** `CcoSplit → PlanIR`  
- 旧 `plan.proposed.json`：迁移期可导出快照，不再当唯一真源  

---

## 4. 实施勾选

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **C0** | 本文为拆分存储真源；纠正 soft-sqlite 文档口径 | 文档互链 | ✅ 本文件 |
| **C1** | 定义 `CcoSplitJob` / `CcoSplitTask` Rust 类型（domain 或 state） | 无 IO 类型 + 测 | ✅ `domain/plan/cco_split/` |
| **C2** | SQLite 表 `cco_split_jobs` / `cco_split_tasks` 为 SoT 写入 | 拆完必进库；可读回 desk DTO | ✅ `state/cco_split_store` |
| **C3** | Producer：heuristic / 现 LLM 结果 → **先转 CcoSplit 再入库** | 不经「硬 collab 否决整图」 | ✅ `write_proposed` → SoT |
| **C4** | `confirm_start`：从 SQLite 读 → materialize PlanIR → run | 唯一开跑不变 | ✅ `load_proposed` / run_gate |
| **C5** | 桌面拆分台读 SQLite DTO | 顺序/波次/是否执行/说明完整 | ✅ job_view 优先 SoT |
| **C6** | 规划不卡：默认快路径（heuristic 或 cap）+ 僵尸收尸 | planning 不再假转圈数分钟无结果 | ✅ `fast` 默认 + kill pid |
| **C7** | 迁移：旧 plan.proposed 可导入 CcoSplit 一次 | 兼容 | ✅ load 时 import |

**推荐序**：C1→C2→C3→C4→C5 并行 C6。

---

## 5. 明确不听成什么

- ❌ 只存四个字段 ord/wave/optional/include  
- ❌ SQLite 仅 dual-write 镜像 PlanIR 就算完事  
- ❌ 继续以 Claude CLI 完整 JSON + 硬 validate 为唯一拆分路径还声称「不卡」  

---

## 6. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 用户纠正：独立 cco 格式 + SQLite SoT；补「现路径仍会卡」 |
| 2026-07-21 | C1–C7 落地：domain cco_split · SQLite SoT · confirm 物化 · 桌面 DTO · fast 默认 · 杀僵尸 pid |
