# cco 聊天构建计划（Chat → Plan → 分配）

> 状态：**已落地**（C0–C2 ✅ · 五指标全绿 · **§9 验证清单 t11 已冻（七项全绿）** · **§10 文档/GEB t12 已同步** · **§11 修订历史 t13 已闭环**；**C3 多会话 + 方案 B 开关 + 计划 diff + 流式 partial t14/t32–t34**）  
> 日期：2026-07-18  
> 范围：桌面 `web/` 聊天页 + 后端「对话写计划」服务 + 落盘 `.md` 后接入现有「分配计划」  
> 角色：主路径**增量**子计划——补齐「无合适计划时，用 AI 先聊出一份计划」；**不**另开第二套分配/Scheduler；**不**替代「选已有计划 → 分配」  
> 关联真源：
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md)（未完善唯一总账；**P-chat ✅ C0–C2** · C3→**D5/P2-9**；**勿**回灌已冻 D0–D4）
> - 主路径 → [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（三步主路径已落地；本计划为其可选支路）
> - Mode B → [`product-mode-b-ai-planner.md`](../product-mode-b-ai-planner.md)（分配/拆分执行真源；聊天只产出散文 `.md`）
> - UX 壳 → [`desktop-ux-redesign-plan.md`](./desktop-ux-redesign-plan.md)（桌面壳 0–4 已实施；本计划扩 `page=chat`）
> - 体验修补 → [`chat-ux-focus-2026-07-19.md`](./chat-ux-focus-2026-07-19.md)（注意力/fake/CTA · U0–U2 → **D5/P2-10**；**不**改本计划 C0–C2 勾选与方案 A）
> - 稳定性热修 → [`chat-utf8-fence-panic-2026-07-19.md`](./chat-utf8-fence-panic-2026-07-19.md)（plan fence UTF-8 · **P-chat-utf8** · F0 已落地；**不**改本计划 C0–C2 / 方案 A）
> GEB 入口：[`/CLAUDE.md`](../../CLAUDE.md)（L1）· [`./CLAUDE.md`](../CLAUDE.md)（L2 docs）  
> **不替代**现有「选已有计划 → 分配」；是**补齐**入口

> **定稿（t1）**：本前言冻结**角色 · 范围 · 关联真源四件套 · GEB 入口 · PROTOCOL**。  
> **定稿（t3）**：§1 现状分析（布局与能力）已对照 `web/` · `src/services` · `src-tauri` · `src/plan/planner` 冻结。  
> **定稿（t4）**：§2 产品目标与用户流程（增量八步 · 三句心智 · 入口可见性）已冻结。  
> **冻结（t5）**：§3 界面规格已冻结（实现走 §5 C0）。  
> **冻结（t6）**：§4 技术设计已冻结（实现已走 §5 C0–C2）。  
> **冻结（t7）**：§5 阶段切分与勾选冻结；**C0–C2 全 ✅**；C3 不排期则不碰。  
> **冻结（t8）**：§6 非目标（N1–N6）已冻结。  
> **冻结（t9）**：§7 成功标准（五指标全绿）已冻结。  
> **冻结（t10）**：§8 风险与决策默认（Q1–Q5 **按默认**）已冻结。  
> **冻结（t11）**：§9 验证清单（七项全绿 · 证据锚点 · 边界 · 修订条件）已冻结；`node --check` + `cargo test --lib` 34 passed。  
> **落地（t12）**：§10 文档与 GEB 同步完成；状态改 **已落地**；总账 **P-chat ✅** + C3→**D5/P2-9**；L1/L2 指针齐。  
> **闭环（t13）**：§11 修订历史（初稿 · C0–C2 · t1 / t3–t10 · t12 · 年表规则）**已闭环**；**既有行语义禁止改写**；后续变更 **另起行追加**。  
> 子节 §0–§11 为计划正文；实施勾选真源 = **§5**（C0–C2 ✅ · C3 不排期）；**不改产品默认、不触 C3**。  
> 与总账边界：**P-chat ✅** 记总账 §2；C3 → D5/P2-9；**禁止**第二份 P0–P2 总览，**禁止**回灌 D0–D4。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**聊天页常驻可用**：用户用自然语言和 AI 沟通，**先产出一份计划文档**；计划就绪后显示 **「分配计划」**，一点就跳进现有分配主路径（规划拆分 → 默认 auto-start / 暂停确认 → 监视）。

---

## 1. 现状分析（布局与能力）

> **定稿（t3）**：下列 IA / 能力表 / 断点 / 复用边界 / Mode B 硬规则 **已按 2026-07-18 工作树核对**；变更须先改代码再回写本表，禁止空改文档。

### 1.1 当前桌面信息架构

```text
侧栏：项目列表
顶栏：标题 · [选择计划] · [分配计划] · 拆分 chip · 预算 · 刷新
主区 page：
  welcome | workspace(pick/planning/confirm/running) | doctor | settings | help
弹层：plan-chooser（选计划 + 拆分选项 + 底部「分配计划」）
```

**DOM / 状态真源（已核）：**

| 壳层 | 证据 |
|------|------|
| 侧栏 `project-list` | `web/index.html` `#project-list`；`state.js` `renderProjectList` |
| 顶栏动作 | `#btn-plan-choose` · `#btn-pp-analyze` · `#split-plan-chip` · `#budget-chip` · `#btn-refresh` |
| page 枚举 | `state.page`: `welcome \| workspace \| doctor \| help \| settings`（**无 `chat`**）；`showPage` 仅上述分支 |
| workspace phase | `state.phase`: `pick \| planning \| confirm \| running \| done`（Mode B） |
| 弹层 | `#plan-chooser` + 底部 `#btn-chooser-assign` → `analyzePlanFromPicker()` |

| 能力 | 状态 | 锚点（代码） |
|------|------|------|
| 选项目 | ✅ | 侧栏 `#project-list` · `state.selectedPath` · `add_project` |
| 选**已有**计划文件 | ✅ | `#plan-chooser` · `list_plans`（`src/services/runs.rs` / `src/plan/mod.rs`）· `#btn-plan-choose` · `selectPlan` |
| AI **拆分**计划 → 任务图 | ✅ Mode B | `analyzePlanFromPicker` → `start_plan_job_cmd` → `start_plan_job` · phase `planning`/`confirm` |
| 分配后 auto-start | ✅ D1 | `state.autoStartAfterPlan`（默认 true）· `advancePlannedJob` → `confirmAndStart` → `confirm_start_cmd` |
| 多 CLI 监视 | ✅ | `#cli-board` · `web/js/log.js` / `monitor.js` · runtime `log_events` |
| **无计划时对话共建计划** | ❌ **缺口** | **无** `#page-chat` / `web/js/chat.js` / `chat_*` Tauri command / `src/services/chat*` |
| **从对话一键进入分配** | ❌ **缺口** | `analyzePlanFromPicker` **硬依赖** `state.selectedPlan`；无 plan 时仅 `openPlanChooser` + toast「请先选择计划」 |

### 1.2 主路径断点（用户语言）

现主路径（已落地，见 [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)）：

```text
加项目 → 选一份计划文档 → 分配计划 → AI 拆任务 → 跑
```

痛点（文案/空态证据）：

1. **没有合适计划**时只能自己先写 `.md` 或到项目外用别的工具写好再选文件。  
2. 「选择计划」弹窗空态只有「选择文件…」，**不会帮用户起草**  
   — `#chooser-empty`：「未发现计划文档。可点「选择文件…」指定一份 .md」；工具仅 `#btn-chooser-scan` / `#btn-chooser-pick`。  
3. 欢迎文案与帮助都假设用户**已有**计划文档  
   — welcome：`添加项目 → 选计划 → 点「分配计划」…`  
   — `#cli-empty`：`选一份计划 →「分配计划」→ 自动拆分并执行。`  
   — help 上手 ol：步骤 2 =「选择一份计划文档」。

### 1.3 可复用（禁止重造）

| 层 | 复用什么（现网符号） | 禁止 |
|----|----------------------|------|
| 分配 / 拆分 | `analyzePlanFromPicker` → `invoke("start_plan_job_cmd")` → `advancePlannedJob` / `confirmAndStart` → `confirm_start_cmd` · 后端 `services::start_plan_job` / `confirm_start`（`src/services/runs.rs` 注：Mode B **唯一**业务开跑入口） | 新开第二套 Scheduler / 聊天直调 `start_run*` 绕过 `confirm_start` |
| Provider | 本机 Claude CLI：`resolve_provider_bin`（`src/runtime/provider/mod.rs`）；Planner LLM 同路径（`src/plan/planner/llm.rs` · `job.rs`） | 在 web 直连 Anthropic API |
| 计划形态 | 落盘 **散文/大纲 `.md`**，再走 Mode B `plan_mode=ai`（chooser `#pp-plan-mode` 默认 `ai`） | 聊天直接产出 PlanIR 当执行图（那是「分配」/Planner 的职责） |
| 导航壳 | `showPage` + workspace `phase`；page 扩展加 `chat`，**不**改 phase 语义 | 把聊天塞进 `#plan-chooser` 弹窗（空间与职责都不够） |
| 会话缓存 | 参照 `state.planSessions` + `stashPlanSession` / `restorePlanSession`（按项目 path 缓存） | 切项目丢对话且无恢复；勿与 plan job 会话混写同一字段而不分 key |

### 1.4 与 Mode B 的边界（硬规则）

```text
【聊天 / 建计划】          【分配 / 拆分执行】← 已有 Mode B
  人话往返                  读计划文件
  产出一份 .md 计划文档  →  Planner 拆任务图 → confirm_start → workers
  不 spawn worker           业务 worker 只经 confirm_start
```

- 聊天 **只负责**「写出/改好计划文档」。  
- **「分配计划」之后**完全复用现网：planning → confirm → running（`autoStartAfterPlan` 时 UI 自动调 `confirm_start`，**不**另开业务入口）。  
- **禁止**聊天消息直接 `start_run` / 绕过 `confirm_start`（与 `src/services/runs.rs` · Mode B 真源一致）。

**衔接契约（为后续 C0–C2 预置，本任务不实现）：**

```text
chat_save_plan → 相对路径 plan_rel
  → selectPlan(plan_rel) 填 state.selectedPlan
  → showPage("workspace") + openPlanChooser(true)   // 方案 A
  → 用户点 chooser「分配计划」→ analyzePlanFromPicker()  // 与顶栏同源
```

---

## 2. 产品目标与用户流程

> **定稿（t4）**：下列**增量八步流程 · 三句心智 · 入口可见性**为产品真源；与 §1 断点对齐、与 §3 界面规格 / §8 Q2·Q5 一致。  
> 变更须先改产品决议再回写本表；**禁止**空改文档。**本任务不写实现代码**（实现走 §5 C0–C2）。

### 2.1 目标用户流程（增量）

在现网主路径（选已有计划 → 分配）**之外**补一条「无合适计划时」支路；**不**替换原路径。

```text
① 侧栏选项目（与现网一致）
② 打开「聊天」（常驻入口；无计划时也可从空态 CTA 进）
③ 与 AI 多轮沟通：目标、约束、范围、验收…
④ AI 给出「计划草稿」；用户可继续改
⑤ 用户认可 → 「保存为计划」落盘 project 下 .md
⑥ 界面出现主 CTA「分配计划」
⑦ 点击 → 选中该计划 + 跳到 workspace 分配流
      · 默认（方案 A / §8 Q2）：打开 plan-chooser 且已选中新建计划，用户可调并发/规划方式后点分配
      · 或一键（方案 B）：直接 analyzePlanFromPicker（选项用当前/默认）—— v1 不做，见 §3.3 / C3
⑧ 之后 = 现网 Mode B（规划中 → 确认/auto-start → CLI 看板）
```

| 步 | 用户动作 | 系统结果（契约） |
|----|----------|------------------|
| ① | 侧栏点项目 | `state.selectedPath`；与现网一致 |
| ② | 顶栏/侧栏「聊天」或空态 CTA | `showPage("chat")`；按项目恢复会话 |
| ③–④ | 多轮澄清 + 草稿 | 仅对话态；**不** spawn worker |
| ⑤ | 「保存为计划」 | 落盘 `.md` → `chatDraftPlan` / 可 `selectPlan` |
| ⑥–⑦ | 「分配计划」 | 方案 A：预选 plan + `openPlanChooser` → 用户点分配 → `analyzePlanFromPicker` |
| ⑧ | （现网） | planning → confirm/auto-start → CLI 看板；**同源** `confirm_start` |

### 2.2 用户心智（三句话）

1. 没有计划？去**聊天**说清楚要干什么。  
2. AI 帮我写成一份计划文档。  
3. 点**分配计划**，后面和选文件分配一样。

### 2.3 入口与可见性

| 场景 | 入口 | 优先级 |
|------|------|--------|
| 任意已选项目 | 顶栏或侧栏常驻 **「聊天」**（与「选择计划」并列级，**不抢**「分配计划」主色 / §8 Q5） | 常驻 · ghost |
| workspace 空态（无 plan / 未跑） | `#cli-empty` 增加次要链：「没有计划？和 AI 聊聊先写一份」 | 次要链 |
| plan-chooser 空列表 | 次要按钮「用聊天生成计划」→ 关弹窗进聊天页 | 次要按钮 |
| 欢迎页（未选项目） | 不强制聊天；仍「添加项目」优先 | 不展示/不强制 |

**聊天页一直可用**：

- 切换 doctor/settings/help 后再回聊天，**按项目恢复**会话（同 `planSessions` 思路）。  
- 运行中也可打开聊天（只读历史 + 可继续改**下一份**计划）。  
- 运行中点「分配」走现网 `toastRunLocked` / `hasActiveRun` 锁——**不**另开第二套锁。

---

## 3. 界面规格 — **t5 已冻结**

> **冻结（t5）**：下列为 2026-07-18 对照后的**聊天建计划界面规格唯一真源**（流程见 §2；实现锚点见 §3.5；阶段见 §5 C0–C2）。  
> 本 § **只定 UI 信息架构 / 布局 / 就绪态 / 与顶栏分配关系**；**不**写后端 API（→ §4）、**不**勾 C0 实现（→ §5）。  
> 执行态：规格已冻；**C0 起**按本 § 落地 `#page-chat` 与顶栏入口；子计划 / 实现 **不得**另写平行 page 枚举或第二套分配入口。  
> 默认决议与 §8 Q2/Q5 一致：**分配跳转 = 方案 A**；聊天 **ghost**、分配仍 **primary**。

### 3.1 信息架构（增量）

```text
page 枚举扩展：welcome | workspace | chat | doctor | help | settings

顶栏（已选项目时）：
  [聊天]  [选择计划]  [分配计划]   …chip…
         ↑ ghost     ↑ primary（有 selectedPlan 且可分配时）
```

| 项 | 冻结规则 | 现网对照（t5） |
|----|----------|----------------|
| `state.page` | 在现有五页上 **+ `chat`**；`showPage("chat")` 切主区 | `web/js/state.js`：`page: "welcome" \| "workspace" \| "doctor" \| "help" \| "settings"`（尚无 `chat`） |
| 主区 section | 新增 `#page-chat.page`；与 `#page-workspace` 等同级 | `web/index.html`：`#page-welcome` · `#page-workspace` · `#page-doctor` · `#page-settings` · `#page-help` |
| 顶栏「聊天」 | 已选项目时可见；**ghost**；id 建议 `#btn-open-chat` | 现顶栏：`#btn-plan-choose`（ghost）· `#btn-pp-analyze`（primary「分配计划」） |
| 顶栏「选择计划」 | **仍 ghost**；行为不变 | `#btn-plan-choose` → `openPlanChooser` |
| 顶栏「分配计划」 | **仍 primary**；有 `selectedPlan` 且可分配时启用 | `#btn-pp-analyze` → `analyzePlanFromPicker`（无 plan 时开 chooser） |
| chip / 预算 / 刷新 | 位置与现网一致，**不**因聊天改序 | `#split-plan-chip` · `#budget-chip` · `#btn-refresh` |

**禁止**：把聊天塞进 `plan-chooser` 弹层；把「聊天」做成 primary 抢「分配计划」主色（§8 Q5）。

### 3.2 聊天页布局

```text
┌──────────────────────────────────────────────────────────┐
│ 与 AI 共建计划 · {项目名}                                  │
│ 会话：默认 / 历史下拉（v1 可只做当前会话）                    │
├──────────────────────────────────────────────────────────┤
│  消息流（用户 / 助手）                                       │
│  · 助手可夹「计划卡片」：标题 + 摘要 + 任务大纲预览           │
│  · 流式/分块显示（若 CLI 流可用；否则整段）                   │
├──────────────────────────────────────────────────────────┤
│  [计划就绪时]  已保存：plans/foo.md                         │
│               [打开预览]  [分配计划]  ← 主 CTA                │
├──────────────────────────────────────────────────────────┤
│  输入框（多行）                              [发送]          │
│  提示：说清目标与约束；满意后点「生成计划文档」或让 AI 收口   │
└──────────────────────────────────────────────────────────┘
```

| 区域 | 职责 | v1 最低 | 延后 |
|------|------|---------|------|
| 页头 | 标题「与 AI 共建计划 · {项目名}」；会话切换 | 标题 + 当前会话；`showPage("chat")` 时 `#page-title` =「共建计划」 | 历史下拉 → C3 多会话 |
| 消息流 | 用户 / 助手气泡；助手可夹计划卡片 | 整段渲染；卡片 = 标题 + 摘要 + 大纲预览 | 流式 → C3（CLI stream 可用时） |
| 就绪条 | 仅 `chatDraftPlan` 有路径时显示 | 路径文案 +「打开预览」+「分配计划」主 CTA | — |
| 输入区 | 多行输入 +「发送」；底提示 | 发送进会话；提示固定一句 | 「生成计划文档」独立按钮可与 AI 收口并存（C0/C2） |

**计划卡片**（助手消息内嵌，非独立 page）：

- 展示：标题 · 摘要 · 任务大纲预览（列表级即可）  
- 动作：「采用此稿并保存」→ 落盘后写 `state.chatDraftPlan`（见 §3.3）  
- **禁止**卡片上直接「开始运行 / spawn worker」

### 3.3 计划就绪态（关键）

满足任一即显示「分配计划」：

1. 本会话已 **成功落盘** 至少一份 `.md`（`state.chatDraftPlan` 有路径）；或  
2. 用户点了助手消息上的「采用此稿并保存」。

按钮行为（推荐默认 **A**）：

| 方案 | 行为 | 选用 |
|------|------|------|
| **A** | 设 `selectedPlan` → `showPage(workspace)` → `openPlanChooser(true)` 并预选 → 用户确认选项后点分配 | **默认**（与现网「选计划+选项」一致，少误触开跑） |
| B | 设 `selectedPlan` → 直接 `analyzePlanFromPicker()` | 高级/二次确认后可做；**v1 不做**（§8 Q2） |

「分配计划」文案与顶栏主按钮一致；聊天页内按钮仅在有 `chatDraftPlan` 时启用。

| 状态 | 聊天页「分配计划」 | 说明 |
|------|-------------------|------|
| 无 `chatDraftPlan` | **disabled** | 可 toast「请先保存计划」 |
| 有 `chatDraftPlan`，无 active run | **enabled** → 方案 A | 见 §3.5 伪代码锚点 |
| 有 active run | 走现网锁 | `hasActiveRun()` → `toastRunLocked("分配计划")`（与顶栏同源） |

### 3.4 与顶栏「分配计划」关系

- 顶栏「分配计划」：**仍** = 对 `selectedPlan` 开跑拆分（无 plan 时开 chooser）。  
- 聊天页「分配计划」：先绑定聊天落盘的 plan，再进 chooser/分配。  
- 二者最终都进 `analyzePlanFromPicker`，**不**分叉业务。

```text
顶栏 #btn-pp-analyze          聊天 #btn-chat-assign（建议 id）
        │                              │
        │  selectedPlan 已有            │  selectPlan(chatDraftPlan)
        │  或无 → openPlanChooser       │  → showPage("workspace")
        │                              │  → openPlanChooser(true) 预选
        └──────────────┬───────────────┘
                       ▼
              analyzePlanFromPicker()
                       ▼
              start_plan_job → confirm_start（现网 Mode B）
```

### 3.5 现网锚点与建议 DOM/API（实现对照表）

> t5 **只冻结对照关系**；id 名以建议为准，C0 落地时可微调，但 **不得**改 §3.1–3.4 行为语义。

| 规格点 | 建议 id / 字段 | 复用现网 |
|--------|----------------|----------|
| 聊天 page | `#page-chat` | `showPage` 扩 `chat` 分支；`$$(".page")` 已按 `page-${name}` 切换 |
| 顶栏入口 | `#btn-open-chat` | 已选项目时与 `#btn-plan-choose` 同显隐策略 |
| 消息列表 | `#chat-messages` | 新；样式 `web/css/chat.css` |
| 计划卡片 | `.chat-plan-card` | 新；内嵌「采用此稿并保存」 |
| 就绪条 | `#chat-draft-bar` | 有 `state.chatDraftPlan` 时显示 |
| 打开预览 | `#btn-chat-preview` | 可复用现有 plan 预览路径或只读打开 rel 路径 |
| 聊天分配 CTA | `#btn-chat-assign` | 文案「分配计划」；方案 A |
| 输入 / 发送 | `#chat-input` · `#btn-chat-send` | 新 |
| 状态字段 | `state.chatSession` · `state.chatDraftPlan` · `state.chatBusy` | 旁路；**不**改 `phase`（phase 仅 workspace） |
| 选中计划 | `selectPlan(rel)` | `web/js/plan.js` 已导出行为 |
| 开 chooser | `openPlanChooser(true)` | 同；预选 = 先 `selectPlan` 再开 |
| 分配 | `analyzePlanFromPicker()` | 唯一业务入口（与 chooser 底栏 `#btn-chooser-assign` 同） |
| 运行锁 | `hasActiveRun` · `toastRunLocked` | `web/js/state.js` |

**方案 A 伪代码（冻结行为，与 §4.3 一致）**：

```javascript
// 聊天页「分配计划」— 不得绕过 analyzePlanFromPicker
async function assignFromChat() {
  if (!state.chatDraftPlan) return toast("请先保存计划");
  if (hasActiveRun()) return toastRunLocked("分配计划");
  await selectPlan(state.chatDraftPlan);
  showPage("workspace");
  openPlanChooser(true);
  // 用户点 chooser「分配计划」→ analyzePlanFromPicker()
}
```

### 3.6 边界（防与 §2 / §4 / §5 / Mode B 混淆）

| 勿再写入本 § / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 在本 § 实现 `#page-chat` / 写 `chat.js` | **§5 C0** 实现任务 |
| 在本 § 定义 `chat_send` / 落盘路径 | **§4** 技术设计 |
| 聊天消息直接 `start_run` / `confirm_start` | **禁止**；Mode B 硬规则 §1.4 |
| 聊天产出 PlanIR JSON 当执行图 | **禁止**；散文 `.md` + 分配（§6 非目标） |
| 方案 B 作 v1 默认 | **禁止**；v1 = 方案 A（§8 Q2） |
| 聊天按钮 primary、分配改 ghost | **禁止**；§8 Q5 |
| 把聊天塞进 `plan-chooser` | **禁止**；§1.3 导航壳 |
| 改 `state.phase` 为 chat 相位 | **禁止**；phase 仅 workspace（pick/planning/confirm/running/done） |
| 另开第二份「聊天 UI 规格」 | **禁止**；只维护本 §3 |
| 欢迎页未选项目强制进聊天 | **禁止**；§2.3 欢迎仍「添加项目」优先 |

### 3.7 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 改 page 枚举 / 顶栏按钮角色 / 方案 A↔B 默认 | 改 §3.1–3.4 + 头部「§3 冻结」句，**同 commit** 回写 L1/L2 指针；**须显式产品决议** |
| 仅 C0 落地建议 id 微调（行为不变） | 可改 §3.5 对照表；**不**改 §3.1–3.4 语义 |
| 流式 / 多会话 / 方案 B 开关 | **C3**；出池或排期后再改本 § 对应行 |
| 后端 API / 落盘路径变更 | 改 **§4**，本 § 仅当就绪态字段名变时回写 `chatDraftPlan` 表述 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 4. 技术设计 — **t6 已冻结**

> **冻结（t6）**：下列为 2026-07-18 对照工作树后的**聊天建计划技术设计唯一真源**（流程见 §2；UI 规格见 §3；默认见 §8；阶段勾选见 §5）。  
> 本 § **只定**后端 API / 落盘 / 模型调用 · 前端文件图 · 方案 A 分配跳转 · 状态机关系；**不**改产品默认、**不**扩 C3、**不**在本任务写实现代码。  
> 执行态（2026-07-18）：**C0–C2 已按本设计落地**；下列符号与路径以工作树为准。  
> 子计划 / 后续 PR **不得**另写平行 `chat_*` API 表或第二套分配跳转。

### 4.1 后端（`src/services` + Tauri 薄壳）

**模块（已落地）**：`src/services/chat.rs`，经 `src/services/mod.rs` re-export；`src/lib.rs` 对外 re-export；`src-tauri/src/lib.rs` 只挂薄 command（`chat_*_cmd`），**禁止**在 Tauri crate 堆业务。

| 服务 API | Tauri command | 入参 | 出参 | 说明 |
|----------|---------------|------|------|------|
| `chat_session_get` | `chat_session_get_cmd` | `project`, `session_id?`（默认 `"default"`） | `ChatSession`：`{ session_id, project, messages[], draft_plan?, updated_at? }` | 读盘；无文件 → 空会话 |
| `chat_send` | `chat_send_cmd` | `project, message, session_id?` | `ChatSendResponse`：`{ session_id, reply, messages[], draft_plan?, fake }` | 同步一轮；Claude CLI print 或 fake |
| `chat_save_plan` | `chat_save_plan_cmd` | `project, session_id?, title?, markdown` | `ChatSavePlanResponse`：`{ plan_rel, abs_path, session_id }` | 写项目下计划 `.md` 并回写会话 draft |
| `chat_list_sessions` | `chat_list_sessions_cmd` | `project` | `ChatSessionSummary[]` | **C3 t32 ✅**；无文件时含合成 `default` |
| `chat_new_session` | `chat_new_session_cmd` | `project`, `title?` | `ChatSession` | **C3 t32 ✅**；id=`s-YYYYMMDD-HHMMSS` |
| `chat_delete_session` | `chat_delete_session_cmd` | `project`, `session_id` | `()` | **C3 t32 ✅**；删 JSON + attachments 目录 |

**类型要点（`src/services/chat.rs`）**：

| 类型 | 字段要点 |
|------|----------|
| `ChatMessage` | `role` · `content` · `at?` |
| `ChatDraftPlan` | `path`（相对 project）· `title?` · `markdown?` · `saved` |
| `ChatSession` | `session_id` · `project` · `messages` · `draft_plan?` · `updated_at?` · `title?`（C3） |
| `ChatSessionSummary` | `session_id` · `title?` · `updated_at?` · `message_count` · `preview?` · `draft_plan_path?` · `draft_plan_title?`（C3 list） |

**落盘约定（v1 · 与 §8 Q1/Q4 一致）**：

```text
{project}/.cco/chat/{session_id}.json     # 消息与元数据（cco 私有；session_id 安全化）
{project}/plans/chat-{YYYYMMDD-HHMM}.md   # 用户可见计划（优先；可 create_dir_all plans/）
  若 plans/ 不可用：{project}/cco-plan-{YYYYMMDD-HHMM}.md
```

- `list_plans`（`src/plan/mod.rs`）扫 `plans/**.md` 与根 `cco-plan-*.md`：保存后应出现在 chooser。  
- `chat_save_plan` 返回 **相对 project 的 `plan_rel`**，前端 `selectPlan(rel)` / `state.chatDraftPlan = plan_rel`。  
- fence 解析成功只预填 `draft_plan.markdown`（`saved=false`）；**须**用户点「保存」/`chat_save_plan` 再写盘（§8 Q3）。

**模型调用（v1）**：

- 复用 Planner 同源：`ClaudeProvider` + `resolve_provider_bin`（`CCO_CLAUDE_BIN`），`mode=print`。  
- System 角色固定为「cco 计划写作助手」：引导澄清 → 产出 Markdown 计划（标题/目标/范围/任务大纲/验收），**不要**输出 cco-plan/v1 JSON（JSON 是分配阶段 Planner 的事）。  
- 收口时 assistant 用约定 fence 包计划正文；服务端 `extract_plan_fence` 解析（last fence wins）：

````markdown
```plan
# 标题
...
```
````

- fake 路径：`CCO_CHAT_FAKE=1` / `default_provider=fake` / CLI 失败 soft-fallback → 固定模板（含 ` ```plan `），`ChatSendResponse.fake=true`，便于无密钥联调。  
- 硬边界：`chat_*` **不** spawn 业务 worker、**不**调 `confirm_start` / `start_plan_job`（Mode B §1.4 / §6 N1）。

**证据锚点（2026-07-18）**：`src/services/chat.rs` · `src/services/mod.rs` · `src/lib.rs` re-export · `src-tauri/src/lib.rs`（`chat_session_get_cmd` / `chat_send_cmd` / `chat_save_plan_cmd` + `generate_handler!`）· `cargo test --lib` 含 `services::chat`。

### 4.2 前端（`web/`）

| 文件 | 改动（已落地） |
|------|----------------|
| `web/index.html` | `#page-chat` 四区；顶栏 `#btn-open-chat`（ghost）；`#btn-empty-to-chat` / `#btn-chooser-to-chat`；script 顺序含 `js/chat.js` |
| `web/js/state.js` | `page` 含 `"chat"`；`chatSession` / `chatDraftPlan` / `chatBusy`；`showPage("chat")` → 标题「共建计划」 |
| `web/js/chat.js` | `loadChatSession` · `sendChatMessage` · `saveChatPlan` · `assignFromChat` · `renderChatPage`；`invoke("chat_*_cmd")` |
| `web/js/plan.js` | `selectPlan` 复用；顶栏/空态与 chat 显隐；切项目清 chat 态；chooser 链 |
| `web/js/doctor.js` | 按钮委托：`btn-open-chat` / `btn-empty-to-chat` / `btn-chooser-to-chat` / `btn-chat-send` / `btn-chat-save` / `btn-chat-assign` / `btn-chat-preview`；Enter 发送 |
| `web/css/chat.css` + `app.css` `@import` | 消息气泡 · 计划卡片 · 就绪条 · 输入区 |
| `web/app.js` | 入口注释：加载序 `state → plan → monitor → log → chat → doctor` |
| `web/CLAUDE.md` | 成员清单含 chat |

**DOM / 状态字段（与 §3.5 对齐）**：

| 规格 | id / 字段 |
|------|-----------|
| page | `#page-chat` · `state.page === "chat"` |
| 顶栏入口 | `#btn-open-chat`（ghost；不抢 `#btn-pp-analyze` primary） |
| 消息 / 输入 | `#chat-messages` · `#chat-input` · `#btn-chat-send` |
| 就绪条 | `#chat-ready-bar` · `#chat-saved-path` · `#btn-chat-save` · `#btn-chat-preview` · `#btn-chat-assign` |
| UI 态 | `state.chatSession` · `state.chatDraftPlan`（已落盘 rel）· `state.chatBusy` |

### 4.3 分配跳转（精确行为 · 方案 A）

```javascript
// 聊天页「分配计划」— 不得绕过 analyzePlanFromPicker
async function assignFromChat() {
  if (!state.chatDraftPlan) return toast("请先保存计划");
  if (hasActiveRun()) return toastRunLocked("分配计划");
  await selectPlan(state.chatDraftPlan); // 相对路径
  showPage("workspace");
  openPlanChooser(true);                 // 方案 A：带选项再分配
  updateChooserAssignState();
  // 用户点 chooser 内「分配计划」→ analyzePlanFromPicker()
}
```

- 已落地增强：chooser 打开时 toast「已选中聊天生成的计划…」。  
- **禁止**方案 B 作 v1 默认（直开 `analyzePlanFromPicker`）——见 §3.3 / §5 C3 / §8 Q2。  
- 运行锁与顶栏同源：`hasActiveRun` / `toastRunLocked`。

### 4.4 状态机关系

```text
state.page = welcome | workspace | chat | doctor | help | settings
state.phase 仅 workspace 有效：pick | planning | confirm | running | done

聊天不改 phase；保存计划只影响 selectedPlan + chatDraftPlan。
分配开始后 phase 仍由 plan.js 推进（planning → confirm/auto-start → running）。
```

| 规则 | 说明 |
|------|------|
| page vs phase | `showPage("chat")` **不**写入 `phase`；phase 语义仍只服务 workspace |
| 会话恢复 | 按项目磁盘 `.cco/chat/{session}.json`；切页不丢；切项目隔离 |
| 与 Mode B | 聊天结束于「有 `plan_rel`」；开跑只经 `analyzePlanFromPicker` → `start_plan_job` → `confirm_start` |

### 4.5 边界（防与 §3 / §5 / §6 / Mode B 混淆）

| 勿再写入本 § / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 在本 § 实现流式 / 多会话列表 / 方案 B | **C3**；不排期则不碰 |
| 另写平行 `chat_*` API 或第二套 Scheduler | **禁止**；只维护本 §4 |
| 聊天直调 `start_run` / `confirm_start` | **禁止**；§6 N1 / Mode B |
| 聊天产出 PlanIR JSON 当执行图 | **禁止**；§6 N2；散文 `.md` + 分配 |
| 会话写 `~/.cco` 全局 | **禁止**；§8 Q4 = 项目内 `.cco/chat/` |
| fence 自动写盘 | **禁止**；§8 Q3 手动保存 |
| 改 Tauri 薄壳为业务堆栈 | **禁止**；业务在 `src/services/chat.rs` |
| 把本设计回灌为「待实施」而忽略 C0–C2 证据 | **禁止**；与工作树冲突时先改代码再改本 § |

### 4.6 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 改 API 入参/出参、落盘路径、模型调用路径、方案 A 跳转语义 | 改 §4.1–4.4 + 头部「§4 冻结」句，**同 commit** 回写 L1/L2 指针；**须与代码同改** |
| 仅建议 id / 注释微调（行为不变） | 可改证据锚点路径；**不**改契约表语义 |
| 实现 `chat_list_sessions` / 流式 | **C3** 出池后再扩本 § 对应行 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 5. 阶段切分与勾选 — **t7 已冻结**

> **冻结（t7）**：下列为 2026-07-18 对照工作树后的**聊天建计划实施阶段唯一勾选真源**（流程见 §2；UI 规格见 §3；API 见 §4）。  
> 本 § **只定 C0–C3 切分、勾选态与证据锚点**；**不**改产品默认（§8）、**不**扩写 C3、**不**在本任务写实现代码。  
> 执行态：**C0–C2 全 ✅ 已闭环**；**C3 不排期则不碰**（可进总账 D5 池出池后再动）。  
> 子计划 / 后续 PR **不得**另写平行阶段序列，或把 C0–C2 回灌为「待实施」。

### 5.0 阶段总览

| 阶段 | 目标 | 状态 | 证据目录 |
|------|------|------|----------|
| **C0** | 产品与壳（UI 骨架；可 mock） | ✅ | `web/index.html` · `web/js/{state,chat,doctor,plan}.js` · `web/css/chat.css` |
| **C1** | 后端一轮对话 + 落盘 | ✅ | `src/services/chat.rs` · `src/services/mod.rs` · `src-tauri/src/lib.rs` |
| **C2** | 前端接通与分配跳转 | ✅ | `web/js/chat.js` · 帮助/空态链 · 方案 A `assignFromChat` |
| **C3** | 打磨（流式 / 多会话 / 方案 B / diff） | ✅ **t32–t34** 多会话 · 方案 B · 计划 diff · 流式 partial | `chat_list/new/delete` · `#s-chat-assign-direct` · `plan-full-diff` · `chat_stream_partial` |

**建议实施序**：C0 → C1 → C2 同迭代闭环（**已完成**）；C3 不排期则不碰。

### C0 — 产品与壳（UI 骨架，可 mock）✅

| 勾选 | 证据锚点（2026-07-18） |
|------|------------------------|
| [x] `#page-chat` + 顶栏「聊天」入口 | `web/index.html` `#page-chat` · `#btn-open-chat`（ghost） |
| [x] 空态 / chooser 链入聊天 | `#btn-empty-to-chat` · `#btn-chooser-to-chat`；`doctor.js` 委托 `openChatPage` |
| [x] 本地 mock 消息流（无后端） | `CCO_CHAT_FAKE` / 无 CLI 时 fake 回落（`src/services/chat.rs`） |
| [x] 计划卡片 + 禁用态「分配计划」 | `.chat-plan-card` · `#btn-chat-assign` 默认 `disabled` |
| [x] `showPage("chat")` 标题「共建计划」 | `web/js/state.js` `name === "chat"` → `#page-title` =「共建计划」 |

### C1 — 后端一轮对话 + 落盘 ✅

| 勾选 | 证据锚点（2026-07-18） |
|------|------------------------|
| [x] `services/chat`：session 读写、`chat_send`、`chat_save_plan` | `src/services/chat.rs` · `mod.rs` re-export |
| [x] Tauri commands 注册 | `src-tauri/src/lib.rs`：`chat_session_get_cmd` · `chat_send_cmd` · `chat_save_plan_cmd` |
| [x] Claude CLI print + plan fence 解析；fake 回落 | `extract_plan_fence` · `CCO_CHAT_FAKE`；system 收窄为计划写作 |
| [x] 落盘路径进 `list_plans` | 优先 `plans/chat-*.md`，否则根 `cco-plan-*.md`；`list_plans` 扫项目计划 |

### C2 — 前端接通与分配跳转 ✅

| 勾选 | 证据锚点（2026-07-18） |
|------|------------------------|
| [x] `chat.js` 真 API | `invoke("chat_send_cmd")` / `chat_save_plan_cmd` / `chat_session_get_cmd` |
| [x] 保存后启用「分配计划」→ 方案 A 进 chooser | `assignFromChat`：`selectPlan` → `showPage("workspace")` → `openPlanChooser(true)` |
| [x] 按项目恢复会话 | `.cco/chat/{session}.json`；进页 `chat_session_get` 回填 |
| [x] 运行中分配锁定与现网一致 | `hasActiveRun()` → `toastRunLocked("分配计划")`（与顶栏同源） |
| [x] 帮助文案补一句「可先聊天生成计划」 | `#page-help` 上手 li：无计划时可用顶栏**聊天** |

### C3 — 打磨（可跟 D5 池）✅ t32–t34

> **t32–t34 出池切片**：多会话 · 方案 B · 计划 diff · 流式 partial 已落地。**勿**回灌本 § 把 C0–C2 勾掉。

- [x] 流式输出 — `chat_stream_partial` 轮询 `__chat__` stdout 增量；失败降级 wait label；完成后仍整段 `chat_send` 落盘（**t34**；非 token 级 SSE，CLI print 路径可接受）  
- [x] 多会话列表 — `chat_list_sessions` / `chat_new_session` / `chat_delete_session` · Tauri `*_cmd` · `#chat-session-select` · 缓存键 `project::session_id`（**t32**）  
- [x] 聊天内直接「分配并沿用上次并发」（方案 B 开关）— 设置 `#s-chat-assign-direct` 默认关；开则 `startExecuteFromSelection` → `analyzePlanFromPicker`（**仍经 Mode B**，不跳 `confirm_start`；**t33**）  
- [x] 计划 diff — plan-full modal「对比改动」磁盘稿 vs 当前草稿；LCS 行 diff；采用左/右写回草稿，落盘仍 `chat_save_plan`（**t34**；非第二套编辑器）  

### 5.1 边界（防与 §3 / §4 / §6 / Mode B / 总账混淆）

| 勿再写入本 § / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 把 C0–C2 改回 ☐「待实施」 | **禁止**；与工作树证据冲突时先改代码再改勾选 |
| 在本 § 实现流式 / 多会话 / 方案 B / diff | **C3**；不排期则不碰 |
| 聊天消息直接 `start_run` / `confirm_start` | **禁止**；Mode B 硬规则 §1.4 |
| 方案 B 作 v1 默认分配跳转 | **禁止**；v1 = 方案 A（§3.3 / §8 Q2） |
| 另开第二份「聊天阶段计划」或平行 Cx 序列 | **禁止**；只维护本 §5 |
| 把 C3 回填总账为 P0/P1「未完成」 | **禁止**；可进 D5 池，出池单独立项 |
| 改 §8 默认决议（落盘目录 / 手动保存 / ghost 等） | 须显式产品决议；**非**本 § 勾选能改 |

### 5.2 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 热改导致 C0–C2 行为/API 变更 | 改代码后 **同 commit** 回写本 § 勾选行证据锚点 + 修订历史 |
| C3 出池单独立项 | 勾选对应 C3 行；头部状态补「C3 部分/全 ✅」；总账 D5 出池记录 |
| 仅文案/建议 id 微调（阶段语义不变） | 可改证据锚点路径；**不**改 C0–C3 切分定义 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 6. 非目标（v1 明确不做）— **t8 已冻结**

> **冻结（t8）**：下列为 2026-07-18 对照后的**本计划 v1 明确不做清单**（边界见 §1.4；流程见 §2；UI 见 §3；阶段见 §5 C0–C3；默认见 §8）。  
> 本表**只划「v1 / 本计划不碰」边界**；**不**开产品功能、不改 C0–C2 勾选、不排 C3、不触 Mode B `confirm_start`。  
> 执行态（2026-07-18）：六条非目标均与 Mode B §10 / 总账 §7（N2 本机、N3 非 IDE）/ §1.4 硬规则一致；后续会话 **不得**以「顺手做」突破本表。  
> 子计划 / 实现 **不得**另写平行「聊天不做什么」清单或与本表冲突的范围声明。

### 6.1 非目标总表

| # | 非目标 | 含义（本计划内） | 为何不做 | 误判时正确动作 |
|---|--------|------------------|----------|----------------|
| **N1** | **聊天里直接跑 worker / 多 agent** | 聊天页**只**对话与落盘散文 `.md`；**禁止**从 `chat_*` / 消息流 spawn worker、起 DAG、或并起多 agent | 破坏 Mode B「**`confirm_start` 唯一业务开跑入口**」（§1.4 · Mode B 真源）；聊天不是第二 Scheduler | 停；要跑任务 → 保存计划 → 方案 A 进 chooser / 顶栏「分配计划」→ `start_plan_job` → `confirm_start` |
| **N2** | **聊天产出 PlanIR JSON 当执行图** | 助手输出 = 人读散文/大纲 `.md`（可 fence 预填 draft）；**不是**可直接进 Scheduler 的 PlanIR / 任务图 JSON | 与 **Planner** 职责重叠；散文 md + 分配（`plan_mode=ai`）更稳、可确认、可复用 `list_plans` | 停；JSON 任务图只由分配阶段 Planner 产出；聊天侧若误夹 JSON → 当草稿说明，**不**当 exec 图 |
| **N3** | **替代 plan-chooser** | 已有计划用户仍走「选择计划」弹层选文件；聊天是**补齐**「无计划时起草」入口，**不是**替换选文件 | 主路径「选已有 → 分配」已落地（ux-simple）；替换 chooser = 破坏有计划用户习惯与顶栏语义 | 停；有 `.md` 计划 → `#btn-plan-choose` / chooser；无合适计划 → 聊天落盘后再选/分配 |
| **N4** | **云端账号 / 多端同步会话** | 会话与计划均落**本机**项目目录（默认 `.cco/chat/` · `plans/`）；无登录、无远端会话同步、无多端合并 | 本机工具定位（总账 §7 N2 / Mode B §10）；云端账号 = 另一产品 | 停；用户自管本机文件/网盘；任何「登录 / 云同步聊天」→ **新计划**，不写回本 § 或 C3 |
| **N5** | **通用闲聊助理（写代码、答百科）** | 系统提示与产品文案收窄为「**共建计划**」；不做通用编程助教、百科、跨域闲聊 | 范围 = 补齐计划文档入口；泛化聊天 = 聊天 IDE（Mode B §10 禁「无限多轮产品经理对话」的变体） | 停；提示词/空态引导回「目标 · 约束 · 任务大纲」；写代码/修 bug 走分配后的 worker，不在聊天页完成 |
| **N6** | **TUI/CLI 同步做聊天** | v1 聊天 **仅桌面** `web/` + Tauri；TUI/CLI **不**加对等聊天页或 `cco chat` 多轮 REPL | 桌面优先；CLI 已有「计划文件路径 / `--plan` / 选文件」入口；双端同做 = 范围爆炸 | 停；CLI/TUI 用户继续文件路径进 Mode B；桌面聊天落盘的 `.md` 仍可被 CLI `list_plans`/run 消费 |

### 6.2 与相邻边界的对照

| 本表（§6 非目标） | 容易混淆 | 真实归属 |
|-------------------|----------|----------|
| N1 不聊天直跑 worker | 「分配后 auto-start 也是自动跑」 | **允许**；那是 `confirm_start` 链上的 D1 默认，**不是**聊天旁路 |
| N1 不聊天直跑 worker | 「C3 聊天内直接分配并沿用并发」 | **C3 池**（方案 B 开关）；出池前仍须进 `start_plan_job`/`confirm_start`，**禁止**跳过 |
| N2 不产出 PlanIR | 「助手消息里出现 JSON 片段」 | 可作说明/草稿；落盘与分配仍以 **散文 `.md`** 为准 |
| N3 不替代 chooser | 「保存后方案 A 打开 chooser」 | **正是**本计划行为；补齐后仍用 chooser，不是取代 |
| N4 不云端同步 | 「会话按项目恢复」 | **本机** `.cco/chat/`（§8 Q4）；跟仓库走，好删 |
| N5 不通用闲聊 | 「多轮改大纲直到满意」 | **允许**；仍是共建**计划文档**，不是写业务代码 |
| N6 不做 TUI/CLI 聊天 | 「CLI 读聊天落盘的 md」 | **允许**；消费文件路径即可，**不**要 CLI 内嵌聊天 UI |

### 6.3 边界（防与 §2 / §3 / §5 / §7 / Mode B 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「把 C3 打磨写成非目标所以永远不做」 | **错误**；C3 = 可出池打磨（流式/多会话/方案 B 开关）；§6 是 **形态边界**，不是 backlog 垃圾桶 |
| 「非目标 = 成功标准没过」 | **错误**；成功标准见 **§7**；非目标达标 = **没做**这些事 |
| 「t8 里实现 chat 直调 `confirm_start` / 上云 / TUI 聊天」 | **禁止**；范围 = 非目标冻结 + L1/L2 指针 |
| 「§8 默认可以覆盖非目标」 | **否**；§8 = 落盘/跳转/会话等默认；**改 N1–N6 须显式修订本 §6**（用户决议） |
| 「总账 §7 / Mode B §10 与本表冲突时另写第三份」 | **禁止**；应对齐本表与总账 N2/N3、Mode B §10；冲突 → 同 commit 修订相关真源 |
| 「§3.6 边界行与本表重复就删一边」 | **保留两边**：§3.6 = UI 规格防混淆；本表 = v1 产品形态非目标真源 |

### 6.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 六条非目标任一语义变更（含「聊天可直跑 / 可出 PlanIR / 上云 / 通用闲聊 / CLI 聊天」） | 改 §6.1 + 对照表，**同 commit** 回写 L1/L2 指针；**须用户显式决议** |
| 仅 C0–C2 热修、C3 出池、或 §5 勾选 | **不**改本非目标骨架（C3 方案 B 仍不得突破 N1） |
| 新增第七条非目标 | 写入 §6.1 表，说明与 Mode B / 总账 §7 关系；**禁止**散落在 worker 任务说明里当隐式范围 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---


## 7. 成功标准 — **t9 已冻结**

> **冻结（t9）**：下列为 2026-07-18 对照后的**本计划自身成功标准唯一验收表**（流程见 §2；界面见 §3；技术见 §4；阶段见 §5 C0–C2）。  
> 本表**只验收「聊天建计划支路是否可自证完成」**；**不**再开产品功能、不触 C3、不改 Mode B 默认。  
> 执行态（2026-07-18）：**五指标均 ✅**；证据以工作树代码 + `node --check` + `cargo test --lib`（34 passed，含 `services::chat`）+ 打包脚本 `cp -R web` 为准。  
> 子计划 / 后续会话 **不得**另写平行「聊天完成度计分卡」或与本表冲突的成功标准。

### 7.1 指标总表

| # | 指标 | 目标 | 状态 | 核验（t9） |
|---|------|------|------|------------|
| 1 | **无计划可开工** | 新项目只有源码、无 `.md` 计划时，仅通过 App 内聊天可得可分配计划并进入拆分 | ✅ | 聊天页 + `chat_send` / `chat_save_plan` 落盘 `plans/chat-*.md`（无 `plans/` 则根 `cco-plan-*.md`）→ `list_plans` 可见 → 方案 A 进 chooser → `analyzePlanFromPicker`；fake 可无 Claude CLI 走通 |
| 2 | **主路径不破坏** | 已有计划用户仍「选择计划 → 分配」；回归绿 | ✅ | 顶栏/chooser 路径未改；`node --check web/js/*.js` 全过；`cargo test --lib` **34 passed**（含 plan/scheduler/chat） |
| 3 | **分配同源** | 聊天「分配计划」最终调用与顶栏/chooser 相同的 `start_plan_job` / `confirm_start` 链 | ✅ | `assignFromChat` → `selectPlan` + `openPlanChooser` → 用户点分配 → **仅** `analyzePlanFromPicker` → `start_plan_job_cmd` / `confirm_start_cmd`；`chat.js` **无**直调 `start_run` / `confirm_start` |
| 4 | **常驻** | 已选项目下任意时刻可进聊天；会话不因进设置/帮助而丢（同项目） | ✅ | 顶栏 `#btn-open-chat` 已选项目常驻（不因 phase 隐藏）；会话落盘 `.cco/chat/{session}.json`；`openChatPage` → `loadChatSession` 按项目恢复 |
| 5 | **可打包目视** | `scripts/package-app.sh` 后：聊天入口可见 → 假/真一轮 → 保存 → 分配进 planning | ✅ | 打包 `cp -R web` 含 `index.html`/`js/chat.js`/`css/chat.css`；sanity 含 `btn-open-chat`·`page-chat`·`btn-chat-assign`；资源链齐全（入口→send/save→assign→planning） |

### 7.2 证据明细

#### 7.2.1 无计划可开工

| 检查 | 结果（2026-07-18 工作树） |
|------|---------------------------|
| 聊天入口 | 顶栏 `#btn-open-chat`（ghost）；空态 `#btn-empty-to-chat`；chooser `#btn-chooser-to-chat` |
| 一轮对话 | `chat_send` → Claude CLI print 或 **fake**（`CCO_CHAT_FAKE` / 无 bin）；plan fence 解析预填 draft（**不**自动写盘） |
| 落盘 | `chat_save_plan` → 优先 `plans/chat-{YYYYMMDD-HHMM}.md`，无 `plans/` → 根 `cco-plan-*.md` |
| 进 chooser | 保存后 `loadPlansForPicker`；`list_plans` 扫 `plans/**.md` + 根 `cco-plan-*.md` |
| 进拆分 | 方案 A：`selectPlan(plan_rel)` → `showPage("workspace")` → `openPlanChooser(true)` → 用户点「分配计划」 |

**完成定义**：无现成 `.md` 计划的项目，用户**不必**离开 App 手写文件，也能走到 Mode B planning。

#### 7.2.2 主路径不破坏

| 检查 | 结果 |
|------|------|
| 选已有计划 | `#btn-plan-choose` / `#plan-chooser` / `#btn-chooser-assign` 行为不变 |
| 顶栏分配 | `#btn-pp-analyze` → `analyzePlanFromPicker`（无 plan 时开 chooser）不变 |
| JS 语法 | `node --check web/js/*.js`（含 `chat.js` · `state.js` · `plan.js` · `doctor.js` · `log.js` · `monitor.js`）全过 |
| 单元测试 | `cargo test --lib`：**34 passed**（含 `services::chat::{session_roundtrip_and_save_plan,fake_send_persists_messages,extract_plan_fence_last_wins}` 与既有 plan/runtime） |

**完成定义**：有计划用户心智仍是「选择计划 → 分配」；聊天是**支路**，不是替换。

#### 7.2.3 分配同源

```text
顶栏 #btn-pp-analyze          聊天 #btn-chat-assign
        │                              │
        │  selectedPlan 已有            │  assignFromChat()
        │  或无 → openPlanChooser       │    selectPlan(chatDraftPlan)
        │                              │    showPage("workspace")
        │                              │    openPlanChooser(true)
        └──────────────┬───────────────┘
                       ▼
              analyzePlanFromPicker()     ← 唯一业务入口（chooser 底栏同源）
                       ▼
              start_plan_job_cmd → confirm_start_cmd（Mode B）
```

| 检查 | 结果 |
|------|------|
| 聊天分配函数 | `web/js/chat.js` `assignFromChat`：方案 A；有 `chatDraftPlan` 才启用 |
| 运行锁 | `hasActiveRun()` → `toastRunLocked("分配计划")`（与顶栏同源） |
| 禁止旁路 | `chat.js` **无** `start_plan_job` / `confirm_start` / `start_run` 直调 |
| 后端入口 | `services::{start_plan_job,confirm_start}` 仍是业务 worker 唯一开跑链（`src/services/runs.rs`） |

**完成定义**：问「聊天分配会不会另开一套 Scheduler？」→ **不会**；最终只进 `analyzePlanFromPicker`。

#### 7.2.4 常驻（会话按项目）

| 检查 | 结果 |
|------|------|
| 顶栏可见 | `renderPlanPicker`：`#btn-open-chat` 在 `selectedPath` 且非 welcome 时显示；**不**因 `phase=planning/confirm/running` 隐藏 |
| 切页不丢 | 会话在磁盘 `.cco/chat/{session_id}.json`；再进聊天 `loadChatSession` → `chat_session_get_cmd` |
| 切项目隔离 | 会话 key = 项目 path + session_id；换项目重新 `chat_session_get` |
| 设置/帮助 | `showPage("settings"|"help")` **不**清 `chatSession` 磁盘；回聊天可恢复（同项目） |
| page 枚举 | `state.page` 含 `chat`；`showPage("chat")` 标题「共建计划」 |

**完成定义**：已选项目时，任意时刻可点「聊天」；同项目会话在进设置/帮助后再回仍在。

#### 7.2.5 可打包目视

| 检查 | 结果 |
|------|------|
| 打包复制 | `scripts/package-app.sh`：`cp -R web` → `Contents/MacOS/web` 与 `Contents/web`（含 `js/chat.js` · `css/chat.css` · `#page-chat`） |
| touch 刷新 | 脚本 `find web/js web/css … touch`，Tauri embed 与旁挂 web 同步 |
| sanity 标记 | 打包后 rg：`btn-open-chat` · `page-chat` · `btn-chat-assign`（及既有主路径标记） |
| 目视路径 | 入口可见 → fake/真 `chat_send` 一轮 →「保存为计划」→「分配计划」→ chooser → planning |

**完成定义**：本机跑 `scripts/package-app.sh` 打开 `CCO.app`，可无手写 `.md` 走通「聊天 → 保存 → 分配 → planning」；真 Claude 可选（fake 不阻塞）。

### 7.3 边界（防与 §2 / §3 / §5 / §6 / Mode B 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「C3 流式/多会话没做所以失败」 | **C3 池**；本成功标准**不**要求 C3 |
| 「没有桌面 E2E 自动化所以不可发布」 | 目视清单 + sanity 标记即可；E2E 属增强 |
| 「聊天必须直开 analyze（方案 B）」 | **否**；v1 = 方案 A（§8 Q2） |
| 「再写一份聊天 ship README」 | **禁止**；只维护本 §7 |
| 「t9 里实现功能 / 改 C0–C2 / 出 C3」 | **禁止**；范围 = 验收冻结 + 指针 |
| 「聊天可直接 spawn worker」 | **禁止**；Mode B 硬规则 §1.4 / §6 非目标 |

### 7.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 五指标任一目标句变更 | 改 §7.1 + 证据节，**同 commit** 回写 L1/L2 指针；**须显式产品决议** |
| 仅 C3 出池或热改 | **不**改本成功标准骨架；产品状态改 §5 C3 勾选 |
| 新增平行「聊天完成度」文档 | **禁止**；并入本 §7 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---


## 8. 风险与决策默认 — **t10 已冻结**

> **冻结（t10）**：下列为 2026-07-18 对照后的**本计划产品默认假设唯一答卷**（执行前问卷；答「按默认」一次即可）。  
> 本表**只冻结落盘 / 跳转 / 保存 / 会话 / 顶栏角色**五默认；**不**开新功能、不改 C0–C2 勾选、不触 C3、不改非目标 §6 / 成功标准 §7。  
> 执行态（2026-07-18）：五项均 **按默认** 采纳；落地证据见下表「执行备注」列（与 §3 Q2/Q5 · §4 落盘约定 · C0–C2 代码一致）。  
> 后续会话 **不得**静默改默认；**新**默认变更仍须用户显式决议。

### 8.1 默认假设总表（按默认）

| # | 议题 | 默认（按默认即冻） | 决议 | 执行备注 |
|---|------|-------------------|------|----------|
| **Q1** | 落盘目录 `plans/` vs 项目根 | **优先 `plans/`**，无则根目录 `cco-plan-*.md` | **按默认** | ✅ `chat_save_plan`：有 `plans/` → `plans/chat-{stamp}.md`，否则根 `cco-plan-*.md`；`list_plans` 扫 `plans/**.md` + 根 `cco-plan-*.md`（`src/services/chat.rs` · `src/plan/mod.rs`） |
| **Q2** | 分配跳转 A（chooser）vs B（直开） | **A**（设 `selectedPlan` → workspace → `openPlanChooser(true)` 再点分配） | **按默认** | ✅ `assignFromChat` → `selectPlan` + `openPlanChooser(true)`；**不**直调 `analyzePlanFromPicker`；方案 B = C3 池（§3.3 / §5 C3） |
| **Q3** | 自动保存 vs 手动保存计划 | **手动「保存/采用」**；fence 只预填 draft | **按默认** | ✅ `extract_plan_fence` 预填 draft；`// Not saved until chat_save_plan`；写盘仅 `#btn-chat-save` / 卡片「采用此稿并保存」 |
| **Q4** | 会话存项目内 vs `~/.cco` | **项目内 `.cco/chat/`**（跟仓库走，好删） | **按默认** | ✅ `{project}/.cco/chat/{session_id}.json`；`chat_session_get` / `chat_send` 按项目 path；**不**写 `~/.cco`（§6 N4 本机边界） |
| **Q5** | 是否占用顶栏 primary | **否**；聊天 ghost，分配仍 primary | **按默认** | ✅ `#btn-open-chat` class `ghost`；`#btn-pp-analyze` class `primary`；聊天页内分配 CTA 可 primary，**不**抢顶栏主色（§3.1） |

**总答**：**按默认**（Q1–Q5 全部采纳，无逐条改写）。

### 8.2 与相邻边界的对照

| 本表（§8 默认） | 容易混淆 | 真实归属 |
|-----------------|----------|----------|
| Q1 优先 `plans/` | 「永远只写项目根」/「强制建 `plans/`」 | 有目录则用；**无则根** `cco-plan-*.md`；不自动 mkdir 策略外目录 |
| Q2 方案 A | 「聊天一点就开跑 / 直开 analyze」 | **方案 B** = C3 可选；v1 **禁止**作默认（§3.3 / §5 C3 / §6 N1） |
| Q3 手动保存 | 「fence 一解析就写盘」 | fence **只预填**；误写防呆 = 用户点保存/采用 |
| Q4 项目内 `.cco/chat/` | 「会话进 `~/.cco` 全局缓存」 | **禁止** v1；跟仓库走、好删、切项目天然隔离 |
| Q5 聊天 ghost | 「聊天抢分配主色 / 分配改 ghost」 | **禁止**；顶栏角色固定（§3.1） |
| 本节 vs 总账 §8 | 「总账 A1–A5 = 本表 Q1–Q5」 | **否**；总账 §8 = 全仓规范根/auto-start/D 序；**本表** = 聊天支路默认 |

### 8.3 边界（防与 §2 / §3 / §5 / §6 / §7 / Mode B 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「t10 里改默认 / 实现功能 / 出 C3」 | **禁止**；范围 = 答卷冻结 + L1/L2 指针 |
| 「Agent 可静默改 Q1–Q5」 | **禁止**；新默认须用户决议 |
| 「默认覆盖非目标 §6」 | **否**；改 N1–N6 须显式修订 §6 |
| 「默认覆盖成功标准 §7」 | **否**；改五指标须显式修订 §7 |
| 「方案 B 当 v1 默认因为更好」 | **禁止**；须先出 C3 池 + 用户决议改 Q2 |
| 「会话改 `~/.cco` 当全局多项目」 | **禁止**；破 Q4；云/全局同步属另一产品（§6 N4） |
| 「再答一次默认当日常」 | **否**；执行前只需答一次；已冻结 |
| 「另开第三份聊天默认清单」 | **禁止**；并入本 §8 |

### 8.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 五项默认任一语义变更（含「强制根目录落盘」「方案 B 默认」「自动写盘」「`~/.cco` 会话」「聊天顶栏 primary」） | 改 §8.1 + 对照表，**同 commit** 回写 L1/L2 指针；**须用户显式决议** |
| 仅 C0–C2 热修、C3 出池、或 §5 勾选 | **不**改本答卷骨架（C3 方案 B 开关 ≠ 改 Q2 默认，除非显式决议） |
| 新增第六条默认假设 | 写入 §8.1 表；**禁止**散落在 worker 任务说明里当隐式范围 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 9. 验证清单 — **t11 已冻结**

> **冻结（t11）**：下列为 2026-07-18 对照工作树后的**本计划发布前手测/机测唯一清单**（流程见 §2；界面见 §3；技术见 §4；成功标准见 §7）。  
> 本 § **只验收七项路径是否可自证**；**不**开产品功能、不触 C3、不改 §6/§7/§8 默认。  
> 执行态（2026-07-18）：**七项均 [x]**；机测 `node --check web/js/*.js` 全过 + `cargo test --lib` **34 passed**（含 `services::chat`）；路径证据见下表。  
> 子计划 / 后续会话 **不得**另写平行「聊天回归清单」或把本表回灌为总账第二份验收表。

### 9.1 勾选总表

| # | 项 | 状态 | 核验（t11 · 2026-07-18） |
|---|----|------|--------------------------|
| 1 | 无计划项目：聊天 → 保存 → chooser 列表出现新文件 → 分配 → planning | [x] | `chat_send` → `chat_save_plan` 落盘 `plans/chat-*.md` → `loadPlansForPicker`/`list_plans` 扫 `plans/**.md` → `assignFromChat` → `selectPlan` + `openPlanChooser(true)` → `analyzePlanFromPicker` → `start_plan_job`；单测 `session_roundtrip_and_save_plan` |
| 2 | 有计划项目：原选择/分配不受影响 | [x] | 顶栏 `#btn-plan-choose` / `#btn-pp-analyze` / chooser `#btn-chooser-assign` 仍走 `selectPlan` · `analyzePlanFromPicker`；聊天为增量 `page=chat`，**不**替换 chooser；`cargo test --lib` 既有 plan/scheduler 全绿 |
| 3 | 规划中/运行中：聊天可浏览；分配按钮走 `hasActiveRun` 锁 | [x] | `#btn-open-chat` 不因 `phase=planning/confirm/running` 隐藏（`renderPlanPicker`）；`showPage("chat")` **不**写 `phase`；`assignFromChat` / `openPlanChooser` / `analyzePlanFromPicker` 均 `hasActiveRun()` → `toastRunLocked` |
| 4 | 切项目：会话隔离 | [x] | 切项目重置 `chatSession`/`chatDraftPlan`（`plan.js` select 项目）；磁盘会话 key = 项目 `.cco/chat/{session}.json`；再进页 `loadChatSession` → `chat_session_get_cmd` 按项目回填 |
| 5 | fake provider：无 Claude 也能走通 UI | [x] | `CCO_CHAT_FAKE=1` / `default_provider=fake` / CLI 失败 soft-fallback → `fake_chat_reply`（含 ` ```plan `）；`ChatSendResponse.fake=true`；单测 `fake_send_persists_messages` |
| 6 | `node --check web/js/chat.js`（及其他改动 js） | [x] | `node --check`：`chat.js` · `state.js` · `plan.js` · `doctor.js` · `log.js` · `monitor.js` · `app.js` → **ALL_JS_OK** |
| 7 | `cargo test --lib` | [x] | **34 passed**；含 `services::chat::{session_roundtrip_and_save_plan,fake_send_persists_messages,extract_plan_fence_last_wins,extract_assistant_text_from_result_line}` + plan/runtime 既有 |

### 9.2 证据明细（路径锚点）

| 项 | 代码/命令锚点 |
|----|----------------|
| 1 无计划支路 | `src/services/chat.rs` `chat_send`/`chat_save_plan`；`src/plan/mod.rs` `list_plans`（`plans/` 全 `.md` + 根 `cco-plan-*.md`）；`web/js/chat.js` `saveChatPlan` → `loadPlansForPicker` · `assignFromChat`；`src-tauri` `chat_*_cmd` |
| 2 主路径不破 | `web/js/plan.js` `selectPlan` · `analyzePlanFromPicker`；`web/js/doctor.js` `btn-plan-choose` / `btn-pp-analyze` / `btn-chooser-assign`；`web/index.html` 顶栏与 chooser DOM 未改职责 |
| 3 运行锁 + 可浏览 | `web/js/state.js` `hasActiveRun` / `toastRunLocked`；`web/js/chat.js` `assignFromChat` L324–326；`web/js/plan.js` `renderPlanPicker` 聊天常驻 · `openPlanChooser` 锁；`showPage("chat")` 仅改标题 |
| 4 会话隔离 | `web/js/plan.js` 切项目清 chat 态；`src/services/chat.rs` `chat_dir` = `project/.cco/chat/`；`loadChatSession` 按 `state.selectedPath` |
| 5 fake | `src/services/chat.rs` `force_fake` / soft-fallback / `fake_chat_reply`；测试 `fake_send_persists_messages` |
| 6–7 机测 | 本机 t11：`node --check web/js/*.js` 全过；`cargo test --lib` 34 passed |

### 9.3 边界（防与 §5 / §7 / 目视混淆）

| 勿再写入本 § / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「没有桌面 E2E 自动化所以 §9 失败」 | **否**；本表 = 路径证据 + 机测 + 与 §7 目视可打包对齐；E2E 属增强 |
| 「C3 流式/多会话没做所以失败」 | **C3 池**；本清单**不**要求 C3 |
| 「把本表扩成第二份成功标准」 | **禁止**；完成度计分卡 = **§7**；本表 = 发布前勾选 |
| 「t11 里实现功能 / 改默认 / 出 C3」 | **禁止**；范围 = 核验 + 冻结 + 指针 |
| 「另开 chat-qa.md / 回归脚本冒充本 §」 | **禁止**；只维护本 §9 |
| 「聊天可绕过 `hasActiveRun` 分配」 | **禁止**；与顶栏同源锁 |

### 9.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 七项任一路径语义变更（含分配旁路、会话目录、fake 规则） | 改 §9.1–9.2 + 头部「§9 冻结」句，**同 commit** 回写 L1/L2；**须与代码同改** |
| 仅热修文案/证据路径（行为不变） | 可改锚点路径；**不**改七项定义 |
| C3 出池后增验项 | **追加**勾选行或另立 C3 验收；**不**改写既有七项为失败 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 10. 文档与 GEB — **t12 已同步**

> **落地（t12）**：下列交叉引用与状态句已于 2026-07-18 同步；**不**改 C0–C2 勾选、**不**触 C3、**不**回灌总账 D0–D4。

| 文件 | 动作 | 状态 |
|------|------|------|
| 本文件 | 阶段勾选 + 状态改「已落地」 | ✅ t12 |
| [`docs/CLAUDE.md`](../CLAUDE.md) | 成员清单加本计划一行 | ✅ |
| [`/CLAUDE.md`](../../CLAUDE.md) | config 区链到本计划（一句话） | ✅ |
| [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md) | 主路径图增加可选「聊天生成计划」支路 | ✅ |
| [`product-mode-b-ai-planner.md`](../product-mode-b-ai-planner.md) | §2 前「计划从哪来：文件 \| 聊天落盘」 | ✅ §1.1 |
| [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md) | §2 **P-chat ✅ C0–C2**；C3→**D5/P2-9**；**勿**与已冻 D0–D4 冲突 | ✅ t12 |
| `web/CLAUDE.md` · `src/services/CLAUDE.md` · `src-tauri/CLAUDE.md` | 成员清单含 chat | ✅ |

---

## 11. 修订历史 — **t13 已闭环**

> **闭环（t13）**：下列为 2026-07-18 本计划**从初稿到 §1–§8 冻结 + C0–C2 落地**的完整修订年表（初稿 · C0–C2 · t1 / t3–t10 · t13）。  
> 本表**只记历史事件**；**不**开新功能、不改 C0–C2 勾选、不排 C3、不改非目标 §6 / 成功标准 §7 / 默认 §8。  
> 执行态：年表按任务序整理；**既有行语义禁止改写**；后续产品/文档变更 **另起行追加**（同日可多行）。  
> 总账 / 其他子计划 **不得**另开第三份「聊天修订年表」冒充本计划变更史。

| 日期 | 说明 |
|------|------|
| 2026-07-18 | 初稿：现状分析 + Chat→落盘→分配（方案 A）+ C0–C3 + 非目标与成功标准 |
| 2026-07-18 | **C0–C2 落地**：services/chat + Tauri cmds + web 聊天页 + 方案 A 分配跳转 |
| 2026-07-18 | **t1 / 前言**：定稿（角色 · 范围 · 关联真源四件套 · GEB 入口 · PROTOCOL · 与总账边界）；L1/L2 指针 |
| 2026-07-18 | **t3 / §1**：现状分析定稿——对照 `web/index.html`·`js/{state,plan,doctor,log,monitor}.js`·`src/services`·`src-tauri`·`src/plan/planner` 冻结 IA、能力表锚点、主路径断点文案证据、可复用符号与 Mode B 硬边界；确认缺口：无 `page=chat` / 无 `chat_*` API / 分配强依赖 `selectedPlan` |
| 2026-07-18 | **t4 / §2**：产品目标与用户流程定稿（增量八步 · 三句心智 · 入口可见性 · 方案 A 默认 · 会话按项目恢复）；L1/L2/总账指针；**不写实现** |
| 2026-07-18 | **t5 / §3**：界面规格冻结（信息架构 page+`chat` · 顶栏 ghost/primary · 聊天页四区布局 · 就绪态方案 A · 与顶栏分配同源 `analyzePlanFromPicker` · §3.5 现网锚点 · §3.6 边界 · §3.7 修订条件）；L1/L2 指针；**不写实现** |
| 2026-07-18 | **t6 / §4**：技术设计冻结（后端 `chat_*` + Tauri 薄壳 · 落盘 `.cco/chat`/`plans/chat-*` · Claude print+fence+fake · 前端文件图 · 方案 A `assignFromChat` · 状态机 page≠phase · 边界/修订条件）；对照 C0–C2 工作树；L1/L2/总账指针；**不写实现** |
| 2026-07-18 | **t7 / §5**：阶段切分与勾选冻结——对照工作树核验 C0–C2 全 ✅（壳/后端/前端接通+方案 A）、C3 ☐ 不排期则不碰；增 §5.0 总览 · 证据锚点 · §5.1 边界 · §5.2 修订条件；总账/L1/L2 指针；**不写实现、不触 C3** |
| 2026-07-18 | **t8 / §6**：非目标冻结（N1–N6 · 对照表 · 边界 · 修订条件）；与 Mode B §10 / 总账 §7 N2·N3 / §1.4 对齐；L1/L2 指针；**不写实现** |
| 2026-07-18 | **t9 / §7**：成功标准冻结（五指标全绿 · 证据明细 · 边界 · 修订条件）；回归 `node --check web/js/*.js` + `cargo test --lib` 34 passed；打包 sanity 含 `btn-open-chat`·`page-chat`·`btn-chat-assign`；L1/L2 指针；**不写产品功能 / 不触 C3** |
| 2026-07-18 | **t10 / §8**：风险与决策默认冻结（Q1–Q5 **按默认** · 对照表 · 边界 · 修订条件）；与 §3 Q2/Q5 · §4 落盘 · C0–C2 代码对齐；L1/L2 指针；**不写实现** |
| 2026-07-18 | **t13 / §11**：修订历史闭环（年表按任务序整理 · 禁止改写既有行 · 追加规则 · 边界）；头部/L1/L2 指针；本计划 t1 / t3–t10 + C0–C2 年表闭环 |
| 2026-07-18 | **t12 / §10**：文档与 GEB 同步——状态改 **已落地**；总账 §2 **P-chat ✅ C0–C2** + C3→**D5/P2-9**；L1/L2 · ux-simple 支路 · Mode B §1.1 · web/services/src-tauri 成员清单；**不触 C3、不回灌 D0–D4** |
| 2026-07-18 | **t11 / §9**：验证清单冻结（七项全绿 · 证据明细 · 边界 · 修订条件）；重跑 `node --check web/js/*.js` ALL_JS_OK + `cargo test --lib` 34 passed；L1/L2 指针；**不写产品功能 / 不触 C3** |
| 2026-07-20 | **t14 / C3 多会话部分落地**：`chat_list_sessions`/`chat_new_session`/`chat_delete_session` + 桌面切换器；§5 C3 多会话行 [x]；流式/方案 B/diff 仍 ☐；总账 **P2-9 ⚠ t32**；**不**改 Q2 方案 A 默认 / **不**回灌 C0–C2 |
| 2026-07-20 | **t15 / C3 方案 B 开关**：设置页「执行时跳过二次确认」`#s-chat-assign-direct`（localStorage，默认关）；`startExecuteFromSelection` 可选 `direct` → 直调 `analyzePlanFromPicker`；Q2 默认仍为 A；流式/diff 仍 ☐；总账 t33 |
| 2026-07-20 | **t16 / C3 计划 diff + 流式 partial**：plan-full「对比改动」磁盘稿 vs 草稿（LCS 行 diff · 采用左/右写回草稿 · 落盘仍 `chat_save_plan`）；`chat_stream_partial` + Tauri + 待发气泡轮询；失败降级 wait label；§5 C3 流式/diff [x]；总账 **P2-9 ✅ t34**；**不**改 Q2 默认 / **不**回灌 C0–C2 |
| 2026-07-19 | **指针 / 体验修补**：关联真源增 [`chat-ux-focus-2026-07-19.md`](./chat-ux-focus-2026-07-19.md)（后台降噪 · fake 可信 · CTA · U0–U2 → 总账 **P2-10**）；**不**改 C0–C2 勾选 / 方案 A / §6–§8 |

### 11.1 边界（防与产品变更混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「t13 里改产品默认 / 实现功能 / 出 C3」 | **禁止**；范围 = 年表闭环 + 指针 |
| 「改写既有行语义以「更正」历史」 | **禁止**；勘误 **另起一行**说明 |
| 「另开 chat-changelog / 第三份修订表」 | **禁止**；只维护本 §11 |
| 「C3 出池 / 热改不记修订历史」 | **应**追加一行；**不**改 §5 C0–C2 勾选 / §6 / §7 / §8 骨架 |
| 「把 C3 打磨塞进年表当已完成」 | **否**；C3 = 不排期则不碰，出池实现后才追加 |
| 「把本表回灌为总账第二份修订史」 | **禁止**；总账变更记 [`gap-and-landing-plan`](../gap-and-landing-plan-2026-07-18.md) §9；本表只记**本子计划** |

### 11.2 何时本节省略修订

| 条件 | 动作 |
|------|------|
| C3 出池 / C0–C2 热改 / 用户决议改默认·非目标·成功标准·界面/技术设计 | **追加**一行（日期 + 简述）；同 commit 改对应 § + 头部状态 |
| 仅措辞润色既有冻结节 | **不**改年表既有行；若值得记 → 追加「润色 §X 表述」 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

[PROTOCOL]: 变更时更新此头部，然后检查 docs/CLAUDE.md
