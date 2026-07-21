# cco 架构重构计划（大改 · 高内聚低耦合）

> 状态：**A0–A5 ✅**（A5-0…A5-4 勾满 · **A5-5 可选 ☐ 本轮不做** · 评估见 [`a5-5-workspace-crates-eval-2026-07-21.md`](./a5-5-workspace-crates-eval-2026-07-21.md)）  
> 日期：2026-07-20（A0–A4 2026-07-20…21 · **A5-0…A5-3 / A5-2\* 2026-07-21** · **A5-4 GEB 2026-07-21**）  
> 角色：**系统架构真源（本轮大改 · 已收口）**——UI · 计划拆分 · 多 CLI 执行 · 编排 · 计划执行 · CLI 一并按新边界  
> **不**在 [`product-mainpath-optimize-2026-07-20.md`](./product-mainpath-optimize-2026-07-20.md) / 各历史 P2 子计划上打补丁；那些文档 = **历史与业务规则参考**（**非**实施真源）  
> 产品方向仍真源：[`../PRODUCT.md`](../PRODUCT.md)（给谁用 · 五步 · 轻量）  
> 总账条目：**P2-17 / P-arch-redesign**（**A0–A5 ✅ 收口** · A5-5 可选不排期；与 P2-16 冲突时 **本计划优先定边界**）  
> 契约目录：[`./contracts/`](./contracts/)（behavior-golden · run-dir · plan-job）  
> GEB：L1/L2 + [`scripts/check-arch.sh`](../scripts/check-arch.sh) 已与地形同构

[PROTOCOL]: 阶段勾选只认 **§11 任务表**；禁止第二份「架构总览」并行改写；禁止把本计划缩成「只改 CSS」或「只拆 JS 文件」。

**已固化仓库规则（防复发）**：根目录 [`../CLAUDE.md`](../CLAUDE.md)「工程硬规则（P2-17 起）」+ 各 L2（`src/` `web/` `src-tauri/` `cli/` `docs/`）+ 门禁 [`../scripts/check-arch.sh`](../scripts/check-arch.sh)。本文件 §4 为细则；与 L1 冲突时 **先改 L1/本计划再改代码**。

---

## 0. 一句话

**把 cco 从「能跑的功能堆叠」收成「六条清晰边界 + 一层应用用例 + 薄适配器」：桌面用 MVVM 清爽主路径，内核用端口/适配器编排多 CLI，CLI/TUI/桌面只消费同一套命令与查询，单文件不再超过认知极限。**

```text
旧：页面脚本 ↔ 40+ Tauri 命令 ↔ 肥 services ↔ 上帝 plan/handoff/scheduler
新：View → ViewModel → App Commands/Queries → Domain Ports ← Adapters(provider/fs/ipc)
```

---

## 1. 为什么必须大改（诊断，不是情绪）

### 1.1 体量与形状（2026-07-20 工作树实测）

| 热点 | 规模 | 症状 |
|------|------|------|
| `src/plan/mod.rs` | ~2278 行 | IR + 校验 + 物化 + 路由 + 常量混居 |
| `src/runtime/handoff.rs` | ~2313 行 | 账本 + VERDICT + rework + 门禁 + 视图 |
| `src/services/chat.rs` | ~~2349~~ → **多文件** | **A1-6 已拆** `services/chat/*` + `domain/chat` + `app/chat` |
| `src/runtime/scheduler.rs` | ~1393 行 | 调度 + 停止语义 + inspect 门 + failover |
| `web/js/plan.js` + `chat.js` | ~3k × 2 | DOM + 业务规则 + invoke 串在一起 |
| `web/js/*` 合计 | ~10k 行 | 无 ViewModel、无单向数据流、全局 `state` |
| Tauri commands | **41** | 扁平命令袋，无用例边界 |
| Provider trait | 已有 | 能力不齐（bg/stop/session）；编排仍知太多细节 |

### 1.2 耦合病（架构债清单）

| ID | 病 | 后果 |
|----|----|------|
| **C1** | **UI 规则泄漏**：确认屏、可选任务、分配直通逻辑在 JS 与 Rust 双份 | 改一处漏一处；PM 文案与引擎语义缠死 |
| **C2** | **Services 上帝层**：`runs`/`chat`/`live` 既编排又拼 DTO 又杀进程 | 无法单测业务；Tauri 与 CLI 只能「整段抄」 |
| **C3** | **Scheduler 知 handoff 细节**：巡检门禁、sys-post、rework 嵌在调度循环 | 调度器无法替换策略；读不懂「谁在推进 DAG」 |
| **C4** | **PlanIR 超载**：文档模型 + 运行模型 + 系统尾任务 + tags 路由 | 拆分质量与执行语义互相污染 |
| **C5** | **多 CLI 仍是字段，不是子系统**：provider/role/scope 散落 TaskIR + prompt 拼接 | 混跑策略无法独立演进；CLI 覆盖语义易伤混部 |
| **C6** | **前端无架构**：script 顺序拼全局；页面直接 `invoke` | 清爽 UI 大改必回归；无法做状态时间旅行/可测 VM |
| **C7** | **方法纵深**：单函数既 IO 又分支又拼 prompt | 难审、难复用、难并行改 |

### 1.3 本轮要改的产品面（用户点名，全部纳入架构）

| 面 | 大改含义（架构层） |
|----|-------------------|
| **UI** | 新信息架构 + MVVM 前端骨架；Codex 式清爽是**结果**，不是贴皮 |
| **计划拆分** | 独立 **Split** 限界上下文：提案图、波次、人工编辑、确认契约 |
| **多 CLI 执行** | 独立 **Worker** 端口：能力矩阵、路由、隔离、failover 策略可插拔 |
| **编排** | 薄 **Orchestrator**：只推进 Run 状态机与就绪集；副作用走端口 |
| **计划执行** | **Run** 生命周期与 **Inspect/Rework** 策略分离 |
| **CLI** | 与桌面同构的 **Application API** 薄壳；禁止第二套业务 |

### 1.4 明确不沿用的旧计划姿态

| 旧姿态 | 本计划 |
|--------|--------|
| O0 文案小改 / 只抛光 | ❌ |
| 「不换框架、只动 web」 | ❌ 前端允许引入轻量 MVVM 运行时；后端允许模块/目录级切开（甚至 workspace crate） |
| 在 P2-1…15 上继续叠阶段 | ❌ 业务规则可参考，阶段勾选作废于本轮 |
| 禁止动 Scheduler/PlanIR | ❌ **允许并要求**按边界切开；**行为契约**用金样锁，不是禁止重构 |

---

## 2. 目标架构

### 2.1 风格选择（决议）

| 层 | 选型 | 理由 |
|----|------|------|
| 后端整体 | **六边形（Ports & Adapters）+ 用例层** | 多入口（CLI/Tauri/TUI/测试）必须同核；比纯 MVC 更贴本仓库 |
| 领域 | **富模型 + 纯函数校验**（非重度 DDD 工厂地狱） | Plan/Run/Split 规则多，但团队要浅调用栈 |
| 前端 | **MVVM + 单向数据流** | 桌面状态多、异步多；View 不写业务；比继续堆 jQuery 式全局强 |
| 前端实现 | **Phase A：原生 ES modules + 自研薄 VM 运行时；Phase B 可选 Svelte 5 / Solid** | 先定边界再选框架，避免「为换框架而换」；默认 **A 必须完成**，B 仅当 A 证明 DOM 成本仍过高 |
| 状态持久 | **仍以 run_dir / plan job 文件为源** | 不引入云 DB；适配器读盘，领域不碰路径拼接细节 |

**不选**：完整 Clean Architecture 八股（过多 interface 文件）、Electron 重写、把编排搬进 LLM agent 框架、默认上 React 全家桶。

### 2.2 逻辑分层

```text
┌─────────────────────────────────────────────────────────────┐
│ Presentation                                                  │
│  desktop (MVVM)  ·  cli (clap)  ·  tui (ratatui observe)     │
└───────────────────────────┬─────────────────────────────────┘
                            │ DTO / commands / queries only
┌───────────────────────────▼─────────────────────────────────┐
│ Application（用例，无 UI，无 CLI 字符串）                      │
│  ChatUseCase · SplitUseCase · RunUseCase · InspectUseCase     │
│  ProjectUseCase · SettingsUseCase · DoctorUseCase             │
└───────────────────────────┬─────────────────────────────────┘
                            │ ports (traits)
┌───────────────────────────▼─────────────────────────────────┐
│ Domain（纯/近纯）                                              │
│  plan_model · split_graph · run_machine · worker_policy       │
│  inspect_rules · budget · optional_tasks                      │
└───────────────────────────┬─────────────────────────────────┘
                            │ implemented by
┌───────────────────────────▼─────────────────────────────────┐
│ Adapters                                                      │
│  provider/{claude,codex,fake} · fs_store · process · git_wt    │
│  log_tail · tauri_ipc · llm_planner · report_writer           │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 限界上下文（高内聚切刀）

```text
┌──────────┐   ┌──────────┐   ┌──────────────┐   ┌────────────┐
│ Project  │   │  Chat    │   │    Split     │   │    Run     │
│ 项目/权限 │   │ 生成计划  │──▶│ 拆分/确认台   │──▶│ 执行状态机  │
└──────────┘   └──────────┘   └──────┬───────┘   └─────┬──────┘
                                     │ confirm         │
                                     ▼                 ▼
                              plan.proposed      Scheduler loop
                                                     │
                     ┌───────────────────────────────┼──────────────┐
                     ▼                               ▼              ▼
              ┌────────────┐                 ┌────────────┐  ┌──────────┐
              │  Worker    │                 │  Handoff   │  │ Inspect  │
              │ 多 CLI 端口 │                 │ 事中账本    │  │ 巡检/回补 │
              └────────────┘                 └────────────┘  └──────────┘
```

| 上下文 | 拥有 | 不拥有 |
|--------|------|--------|
| **Project** | 允许目录、显示名、默认路径 | 任务图 |
| **Chat** | 会话、附件、计划 md 草稿 | 调度、provider 启停 |
| **Split** | plan job、proposed DAG、人工编辑、波次视图、`confirm` 输入 | worker 进程 |
| **Run** | run 状态机、并行窗口、停止/恢复 | 如何拼 Claude flag |
| **Worker** | Provider 启停轮询、能力、隔离 worktree | 业务验收文案 |
| **Handoff** | 接力账本、outputs 清单 | 调度循环控制流 |
| **Inspect** | VERDICT/遗漏分级/rework 计划构建 | UI 布局 |

**跨上下文通信**：只通过 Application 用例与明确的领域事件/命令（内存或落盘记录），禁止 `chat.js` 直接理解 `Scheduler` 字段名。

### 2.4 目录/crate 目标形态

**阶段 A（单 crate 模块切开，优先）**

```text
src/
  domain/
    plan/          # PlanDoc, TaskSpec, validate, materialize
    split/         # ProposedGraph, waves, edits, optional
    run/           # RunState machine, status, transitions
    worker/        # ProviderId, Route, Scope, Capability
    inspect/       # Verdict, Issues, ReworkPlan pure builders
    handoff/       # Board/Fragment model + pure merge rules
  app/
    chat.rs        # use cases
    split.rs
    run.rs
    inspect.rs
    project.rs
    settings.rs
    doctor.rs
    dto/           # 稳定 IPC/CLI JSON 形状
  adapters/
    provider/
    store/         # run_dir, job_dir, chat sessions
    process/
    worktree/
    planner_llm/
    report/
  ports/           # traits only（或放 domain 旁）
  presentation/    # 可选：cli/tui 仍顶层，但只调 app
```

**阶段 B（可选 workspace 拆 crate，当单 crate 编译/边界仍糊）**

```text
crates/
  cco-domain
  cco-app
  cco-providers
  cco-store
  cco            # bin: cli
  cco-desktop    # tauri
```

默认 **先 A 后 B**；A 完成即算架构大改达标，B 不阻塞 UI。

### 2.5 前端目标形态（MVVM）

```text
web/
  index.html              # 壳，无业务
  src/
    main.js               # 组装 root VM + router
    app/
      AppViewModel.js     # 壳：项目/阶段/导航
      routes.js           # 五步 ↔ 页面
    features/
      chat/
        ChatView.js
        ChatViewModel.js
        chatApi.js        # 只调 app gateway
      split/
        SplitView.js      # 拆分台（核心屏）
        SplitViewModel.js
        splitApi.js
      run/
        RunView.js
        RunViewModel.js
      result/
        ResultView.js
        ResultViewModel.js
      project/
      settings/
    shared/
      store.js            # 可订阅状态（单向）
      gateway.js          # invoke 唯一出口；命令名集中
      bindings.js         # 声明式 data-bind 小工具
      ui/                 # 按钮/卡片/阶段条原子
    styles/
      tokens.css
      layout.css
      features/*.css
```

**MVVM 规则（硬）**

1. **View** 只绑数据与发意图（intent）；不写 `if (status===...) startJob` 业务链  
2. **ViewModel** 持有展示状态 + 调用 gateway；可单测（假 gateway）  
3. **Model/API** 在 Rust Application；JS 不复制 Mode B 规则  
4. **单一 gateway**：禁止 feature 文件直接 `__TAURI__.invoke`  
5. **阶段路由**：`AppViewModel.phase ∈ {home, author, split, run, result}` 驱动主区，而不是 10 个 `showPage` 散落  

**与 Codex 清爽的关系**：主区一次只强化一个 phase；侧栏弱；拆分台与结果台是一等屏；日志是 Run 的次级面板。

---

## 3. 领域与契约（大改后的真源草案）

### 3.1 计划文档 vs 可执行图

| 概念 | 说明 |
|------|------|
| **PlanDoc** | 人读 md（聊天/模板产物）；无 provider |
| **SplitJob** | 一次拆分作业：`pending→planning→planned|failed` |
| **ProposedGraph** | 可编辑任务图（节点、依赖、optional、route 建议） |
| **ConfirmedPlan** | `confirm` 快照 → 物化为 **ExecutablePlan**（旧 PlanIR 的干净继承者） |
| **Run** | 一次执行实例；绑定 ExecutablePlan + run_dir |

**禁止**：Chat 直接生成 ExecutablePlan 并 `start_run` 旁路 Split（高级「结构化计划 skip split」走明确 `SplitMode::ParseOnly`，仍经同一用例）。

### 3.2 多 CLI（Worker）一等模型

```text
TaskRoute {
  provider: ProviderId,     // claude | codex | fake | …
  role: Role,               // implement | inspect | integrate | …
  scope: Scope,             // 路径/能力边界声明
  tags: [String],           // 路由提示，不是执行器
}
ProviderCapabilities { print, background, stop, cost, session_resume, worktree }
IsolationPolicy { worktree: Required|OnFail|Off, fail_closed: bool }
FailoverPolicy { on_stall, on_provider_error, max_retries }  // 从 scheduler 挪出
```

- **路由**：Split 阶段可建议；Confirm 可改；Run 阶段默认不静默改写（`--provider` soft-fill / force 语义下沉到 `RunUseCase`，有单测）  
- **隔离**：混 provider 默认 fail-closed worktree（产品可关，但默认安全）  
- **能力**：Orchestrator 只调 port；无 bg 的 provider 走统一「进程句柄」适配，不在调度里写 if claude  

### 3.3 编排状态机（薄）

```text
RunStatus:
  Created → Running → (Paused) → Completed
                   ↘ Failed | Aborted

TaskStatus: 保持现有语义收敛表（Pending/Running/Done/Failed/Stopped/Skipped/Timeout）
            转换表放 domain/run，禁止 UI 私自发明状态词映射多份
```

Orchestrator 每一拍只做：

1. 读 Run 快照  
2. 算 ready 集（纯函数）  
3. 问 WorkerPort 启动/轮询/停止  
4. 发领域事件：`TaskStarted|TaskFinished|RunHalted`  
5. 适配器写盘 / HandoffPort 更新  

Inspect 门禁、sys-post、budget **是策略端口**，不是写死在循环中部的 200 行 `if`。

### 3.4 拆分台（产品+架构）

拆分是**独立 phase + 独立 use case**，不是 confirm 闪屏：

| 能力 | 归属 |
|------|------|
| 启动/轮询 SplitJob | `SplitUseCase` |
| 波次计算展示 | domain 纯函数 `waves(graph)` |
| 删任务/改依赖/optional | `SplitUseCase` + domain 校验 |
| 重新拆分并保留人工编辑 | domain `merge_edits` |
| 确认并开跑 | `SplitUseCase.confirm` → `RunUseCase.start`（唯一业务入口） |

IPC 收敛示例（取代零散 8 个 plan 命令）：

```text
split.start / split.get / split.latest
split.update_task / split.remove_task / split.set_includes
split.replan
split.confirm   → returns run_id
```

### 3.5 Application API（CLI 与桌面同构）

| 用例组 | 命令（逻辑名） | CLI 映射示例 |
|--------|----------------|--------------|
| project | list/add/remove | `cco project …` |
| chat | send/save/sessions | `cco chat …`（可后置） |
| split | plan/confirm/edit | `cco split …` 或保留 `cco plan`+`confirm` 别名 |
| run | start/stop/resume/status | `cco run` / `stop` / `resume` / `status` |
| inspect | summary/rework/accept | `cco inspect …` |
| doctor | report | `cco doctor` |

**规则**：CLI handler ≤ 30 行：解析 argv → 调 app → 打印 DTO。禁止在 `commands/run.rs` 再写调度策略。

---

## 4. 代码级工程法则（防再次腐化）

### 4.1 体积与深度

| 规则 | 阈值 |
|------|------|
| 单文件软上限 | **400 行**；硬上限 **600 行**（超则必须拆） |
| 单函数软上限 | **40 行**；硬上限 **80 行** |
| 调用深度 | 业务路径上自定义封装 **≤ 4 层**（Presentation→App→Domain→Adapter） |
| 模块 fan-out | 一个文件 `use` 的兄弟模块宜 ≤ 7；过多说明边界错 |

### 4.2 依赖方向（lint 意识）

```text
presentation → app → domain
adapters → domain ports
app → ports (traits)
禁止：domain → app/presentation/adapters
禁止：web View → 直接拼 provider 参数
禁止：scheduler → 解析 VERDICT 文本（应 inspect adapter/domain）
```

### 4.3 测试金字塔

| 层 | 必补 |
|----|------|
| Domain 纯函数 | 图校验、波次、状态转换、optional 物化、provider soft-fill |
| App 用例 | fake store + fake worker 的 confirm/start/stop/rework |
| Adapter 契约 | 每 provider：start/poll/stop/collect 金样（已有延用） |
| UI VM | 假 gateway：拆分台意图序列、phase 切换 |
| 烟测 | 桌面 fake 五步；CLI `plan→confirm→run` 一条龙 |

### 4.4 可观测

- 领域事件写入 `events.jsonl` 的 schema 版本化（`v2`）  
- UI 只消费 **View DTO**（人话状态已在 app 层映射），禁止前端解释 `VERDICT` 原文  

---

## 5. UI 信息架构（大改目标，依附新前端骨架）

```text
Shell
├─ 左：项目（弱）
└─ 主区按 phase
    ├─ author   写/改计划（聊天 + 模板 + 选已有）
    ├─ split    拆分台（核心，默认可停留）
    ├─ run      执行台（进度优先，日志次级）
    └─ result   结果台（计划 vs 完成 vs 遗漏 + 回补）
高级：设置 / doctor / 原始日志 / 多 CLI 明细
```

视觉：在新 `styles/tokens` 上做**一体密度**（留白、单主 CTA、状态色三种）；不复制 Codex 像素，复制**主区单一焦点**。

TUI：保持 **观察 + 轻控制** 适配器，消费同一 `Run` 查询；不重做拆分台于终端（除非后续单独立项）。

---

## 6. 迁移策略（大改但不自杀）

### 6.1 原则

1. **绞杀者（Strangler）**：新 `app/` + `domain/` 先建，旧 `services/` 变薄委托，再删  
2. **契约锁**：Mode B「唯一 confirm 开跑」、run_dir 布局、金样测试先固化再搬  
3. **垂直切片**：每个 wave 交付「可演示的一条用户路径」，禁止只拆文件无行为  
4. **UI 与内核可交错**：A1 先打 app 边界，A2 上 MVVM 壳，A3 拆分台切新 API，避免大爆炸 PR  

### 6.2 兼容

| 项 | 策略 |
|----|------|
| 旧 run_dir | 读 v1；新写带 `schema_version` |
| 旧 Tauri 命令 | 过渡期双注册：旧名 → 委托新 use case；桌面全切后删 |
| 旧计划文档 | adapters 保留；IR 改名内部化 |
| 历史 docs 计划 | 只读参考；勾选不迁移 |

---

## 7. 阶段划分（实施真源）

> 顺序固定：**A0 → A1 → A2 → A3 → A4 → A5**。  
> 可并行的仅「文档/金样加固」与「无依赖的 domain 纯函数提取」。

### A0 — 基线与契约冻结 ✅

**目的**：大改前先能证明「行为没偷偷变」。

- [x] 列出 Mode B / confirm / stop / multi-cli soft-fill 的**行为金样清单**并补测到可自动跑（[`contracts/behavior-golden.md`](./contracts/behavior-golden.md) · `tests/a0_behavior_golden.rs` + 既有 mode_b/mixed/unit）  
- [x] 冻结 `run_dir` / `plan job` 目录契约表（[`contracts/run-dir.md`](./contracts/run-dir.md) · [`contracts/plan-job.md`](./contracts/plan-job.md)）  
- [x] 建立 `src/domain` `src/app` `src/ports` 空骨架与依赖方向说明（`lib.rs` 挂载；**无**业务搬家）  
- [x] 总账登记 **P2-17** 出池实施；与 P2-16 关系：UI 波次已在旧壳落地，架构边界/搬家从 A1 起、主路径 UI 再迁在 A2+  

**完成定义**：CI 金样绿；骨架可编译；无行为变化。 **本阶段达成 · 不启动 A1。**

### A1 — 后端绞杀：Application + Domain 切开 ✅

**目的**：消灭 services/plan/runtime 上帝文件，调度变薄。

- [x] **Domain（A1-1 首刀）**：`PlanIR`/`TaskIR`/校验/物化/软路由 → `domain/plan/*`；`plan/mod.rs` 变 IO facade（~230 行 + `plan_tests.rs`）  
- [x] **Split 入口（A1-2 薄壳）**：`app/split::confirm` 为唯一业务开跑；`services::confirm_start` 委托之；job 编辑 API 挂在 app/split  
- [x] **Run（A1-3）**：状态/重试/活跃集纯规则 → `domain/run/*`；`Scheduler` 拆 `runtime/scheduler/{mod,tick,finish,start,patrol,gates,active,types}.rs`（单文件 ≤600）；`app/run` 用例面；VERDICT 解析仍 handoff  
- [x] **Worker（A1-4）**：`ports::WorkerPort` + DTO；claude/codex/fake 实现 port（`WorkerProvider` 别名兼容）；`domain/worker`（ProviderId/Route/FailoverPolicy/IsolationOnFail/soft-fill）；scheduler 经 port + 策略；混跑/retry/A0 绿  
- [x] **Handoff / Inspect（A1-5）**：`domain/inspect`（VERDICT/ISSUES 纯解析 · gate 决策）；`ports::HandoffStore`；`runtime/handoff/*` 多文件适配器（单文件 ≤600）；scheduler/gates 只经 facade + `inspect_gate_fail_reason`，**零**正文解析  
- [x] **Chat（A1-6）**：`domain/chat` 纯规则（fence/title/normalize/stream_parse）；`app/chat` 用例面；`services/chat/*` 多文件 IO 适配（session/send/stream/plan_md/attachment/cli_call…，单文件 ≤600）；**不**旁路 confirm；ChatStore trait 本刀未建（free-fn facade）  
- [x] **services/** 变为 `app` 的 deprecated facade（A1-7：mod 头标注迁移；confirm/chat/stop 等经 app；IO 仍住 services，**未**删光）  
- [x] Tauri `lib.rs` + CLI `commands/*` 改委托 `app::*`（A1-7：chat/split/run 无业务策略；IPC 名/DTO 兼容；live/projects/settings 仍 thin services）  

**完成定义**：无 >600 行业务文件；关键路径测试仍绿；桌面/CLI 冒烟通过。 **A1 本阶段达成。**  
**本阶段后**：`plan/mod` · `runtime/scheduler/*` · `runtime/handoff/*` · `services/chat/*` · `ports/{worker,handoff}` · `domain/{plan,run,worker,inspect,chat}` · `app/{split,run,chat}` 已 ≤600；Presentation → app；前端 plan.js/chat.js 巨石仍在 → **A2 已上 MVVM 骨架**（巨石清空属 A5）。

### A2 — 前端 MVVM 骨架 + 壳导航 ✅

**目的**：UI 大改的地基。

- [x] ES module 入口：`web/js/main.js`（`type=module`）；经典 script 仍 strangler 加载，**源码侧**模块边界成立  
- [x] `web/js/shared/gateway.js` 集中 IPC（A1-7 命令名 1:1）；`window.ccoGateway`  
- [x] `AppViewModel` + phase 路由：author/split/run/result（`routes.js` ↔ legacy page；冷启动 soft-sync 不抢 classic boot）  
- [x] tokens 主 CTA 变量 + layout 对齐（A2-3）  
- [x] author 最小路径：`features/chat/*`（listSessions / send / savePlan 经 gateway）；旧 `chat.js`/`plan.js` **只桥接/委托，禁止堆新功能**  

**完成定义**：冷启动可进 author；phase 切换不丢项目；无业务回归红线。 **A2 本阶段达成（骨架）**；完整清空 chat 巨石 → A2b/A5。

### A3 — 拆分台 + 多 CLI 路由 UX（垂直切片） ✅

**目的**：核心产品屏与多引擎声明在新架构落地。

- [x] Split 三栏：波次 · 卡片 · 详情编辑（`web/js/features/split/*`）  
- [x] 任务级 provider 编辑 + role/scope **展示**（高级折叠默认藏；改通道走 `update_plan_task`；**不**在 JS 复制 soft-fill；role/scope 写字段待 DTO 扩展时再接线）  
- [x] replan 保编辑（`preserve_from_job_id` 既有后端）、optional 勾选必停 auto-start（`planNeedsOptionalConfirm` 未改）  
- [x] `confirm_start_cmd` 经 gateway → `AppViewModel.goRun()`；**唯一开跑**  
- [x] CLI 同构：本刀 **零 Rust 语义 diff**；`cco plan` / confirm 仍 A1-7 同 app  

**完成定义**：拆分台可停留编辑 → 确认开跑进 run phase；optional 不静默全开；features 无散落 invoke。 **A3 本阶段达成**；完整清空 plan.js 巨石 → A5。

### A4 — 执行台 + 结果台 + Inspect ✅

**目的**：编排可见、完成可收口。

- [x] Run：步骤进度、stall、停止/重试/换 CLI（调 app，不写策略）— `web/js/features/run/*` · `window.ccoRun`  
- [x] 日志次级面板（`logPanel` 折叠；**A5-2c** 虚拟列表 `features/run/logVirtual` · `log.js` facade，不挡主进度）  
- [x] Result：计划要点 / 完成 / 遗漏 / 回补 / 接受残留 — `features/result/*` · `window.ccoResult`  
- [x] Inspect DTO 人话化 — `inspectCopy` 只读 `inspect_loop` 字段；**无** UI 解析 VERDICT 正文；主路径无裸 VERDICT  

**完成定义**：不读原始日志能回答「做完没、漏啥」；回补经 `start_rework`（非 confirm / 非 `start_run`）；终态 `goResult`。 **A4 本阶段达成**；完整清空 monitor/result/log 巨石 → A5。

### A5 — CLI 面收敛 + 删旧双轨 + 文档地图 ✅（A5-5 可选 ☐）

**目的**：入口同构，腐化入口关掉。

- [x] **A5-0 清单**（只调研不删）：classic JS / CLI↔app / TUI 触点 / 删除序 — 见 **§16 附录 C** + [`../web/CLAUDE.md`](../web/CLAUDE.md)「A5-0」  
- [x] CLI 子命令与 app 用例 1:1 表；删重复旁路（**A5-1**）— 表真源 [`../src/cli/CLAUDE.md`](../src/cli/CLAUDE.md)；`confirm_materialize` / `materialize_run` / `prepare_scheduler`  
- [x] 删除旧 Tauri 命令别名与旧 `web/js/*.js` 巨石（**A5-2**；序见附录 C · web L2 D1–D8）  
  - classic facade **S8**：`chat.js`/`doctor.js`/`result.js` ≤80 · `plan.js`/`log.js`/`monitor.js` ≤200 · `split.js` 空壳未引用  
  - 真源：`features/{chat,project,split,run,result,settings}` · IPC 只 `shared/gateway`  
  - **遗留（非 S8 巨石 · 非本刀范围）**：`state.js` 仍厚（D9 未做 · invoke 桥）· `templates.js` 体量可后收 · 部分 feature 文件软超 400 但 ≤600  
- [x] TUI 只依赖 app 查询 DTO（**A5-3** · load_by_dir / load_resolved_plan / stop_task · ports::TaskStatus）  
- [x] 更新 L1/L2、历史计划「非实施真源」、门禁 GIANTS 与地形同构（**A5-4 GEB 2026-07-21**）  
- [ ] （可选）workspace 拆 crate（**A5-5**）— **本轮明确不做**；2026-07-21 评估延期零 crate diff；见 [`a5-5-workspace-crates-eval-2026-07-21.md`](./a5-5-workspace-crates-eval-2026-07-21.md)  

**完成定义**：贡献者只读本计划 + L2 即可改；无第二业务入口。 **A5 主线达成（A5-5 可选未做）**。

---

## 8. 非目标

- 云端多租户、实时协作、完整 IDE  
- 默认真交互 PTY 写入（仍可外部终端）  
- 为「像 Codex」堆 diff/PR 平台能力  
- 重写 provider 协议到非 CLI（SDK provider 仍属池内可选项，不进本计划必做）  
- 一次 PR 改完 A0–A5（禁止）  
- 把 TUI 做成第二套完整拆分台  

---

## 9. 成功标准

| # | 标准 | 验证 |
|---|------|------|
| **S1** | 依赖方向符合 §4.2；domain 测试不链 tokio provider | 结构检查 + `cargo test -p … domain` |
| **S2** | 无业务源文件 >600 行；新增函数常态 ≤40 行 | `tokei`/脚本门禁（可先 CI warn） |
| **S3** | 桌面主路径 phase 四态可指认；拆分台可停留编辑 | 目视 + fake 脚本 |
| **S4** | 开跑唯一经 `SplitUseCase.confirm` / 显式 ParseOnly | 测试锁定；grep 无 UI `start_run` 旁路 |
| **S5** | 同 run 混 claude+codex；soft-fill 不覆盖显式 route | 集成测 |
| **S6** | CLI 与桌面同一 app 测例共享 | 用例测试 |
| **S7** | 结果台可不打开日志完成巡检理解 | 目视 + DTO 字段断言 |
| **S8** | 旧巨石文件删除或 <200 行 facade | tree 审查 · **A5-4 实测**：plan/chat/log/doctor/monitor/result facade 均 ≤200；**例外** `state.js` 仍厚（D9 未做 · 非 classic 业务巨石） |

---

## 10. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 大改期间主路径不可用 | 绞杀 + 每波垂直切片；主分支可演示 |
| 行为静默漂移 | A0 金样；confirm/stop/混跑优先锁 |
| 前端换模块方式导致 Tauri 打包挂 | 先 esm 兼容方案再考虑打包器；`scripts` 烟测 |
| 过度设计 ports | 端口数量控制：Worker/Store/Planner/Handoff/Clock/Process ≤ 一屏能列完 |
| 与 P2-16 产品稿冲突 | **边界听本计划，交互稿听 PRODUCT+拆分台三栏**；P2-16 降为交互参考不再当实施真源 |

---

## 11. 任务表（勾选真源）

### A0 基线 ✅

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| A0-1 | 行为金样清单 + 补测 | CI 绿 | ✅ `docs/contracts/behavior-golden.md` · `tests/a0_behavior_golden.rs` |
| A0-2 | run_dir/job 契约文档 | `docs/contracts/` 或本附录 | ✅ `run-dir.md` · `plan-job.md` |
| A0-3 | domain/app/ports 骨架 | 编译通过 | ✅ `src/{domain,app,ports}/mod.rs` |
| A0-4 | 总账 P2-17 出池声明 | gap 文档一行 | ✅ gap t47 · 状态「出池实施 · A0 完成」 |

### A1 后端 ✅

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| A1-1 | plan 领域切开 | mod 巨石消失 | ✅ `domain/plan/*`；`plan/mod.rs` ~232 行 facade |
| A1-2 | split 用例 + 编辑/confirm | 测绿 | ✅ 入口 `app/split`；confirm 委托绿；job 状态机仍在 planner（下刀再迁 domain/split） |
| A1-3 | run 状态机 + 薄编排循环 | scheduler 文件≤400 或拆多文件达标 | ✅ `domain/run` + `runtime/scheduler/*`（mod~252 · tick~536 · 均 ≤600）· `app/run`；行为金样/scheduler/retry/mixed 绿 |
| A1-4 | worker port + 策略对象 | 混跑测绿 | ✅ `ports::WorkerPort` + DTO；`domain/worker`（route soft/force · FailoverPolicy · IsolationOnFail）；provider 适配；scheduler 经 port；mixed/retry/A0/mode_b 绿 |
| A1-5 | handoff/inspect 分离 | scheduler 无 VERDICT 解析 | ✅ `domain/inspect` 纯解析/门禁；`ports::HandoffStore`；`runtime/handoff/{mod,model,paths,inspect_io,lifecycle,prefix,rework,store,tests}` 多文件 ≤600；gates 经 `inspect_gate_fail_reason`；handoff 出巨石榜；A0+scheduler_fake+handoff_ledger+mixed 绿 |
| A1-6 | chat 切开 + app/chat | 测绿 | ✅ `domain/chat`（fence/title/normalize/stream_parse/id/text）· `app/chat` 用例面 · `services/chat/*` 多文件 ≤600；chat 出巨石榜；A0+mode_b+lib chat 绿；**不**改 session/plan 路径；**未**建 ChatStore trait |
| A1-7 | Tauri/CLI 改委托 | handler 无业务 | ✅ Tauri → `app::{split,run,chat}`；CLI stop/plan/`--provider` → app；`services` deprecated facade 保留；IPC 名/DTO 兼容；A0+mode_b+lib 绿；**未**做 A2 / 删 services / 改 web |

### A2 前端骨架 ✅

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| A2-1 | esm + gateway + store | 可加载 | ✅ `js/main.js` · `shared/gateway.js` · `shared/store.js` |
| A2-2 | AppViewModel phase 路由 | 四 phase | ✅ `app/AppViewModel.js` · `routes.js` · `window.ccoApp` |
| A2-3 | tokens/原子 UI | 主 CTA 单一视觉 | ✅ `css/tokens.css` CTA 变量 · layout 对齐 |
| A2-4 | 迁移 author（chat）到 feature | 旧 chat 巨石删除或空壳 | ✅ **最小路径** `features/chat/*` + chat.js 桥接 list/send；巨石未删（A5/A2b） |

### A3 拆分 + 多 CLI ✅

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| A3-1 | Split 三栏台 | 可编辑依赖/optional | ✅ `features/split/{splitRender,SplitView,splitDetail}`；plan.js 委托 `ccoSplit.render` |
| A3-2 | route 编辑 UX | 高级折叠 | ✅ provider 可改经 gateway；role/scope 展示折叠；默认藏 |
| A3-3 | replan 保编辑 | 金样 | ✅ UI 接线 `preserve_from_job_id`；optional 必停 auto-start 未改；A0/mode_b 金样绿 |
| A3-4 | confirm→run 切换 | 唯一入口 | ✅ `confirmStart` → `goRun`；无 UI `start_run` 旁路 |
| A3-5 | CLI 同路径 | 文档+测 | ✅ 零 Rust 语义 diff；app/split 表见 L2；CLI 仍 A1-7 |

### A4 执行 + 结果 ✅

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| A4-1 | Run 进度台 | stall/停/重试 | ✅ `features/run` · stop/resume/stopTask 经 gateway；monitor.js 委托 `ccoRun` |
| A4-2 | 日志次级 | 不挡主进度 | ✅ `logPanel` 默认折叠；**A5-2c** 虚拟列表 `features/run/log*` · log.js facade |
| A4-3 | Result 摘要 + 回补 | S7 | ✅ `features/result` · startRework/acceptResidual；终态 goResult |
| A4-4 | Inspect DTO 人话 | 无裸 VERDICT 主路径 | ✅ `inspectCopy` 读 inspect_loop；无 UI 解析正文 |

### A5 收敛 ✅（A5-5 可选 ☐ 本轮不做）

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **A5-0** | 清单：classic JS / CLI↔app / TUI / 删除序 | 写入 web L2 + 本文件附录 C；**不删代码** | ✅ 2026-07-21 |
| **A5-1** | CLI 子命令表收敛 | 1:1 app；`run`/`resume` 不手搓 Scheduler；Mode B 经 `split::confirm_materialize`；ParseOnly 经 `materialize_run`；表在 `src/cli/CLAUDE.md` | ✅ 2026-07-21 |
| **A5-2** | 删旧命令/旧 JS | tree 净；S8 classic facade ≤200 或删除；features 无散落 invoke | ✅ 2026-07-21 · **2a chat** · **2b-fin plan→project** · **2c log** · **2d settings** · **2e gateway 清扫** · **2f result/monitor/split 壳** · facade 行数见附录 C / web L2 |
| **A5-3** | TUI 接 app 查询 | 只读观察；stop 经 `app::run`；不直写 `.done`/provider 类型 | ✅ 2026-07-21 |
| **A5-4** | L1/L2/门禁/总账 GEB | 地图=地形；S1–S8 自检；A5-5 声明不做 | ✅ 2026-07-21 |
| A5-5 | （可选）workspace crates | 编译边界 | ☐ **本轮不做** · 评估延期 [`a5-5-workspace-crates-eval-2026-07-21.md`](./a5-5-workspace-crates-eval-2026-07-21.md) · 零 crate diff · 重开条件见该文 §5 |

---

## 12. 与既有文档的关系

| 文档 | 本轮角色 |
|------|----------|
| [`PRODUCT.md`](../PRODUCT.md) | **产品方向真源**（受众、五步、轻量）——不因架构改而改定位 |
| **本文件** | **架构与实施真源（大改）** |
| [`product-mainpath-optimize-2026-07-20.md`](./product-mainpath-optimize-2026-07-20.md) | **交互参考**（拆分台三栏、结果台）；**不再**当实施勾选真源 |
| [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md) | 业务规则参考：confirm 唯一开跑——**规则保留，实现搬家** |
| [`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md) | 混跑语义参考；执行迁入 Worker 上下文 |
| [`claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md) | 历史 M0–M4 已落地说明；M5 仍池内；本轮重画内部边界 |
| [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md) | 登记 P2-17；D0–D4 不回灌 |

---

## 13. 附录 A — 旧 → 新 映射（搬家指南）

| 旧位置 | 新位置 |
|--------|--------|
| `plan/mod.rs` 巨型 IR | `domain/plan/*` |
| `plan/planner/*` | `app/split` + `adapters/planner_llm` + `domain/split` |
| `runtime/scheduler.rs` | `app/run` 循环 + `domain/run` 转换 + 策略 port |
| `runtime/handoff.rs` | `domain/handoff` + `adapters/store/handoff_fs` + `domain/inspect` |
| `runtime/provider/*` | `adapters/provider/*` 实现 `ports::WorkerPort` |
| `services/*` | 删除或 `app/*` |
| `web/js/plan.js` | `features/split/*` + `features/run/*` |
| `web/js/chat.js` | `features/chat/*` |
| `web/js/state.js` | `shared/store.js` + `AppViewModel` |
| `src-tauri` commands | 薄 IPC → `app` |
| `src/cli/commands/*` | 薄 argv → `app` |

## 14. 附录 B — 首批端口清单（控制数量）

```text
ports::WorkerPort        start/poll/stop/collect/preflight/capabilities
ports::PlanJobStore      save/load/list proposed + edits
ports::RunStore          run.json / task states / events append
ports::HandoffStore      read/write board + fragments
ports::ChatStore         sessions + attachments meta
ports::PlannerPort       prose plan → ProposedGraph
ports::ProcessPort       kill/pid/wait（供 stop）
ports::WorktreePort      create/cleanup
ports::Clock             now（可测）
```

禁止再发明 `XxxManager` 上帝对象；需要组合时在 **app 用例** 内显式调用。

---

## 15. 决议签署（实施前核对）

| # | 问题 | 本计划答案 |
|---|------|------------|
| 1 | 是否小改 UI？ | **否，系统级大改** |
| 2 | 是否保留 PRODUCT 受众？ | **是** |
| 3 | 框架？ | **后端六边形+用例；前端 MVVM；不强制 React** |
| 4 | 多 CLI？ | **Worker 一等上下文，非字段点缀** |
| 5 | 编排？ | **薄状态机 + 策略端口** |
| 6 | 旧计划？ | **参考不继承勾选** |
| 7 | 第一刀？ | **A0 金样 → A1 后端边界 → A2 MVVM → A3 拆分台** |

---

## 16. 附录 C — A5-0 清单（2026-07-21 · 调研不删）

> **不**另开阶段表。前端表副本：[`../web/CLAUDE.md`](../web/CLAUDE.md)「A5-0」。  
> 测量时点：A5-4 收口（2026-07-21）；classic facade S8 达标；features ~14k 行拆多文件。

### C.1 classic JS（摘要 · A5-4 实测 2026-07-21）

| 文件 | 行数 | 职责 | 直接 invoke（摘要） | feature 委托 / S8 |
|------|------|------|---------------------|-------------------|
| `state.js` | **820** | 全局 state + **invoke 桥** | 仅桥：`getInvoke`/`invoke`；优先 gateway | **D9 未做**（非 classic 业务巨石） |
| `flow.js` | 346 | 流程文案 | 无 | 可长期保留 |
| `split.js` | 空壳 / index 未挂 | — | 无 | **A5-2f D3 ✅** · `ccoSplit` 单轨 |
| `templates.js` | 389 | 模板落盘 / 写回 | **无**（A5-2e → ccoChat/gateway） | 体量可后收 |
| **`plan.js`** | **108** facade | classic → `ccoProject` | **无** | **S8 ✅** · `features/project/*` · confirm→`ccoSplit` |
| **`monitor.js`** | **198** facade | workspace 壳 | 无 | **S8 ✅** · `ccoRun.renderProgress` |
| **`result.js`** | **35** facade | 结果壳 | 无 | **S8 ✅** · `ccoResult.*` |
| **`log.js`** | **110** facade | classic → `ccoLog` | **无** | **S8 ✅** · `features/run/log*` |
| **`chat.js`** | **75** facade | classic → `ccoChat` | **无** | **S8 ✅** · `features/chat/*` |
| **`doctor.js`** | **65** facade | classic → `ccoSettings` | **无** | **S8 ✅** · `features/settings/*` |
| `main.js` | 563 | ESM 入口 | 无 | 装配全部 cco*（非巨石） |

- features/**：**无真实** `invoke`/`__TAURI__`（仅注释禁令）。  
- classic 业务：**无** `invoke("…_cmd")`；仅 `state.js` 保留 getInvoke 桥 + pre-main 兜底。  
- UI：**无** `start_run` 调用；gateway 不暴露 `startRun`。  
- Tauri 仍注册 legacy `start_run` → `app::run::start_from_request`（ParseOnly；非 Mode B 主路径）。

**删除序（A5-2 已完成项）**：result/monitor/split 壳 ✅ · log ✅ · plan ✅ · chat ✅ · doctor ✅ · templates IPC ✅ · **state 瘦身 D9 待后** · flow 可选。详见 web L2。

### C.2 CLI 子命令 ↔ `app::{split,run,chat}`（**A5-1 ✅**）

| CLI | 调用（A5-1 后） | app 对齐 | 备注 |
|-----|-----------------|----------|------|
| `plan` | `app::split::{start_job,get_job}` | ✅ | 开跑指引桌面 confirm |
| `stop` | `app::run::{stop,stop_task}` | ✅ | — |
| `run`（散文） | plan → split；开跑 → **`split::confirm_materialize`**；loop → `prepare_scheduler` | ✅ | 与桌面 confirm 同 optional/soft 契约 |
| `run`（结构化 / `--skip-plan`） | `apply_provider_override` → **`materialize_run`** → `prepare_scheduler` | ✅ | 文档化 ParseOnly；非 Mode B 旁路 |
| `resume` | `prepare_resume` + `prepare_scheduler` | ✅ | 与 stop 对称经 app |
| `status` | `load_by_dir` + `handoff_paths` | ✅ | 不碰 handoff 内部 |
| `plans` | `run::plans` | ✅ | — |
| `report` / `logs` / `parse` | 观察面 | 部分 | 可后收 |
| `doctor` / `init` / `term` / `tui` | 壳；tui 经 app（A5-3） | ✅/壳 | 非 Mode B 开跑 |
| chat | **无** CLI 子命令 | — | 桌面-only |

**A5-1 目标**（✅）：`cco run` Mode B → `split::confirm_materialize`（前台）/ 桌面 `split::confirm`；ParseOnly → `run::materialize_run`；scheduler 装配 `run::prepare_scheduler`；handler ≤ 打印+flags；**禁止** UI/CLI 第二套 soft-fill。1:1 表 → [`../src/cli/CLAUDE.md`](../src/cli/CLAUDE.md)。

### C.3 TUI 仍碰内部的点

| 位置 | 触点 | 问题 | A5-3 目标 | 状态 |
|------|------|------|-----------|------|
| `tui/app.rs` | `RunState::load` 直读盘 | 绕过 app 查询 | `app::run::load_by_dir` | ✅ |
| `tui/app.rs` | `load_resolved_plan` → `PlanIR` | 知 plan 文件布局 | `app::run::load_resolved_plan` | ✅ |
| `tui/app.rs` | `stop_selected`：`kill_pid` · 写 `.done` · 改 `TaskStatus` · `state.save` | **第二 stop 语义** | 只调 `app::run::stop_task` | ✅ |
| `tui/app.rs` | `TerminalManager` 直构 | 终端适配器细节 | 保持（观察适配） | ✅ 保持 |
| `tui/widgets|pages` | `runtime::provider::TaskStatus` | Presentation 依赖 provider 类型 | `ports::TaskStatus` | ✅ |
| `cli/status` | `Handoff::path_*` | 同上 | app/handoff 查询 | ✅ A5-1 |

TUI **不做**完整拆分台（§8 非目标不变）。

### C.4 建议删除 / 收敛序与风险

| 序 | 动作 | 风险 | 并行 |
|----|------|------|------|
| 0 | **A5-0 清单**（本附录） | 无 | 串行 · ✅ |
| 1 | A5-1 CLI `run`/`resume`/`status` → app | 中（金样+mode_b） | 可与 2 分树并行 |
| 2a | A5-2 薄删 result/monitor/split 壳 | 低 | 可并行 agent |
| 2b | A5-2 log 抽列表 + 删 invoke fallback | 中 | **✅ A5-2c** 单 agent |
| 2c | A5-2 plan.js 迁尽后删/空壳 | **高** | **✅ A5-2b-fin D5** facade ≤200 · `features/project` · confirm 仍仅 `ccoSplit` |
| 2d | A5-2 chat.js 迁尽后删/空壳 | **高** | **串行 1 agent**（勿与 2c 同 PR 硬切） |
| 3 | A5-3 TUI stop/load → app | 中 | 可与 1 并行（注意 app/run） |
| 4 | A5-4 GEB | 低 | 收口串行 |
| 5 | A5-5 crates（可选） | 高编译 | 单独 PR |

**红线保持**：confirm 唯一业务开跑；rework 只 `start_rework`；soft-fill 不盖显式 route；optional 不静默 auto-start；features 无散落 invoke；不改 run_dir/job/session 路径与 IPC 名（除非兼容加字段）。

### C.5 并发建议（一页）

```text
A5-0 ✅（本刀）
    │
    ├─► A5-1 CLI→app（1 agent） ──┐
    │                              ├─► merge 后 A5-4 GEB
    ├─► A5-3 TUI→app（1 agent） ──┘
    │
    └─► A5-2 前端绞杀
            D1–D3 薄壳并行（可多 agent）
            D4 log 单 agent
            D5 plan 单 agent 串行
            D6 chat 单 agent 串行
            D7–D9 收尾
```

---

*本文件是 2026-07-20 起的架构大改唯一实施真源。**A0–A5 主线已收口（A5-4 GEB 2026-07-21）**；A5-5 workspace crates **本轮不做**。变更阶段勾选时更新头部状态；GEB 与总账 P2-17 已同步。*
