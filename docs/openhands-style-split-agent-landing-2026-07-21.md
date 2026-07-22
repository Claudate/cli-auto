# OpenHands 气质 · 专用拆分 Agent 完整落地计划

> 日期：2026-07-21  
> 角色：**换窗口可直接执行的实施真源**（派工 / 勾选 / 完成定义 / 文件落点）  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)  
> 模式借鉴：OpenHands Planning Mode — **专用 Planning Agent → 结构化计划 → 人确认 → 再执行**（只借思路，不换栈、不抄 IDE）  
> 关联：  
> - 格式/SQLite SoT：[`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)（**C1–C7 ✅**）  
> - Agent 放哪：[`split-agent-model-path-2026-07-21.md`](./split-agent-model-path-2026-07-21.md)  
> - 软校验过渡：[`split-soft-sqlite-2026-07-21.md`](./split-soft-sqlite-2026-07-21.md)  
> 硬契约：**唯一业务开跑 `confirm_start`** · 拆分走**模型**（非 heuristic 主路径）· **cco-split 参数 + SQLite SoT** · 不重开 A0–A5  

[PROTOCOL]: **勾选只认本文 §4 任务表**。改边界先更新本文与 L2，再改代码。每波结束：相关 `cargo test` + 桌面「拆成步骤→拆分台→确认」目视。

---

## 0. 一句话目标

```text
计划 md
  →【ModelSplitAgent · 提示词 + cco-split/v1 结构化输出】
  → CcoSplitJob/Task 写入 SQLite（cco 存储真源）
  → 拆分台：顺序 / 波次 / 是否执行 / 说明… 完整展示、可改
  → 人点「确认并开始」
  → materialize → PlanIR → Scheduler → 执行 Worker（另一路 AI）
```

对标 OpenHands：**Plan Mode ≠ Code Mode**；cco 已有 confirm 闸，补的是 **专职拆分 Agent + 独立格式 SoT**。

---

## 1. 非目标

| 不做 | 原因 |
|------|------|
| 换 React / 引入 LangGraph·CrewAI 运行时 | 栈与 PRODUCT 禁止 |
| 像素抄 OpenHands / 做成 IDE | PRODUCT |
| 默认 heuristic 当拆分主路径 | 用户明确：拆分走模型（**2026-07-22 起桌面默认 ai**；fast 仅高级/显式） |
| UI `start_run` 旁路 Mode B | L1 硬规则 |
| 一次删光 plan.proposed.json | 迁移期 dual 快照可留 |
| 默认打开巡检/push | 高级关 |
| **重做 C1–C7 类型/表/confirm 物化** | 已在 uncommitted 工作树落地 |

---

## 2. 仓库现状（2026-07-21 审计 · 避免重复造）

### 2.1 已落地（✅ · 勿重做）

| 能力 | 位置 | 说明 |
|------|------|------|
| **CcoSplit 类型 + soft_accept + waves + from/to PlanIR** | [`src/domain/plan/cco_split/`](../src/domain/plan/cco_split/)（types/accept/convert） | schema `cco-split/v1`；单文件路径文过时 |
| **SQLite SoT 表 + save/load/mark_confirmed** | [`cco_split_store.rs`](../src/state/cco_split_store.rs) · [`sqlite.rs`](../src/state/sqlite.rs) | `cco_split_jobs` / `cco_split_tasks` |
| **PlanIR soften** | [`soften.rs`](../src/domain/plan/soften.rs) · llm.rs | scope 重叠串行等 |
| **写 proposed 时 try_save_cco_split** | [`view.rs`](../src/plan/planner/view.rs) `write_proposed` | PlanIR → from_plan_ir → SQLite |
| **读 desk 优先 load_cco_split** | view.rs job_view / load_proposed | DTO 含 summary/wave/ord/kind |
| **exec 路径 run_gate + to_plan_ir** | load_proposed_for_exec | confirm 可从 SoT 出 PlanIR |
| **mark_confirmed → cco_split status** | view.rs + `try_mark_cco_split_confirmed` | P3-3 ✅ |
| **编辑经 write_proposed 回写 SoT** | update/remove → write_proposed | P3-1 实质 ✅（先 PlanIR 再 SoT；读仍 SoT 优先） |
| **僵尸 planning reap + kill pid** | job.rs `try_reap_zombie_planning` · `kill_planner_pid` | 5min hard timeout |
| **supersede kill 旧 planner pid** | `supersede_planning_jobs` | P4-2 ✅ |
| **LLM 心跳 updated_at** | llm.rs | 防误杀 |
| **桌面默认 plan_mode=ai**（Q0 ✅ · 2026-07-22） | index.html · jobPoll.js | **原**默认 fast 已废；fast=高级/显式 |
| **critic LLM 默认关** | config `planner_critic_enabled=false` | P4-3 ✅ |
| **规划 UX 已等待秒数** | flow.js `flowPlanningSub` | P4-1 大半 ✅（可再强化 >60s 取消提示） |

### 2.2 缺口（本轮要做）

| 缺口 | 说明 |
|------|------|
| **✅ ModelSplitAgent** | `plan/split_agent` · fixture/Messages/CLI |
| **✅ SplitAgentPort** | `ports/split_agent.rs` |
| **✅ plan_mode=ai 直出 cco-split** | soft_accept → SQLite → PlanIR 快照；失败 fallback legacy LLM/heuristic |
| **✅ P3-4 sanitize SoT** | `sanitize_cco_split_deps` + `planner/sanitize.rs` |
| **✅ P4-1/P4-4/P5-3** | ≥60s 取消文案 · package · fast 确认集成测 |

### 2.3 优化后的执行序（跳过已做）

```text
P0 对齐（测+文档）→ P1 Agent 骨架 → P2 ai 主路径接入 → P5 收口
跳过重做：C1–C7 · P3-1/2/3 核心 · P4-2 · P4-3
可后置：P3-4 原生 Cco depends sanitize · P4-1 文案微调 · P4-4 打包目视
```

---

## 3. 目标架构（终态）

```text
Presentation          web/CLI 「拆成步骤」
        │
        ▼
Application           app::split::start_job / confirm
        │
        ▼
SplitAgentPort        ports/split_agent.rs
        │
        ├─ ModelSplitAgent   plan/split_agent/model.rs
        │     提示词 + schema 强制 cco-split/v1
        │     调用：fixture/env → Messages HTTP（有 key）→ Claude CLI print
        │
        ▼
soft_accept_split     domain
        │
        ▼
cco_split_store       state · SQLite SoT
        │
        ▼
PlanJobView           桌面拆分台（读 SoT）
        │ 用户确认
        ▼
to_plan_ir + validate_run_gate → confirm_start → Scheduler/Worker
```

**原则**

1. 拆分 Agent **不写业务代码**（OpenHands Plan Mode 气质）。  
2. 执行 Worker **不负责拆分**。  
3. 代码识别靠 **固定参数**（depends_on / wave / enabled / optional / kind / body / done_when / plan_ref）。  
4. 校验分层：**soft_accept（拆分）** vs **run_gate（开跑）**；禁止 collab scope 硬拒整图（已有 soften，Agent 路径默认不产出强 scope）。  
5. **桌面默认 ai**（2026-07-22 Q0）；**fast** 仅高级/显式本地不卡；CLI 与桌面一致默认 ModelSplitAgent。

---

## 4. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成 · ⏭ 跳过（已由 C*/他处覆盖）  
> 估时为人日量级（单人熟仓）。

### 波次 0 — 对齐与验收基线

| ID | 任务 | 文件/动作 | 完成定义 | 状态 |
|----|------|-----------|----------|------|
| **P0-1** | 拉齐现状：跑测 + 读 cco_split / store / view / reap | `cargo test -p cco --lib cco_split soft_accept dual reap` | 全绿；列出现状与缺口一致 | ✅ 2026-07-21 审计 |
| **P0-2** | 回写文档状态 | 本文 · cco-split-format · split-agent-model · L2 | 地图 = 地形；勾选只认本文 | ✅ 本修订 |
| **P0-3** | 冻结 `cco-split/v1` 字段表（与 `CcoSplitTask` 1:1） | domain/cco_split 头 + 附录 A | Agent 提示词与类型无漂移 | ✅ 附录 A + types.rs |

### 波次 1 — ModelSplitAgent 骨架（P0 · 模型主路径）

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P1-1** | 新增 `SplitAgentPort` | [`src/ports/split_agent.rs`](../src/ports/split_agent.rs) · ports/mod · L2 | `fn split(&self, req) -> Result<CcoSplitJob>`；无 UI | ✅ |
| **P1-2** | `ModelSplitAgent`：system 提示词（专业拆分 Agent） | [`src/plan/split_agent/`](../src/plan/split_agent/) `mod.rs` · `prompt.rs` · `model.rs` | 角色：只拆不写代码；输出仅 JSON `cco-split/v1` | ✅ |
| **P1-3** | 解析器：从模型文本提取 cco-split JSON | `split_agent/parse.rs` | 支持 fence / 裸 JSON；失败返回可理解 Err | ✅ |
| **P1-4** | 调用适配：fixture → Messages HTTP → CLI print | `model.rs`；单测 fixture 文本 → CcoSplitJob | 无活网依赖单测绿 | ✅ |
| **P1-5** | soft_accept + recompute_waves + try_save_cco_split | agent/job 出口强制 | 入库 SoT；notes 写 planner.log | ✅ |

**P1 提示词要点（写入 prompt.rs）**

```text
你是 cco 的计划拆分 Agent（Plan Mode）。
输入：Markdown 计划。输出：仅一个 JSON，schema=cco-split/v1。
字段：tasks[].id,title,summary,body,depends_on,optional,enabled,kind,done_when,plan_ref,can_parallel
规则：
- 一步一个可完成结果；title 待办风，非目录名
- depends_on 仅真实先后；无依赖 []；可并行则 can_parallel=true
- max_parallel 为同时路数上限，禁止为凑波次加边
- optional 步骤 enabled 默认 false
- kind: do|check|system
- 禁止把修订历史/非目标/PROTOCOL 拆成任务
- 禁止写业务代码；body 是给后续执行 AI 的说明
```

### 波次 2 — 接入 start_plan_job 主路径

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P2-1** | `plan_mode=ai`：先 ModelSplitAgent，成功则 **不强制** 再走「PlanIR 全图 + 硬 collab 否决」 | [`job.rs`](../src/plan/planner/job.rs) `run_planner` | 成功：CcoSplit SoT 已写；job.status=planned；adapter `split-agent-llm` / `cco-split/llm` | ✅ |
| **P2-2** | Agent 失败：fallback 现有 llm PlanIR→from_plan_ir（保留 soften）或 heuristic | job.rs | 日志写清原因；**禁止**静默空壳四波当成功（既有 diagnose 保留） | ✅ |
| **P2-3** | heuristic/fake/fast：仍 from_plan_ir 写 SoT（兼容测试） | write_proposed 既有 | fake/parse/fast 测绿 | ⏭ write_proposed 已覆盖 |
| **P2-4** | write_proposed：SoT 权威，JSON 作导出快照 | view.rs | 旧工具不炸；SoT 优先读 | ⏭ 已落地 |
| **P2-5** | 集成测：fixture JSON → start_plan_job(ai) → load_cco_split 有任务 | tests / lib | 绿 | ✅ |

### 波次 3 — 确认 / 编辑双向 SoT

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P3-1** | update/remove/optional → 回写 SoT | view.rs | 刷新后 SoT 与 UI 一致 | ⏭ 经 write_proposed ✅ |
| **P3-2** | confirm：run_gate → to_plan_ir → materialize → confirm_start | view/app | **仍唯一 confirm_start** | ⏭ ✅ |
| **P3-3** | mark_confirmed → cco_split_jobs.confirmed + run_id | store | 可查询 | ⏭ ✅ |
| **P3-4** | sanitize 在 CcoSplit depends 上操作 | `domain/.../accept.rs` + `planner/sanitize.rs` | SoT 优先；view 只委托 | ✅ |

### 波次 4 — 体验与稳定性

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P4-1** | 规划中 UI 等待秒数 + 可取消语义 | flow.js `flowPlanningSub` · `btn-cancel-planning` | ≥60s 提示可取消 | ✅ |
| **P4-2** | supersede kill 旧 planner pid | job.rs | 少幽灵进程 | ⏭ ✅ |
| **P4-3** | critic LLM 默认关 | config | 少一轮等待 | ⏭ ✅ |
| **P4-4** | package-app + 目视 | scripts | 清单过 | ✅ `dist/CCO.app` 2026-07-21 |

### 波次 5 — 收口

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **P5-1** | L1/L2 若字段有变则更新 | 地图同构 | ✅ ports/plan L2 |
| **P5-2** | 本文任务勾选回写 | 无双真源勾选 | ✅ |
| **P5-3** | 非开发脚本：拆分台见顺序/波次/可选；确认能开跑 | `fast_path_desk_fields_and_confirm` 绿 | ✅ lib 门禁 |

---

## 5. 推荐 PR 切片

| PR | 含任务 | 标题建议 |
|----|--------|----------|
| **PR0** | P0-* | `docs: OpenHands-style split agent landing (truth table)` |
| **PR1** | P1-1…P1-5 + P2-* | `feat(split): ModelSplitAgent → cco-split SoT on plan_mode=ai` |
| **PR2** | P4-4 + P5 | `chore(split): package + landing closeout` |

（原 PR3/PR4 中 P3/P4 多数已由 C1–C7 覆盖，不再单开。）

---

## 6. 文件触达总表

| 区域 | 路径 |
|------|------|
| 类型（已有） | `src/domain/plan/cco_split/*` |
| 存储（已有） | `src/state/cco_split_store.rs` · `sqlite.rs` |
| **新建** Port | `src/ports/split_agent.rs` |
| **新建** Agent | `src/plan/split_agent/{mod,prompt,parse,model}.rs` |
| 编排 | `src/plan/planner/job.rs`（薄接线） |
| 用例 | `src/app/split.rs`（尽量不动） |
| 文档 | 本文 · L2 ports/plan/state |

---

## 7. 成功标准

| # | 标准 |
|---|------|
| **S1** | `plan_mode=ai` 时拆分由 **ModelSplitAgent** 优先产出 **cco-split**，写入 **SQLite SoT** |
| **S2** | 拆分台展示完整：顺序、波次、是否执行、标题/说明/完成标志 |
| **S3** | 人确认前不跑业务 worker；`confirm_start` 唯一开跑 |
| **S4** | 不再因 scope 重叠等 collab 规则 **整图丢弃** 模型结果（soft_accept） |
| **S5** | 僵尸 planning：进程死/超时 → `plan_failed`，UI 离开转圈 |
| **S6** | 执行侧仍用现有 Worker；拆分 Agent 不替代执行 |
| **S7** | 测试：domain cco_split · store · agent parse · planner fixture 集成 绿 |
| **S8** | 桌面默认 **ai**；**fast** 本地拆分仍可用（高级/显式，不卡主路径） |

---

## 8. 风险

| 风险 | 缓解 |
|------|------|
| Agent 仍走慢 CLI | Messages HTTP 优先（有 key）；fixture 测；5min reap；默认桌面 ai · fast 兜底 |
| 双写 JSON/SQLite 不一致 | 读路径 SoT 优先；写路径 write_proposed 再落 SoT |
| 与旧 desk 字段不兼容 | PlanTaskView 已扩 summary/wave；保持 id 稳定 |
| 提示词漂移 | 附录 A + parse fixture 单测 |
| `view.rs`/`job.rs`/`llm.rs` 已超硬上限 | **禁止再堆**；Agent 新目录；job 只加薄委托 |

---

## 9. 换窗口启动指令（复制即用）

```text
按 docs/openhands-style-split-agent-landing-2026-07-21.md 实施。
已 ✅：C1–C7 SoT、reap/kill supersede、confirm/edit dual-write、critic 默认关、桌面 **ai**（Q0）。
本轮只做：P1 SplitAgentPort+ModelSplitAgent + P2 ai 路径接入 + 测 + 勾选回写。
禁止：heuristic 当 ai 主路径、旁路 confirm、换框架、重做类型/表、往 view.rs 巨石堆逻辑。
```

---

## 附录 A — `cco-split/v1` 任务参数（代码识别）

与 `CcoSplitTask` / `CcoSplitJob` 对齐（以 [`types.rs`](../src/domain/plan/cco_split/types.rs) 为准）：

| 字段 | 用途 |
|------|------|
| `id` / `task_id` | 稳定 id（Agent JSON 可用 `id`） |
| `title` | 列表标题 |
| `summary` | 卡面一句话 |
| `body` | 执行说明（→ worker prompt） |
| `depends_on` | **顺序** |
| `wave` | **并发波**（soft_accept recompute） |
| `optional` / `enabled` | **是否执行**（optional 默认 enabled=false） |
| `kind` | do / check / system |
| `done_when` | 完成标志 |
| `plan_ref` | 对照计划 |
| `can_parallel` | Agent 提示用；落 meta 或仅影响 depends 习惯 |
| `provider` / `role` / `scope_paths` | 高级可选，默认空 |

Job：`schema`=`cco-split/v1` · `max_parallel` · `status` · `source` · `title` · project/plan_path。

---

## 附录 B — OpenHands 对照（执行时别跑偏）

| OpenHands | cco |
|-----------|-----|
| Planning Agent | ModelSplitAgent |
| PLAN.md | cco-split + 拆分台（md 可选写回） |
| 人批准 | 拆分台确认并开始 |
| Code Mode | Worker / Scheduler |
| 探索全仓库 | **输入是计划文档**，不要默认全库探索 |

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 首版完整落地计划：现状盘点 · P0–P5 · PR 切片 · 换窗口指令 · 附录 schema |
| 2026-07-21 | **审计优化**：C1–C7/P3/P4 大半已落地；缺口收敛为 ModelSplitAgent+P2 接线；勾选表回写；禁止重做 SoT |
| 2026-07-21 | **P1+P2 落地**：SplitAgentPort · ModelSplitAgent · parse/prompt · ai 主路径 · fixture 集成测 · L2 同构 |
| 2026-07-21 | **P3-4/P4-1/P4-4/P5-3 收口**：sanitize SoT · 规划≥60s 文案 · package-app · fast 确认门禁测 |
