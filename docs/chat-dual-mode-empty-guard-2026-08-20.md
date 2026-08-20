# 聊天双模式 + 壳层空态引导 — 深度分析与落地计划

> 类型：**落地计划**（本能力勾选唯一落点 = 本文 F 系列）
> 日期：2026-08-20
> 输入：用户 2026-08-20 任务（两模式拆分 · tab 空态弹窗 · 信息密度判定）+ 漏项盘点
> 参照：[DeepSeek Harness](./harness-inspired-roadmap-2026-08-14.md)（能力预设 H3）· [DSH UI 收口](./ui-redesign-dsh-2026-08-15.md)（P4-0…P4-8）· [聊天写计划 runtime prompt](./runtime-prompts/chat-plan-writing.md)（三入口真源）
> 状态：**F0 ✅ · F1–F5 ☐ 未开工**

---

## 0. 背景与漏项盘点

用户 2026-08-20 原始任务 8 项，逐项对照实况：

| # | 任务 | 状态 | 证据/落点 |
|---|------|------|----------|
| 1 | AI 统一改「小叶」 | ✅ 2026-08-20 完成 | 16 文件 37 处文案；dist 产物 0 残留 |
| 2 | 图 1 优化分析 | ✅ 已交付（对话） | — |
| 3 | 计划拆两种模式：快速出产品 / 深度思考深聊 | ⚠️ 深度侧已有（三入口澄清相），**快速侧缺失** | `chatClarify.js` 三入口；无快速直达车道 → 本文 §4 |
| 4 | 模型显示迁「本轮上下文」右侧只读 + `/model` | ✅ 已打包验证 | `chatControls.js` 徽标；dist 实测 |
| 5 | 头部 tab：聊天/拆分/执行/结果 | ✅ P4-2 已落 | `web/index.html:84-89` view-ring |
| 6 | tab 无信息时弹窗提示 | ❌ 未做 | `web/js/main.js` wireShellNav 段控点击无空态守卫 → 本文 §5 |
| 7 | 聊天窗信息堆叠是否干扰 | ✅ 分析已给 + 部分落地（消息折叠/模型徽标）；**残余处置 ☐** | 本文 §2 |
| 8 | 参照 DeepSeek-Harness 的深度分析 md 计划 | ✅ **即本文** | — |

---

## 1. 对照 DeepSeek-Harness：两模式的设计依据

### 1.1 Harness 给的架构启示（H3 · 能力集可声明组合）

Harness 用 **同一内核装配不同运行预设**（Minimal / Standard / Creative）：用户不必理解内核，只选「我要干什么强度的事」。
对位到 Leaf 的聊天写计划：**底层澄清管线只有一条**（五必需槽 → Brief → 认领 → ```plan），两模式是**同一管线的两种装配**，不是两套引擎：

- **快速出产品** = 装配为「跳过盘问 + 配方默认 + 显式假设」的直达车道；
- **深度思考** = 装配为「澄清相全量（≤5 题）→ Brief → 认领」的对话车道。

这与 `harness-inspired-roadmap` A2（Worker 能力预设 safe/full/inspect）同构：**预设是能力边界声明，不是第二条执行路径**。

### 1.2 dsh UI 已收口的部分不重做

view-ring 段控、StateDot、DisclosureRow 折叠、卡片语言已在 `ui-redesign-dsh-2026-08-15.md` P4 系列落地。本文**只增量**：composer 上方的模式 chip + 段控空态守卫，不重开第三套 UX 计划（红线：主表面 ≤4 · 新概念 ≤3）。

### 1.3 不借什么

| 不借 | 原因 |
|------|------|
| Harness 的 Chat-First 自由会话主路径 | Leaf = Plan-First 五步闭环；聊天只负责「生成/核对」 |
| 每模式独立工具集/权限集 | 两模式只差**澄清深度**，Worker 执行面完全一致 |
| 模式作为 wire 新字段进 domain | 三入口（think_first/idea_to_plan/plan_only）已覆盖语义；UI 层映射即可（见 §4.3） |

---

## 2. 深度分析一：聊天窗信息密度（干扰判定）

### 2.1 判定标准

用户打开聊天页，脑子里只有三个问题：**我在哪 / 下一步做什么 / 小叶在干嘛**。
一切常驻 UI 必须在三问里各有归属；两个元素回答同一问 = 一个该折叠。

### 2.2 已落地的减法（本轮及之前）

| 项 | 处置 | 状态 |
|----|------|------|
| 长消息全文刷屏 | 小叶长回复折叠 + 展开行（参照 dsh DisclosureRow） | ✅ chat-fold 冒烟绿 |
| 模型信息挤占 composer | 下拉移除；模型名只读徽标挂「本轮上下文」右侧，`/model` 切换 | ✅ 已打包 |
| 引擎黑话 | 主路径文案 37 处改「小叶」 | ✅ |

### 2.3 残余干扰盘点与处置（→ F4）

| # | 元素（同屏出现于） | 回答哪一问 | 判定 | 处置 |
|---|------|-----------|------|------|
| R1 | 环境条 env bar + 就绪条 ready bar（会话顶部） | 同答「小叶在干嘛/环境行不行」 | **重叠** | 合并为一行：正常时只留 ready 条一句话，env 异常才展开详情（dsh 模式：状态点 + 披露行） |
| R2 | 上轮摘要 banner + ready 条（重进会话） | 同答「我们聊到哪了」 | **重叠** | 按优先级二选一：有未认领 Brief → 摘要 banner；否则 ready 条 |
| R3 | 场景 chips（更像哪类事）+ persona 示例 chips + coach 句（空态） | 同答「从哪开始」 | **三层堆叠** | 场景 chips 收进「换个例子」披露行，默认折叠；空态首屏 = coach 一句 + 示例 chips + 输入框 |
| R4 | 澄清三入口行（澄清中） | 回答「怎么开始」但澄清中已无关 | **过期常驻** | phase 进入 clarifying 后收起为「换一种方式」linkish（复用现有 `moreWays` 文案钩子） |

结论：**主路径方向是对的（先减后加），残余问题集中在「顶部条重叠」与「空态三层堆叠」两处**，全部用折叠/合并解决，不删信息。

---

## 3. 深度分析二：如何引导用户做项目（从聊天到执行的分流）

PRODUCT 五步主循环 = 生成→核对→拆分→并行→巡检。流失风险集中在第一步**生成**：
用户带着的话天然分两种——「我想看个东西」（结果导向）和「我想把事想清楚」（对齐导向）。
现有三入口按**流程态度**分流（想清楚再说/从想法到计划/直接写计划），问题是：

1. `plan_only` 是逃生舱文案（「我已想清」），**没有承接「我着急要结果」的意图**——快速意向的用户被推进默认澄清相，答两题就烦；
2. 入口选择出现在**空态首屏**，此刻用户还没说话，选「怎么开始」心智成本最高；
3. persona 的 `pathBias`（L/M/H）与 `directExec`（offer/hide）已经编码了同样的分流意图，但**入口行不消费它**——founder（L · offer）和 ops（H · hide）看到的是同一个默认入口。

**修正：两模式按「意图」分流，persona 只做默认推断，不做强制**。这是 §4 的产品逻辑基础。

---

## 4. 双模式设计（F1/F2 落点）

### 4.1 模式定义

| | 快速出产品（fast） | 深度思考（deep） |
|---|---|---|
| 心智一句话 | 一句话描述 → 直接看计划/成品方向 | 先深聊对齐 → 再出计划 |
| 底层装配 | `plan_only` 语义：跳过盘问 + 配方默认 + **显式假设** | `idea_to_plan`（默认）/ `think_first`：澄清相全量 |
| 首轮行为 | 小叶首轮即出 ```plan（含 假设 标签），不等问答 | 首轮澄清题（≤5 题 A/B/C）→ Brief → 认领 |
| 主 CTA | 「生成并看看」（directExec=offer persona 同款） | 「写成计划」→「拆成步骤」 |
| 适配 persona（默认推断，可改） | pathBias **L**（founder/creator） | pathBias **M/H**（pm/ops/edu/admin…） |
| 硬边界 | **同样禁 confirm_start / spawn**；假设必须显式（黄条照旧） | 不变 |

### 4.2 交互落点

- **位置**：composer 上方两枚 chip：`快速出产品`｜`深度思考`（默认枚高亮）；不进 topbar（壳层减法）。
- **默认推断**：进入空态时按当前 persona `pathBias` 选默认（L→fast，M/H→deep）；用户点选后 **session 级记忆**（`state.chatSession.clarify.entry` 已有持久位，不新增存储）。
- **切换时机**：任何时候可切；fast→deep 保留已有 slots（不清空）；deep→fast 提示「未答完的按常见假设处理」。
- **概念预算**：同屏新增 = 2 枚模式 chip ≤ 3 ✅（澄清问题本身不算新概念）。

### 4.3 wire 契约：**不动 domain，UI 层映射**

| UI 模式 | 映射到 `ClarifyEntry` | 理由 |
|---------|----------------------|------|
| fast | `plan_only` | 语义同源：跳过盘问 + 假设；`normalizeEntry` 已兼容 |
| deep | `idea_to_plan`（默认）/ `think_first`（Brief 即终点） | 现状不变 |

改动只在两处：
1. `web/js/features/chat/chatClarify.js`：`CLARIFY_ENTRIES` 渲染层改为两模式 chip（三入口降级为模式内次级选项：deep 模式里保留「先只想清楚」linkish）；
2. `docs/runtime-prompts/chat-plan-writing.md`：三入口表增「快速出产品」行——**当会话 entry=plan_only 且用户意图是『快点看到东西』时，首轮直接出计划，槽位全部记 `假设`，并在计划开头写明『快速模式：以下按常见假设，可改』**。
   （runtime prompt 是行为真源，UI chip 只是触发器；domain `ClarifyEntry` 枚举与 wire schema **零改动**。）

### 4.4 与既有契约一致性

- 规则 10（confirm 唯一开跑）：两模式终点都是「拆成步骤」→ Split confirm。fast **不是**自动开跑。
- 规则 13（soft-fill 不覆盖显式）：fast 的假设槽全部 `assumed`，用户后补即升 `explicit`，走现有 `setSlotFillLocal` 守卫。
- 规则 23（人话第一句）：fast 首轮回复第一句 = 计划标题或「按你的描述，先来一版」，无引擎词。
- 空心黄条（D0）：fast 计划缺验收/不做时黄条照旧提醒，不拦。

---

## 5. Tab 空态守卫（F3 落点）

### 5.1 现状缺陷

`web/js/main.js` `wireShellNav()`（约 489-521 行）：段控点击直接 `appVm.goSplit()/goRun()/goResult()`，无信息时静默回落 welcome——用户点了没反应，不知道该去哪。违反「下一步做什么」判定标准。

### 5.2 空态判定与文案（人话 · 动词 CTA · 一跳到位）

| Tab | 「有信息」条件（读 legacy state） | 空态弹窗文案 | 主 CTA |
|-----|----------------------------------|--------------|--------|
| 拆分 | `selectedPath` &&（`chatSession.draft_plan` 或 `planJobId` 或 plans 列表非空） | 「这个项目还没有可拆分的计划。先和小叶聊出一份？」 | 去聊天写计划 |
| 拆分（无项目） | `!selectedPath` | 「先选一个项目文件夹，再拆计划。」 | 去选项目 |
| 执行 | `hasActiveRun()` 或历史 run 存在 | 「还没有开始执行的任务。计划要先在拆分台确认，才会开跑。」 | 去拆分台看看 |
| 结果 | 最近 run 有 report/终态 | 「还没有执行结果。先跑一轮，这里会收口。」 | 去执行台 |
| 聊天 | 永不空态（空会话即引导输入） | — | — |

### 5.3 实现要点

- 落点：`wireShellNav` ring 分支，`goSplit/goRun/goResult` 前**先判空**；空 → 弹窗 + 不切页；非空 → 原逻辑。
- 弹窗复用 `shared/shellUi.js` `openModal`（或 confirmDialog）：主 CTA 跳目标页（`goAuthor()` 等），次级「留在本页」。
- 频控：同 tab 同一空态原因本会话只弹一次（记 `dataset` 或 state 标记），避免巡检式打扰。
- 文案零技术词（无 plan_id/run_id/schema），符合规则 23。

---

## 6. 落地任务（勾选只认本节）

| # | 任务 | 落点 | 验收 | 状态 |
|---|------|------|------|------|
| F0 | 本文 + docs/CLAUDE.md 索引 | `docs/` | 索引可查 | ✅ 2026-08-20 |
| F1 | 两模式 chip UI + persona 默认推断 + session 记忆 + 切换保留 slots | `chatClarify.js`（entries 渲染）· `chatPersona.js`（读 pathBias）· `chat.css` | 空态见两 chip；founder 默认 fast；切模式不清进度 | ☐ |
| F2 | 快速模式行为契约：runtime prompt 增「快速出产品」节 + 首轮即 plan + 显式假设 | `docs/runtime-prompts/chat-plan-writing.md`（+ 对应 domain normalize 测试若有入口文案断言） | fast 会话首轮出 ```plan 且含『快速模式·常见假设』说明；黄条照旧 | ☐ |
| F3 | tab 空态守卫 | `web/js/main.js` wireShellNav · `shellUi.js`（判定 helper 复用 `hasActiveRun`） | 无项目/无计划/无 run 时点对应 tab 弹窗 + CTA 跳转正确；同因不重复弹 | ☐ |
| F4 | 信息密度残余：R1 顶部条合并 · R2 banner 二选一 · R3 空态场景 chips 折叠 · R4 入口行过期收起 | `chatRender.js`（env/ready/banner）· `chatPersona.js`（scene chips 披露行）· `chat.css` | 空态首屏 ≤ coach+示例+输入；顶部常驻 ≤1 条 | ☐ |
| F5 | 冒烟 + 构建 + 重打包 | `scripts/chat-fold-smoke.mjs` 扩断言（或新 smoke）· `web build` · `package-app.sh` | 全绿；dist 含新模式 UI | ☐ |

依赖序：F1 → F2（chip 先有触发器）；F3、F4 独立可并行；F5 收口。

---

## 7. 不做清单

- **不做**模式进 domain wire / 新增 `ChatMode` 字段（三入口语义已覆盖）。
- **不做** fast 自动 confirm/开跑（directExec 仅指「直接生成交付物」，不是启动 Run）。
- **不做**模式 × persona 强绑定（推断可被一次点击覆盖，不弹确认）。
- **不做** topbar 常驻模式指示（壳层减法；模式态由 chip 自身表达）。
- **不做** Trajectory 式聊天回放/时间轴（dsh 红线表既定）。

---

## 8. 一致性自检（对照工程硬规则）

| 规则 | 检查 | 结论 |
|------|------|------|
| 10 唯一开跑 = Split confirm | 两模式终点一致；fast 仅省「问」，不省「确认」 | ✅ |
| 21 主区 phase 不变 | 不新增 phase；模式只活在 chat 页 | ✅ |
| 23 人话第一句 | 空态弹窗/fast 首轮文案表已人话 | ✅ |
| 24 高级默认折叠 | R3/R4 用披露行，不删信息 | ✅ |
| 26 同屏新概念 ≤3 | 模式 chip 2 个 | ✅ |
| 文档法则 | 无平行第二套阶段表；F 系列为本能力唯一勾选 | ✅ |

---

> [PROTOCOL]: 本文为两模式 + 空态引导唯一勾选落点；改边界须同步 `docs/CLAUDE.md` 索引与涉及 L2（web/CLAUDE.md · runtime-prompts/README）；勾选只认 §6。
