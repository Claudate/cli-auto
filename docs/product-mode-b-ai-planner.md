# 产品模式 B：AI 规划拆分 → 定序 → 按图执行

> 状态：**B0/B1 主线已落地**；B2 主项已落地；**B3 已闭环**（D1/D3）；可选编辑 **P2-1/P2-2 已落地**（t30：删任务/改依赖 · replan 保人工修改）  
> 日期：2026-07-17（状态校正 2026-07-18；**D1 决议 · D3 边界 · D5 池 t15**）  
> 决议：用户明确选择 **B**（真·AI 规划定序），不是 A（仅解析计划里已有任务图）  
> 关联：[`desktop-ux-redesign-plan.md`](./archive/desktop-ux-redesign-plan.md)、[`claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md)、监视日志 → [`terminal-console-plan.md`](./archive/terminal-console-plan.md)、总账 → [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md) §1.3 / §4 D5、执行闭环 → [`plan-execute-inspect-rework-2026-07-19.md`](./plan-execute-inspect-rework-2026-07-19.md)（拆分·巡检·回补 · D5/P2-11；**不**改 confirm_start）  
> **勿再当缺口**：B0–B3 主线见总账 §1.3 / D3；B2 可选编辑 **P2-1/P2-2 已落地**（t30）

[PROTOCOL]: 变更本文件时更新状态与阶段勾选；与 UX 计划冲突时以本文件的「主流程」与 §4 默认规则为准。

---

## 0. 一句话

用户只选一份**计划文档**（可以是人话需求/大纲），cco **先用 AI 自动拆成带依赖的任务图并展示执行顺序**；桌面默认**停在拆分台**让人看清波次后点「确认并开始」（高级可开「拆分后自动开始」），再按顺序/并行拉起多个 worker CLI；界面上能看到**全部已分配的 CLI**，含排队、运行中、**已完成**、失败。

---

## 1. 决议：B，不是 A

| | **A. 解析定序** | **B. AI 规划定序（本决议）** |
|--|-----------------|------------------------------|
| 计划内容 | 作者写好 tasks + depends_on | 可以是散文、大纲、不完整列表 |
| 谁拆任务 | 解析器 / 人 | **Planner AI（一次或可重试）** |
| 谁定先后 | 文件里的 depends_on | **Planner 产出 depends_on / 波次** |
| 点开始前 | 预览已有图 | 桌面默认 **停拆分台**；高级可「拆分后自动开始」 |
| 现有能力 | ✅ 适配器 + 直接 exec / skip-plan | ✅ **B0/B1 主线已落地**（见总账 §1.3）；D1 产品规则已收口 |

**模式 A 不废弃**：若文件已是合法 `cco-plan/v1` / `serial-prompts/v0`，**自动 skip-plan**（或显式 `--skip-plan` / 桌面 `plan_mode=parse`），可走「已结构化 → 直接确认/执行」。  
**默认主路径是 B**：选计划 → 拆成步骤 → 拆分台确认 → 执行 → 监视（高级可拆完 auto-start）。

---

## 1.1 计划从哪来

| 来源 | 说明 |
|------|------|
| 已有文件 | 用户「选择计划」/ 指定 `.md`（主路径） |
| 聊天落盘 | 桌面「聊天」共建散文计划 → 保存 `plans/chat-*.md` 后再「分配计划」（见 [`chat-plan-builder-2026-07-18.md`](./archive/chat-plan-builder-2026-07-18.md)） |

聊天**只写计划文档**，不 spawn worker；分配之后仍走本节 Planner → `confirm_start`。

## 2. 目标用户流程（主路径）

> 计划来源：① 已有 `.md` 文件；② 桌面「聊天」共建后落盘（见 [`chat-plan-builder-2026-07-18.md`](./archive/chat-plan-builder-2026-07-18.md)）。二者分配后同一 Mode B 链。

```text
① 添加/选择项目
② 选择计划文档（.md 为主，不要求已写满 task 表；可无则先聊天生成）
③ 【规划阶段】启动 Planner AI（或自动）
      · 读计划全文 + 可选项目上下文（目录结构摘要等）
      · 输出：任务列表 + 依赖/波次 + 每任务 prompt
④ 【拆分台】界面展示（**默认停此屏**；高级「拆分后自动开始」可跳过，有业务可选时仍停）
      · 全部任务（标题、摘要）
      · 执行顺序：波次 1 → 波次 2 … 或依赖关系
      · 可并行标注
      · （v1 可选）用户微调：删任务 / 改依赖 / 重规划
⑤ **确认并开始**：用户点「确认并开始」→ `confirm_start`（高级 auto-start 时 UI 自动调用）
⑥ 【执行阶段】Scheduler 按 PlanIR + depends_on + max_parallel 跑 worker CLI
⑦ 【监视阶段】看见每个 CLI：
      · 排队（依赖未满足）
      · 运行中
      · 已完成（可点开日志）
      · 失败 / 已取消
```

### 2.1 用户心智（三句话）

1. 我丢给 cco 一份「要干什么」的文档。  
2. AI 帮我拆成步骤并排好谁先谁后（默认自动开跑；高级可先看一眼再确认）。  
3. 多个 Claude 按顺序干活，做完的、正在做的我都看得见。

---

## 3. 与现状能力对照

| 环节 | 现状（2026-07-18） | 备注 |
|------|-------------------|------|
| 选项目 / 选计划 | ✅ 桌面 + CLI | 保留 |
| 解析结构化 plan → PlanIR | ✅ adapters | 「已结构化」快路径 & Planner 输出落盘格式 |
| **Planner AI 拆分** | ✅ plan job + LLM / heuristic / fake | B1 主线；见 `src/plan/planner.rs` |
| **启动前定序确认 UI** | ✅ phase confirm + 拆分台三栏 | B0；桌面默认停台（`autoStartAfterPlan` false）；高级「拆分后自动开始」（D1 → S0 2026-07-20） |
| DAG 调度执行 | ✅ `depends_on` + `ready_tasks` + `max_parallel` | 复用 |
| 多 CLI 监视 | ✅ 主从 + waiting_on / current_wave | B2 主项已落地 |
| 已完成可理解 | ✅ 常驻列表 + 日志回看 | B2 主项已落地 |

**关键结论（校正）**：B0/B1 主线与 B2 监视主项 **已闭环**（总账 §1.3）。**D1 已收口、S0 翻转**：桌面**默认停拆分台**、CLI `run` 结构化 skip / 散文 plan job、`--skip-plan` 入口。**B3 已闭环**（上限 · 预算分栏 · 金样 · skip-plan · 真源；D3 / P1-4/5/6）。B2 可选编辑 **P2-1/P2-2 已落地**（删任务/改依赖 · replan 保人工；t30），**不是**「再做一个调度器」或「从零建 Planner」。

---

## 4. 产品阶段切分（运行生命周期）

一次「运行」在 B 下拆成两个子阶段（可共用一个 `run_id`，或 `plan_job` + `exec_run` 两段 id——实现时二选一，见 §7）。

```text
┌──────────── Plan Job ────────────┐    ┌──────── Exec Run ────────┐
│ pending → planning → planned     │ →  │ running → … → completed  │
│              ↘ failed_plan       │    │              ↘ failed    │
│ planned 可：replan / edit / start│    │                          │
└──────────────────────────────────┘    └──────────────────────────┘
```

| 状态（人话） | 含义 |
|--------------|------|
| 规划中 | Planner CLI/API 正在跑 |
| 待确认 / 拆分台 | 已有任务图，尚未开始 worker（**默认**停此屏；高级「拆分后自动开始」可跳过，有业务可选时仍停） |
| 运行中 | 已按图启动 workers |
| 已完成 / 失败 / 已暂停 | 与现网 run 状态对齐 |

### 4.1 产品默认规则（D1 2026-07-18；**P2-16 / S0 2026-07-20 默认翻转**）

| 项 | 决议 | 实现 |
|----|------|------|
| **桌面默认** | **拆分后停在拆分台**（须「确认并开始」） | `web/js/state.js` `autoStartAfterPlan` 默认 **false**（未写键或 `PAUSE_CONFIRM_KEY=1`）；高级 `#pp-auto-start`「拆分后自动开始」写 `0` 后才 auto |
| **业务 worker 入口** | **唯一** `confirm_start` → `start_run_from_plan` | 高级 auto-start = UI 自动调用 `confirm_start`，**不是**第二套 start API |
| **CLI `run`** | 可 parse 的**结构化** → 直接 exec（auto skip-plan）；**散文/未知** → plan job 后需 `--yes` 或交互确认 | `src/cli/mod.rs`；`--skip-plan` 强制跳过 |
| **结构化** | `cco-plan/v1` / `serial-prompts/v0` **自动 skip-plan**；亦可显式 `--skip-plan` / 桌面 `plan_mode=parse` | `plan::is_structured_adapter` |

**硬规则（修订）**：业务 worker **只**经 `confirm_start`（或 CLI 等价：打印 DAG + 确认/`--yes` 后进 Scheduler）启动。  
- 桌面默认：规划完成后 **停拆分台**，人工点「确认并开始」→ `confirm_start`；高级可开「拆分后自动开始」（有业务可选步骤时仍停台）。  
- CLI：散文路径必须见 DAG 并确认；结构化路径见 DAG 后同样需确认/`--yes`（与旧 `run` 一致）。  
- **禁止**绕过 `confirm_start`/CLI 确认直接 spawn worker（planner 自身除外）。

---

## 5. 界面规格（在 UX 浅色主从之上增量）

### 5.1 未运行：计划 → 规划

```text
选中计划后
  [拆成步骤]     ← 主按钮（B 默认：规划 → 停拆分台 → 确认并开始）
  高级 ▾ 规划方式 ai|parse|fake · 「拆分后自动开始」· 并发数
```

- 规划中：进度人话 + 秒数；**规划日志默认折叠**  
- 拆分台：波次时间线 · 步骤卡 · 详情；主 CTA「确认并开始」  
- 失败：错误摘要 + [重新拆分]  
- `plan_mode=parse`：文案「跳过规划（直接解析）」→ 结构化/可 parse 快路径

### 5.2 拆分台：编排结果（B 核心屏；**默认必见**；仅高级 auto-start 可能一闪而过）

```text
┌─────────────────────────────────────────────────────────┐
│  将执行 5 个任务 · 最多同时 2 个 · 预计 3 波              │
│  [重新拆分]  [让可并行的真正并行]  [确认并开始]          │
├──────────────┬──────────────────────────────────────────┤
│ 波次 / 列表  │  选中任务详情                             │
│ ● 第1波      │  标题、依赖、prompt 摘要                  │
│   T1 调研    │  （v1 只读；v1.1 可编辑）                 │
│   T2 脚手架  │                                          │
│ ● 第2波（等T1,T2）                                       │
│   T3 实现    │                                          │
│ ● 第3波      │                                          │
│   T4 测试    │                                          │
│   T5 文档    │                                          │
└──────────────┴──────────────────────────────────────────┘
```

**必须展示**（默认必见；高级 auto-start 时仍经同一 phase/API）

- 每个任务：标题、是否可与谁并行、依赖谁  
- 全局：波次顺序（由 `depends_on` 拓扑层推导，已有 `topo_layers`）  
- 主 CTA：**确认并开始**（业务 worker **唯一**启动入口 = `confirm_start`；默认人工点；高级 auto 时 UI 调用）

**不要展示（默认）**

- schema 名、adapter 名、raw run_id、provider 内部字段  

### 5.3 执行中：监视（承接 UX 计划）

| 区域 | 要求 |
|------|------|
| 顶栏 | 状态 · 已完成 a/b · 当前波次 · 用时 · 停止/继续 |
| 左列表 | **全部分配的任务**（含排队、运行、已完成、失败），状态中文 |
| 右日志 | 当前选中任务输出，≥14px |
| 已完成 | 可点开看日志；列表中明确「已完成」而非消失 |
| 排队 | 显示「等待：T1, T2」类依赖提示 |

### 5.4 与 `desktop-ux-redesign-plan.md` 的关系

| UX 计划已做 | 对 B 的价值 |
|-------------|-------------|
| 浅色、主路径、主从日志 | **直接复用**为规划日志 + 执行监视 |
| 选计划 + 拆成步骤 | 中间必须 **插入「规划 → 拆分台 → 确认并开始」**，不能选完直接旁路 spawn workers |
| 预览任务名 | 升级为 **拆分台**（波次时间线 + 步骤卡 + 依赖；默认停台） |

UX 计划状态若为「前端 0–4 已落地」，视为 **壳与监视底座**；B 的产品完成度以本文件阶段为准。**S0 后**与 `ux-simple-mainpath` / `product-mainpath-optimize` 同一默认句：拆分后停拆分台。

---

## 6. Planner 行为规格

### 6.1 输入

| 输入 | 说明 |
|------|------|
| 计划文件正文 | 用户选中的 md/yaml 全文 |
| project_root | 绝对路径 |
| （可选）仓库摘要 | 顶层目录、已有 `docs/plans`、package 名等——控制 token，v1 可先不做或极简 |
| 约束 | max_parallel 上限、默认 provider、语言（中文任务标题） |

### 6.2 输出（必须可校验为 PlanIR）

Planner **最终产物**必须是 host 能 `validate()` 的任务图，建议落盘：

```text
~/.cco/runs/<run_id>/
  source-plan.md          # 用户原文拷贝或引用路径
  planner/
    raw.log               # planner 会话日志
    plan.proposed.json    # 或 plan.proposed.yaml — 接近 cco-plan/v1
  plan.resolved.json      # 用户确认后冻结，供 scheduler 使用（现网已有写点）
```

**每条任务至少包含**

- `id`（稳定、唯一、适合文件名）  
- `title`（人话短标题；**可选项**标题须带「（可选）」）  
- `depends_on[]`  
- `prompt`（给 worker 的完整说明，含完成约定如 `CCO_DONE`）  
- 可选：`group` / 波次提示（可由依赖推导，非必须）  
- **可选项**：`optional: true` + `include`（默认 false；确认屏勾选后才进 `confirm_start` 的执行图）

### 6.3 Planner 实现策略（推荐分档）

| 档 | 做法 | 适用 |
|----|------|------|
| **B0 启发式** | 加强 `serial-prompts` / 标题拆段，无 LLM | 过渡、offline、fake |
| **B1 LLM Planner（目标）** | 专用 provider 调用：Claude CLI print 或 API，**单一 planner 任务**，要求输出严格 JSON/YAML | 默认产品路径 |
| **B2 可编辑** | 确认屏改依赖/删任务后再 validate | 增强 |

**B1 输出契约（示例要求写入 planner system prompt）**

- 只输出一个 JSON/YAML 块，schema 兼容 `cco-plan/v1` 子集  
- 依赖无环；任务数 2–N（设上限，如 20）  
- 可并行的独立任务不要串成一条长链（鼓励合理 parallel）  
- prompt 自包含，worker 不依赖聊天历史  

解析失败 → 状态 `failed_plan`，展示 raw 日志，支持重试。

### 6.4 与「业务 worker」隔离

- Planner 使用独立 task id，如 `__planner__`，**不计入**用户可见业务进度的「已完成 a/b」分母（或单独显示「规划 1 步」）。  
- Planner 的 CLI 日志在确认前占主日志区；确认开始后切换为业务任务监视。

---

## 7. 技术落点（实现纲要，非本阶段写码）

### 7.1 建议新增

| 模块 | 职责 |
|------|------|
| `plan/planner.rs`（名可议） | 调 provider → 解析 → `PlanIR::validate` |
| `services::plan_with_ai` / `start_plan_job` | 桌面/CLI 入口：异步规划 |
| `services::confirm_and_start` | 冻结 `plan.resolved.json` 再走现有 `start_run_async` 调度段 |
| Tauri cmd | `start_plan_job` / `get_plan_job` / `replan` / `confirm_start` |
| 前端状态 | `phase: pick \| planning \| confirm \| running \| done` |

### 7.2 可复用

| 现有 | 用法 |
|------|------|
| `PlanIR` / `TaskIR` / `validate` | Planner 输出目标类型 |
| `graph::topo_layers` / `ready_tasks` | 确认屏波次 + 执行调度 |
| `Scheduler` | confirm 之后原样跑 |
| `WorkerProvider`（claude/fake） | planner 与 worker 共用；fake 用于演示拆分 |
| 桌面主从日志 UI | planner 日志 + worker 日志 |

### 7.3 CLI 对齐（可选同期）

```text
cco plan  --project P --plan docs/x.md       # 只规划，打印 DAG，写 proposed
cco run   --project P --plan prose.md --yes  # 散文：plan job → 打印 DAG → exec
cco run   --project P --plan hello.cco.yaml  # 结构化 cco-plan/v1：自动 skip-plan → exec
cco run   --skip-plan --plan any.md          # 强制 parse 跳过 AI 规划
cco run   --plan-mode fake --yes …           # 规划用 fake DAG（调试）
```

---

## 8. 分阶段落地（B 专用）

### 8.0 并行策略（已采纳：允许并行）

**可以并行，但不能四条完全无依赖乱开。** 按「契约接口」对齐后，B0 前端壳 与 B1 后端 Planner **主轨并行**；B2/B3 大量工作可与之重叠，但有几条硬依赖。

```text
时间 →

  B0 ████████ 前端 phase + 确认壳 + confirm 才 start
       │ 契约：PlanPreview/PlanIR JSON、topo_layers、job 状态枚举
       ▼
  B1 ████████████ 后端 Planner + fake + Tauri/CLI API
            │ 接上 B0 的 planning/confirm 数据源
            ▼
  B2    ████████ 确认屏打磨 + 监视（排队/完成/波次）
            │ 需要 B0 壳；真数据优先 B1，mock 可先做
            ▼
  B3      ████████ 上限/预算/跳过规划/黄金用例/真源文档
              │ 文档与上限可早写；端到端金样等 B1 稳定
```

| 组合 | 可否并行 | 说明 |
|------|----------|------|
| **B0 ∥ B1** | ✅ **主并行对** | 前端状态机 / 确认壳 vs Rust Planner；先冻结 API 形状（见下） |
| **B0 ∥ B2 前半** | ✅ | 波次 UI、依赖文案可先接 `topo_layers(parse)` mock |
| **B1 ∥ B2 监视** | ✅ 部分 | 排队/已完成/顶栏波次主要动 `running` UI，不堵 Planner |
| **B1 ∥ B3 文档/上限常量** | ✅ | 真源 md、max_tasks 常量可先定 |
| **B2 可编辑依赖 ∥ B1** | ⚠️ 慎 | 编辑后 validate 依赖 PlanIR；可晚于 B1 核心 |
| **B3 黄金 E2E** | ❌ 需 B1 后 | 无结构 md → 真规划 → 执行 必须 B1 可跑 |
| **「开始运行」改入口** | ❌ 属 B0 先 | 否则 B1 仍会被旧路径绕过确认 |

**冻结契约（并行开工前 30 分钟对齐即可）**

```text
1. phase: pick | planning | confirm | running | done | plan_failed
2. PlanJob / 预览载荷：tasks[{id,title,depends_on,prompt?}], layers[][], max_parallel
3. cmds（名可微调）：
   - start_plan_job(project, plan, mode: ai|parse|fake)
   - get_plan_job(job_id) → status + proposed PlanIR/preview + planner log tail
   - confirm_start(job_id) → run_id   // 唯一启动业务 worker 入口
   - replan(job_id) 可选
4. 硬规则：start_run 对桌面主路径废弃或内部只给 confirm_start 调
```

**推荐并行排期（单人也可切上下文）**

| 轨 | 内容 | 阻塞解除条件 |
|----|------|----------------|
| 轨 A · 壳 | B0 全量 + B2 确认/监视 UI | 用 parse/fake 数据即可验收交互 |
| 轨 B · 脑 | B1 Planner + fake planner + CLI `cco plan` | 输出合法 PlanIR 文件即可，先不接漂亮 UI |
| 轨 C · 边界 | B3 文档、上限、skip-plan 设计 | E2E 金样等轨 B 绿 |

**汇合点（Definition of Done 分段）**

| 里程碑 | 条件 |
|--------|------|
| M-shell | B0：任意计划必经 confirm 才 exec（parse 冒充） |
| M-brain | B1：散文 md → proposed 合法 DAG（CLI 可验） |
| M-join | 轨 A 接真 `start_plan_job`，planning 看 planner 日志 |
| M-polish | B2 验收句「谁先跑/谁完成/谁在等」 |
| M-ship | B3 金样 + 真源文档 + 上限 |

**不要并行的反模式**

- 四阶段各写一套 `start_run` 入口 → 合并地狱  
- B2 可编辑依赖与 B1 输出 schema 同时大改且不共享 `PlanIR::validate`  
- 未冻结 log 路径就两边各读各的 planner 日志  

---

### 阶段 B0 — 产品与 UX 契约（文档 + 前端状态机壳）

| 任务 | 完成 |
|------|------|
| 本文件评审通过（主流程无歧义） | ✅ 2026-07-17 初稿 |
| 并行策略采纳（B0∥B1 主并行） | ✅ 2026-07-17 |
| 前端 `phase`：pick → planning → confirm → running | ✅ 2026-07-17 |
| 确认屏：波次列表（parse/fake/ai 结果） | ✅ 2026-07-17 |
| 「开始运行」仅经 `confirm_start` 进入 exec | ✅ 2026-07-17 |
| 更新 desktop UX 计划「主路径插入规划确认」 | ✅ 2026-07-17 |
| 与 B1 对齐：plan job API 形状（见 §8.0 契约） | ✅ 2026-07-17 |
| D1：默认 auto-start + 高级「规划后暂停确认」 | ✅ 2026-07-18（**S0 2026-07-20 默认翻转：停拆分台**） |

**验收**：业务 worker **只**经 `confirm_start`（默认人工点「确认并开始」；高级 auto-start 时 UI 自动调用）。

**并行**：与 B1、B2 前半、B3 文档轨同时进行。

---

### 阶段 B1 — Planner 管道（真 AI）

| 任务 | 完成 |
|------|------|
| Planner prompt 模板 + 严格输出格式 | ✅ 2026-07-17（LLM system prompt + JSON 契约） |
| 调用 claude provider 跑 planner，写 `plan.proposed.*` | ✅ 2026-07-17（失败回落启发式；fake 跳过 LLM） |
| 解析 + `PlanIR::validate`；失败可重试 | ✅ 2026-07-17 |
| fake planner（固定样例图）便于无 API 演示 | ✅ 2026-07-17 |
| Tauri/services：`start_plan_job` / `get_plan_job` / `confirm_start` | ✅ 2026-07-17 |
| 桌面：规划中看 planner 日志（接 B0 planning 相） | ✅ 2026-07-17（含异步 poll） |
| `ai` 启发式标题/段落拆分（fallback） | ✅ 2026-07-17 |
| CLI：`cco plan` | ✅ 2026-07-17 |
| CLI：`run` 默认先规划（散文）/ 结构化 skip-plan | ✅ 2026-07-18（D1 / P0-1·P0-2） |

**验收**：一篇无 task 表的 md → 得到合法多任务 DAG → 经 `confirm_start`（桌面默认 auto；CLI 确认/`--yes`）后按依赖执行。  
`plan_mode=ai`：优先 Claude LLM，失败/fake 时启发式。

**并行**：与 B0 同时开工；桌面接日志在 B0 planning UI 就绪后汇合（M-join）。

---

### 阶段 B2 — 确认体验与监视强化

| 任务 | 完成 |
|------|------|
| 波次 / 依赖人话展示（非 raw id 墙） | ✅ 2026-07-17（确认屏） |
| 排队中任务显示「等待：…」 | ✅ 2026-07-17（TaskLiveView.waiting_on） |
| 已完成任务常驻列表 + 可回看日志 | ✅ 已有主从监视（不消失） |
| 顶栏当前波次 | ✅ 2026-07-17（current_wave） |
| （可选）确认屏删任务 / 改依赖 | ✅ **P2-1 已落地**（`remove_proposed_task` · `depends_on` 编辑 · 确认屏删除/依赖勾选） |
| （可选）重新规划保留人工修改策略 | ✅ **P2-2 已落地**（`plan.user_edits.json` 按标题匹配 · `preserve_from_job_id` 重拆回放） |

**验收**：用户能不查文档回答「谁先跑、谁跑完了、谁在等谁」。

**并行**：确认屏展示 ∥ B0；监视强化 ∥ B1；「可编辑依赖」建议 M-brain 之后。

---

### 阶段 B3 — 质量与边界

| 任务 | 完成 |
|------|------|
| 任务数上限、prompt 长度、超时 | ✅ 2026-07-18（`MAX_TASKS=20` · `MAX_PROMPT_CHARS` · `MAX_TIMEOUT_SECS` · validate） |
| 规划预算与 worker 预算分离展示 | ✅ 2026-07-18（report Budget · live planner/exec · 顶栏 budget-chip） |
| 结构化计划「跳过规划」入口 | ✅ 2026-07-18（CLI `--skip-plan` + 自动结构化；桌面 `plan_mode=parse` 文案） |
| 黄金用例：散文 plan / 半结构化 / 已是 v1 | ✅ 2026-07-18（`tests/mode_b_golden.rs`） |
| 设计真源 `claude-cli-orchestrator-plan.md` 同步 B 流程 | ✅ 2026-07-18（P0-3 / D1） |

**并行**：上限常量 + 真源文档 + skip-plan 产品文案可与 B0/B1 同步；**黄金 E2E 等 B1 稳定**。

---

## 9. 成功标准

| 指标 | 目标 |
|------|------|
| 主路径 | 选计划 → 拆成步骤 → 拆分台确认 → 见多 CLI（含完成）；高级可 auto-start |
| 无结构 md | 不靠用户手写 depends_on 也能跑通 B1 |
| 定序可见 | 确认屏（暂停时）与执行中都能看出波次/依赖 |
| 已完成 | 不从列表消失，可回看 |
| 误启动 | 业务 worker 只经 `confirm_start` / CLI 确认；禁止旁路 spawn |
| 与 A 兼容 | 合法结构化 plan 自动/显式 skip-plan |

---

## 10. 非目标（本决议不包含）

- 云端托管多租户  
- 用 AI 替代 git/CI  
- 规划阶段无限多轮「产品经理对话」（v1 是单次规划 + 重试，不是聊天 IDE）  
- 推翻现有 Scheduler 重写  

---

## 11. 风险

| 风险 | 缓解 |
|------|------|
| Planner 输出不稳定 / 非 JSON | 强约束 prompt + 重试 + 展示 raw；fake 保底演示 |
| 拆得过碎或一条龙串行 | prompt 鼓励并行；max_tasks；确认屏可重规划 |
| 规划耗时与成本 | 单独状态与预算；可取消规划 |
| 与「直接 run」老用户习惯冲突 | `--skip-plan` / 高级「跳过规划」 |
| 仅做 UI 不接 Planner | 阶段 B0 可先 parse 冒充，但 **不得宣称 B 完成** 直到 B1 |

---

## 12. 决议记录

| 项 | 决议 | 日期 |
|----|------|------|
| 产品模式 | **B：AI 规划拆分 + 定序确认 + 按图执行** | 2026-07-17 |
| 默认是否跳过规划 | 散文否；**结构化自动 skip-plan**；亦可显式 `--skip-plan` | 2026-07-17；D1 修订 2026-07-18 |
| 桌面确认默认 | **拆分后停拆分台**；高级「拆分后自动开始」 | **D1 2026-07-18 · S0 2026-07-20** |
| CLI `run` | 结构化直接 exec；散文 plan job + 确认/`--yes` | **D1 2026-07-18** |
| 执行引擎 | 复用现有 DAG Scheduler | 2026-07-17 |
| UX 底座 | 复用浅色主从；主路径插入拆分台（默认停台） | 2026-07-17；D1 → S0 |
| 实现优先 | B0 状态机与确认屏 → B1 Planner → B2 监视强化 | 2026-07-17 |

---

## 13. 修订历史

| 日期 | 说明 |
|------|------|
| 2026-07-17 | 初稿：用户确认 B；流程、与 A/现状偏差、B0–B3 阶段 |
| 2026-07-17 | 开工：B0 前端 phase+确认屏；B1 plan job API（parse/fake/ai 启发式）；confirm 才 start；单测通过 |
| 2026-07-17 | 续：LLM planner（异步+回落）；`cco plan`；监视 waiting_on/波次；live 拉 resolved 计划 |
| 2026-07-18 | t5：冻结 B0/B1 主线为已完成；校正 §1/§3 叙事，避免再当缺口；残差指向总账 P0/B3 |
| 2026-07-18 | **D1 决议收口**：§4.1 默认规则（当时 auto-start · CLI run 路由 · 结构化 skip-plan）；消灭与 UX 双真相；B1/B3 勾选 |
| 2026-07-20 | **S0/F0**：§4.1 默认翻转为停拆分台；主 CTA「拆成步骤 / 确认并开始」；与 product-mainpath-optimize 对齐 |
| 2026-07-18 | t11：§0/§2/§5 叙事与 §4.1 对齐（一句话与主路径不再写「必须手点确认」）；B0 验收改为 confirm_start 唯一入口 |
