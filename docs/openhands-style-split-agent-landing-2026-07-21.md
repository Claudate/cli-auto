# OpenHands 气质 · 专用拆分 Agent 完整落地计划

> 日期：2026-07-21  
> 角色：**换窗口可直接执行的实施真源**（派工 / 勾选 / 完成定义 / 文件落点）  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)  
> 模式借鉴：OpenHands Planning Mode — **专用 Planning Agent → 结构化计划 → 人确认 → 再执行**（只借思路，不换栈、不抄 IDE）  
> 关联：  
> - 格式/SQLite SoT：[`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)  
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
| 默认 heuristic 当拆分主路径 | 用户明确：拆分走模型 |
| UI `start_run` 旁路 Mode B | L1 硬规则 |
| 一次删光 plan.proposed.json | 迁移期 dual 快照可留 |
| 默认打开巡检/push | 高级关 |

---

## 2. 仓库现状（执行前必读 · 避免重复造）

### 2.1 已落地（视为 ✅，新窗口先 `cargo test` 确认）

| 能力 | 位置 | 说明 |
|------|------|------|
| **CcoSplit 类型 + soft_accept + waves + from/to PlanIR** | [`src/domain/plan/cco_split.rs`](../src/domain/plan/cco_split.rs) | schema `cco-split/v1` |
| **SQLite SoT 表 + save/load** | [`src/state/cco_split_store.rs`](../src/state/cco_split_store.rs) · [`sqlite.rs`](../src/state/sqlite.rs) | `cco_split_jobs` / `cco_split_tasks` |
| **PlanIR soften（LLM 出图不整包丢）** | [`src/domain/plan/soften.rs`](../src/domain/plan/soften.rs) · llm.rs | scope 重叠串行等 |
| **写 proposed 时 try_save_cco_split** | [`view.rs`](../src/plan/planner/view.rs) `write_proposed` 附近 | PlanIR → from_plan_ir → SQLite |
| **读 desk 优先 load_cco_split** | `view.rs` job_view / load_proposed | DTO 含 summary/wave/ord/kind |
| **exec 路径 to_plan_ir** | `view.rs` load_proposed_for_exec | confirm 可从 SoT 出 PlanIR |
| **僵尸 planning reap** | [`job.rs`](../src/plan/planner/job.rs) `try_reap_zombie_planning` | pid 死/超时 → plan_failed |
| **LLM 心跳更新 updated_at** | llm.rs | 防误杀 |

### 2.2 缺口（本计划要做完的）

| 缺口 | 说明 |
|------|------|
| **❌ ModelSplitAgent** | 仍主要是「整包 Claude CLI → PlanIR → 再转 CcoSplit」；没有 **专用拆分 Port + cco-split 提示词直出** |
| **❌ SplitAgentPort** | `src/ports/` 尚无 split_agent |
| **❌ 主路径「模型直出 cco-split」** | 应：模型输出 cco-split JSON → soft_accept → SQLite；PlanIR 仅 confirm 时 materialize |
| **❌ 拆分台/编辑双向写 SQLite SoT** | 改 optional/依赖/标题应落 cco_split_*，不只改 plan.proposed.json |
| **❌ 规划耗时 UX** | 仍可能等 CLI 数分钟；需超时人话 + 可选轻量调用 |
| **❌ supersede kill 旧 planner pid** | 现只标 cancelled |
| **❌ 文档勾选与 L2 与代码同构** | cco-split-format 文头状态过时，需回写 |

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
        │     调用：轻量 Messages/HTTP 优先；可 fallback 现有 CLI print
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

---

## 4. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> 估时为人日量级（单人熟仓）。

### 波次 0 — 对齐与验收基线（0.5d）

| ID | 任务 | 文件/动作 | 完成定义 | 状态 |
|----|------|-----------|----------|------|
| **P0-1** | 拉齐现状：跑测 + 读 cco_split / store / view / reap | `cargo test -p cco --lib cco_split soften reap dual_write planner::` | 全绿；列出现状与缺口一致 | ☐ |
| **P0-2** | 回写文档状态 | 本文 · cco-split-format · split-agent-model · docs/CLAUDE · plan/state L2 | 地图 = 地形；勾选只认本文 | ☐ |
| **P0-3** | 冻结 `cco-split/v1` 字段表（与 `CcoSplitTask` 1:1） | domain/cco_split.rs 头注释 + 本文附录 A | Agent 提示词与类型无漂移 | ☐ |

### 波次 1 — ModelSplitAgent 骨架（P0 · 模型主路径）

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P1-1** | 新增 `SplitAgentPort` | [`src/ports/split_agent.rs`](../src/ports/split_agent.rs) · ports/mod · L2 | `fn split(&self, req) -> Result<CcoSplitJob>`；无 UI | ☐ |
| **P1-2** | `ModelSplitAgent`：system 提示词（专业拆分 Agent） | 新目录建议 [`src/plan/split_agent/`](../src/plan/split_agent/) `mod.rs` · `prompt.rs` · `model.rs` | 角色：只拆不写代码；输出仅 JSON `cco-split/v1` | ☐ |
| **P1-3** | 解析器：从模型文本提取 cco-split JSON | `split_agent/parse.rs` | 支持 fence / 裸 JSON；失败返回可理解 Err | ☐ |
| **P1-4** | 调用适配：优先轻量路径，fallback CLI print | `model.rs` 可先包一层现有 Claude print（timeout 保留 reap）；预留 Messages/HTTP | 单测 fake 文本 → CcoSplitJob | ☐ |
| **P1-5** | soft_accept + recompute_waves + try_save_cco_split | agent 出口强制 | 入库 SoT；notes 写 planner.log | ☐ |

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
| **P2-1** | `plan_mode=ai`：先 ModelSplitAgent，成功则 **不强制** 再走「PlanIR 全图 + 硬 collab 否决」 | [`job.rs`](../src/plan/planner/job.rs) `run_planner` / finish | 成功路径：CcoSplit SoT 已写；job.status=planned；adapter 记 `split-agent-llm` | ☐ |
| **P2-2** | Agent 失败：可 fallback 现有 llm PlanIR→from_plan_ir（保留 soften）或明确 plan_failed | job.rs | 日志写清原因；**禁止**静默空壳四波当成功 | ☐ |
| **P2-3** | heuristic/fake：仍 from_plan_ir 写 SoT（兼容测试） | job.rs | fake/parse 测绿 | ☐ |
| **P2-4** | write_proposed：若 SoT 已是权威，JSON 作 **导出快照** 仍写 plan.proposed.json（to_plan_ir） | view.rs | 旧工具不炸；SoT 优先 | ☐ |
| **P2-5** | 集成测：fake agent 或固定 JSON fixture → start_plan_job → load_cco_split 有任务 | tests | 绿 | ☐ |

### 波次 3 — 确认 / 编辑双向 SoT

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P3-1** | update_proposed_task / remove / optional 勾选 → **先改 CcoSplit 再 save_cco_split**，再同步 proposed JSON | view.rs · task_edit | 刷新后 SoT 与 UI 一致 | ☐ |
| **P3-2** | confirm：`run_gate_ok`（至少 1 个 enabled）→ `to_plan_ir` → materialize_selected → 现有 confirm 开跑 | view.rs · app/split.rs | **仍唯一 confirm_start**；optional 门禁不回退 | ☐ |
| **P3-3** | mark_confirmed 写回 cco_split_jobs.status=confirmed + run_id | cco_split_store | 可查询 | ☐ |
| **P3-4** | sanitize「让可并行」：在 CcoSplit depends 上操作 | view/digest | 人话 toast 仍可用 | ☐ |

### 波次 4 — 体验与稳定性（模型路径不卡死）

| ID | 任务 | 文件 | 完成定义 | 状态 |
|----|------|------|----------|------|
| **P4-1** | 规划中 UI：>60s 人话「正在用 AI 拆分…可取消」；失败展示 error | jobPoll / flow 文案 | 不再假死无字 | ☐ |
| **P4-2** | supersede 时 best-effort **kill** 旧 planner pid（读 meta.json） | job.rs | 少幽灵进程 | ☐ |
| **P4-3** | 默认 critic LLM：**建议关**或失败极速 skip（配置已有） | config 默认 / 文档 | 少一轮等待 | ☐ |
| **P4-4** | 打包 `package-app.sh` + 目视：拆→台→确认（fake 执行可） | scripts | 清单过 | ☐ |

### 波次 5 — 收口

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **P5-1** | L1/L2/web 若 DTO 字段有变则更新 | 地图同构 | ☐ |
| **P5-2** | 本文任务全 ✅；归档「过渡 dual-write 仅索引」表述 | 无双真源勾选 | ☐ |
| **P5-3** | 非开发脚本：拆分台见顺序/波次/可选；确认能开跑 | 通过 | ☐ |

---

## 5. 推荐 PR 切片（换窗口可按 PR 做）

| PR | 含任务 | 标题建议 |
|----|--------|----------|
| **PR0** | P0-* | `docs: OpenHands-style split agent landing plan` |
| **PR1** | P1-1…P1-5 | `feat(split): ModelSplitAgent + cco-split schema prompt` |
| **PR2** | P2-* | `feat(planner): ai path via SplitAgent → SQLite SoT` |
| **PR3** | P3-* | `feat(split): confirm/edit round-trip on cco_split store` |
| **PR4** | P4-* + P5-* | `fix(planner): planning UX + kill superseded + package` |

---

## 6. 文件触达总表

| 区域 | 路径 |
|------|------|
| 类型（已有，可微改） | `src/domain/plan/cco_split.rs` |
| 存储（已有，可扩 API） | `src/state/cco_split_store.rs` · `sqlite.rs` |
| **新建** Port | `src/ports/split_agent.rs` |
| **新建** Agent | `src/plan/split_agent/{mod,prompt,parse,model}.rs` |
| 编排 | `src/plan/planner/job.rs` · `llm.rs` · `view.rs` |
| 用例 | `src/app/split.rs`（尽量薄） |
| 桌面 | `web/js/features/split/*` · 必要时 jobPoll 文案 |
| 文档 | 本文 · cco-split-format · split-agent-model · L2 |

---

## 7. 成功标准

| # | 标准 |
|---|------|
| **S1** | 主路径拆分由 **ModelSplitAgent** 产出 **cco-split**，写入 **SQLite SoT** |
| **S2** | 拆分台展示完整：顺序、波次、是否执行、标题/说明/完成标志 |
| **S3** | 人确认前不跑业务 worker；`confirm_start` 唯一开跑 |
| **S4** | 不再因 scope 重叠等 collab 规则 **整图丢弃** 模型结果（soft_accept） |
| **S5** | 僵尸 planning：进程死/超时 → `plan_failed`，UI 离开转圈 |
| **S6** | 执行侧仍用现有 Worker；拆分 Agent 不替代执行 |
| **S7** | 测试：domain cco_split · store · agent parse · planner 集成 绿 |

---

## 8. 风险

| 风险 | 缓解 |
|------|------|
| Agent 仍走慢 CLI | P1-4 预留 HTTP；P4 超时人话；后续默认轻量调用 |
| 双写 JSON/SQLite 不一致 | 读路径 SoT 优先；写路径先 SoT 再快照 JSON |
| 与旧 desk 字段不兼容 | PlanTaskView 已扩 summary/wave；保持 id 稳定 |
| 提示词漂移 | P0-3 类型与 prompt 同文件注释 + 单测 fixture |

---

## 9. 换窗口启动指令（复制即用）

```text
按 docs/openhands-style-split-agent-landing-2026-07-21.md 实施。
先 P0-1 跑测确认已有 cco_split/store/reap；不要重做已 ✅ 的类型与表。
主目标：ModelSplitAgent（模型+提示词→cco-split/v1）→ SQLite SoT → 拆分台 → confirm_start。
禁止：heuristic 当主路径、旁路 confirm、换框架、引入 LangGraph。
完成后 package-app 并更新本文勾选。
```

---

## 附录 A — `cco-split/v1` 任务参数（代码识别）

与 `CcoSplitTask` 对齐（以源码为准，改类型须改本文）：

| 字段 | 用途 |
|------|------|
| `id` | 稳定 id |
| `title` | 列表标题 |
| `summary` | 卡面一句话 |
| `body` | 执行说明（→ worker prompt） |
| `depends_on` | **顺序** |
| `wave` | **并发波**（可 recompute） |
| `optional` / `enabled` | **是否执行** |
| `kind` | do / check / system |
| `done_when` | 完成标志 |
| `plan_ref` | 对照计划 |
| `provider` / `role` / `scope_paths` | 高级可选，默认空 |

Job：`max_parallel` · `status` · `source` · `title` · project/plan_path。

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
