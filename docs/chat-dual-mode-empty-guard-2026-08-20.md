# 聊天双模式 + 壳层空态引导 — 深度分析与落地计划

> 类型：**落地计划**（本能力勾选唯一落点 = 本文 F 系列）
> 日期：2026-08-20
> 输入：用户 2026-08-20 任务（两模式拆分 · tab 空态弹窗 · 信息密度判定）+ 漏项盘点
> 参照：[DeepSeek Harness](./harness-inspired-roadmap-2026-08-14.md)（能力预设 H3）· [DSH UI 收口](./ui-redesign-dsh-2026-08-15.md)（P4-0…P4-8）· [聊天写计划 runtime prompt](./runtime-prompts/chat-plan-writing.md)（三入口真源）
> 状态：**F0–F5 ✅ 2026-08-20**（F1∥最小 R4 · F4 R1–R3；R4 随 F1 · F5 冒烟/build/playwright webServer 4/4 · dist chip+toast）

---

## 0. 背景与漏项盘点

用户 2026-08-20 原始任务 8 项，逐项对照实况：

| # | 任务 | 状态 | 证据/落点 |
|---|------|------|----------|
| 1 | AI 统一改「小叶」 | ✅ 2026-08-20 完成 | 16 文件 37 处文案；dist 产物 0 残留 |
| 2 | 图 1 优化分析 | ✅ 已交付（对话） | — |
| 3 | 计划拆两种模式：快速出产品 / 深度思考深聊 | ✅ F1+F2 已落 | `chatMode.js` chip + §4.5 首 send 钩子；runtime prompt 快速模式行；domain 三入口零改 |
| 4 | 模型显示迁「本轮上下文」右侧只读 + `/model` | ✅ 已打包验证 | `chatControls.js` 徽标；dist 实测 |
| 5 | 头部 tab：聊天/拆分/执行/结果 | ✅ P4-2 已落 | `web/index.html:84-89` view-ring |
| 6 | tab 无信息时弹窗提示 | ✅ F3 已落 | `shared/tabEmptyGuard.js` + `main.js` wireShellNav 仅 ring 点击；`confirmDialog`；同因一次 |
| 7 | 聊天窗信息堆叠是否干扰 | ✅ F4 R1–R3 + F1 最小 R4 | env/ready 合并 · last_summary↔brief · 场景「换个例子」折 · 三入口主行降级 |
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
| R1 | 环境条 env bar + 就绪条 ready bar（会话顶部） | 同答「小叶在干嘛/环境行不行」 | **重叠** | ✅ **实作**：ready 条始终隐藏（CTA 归计划卡）；正常态不常驻 env；env **异常**才出一条（StateDot + 一句 + 动作）。（计划初稿曾写「正常只 ready」——与实作不符，以 env 主/ready 隐为准） |
| R2 | 上轮摘要 banner + ready 条（重进会话） | 同答「我们聊到哪了」 | **重叠** | ✅ 二选一：`brief_ready` 未认领 → 澄清 Brief 面板独占；否则可出 last_summary；ready 不与之并立 |
| R3 | 场景 chips（更像哪类事）+ persona 示例 chips + coach 句（空态） | 同答「从哪开始」 | **三层堆叠** | ✅ 场景 chips →「换个例子」默认折；首屏 = persona coach + 示例 chips（+ 模式 chip 在 composer） |
| R4 | 澄清三入口行（澄清中 / 与模式 chip 并排时） | 回答「怎么开始」但澄清中已无关；与 2 chip 并排则破概念预算 | **过期常驻 / 双轨** | ✅ **F1 最小集**已做：旧三入口主行降级；deep 内「先只想清楚」linkish |

结论：**主路径方向是对的（先减后加），残余问题集中在「顶部条重叠」与「空态三层堆叠」两处**，全部用折叠/合并解决，不删信息。R4 的**最小降级随 F1 同 PR**，完整 env/banner/场景见 F4。

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
| 首轮行为 | **首条用户消息**即出 ```plan（含 假设 标签），不等问答；点 chip 本身不出任何东西 | 首轮澄清题（≤5 题 A/B/C）→ Brief → 认领 |
| 主 CTA | 「生成并看看」（directExec=offer persona 同款） | 「写成计划」→「拆成步骤」 |
| 适配 persona（默认推断，可改） | pathBias **L**（founder/creator） | pathBias **M/H**（pm/ops/edu/admin…） |
| 硬边界 | **同样禁 confirm_start / spawn**；假设必须显式（黄条照旧） | 不变 |

### 4.2 交互落点

- **位置**：composer 上方两枚 chip：`快速出产品`｜`深度思考`（默认枚高亮）；不进 topbar（壳层减法）。
- **默认推断**：进入空态时按当前 persona `pathBias` 选默认（L→fast，M/H→deep）；用户点选后 **session 级记忆**（`state.chatSession.clarify.entry` 已有持久位，不新增存储）。L→fast 是**可点一次覆盖的启发式**，不是强绑定（0→1 用户可随手切 deep）；本轮不做设置项，先上再看反馈。
- **触发契约（关键 · 防做错产品）**：点模式 chip = **只写 entry 记忆，不触发认领、不出草稿**。现状 `selectClarifyEntry("plan_only")`（`chatClarify.js:1423-1433`）是点击即 `applySkipWithAssumptionsLocal` + 自动 `claimBriefToPlan()` 本地出草稿——两模式 chip **不得复用**该分支，否则用户点完 chip 就收到一份「假设（用户跳过）」占位模板。fast 链路在**该模式下首条用户消息发出时**才走「跳过盘问 → 显式假设 → ```plan」；chip 态下用户没说话 = 什么都不发生。时序真值表见 **§4.5**。
- **老会话兼容**：重进已存 `entry=plan_only` 的会话——phase 已 claimed → chip 高亮 fast、不重放认领；未 claimed → 显示 fast 态、等首条消息触发。默认推断不覆盖历史 entry，除非用户显式点选。
- **切换时机**：任何时候可切；fast→deep 保留已有 slots（不清空）；deep→fast 提示「未答完的按常见假设处理」（未答槽转 `assumed` 保留不丢——现状切 grill 入口会 filter 掉 assumed 槽，两模式切换以本文「保留」为准）。
- **deep 内次级入口（DOM）**：主路径只见 2 chip。deep 高亮且 phase∈{idle,clarifying,brief_ready} 时，chip 行**右侧**一条 linkish「先只想清楚」→ `selectClarifyEntry("think_first")`（进 Brief 可停）。fast 高亮时**不**展示该 link；「直接写计划」linkish 逃生舱仍可走 `selectClarifyEntry("plan_only")` 即时 claim（显式要现在出草稿，与 chip 语义不同）。
- **概念预算（硬）**：同屏主路径新增 = 2 枚模式 chip ≤ 3。**禁止** F1 落地后仍并排渲染旧三入口主按钮行（会变成 2+3）。F1 **必须同 PR 带最小 R4**：旧 `CLARIFY_ENTRIES` 主按钮行降为 deep 内 linkish / 折叠，不得与 chip 双轨常驻。澄清题本身不算新概念。

### 4.3 wire 契约：**不动 domain，UI 层映射**

| UI 模式 | 映射到 `ClarifyEntry` | 理由 |
|---------|----------------------|------|
| fast | `plan_only` | 语义同源：跳过盘问 + 假设；`normalizeEntry` 已兼容 |
| deep | `idea_to_plan`（默认）/ `think_first`（Brief 即终点） | 现状不变 |

改动落点：

1. **新文件 `web/js/features/chat/chatMode.js`**：两模式 chip 渲染 + persona `pathBias` 默认推断 + session 记忆 + 触发契约（chip 点击只写 entry）+ **fast 首 send 钩子**（§4.5）+ deep 内 think_first linkish。经 `installChat.js` 挂到 `ccoChat`（如 `setChatMode` / `getChatMode` / `paintChatMode`），**不**散落新的 `window.*` 业务全局（可与现有 clarify 委托并列）。
2. **`chatClarify.js`**：已 **2528 行**（规则 15 硬上限 600 早超 · 规则 18 禁止续堆）——只留**一行委托**给 mode 渲染/切换，**净减不增**；`selectClarifyEntry` 的 plan_only 即时认领分支（`chatClarify.js:1423-1433`）**仅**保留给「直接写计划」linkish 逃生舱。
3. **`docs/runtime-prompts/chat-plan-writing.md`**：三入口表增「快速出产品」行——**当会话 entry=plan_only 且用户意图是『快点看到东西』时，首轮（=该模式下首条用户消息，不是点 chip 时）直接出计划，槽位全部记 `假设`，并在计划开头写明『快速模式：以下按常见假设，可改』**。F2 若动覆盖序，同步 `runtime-prompts/README`。
4. **`chatPersona.js`**：`pathBias` 已在（founder/creator=L · ops=H · 其余 M），只补读取导出供 `chatMode.js` 消费。
5. **L2**：F1 落地时 `web/CLAUDE.md` 成员清单补 `chatMode.js` 一行（地图与地形同构）。

（runtime prompt + 客户端 skip/假设 **双轨**才是行为真源，UI chip 只是触发器；domain `ClarifyEntry` 枚举与 wire schema **零改动**。确定性**不单赌模型**。）

### 4.4 与既有契约一致性

- 规则 10（confirm 唯一开跑）：两模式终点都是「拆成步骤」→ Split confirm。fast **不是**自动开跑。
- 规则 13（soft-fill 不覆盖显式）：fast 的假设槽全部 `assumed`，用户后补即升 `explicit`，走现有 `setSlotFillLocal` 守卫。
- 规则 23（人话第一句）：fast 首轮回复第一句 = 计划标题或「按你的描述，先来一版」，无引擎词。
- 空心黄条（D0）：fast 计划缺验收/不做时黄条照旧提醒，不拦。

### 4.5 fast 首 send 管线（真值表 · F1/F2 共用 · 防只改文案）

> 目标：chip 安静；首条用户消息才进入「跳过盘问 → 显式假设 → 模型出 ```plan」；本地 state 与 prompt **同时**进入快速语义。

| 时机 | `entry` | `skip_requested` | `applySkipWithAssumptionsLocal` | `claimBriefToPlan` | 用户可见 |
|------|---------|------------------|----------------------------------|--------------------|----------|
| 点 chip → fast | `plan_only` | **false**（保持/清 false） | **不调用** | **不调用** | 仅 chip 高亮；无草稿、无占位模板 |
| 点 chip → deep | `idea_to_plan` | false | 不调用 | 不调用 | chip 高亮；可现 think_first linkish |
| deep 点「先只想清楚」 | `think_first` | false | 不调用 | 不调用 | 进澄清/Brief 可停 |
| 点「直接写计划」linkish | `plan_only` | true | **调用** | **调用**（现状逃生舱） | 本地出草稿（显式要现在写） |
| **fast 下首条 send 之前**（尚无 `draft_plan` / 未 claimed） | `plan_only` | **→ true** | **调用**（userNote 如「快速出产品」） | **不**预 claim 空模板 | 槽位变 assumed；再 `chat_send` |
| fast 下模型回复含 ```plan | 不变 | true | — | 走**现有 fence→草稿**路径（与今日 plan 解析相同，不新开 claim 旁路） | 计划可见；黄条规则照旧 |
| fast 已有 draft 后再 send | 不变 | 保持 | 不再重复 skip | 不自动重 claim | 普通续聊/改计划 |

**接线落点（F1 钩子 · F2 补 prompt）**：

1. `chatMode.js`：`setMode('fast'|'deep')` 只写 entry + mirror/stash + repaint；**禁止**调到 `selectClarifyEntry('plan_only')` 全分支。
2. `sendChatMessage`（或 `chatActions` 发送入口）最前：若当前为 fast（`getChatMode()==='fast'` 或 mode 标记的 `entry===plan_only`）且尚未 skip、尚无 draft → 先 `applySkipWithAssumptionsLocal` + mirror，再走原 send。
3. F2：runtime prompt 增加快速模式节，与上表一致；**单改 prompt 不算 F2 完成**。
4. 失败降级：模型仍先问澄清题 → 本地已是 skip/assumed，UI 不回到「请选入口」主按钮行；用户可再发「直接出计划」或点逃生舱。

---

## 5. Tab 空态守卫（F3 落点）

### 5.1 现状缺陷

`web/js/main.js` `wireShellNav()`（约 489-521 行）：段控点击直接 `appVm.goSplit()/goRun()/goResult()`，无信息时静默回落 welcome——用户点了没反应，不知道该去哪。违反「下一步做什么」判定标准。

### 5.2 空态判定与文案（人话 · 动词 CTA · 一跳到位）

| Tab | 「有信息」条件（读 legacy state） | 空态弹窗文案 | 主 CTA（ok） | 取消 |
|-----|----------------------------------|--------------|--------------|------|
| 拆分 | `selectedPath` &&（**可用** `draft_plan`：`markdown`/`md`/`body` trim 非空，或 `path`/`plan_path` 非空；空对象不算 · 或 `planJobId` 或 plans 列表非空） | 「这个项目还没有可拆分的计划。先和小叶聊出一份？」 | 去聊天写计划 → `goAuthor()` | 留在本页 |
| 拆分（无项目） | `!selectedPath` | 「先选一个项目文件夹，再拆计划。」 | 去选项目 → **真正的** `openModal()`（仅此处，添加项目） | 留在本页 |
| 执行 | `hasActiveRun()` \|\| `isRunPaused()` \|\| 历史 run 存在（见字段表） | 「还没有开始执行的任务。计划要先在拆分台确认，才会开跑。」 | 去拆分台看看 → `goSplit()`（若拆分亦空，**不**链式连弹；停在拆分或回聊天，本会话记下因） | 留在本页 |
| 结果 | 结果终态或本 job/历史 run 已终态（见字段表） | 「还没有执行结果。先跑一轮，这里会收口。」 | 去执行台 → `goRun()`（执行亦空则同「不链式连弹」） | 留在本页 |
| 聊天 | 永不空态（空会话即引导输入） | — | — | — |

**空态判定字段对照**（全部读 legacy state · 复用 `shared/shellUi.js` 现有 helper，不新造判定器）：

| 判定 | 表达式 | 出处 |
|------|--------|------|
| 正在跑 | `hasActiveRun()` ＝ `state.live.run_id && isLiveStatus(run_status)` | `shellUi.js:59-66` |
| 暂停也算有信息 | `isRunPaused()` | `shellUi.js:68-79` |
| 历史 run | `state.projects[selectedPath].last_run_id ?? lastRunId`（被「结束计划」dismiss 过的仍算历史；**只认 id 非空**，不因 run 目录偶发缺失而在段控二次盘问——缺目录时进台后走既有空 live/错误提示） | `shellUi.js:47-56` 同字段读法 |
| 本 job 跑过 | `state.planJob?.run_id ?? runId` | `SplitView.js:89` 同读法 |
| 结果终态 | （`state.live.run_id` 且 status 非活跃非暂停）\|\| `state.phase === "done"` \|\| （历史 `last_run_id` 存在且当前非 live/paused——视为「有过结果可回看」） | `loadLive.js:161-163` 等 |

**负例（有信息 → 不弹）**：

- 有 `draft_plan` 或 `planJobId` 或 plans 非空 → 点拆分不弹。
- `hasActiveRun()` 或 `isRunPaused()` → 点执行/结果不弹（台内自行展示进行中/终态）。
- 仅 `last_run_id` 有值（已结束计划）→ 点执行/结果不弹，进台后用既有空态/历史展示。

### 5.3 实现要点

- **守卫范围（硬）**：**仅** `#view-ring` 用户点击（`wireShellNav` ring 分支）。`jobPoll` / `confirmActions` / `ccoApp.goSplit` / `appVm.go*` 等**程序化导航不拦、不弹**——系统接线不得被引导弹窗打断。
- 落点：ring 分支内 `goSplit/goRun/goResult` 前**先判空**（逻辑在 `shared/tabEmptyGuard.js`，**仅** ring 点击调用）；空 → 弹窗 + **不切页**；非空 → 原逻辑。
- **弹窗组件（硬）**：只用 `shared/confirmDialog.js` 的 `confirmDialog` / `window.ccoConfirm`：

  ```js
  const ok = await confirmDialog({
    title: "还不能打开这里",
    body: "……人话……",
    okLabel: "去聊天写计划", // 或「去选项目」「去拆分台看看」「去执行台」
    cancelLabel: "留在本页",
  });
  if (ok) { /* CTA */ }
  ```

  - **禁止**用 `shellUi.openModal` 做空态文案——`openModal` **仅**「添加项目」`#modal`（`shellUi.js:300-308`）。唯一例外：拆分（无项目）的主 CTA 在用户点「去选项目」**之后**再调 `openModal()` 打开添加项目框。
- 频控：同 tab + 同一空态原因（如 `split:no-plan` / `run:no-run`）**本会话只弹一次**（`sessionStorage` 或 `state` 标记即可）；避免巡检式打扰。切换项目可清与 `selectedPath` 相关的 key（推荐）。
- **禁止链式连弹**：CTA 跳到的目标若仍判空，本轮不再弹第二次（靠频控 key 或一次性 `fromGuard` 标志）。
- 文案零技术词（无 plan_id/run_id/schema），符合规则 23。

---

## 6. 落地任务（勾选只认本节）

| # | 任务 | 落点 | 验收 | 状态 |
|---|------|------|------|------|
| F0 | 本文 + docs/CLAUDE.md 索引 + **施工规格补丁**（§4.5 时序 · §5.3 API/范围 · F1∥最小 R4） | `docs/` | 索引可查；本文含真值表与禁止 openModal | ✅ 2026-08-20 |
| F1 | 两模式 chip + persona 默认 + session 记忆 + **触发契约** + 切换保留 slots + **§4.5 chip/send 钩子** + **最小 R4**（三入口主按钮行降级）+ installChat 挂载 | **`chatMode.js`（新）** · `chatClarify.js`（委托/净减）· `chatPersona.js` · `chatActions.js`（send 前 fast 钩子）· `installChat.js` · `chat.css` · **`web/CLAUDE.md` 一行** | 空态见两 chip、**不见**三入口主按钮并排；founder 默认 fast；**点 chip 不调 claimBriefToPlan、不产生 draft**；fast 首 send 前 `skip_requested===true` 且槽含 assumed；deep 右侧「先只想清楚」linkish 可点；切模式不清 explicit 进度 | ✅ 2026-08-20 |
| F2 | 快速模式 prompt 契约 + 与 §4.5 对齐（**不单改文案**） | `docs/runtime-prompts/chat-plan-writing.md`（+ README 若动覆盖序）· 必要时 normalize/文案测 | prompt 含「快速模式·常见假设」行；与 F1 钩子联调：fast 首轮路径可出 ```plan；黄条照旧；domain 零改 | ✅ 2026-08-20 |
| F3 | tab 空态守卫 | `web/js/main.js` `wireShellNav`（仅 ring 点击）· **`shared/tabEmptyGuard.js`** 判定/文案/频控 · 复用 `hasActiveRun`/`isRunPaused` · **仅** `confirmDialog` | 用户点段控：无项目/无计划/无 run 时弹窗 + CTA 正确；**程序化 go\* 不弹**；**空态路径不调用 openModal**（除非 CTA=去选项目之后）；同因不重复弹；有 draft 点拆分不弹；不链式连弹 | ✅ 2026-08-20 |
| F4 | 信息密度残余：R1 顶部条合并 · R2 banner 二选一 · R3 空态场景 chips 折叠 · R4 收尾（F1 已做最小降级则本项补 env/ready/banner/场景） | `chatRender.js`（env/ready/banner + scene fold）· `chat.css` · `index.html` env StateDot（**未**改 chatPersona，避 F1 冲突） | 空态首屏 ≤ coach+示例+输入+模式 chip；顶部常驻 ≤1 条；clarifying 后无过期三入口主行 | ✅ 2026-08-20 |
| F5 | 冒烟 + 构建 + 重打包 | 见下「F5 断言清单」· `web build` · `package-app.sh` | 全绿；dist 含模式 chip UI | ✅ 2026-08-20 · clarify-click 31/31 · chat-fold F4 · **tab-empty-guard playwright 4/4**（`playwright.config.js` webServer：`python3 -m http.server 3456 --directory web` · reuseExistingServer）· dist 含「快速出产品/深度思考」+ deep→fast toast；draft 判可用 markdown/path；`package-app.sh` 可选未强制 |

**依赖序**：

```text
F1（含最小 R4 + §4.5 客户端钩子） → F2（prompt 对齐）
F3 独立可并行
F4 残余（R1–R3；R4 若 F1 未做完则并入）可与 F3 并行，不得晚于 F5
F5 收口
```

**F5 断言清单**：

| 层 | 文件 | 断言 |
|----|------|------|
| node 源 | `scripts/clarify-click-smoke.mjs`（扩） | `chatMode.js` 存在；chip 文案「快速出产品」「深度思考」；**setMode/chip 路径源码不含**对 `claimBriefToPlan` 的调用；`selectClarifyEntry` 的 plan_only 自动 claim **仍在**（逃生舱）；`sendChatMessage`（或等价）含 fast 首 send 时 `applySkipWithAssumptionsLocal` / `skip_requested` 接线 |
| node 源 | 同上或 prompt 抽查 | `chat-plan-writing.md` 含快速模式/常见假设行 |
| node 源 | `scripts/chat-fold-smoke.mjs`（扩） | R1–R4 相关 class/选择器或合并注释存在（按 F4 实作补） |
| playwright | `tests/l2-interaction/tab-empty-guard.spec.js`（新 · 沿 w1-6） | 无计划点拆分 → 见确认层文案 + 留在本页不切；有 draft 点拆分 → 不弹；同因第二次点不弹；CTA「去聊天」到聊天页 |
| 构建 | `web/build.mjs` + `package-app.sh` | dist 含 chatMode / 新文案；无回归 |

---

## 7. 不做清单

- **不做**模式进 domain wire / 新增 `ChatMode` 字段（三入口语义已覆盖）。
- **不做** fast 自动 confirm/开跑（directExec 仅指「直接生成交付物」，不是启动 Run）。
- **不做**模式 × persona 强绑定（推断可被一次点击覆盖，不弹确认）。
- **不做** topbar 常驻模式指示（壳层减法；模式态由 chip 自身表达）。
- **不做** Trajectory 式聊天回放/时间轴（dsh 红线表既定）。
- **不做**用 `openModal` 充当空态/引导通用弹窗（仅添加项目）。
- **不做**对程序化 `appVm.go*` / job 完成跳转做空态拦截。
- **不做** F1 与旧三入口主按钮行双轨常驻（概念预算）。

---

## 8. 一致性自检（对照工程硬规则）

| 规则 | 检查 | 结论 |
|------|------|------|
| 10 唯一开跑 = Split confirm | 两模式终点一致；fast 仅省「问」，不省「确认」；§4.5 不预开跑 | ✅ |
| 21 主区 phase 不变 | 不新增 phase；模式只活在 chat 页 | ✅ |
| 23 人话第一句 | 空态弹窗/fast 首轮文案表已人话 | ✅ |
| 24 高级默认折叠 | R3/R4 用披露行/linkish，不删信息 | ✅ |
| 26 同屏新概念 ≤3 | 模式 chip 2 个；**F1 强制最小 R4 去掉三入口主行并排** | ✅ |
| 15/18 文件体积·厚文件禁堆 | F1 抽新 `chatMode.js`；`chatClarify.js`（2528 行）只留委托、净减不增 | ✅ |
| 19/20 MVVM·gateway | 空态/模式无新业务策略；不散落 invoke；confirmDialog 为壳层 UI | ✅ |
| 文档法则 | 无平行第二套阶段表；F 系列为本能力唯一勾选 | ✅ |

---

## 9. 风险与回滚（施工时）

| 风险 | 缓解 | 回滚 |
|------|------|------|
| 误复用 `selectClarifyEntry('plan_only')` 导致点 chip 出占位草稿 | §4.5 真值表 + F5 源断言禁止 chip→claim | 去掉 chip 点击绑定，恢复仅三入口 |
| 只改 prompt、send 未 skip → 模型仍盘问 | F1 钩子为 F2 前置；F5 断言 skip 接线 | 保留 chip UI，关闭 fast 默认推断 |
| 空态误用 `openModal` 弹出「添加项目」 | §5.3 硬禁止 + 审查 | 删除 ring 内 guard 即可，程序化路径未改 |
| 程序化 goSplit 被拦导致 job 完成进不了台 | 守卫仅 view-ring | 同上 |
| F1 未降三入口 → 概念超标 | F1 验收强制最小 R4 | — |

---

## 10. 多窗并发执行提示词（复制即开）

> 勾选仍只认 §6。本节 = 施工分工与可粘贴 prompt，**不是**第二套阶段表。  
> 仓库根：本仓。真源：本文 §4–§6、§9。

### 10.1 波次与文件所有权（防互踩）

```text
波次 A（三窗并行，互不改对方主文件）
  窗 W1 = F1      独占: chatMode.js(新) · chatClarify.js · chatPersona.js · chatActions.js · installChat.js · chat.css(模式相关) · web/CLAUDE.md
  窗 W3 = F3      独占: web/js/main.js(wireShellNav) · 可选 shellUi.js 纯函数导出（仅 hasActiveRun 旁薄 helper）
  窗 W4 = F4      独占: chatRender.js · chatPersona.js 仅场景 chips 披露（若 W1 已动 persona pathBias 导出，W4 只加 scene 折叠，先 pull/rebase W1）
                  冲突点: chatPersona.js / chat.css → W4 等 W1 merge 后再改 persona；或 W4 只动 chatRender + css 选择器

波次 B（W1 合并后）
  窗 W2 = F2      独占: docs/runtime-prompts/chat-plan-writing.md · 必要时 README

波次 C（A+B 全绿后，单窗）
  窗 W5 = F5      冒烟/playwright/build · 勾选 §6 · 可 commit
```

**硬规则（每窗开头遵守）**：不动 domain/`ClarifyEntry` wire；不 `confirm_start`/spawn；不往 `chatClarify.js` 堆大段新逻辑；F3 空态弹窗禁用 `openModal`（除 CTA 去选项目之后）；程序化 `go*` 不拦。

### 10.2 窗 W1 — F1（模式 chip + §4.5 钩子 + 最小 R4）

```text
你在 Leaf/cco 仓库。只做 docs/chat-dual-mode-empty-guard-2026-08-20.md 的 **F1**（含 §4.5 客户端钩子 + 最小 R4）。不要做 F2 prompt、F3 空态、F4 顶部条、F5 打包。

必读：该 md §4.1–§4.5、§6 F1 行、§7、§9。

目标：
1. 新建 web/js/features/chat/chatMode.js：composer 上方两 chip「快速出产品」「深度思考」；persona pathBias L→fast 默认、M/H→deep；session 记 entry（复用 state.chatSession.clarify.entry，不新存储）。
2. setMode(fast|deep)：只写 entry + mirror/stash + repaint。**禁止**调用 selectClarifyEntry('plan_only') 全分支（会 applySkip+claimBriefToPlan）。
3. 最小 R4：去掉与 chip 并排的旧三入口主按钮行；deep 时 chip 行右侧 linkish「先只想清楚」→ selectClarifyEntry('think_first')；「直接写计划」linkish 仍可走 selectClarifyEntry('plan_only') 即时 claim（逃生舱）。
4. sendChatMessage（chatActions）最前：fast 且尚无 draft 且未 skip → applySkipWithAssumptionsLocal(…, '快速出产品') + mirror，再 send；不预 claim 空模板。
5. installChat 挂 ccoChat.setChatMode/getChatMode/paintChatMode；chatClarify 只一行委托，净减不增。
6. chatPersona 导出 pathBias 读取；chat.css 模式 chip 样式（token/alias，禁写死色）。
7. web/CLAUDE.md features/chat 成员补 chatMode.js 一行。

验收自检：
- 点 chip 不产生 draft、不调 claimBriefToPlan
- fast 首 send 前 skip_requested===true、槽 assumed
- 空态不见三入口主按钮并排
- domain/Rust 零改

完成后：简述改动文件列表 + 如何手测；不要 commit 除非用户要。不要改 main.js wireShellNav。
```

### 10.3 窗 W3 — F3（tab 空态守卫）

```text
你在 Leaf/cco 仓库。只做 docs/chat-dual-mode-empty-guard-2026-08-20.md 的 **F3**。不要做 F1 模式 chip、F2 prompt、F4 聊天密度、F5。

必读：该 md §5 全文、§6 F3 行、§7（openModal/程序化 go*）、§9。

目标：
1. 仅改 web/js/main.js 的 wireShellNav 里 #view-ring 用户点击分支：goSplit/goRun/goResult 前判空。
2. 弹窗只用 shared/confirmDialog.js 的 confirmDialog / window.ccoConfirm（title/body/okLabel/cancelLabel）。
3. **禁止**用 shellUi.openModal 做空态文案。唯一：拆分无项目且用户点了「去选项目」之后，再 openModal() 添加项目。
4. 判定复用 hasActiveRun/isRunPaused 与 §5.2 字段表；可在 shellUi 加纯函数 helper，禁止新并行 state 机。
5. 程序化 appVm.go* / jobPoll / confirmActions **不**加守卫。
6. 频控：同 tab+原因本会话弹一次；禁止 CTA 目标仍空时链式连弹。
7. 文案用 §5.2 人话表，零 plan_id/run_id/schema。

验收自检：
- 无计划点拆分 → 弹窗，取消不切页；有 draft 不弹
- 源码空态路径无 openModal（除 CTA 去选项目后）
- 不碰 chat/* features

可顺手加 tests/l2-interaction/tab-empty-guard.spec.js 骨架（沿 w1-6），或留给 F5。不要 commit 除非用户要。
```

### 10.4 窗 W4 — F4（信息密度 R1–R3；R4 主降级归 W1）

```text
你在 Leaf/cco 仓库。只做 docs/chat-dual-mode-empty-guard-2026-08-20.md 的 **F4 残余**（R1/R2/R3）。**R4 三入口主行降级由 F1 负责**——若 F1 未合并，不要重做三入口大改，只做顶部条与空态场景折叠。

必读：该 md §2.3、§6 F4 行。

目标：
1. R1：chatRender 合并 env + ready——**ready 始终隐藏**（CTA 在计划卡）；env **仅异常**出一条（StateDot+一句+动作）。勿按「正常 ready 常驻」实现。
2. R2：有未认领 Brief → 澄清 Brief 独占；否则可出 last_summary；ready 不与之并立。
3. R3：空态场景 chips 收进「换个例子」默认折叠披露；首屏 = coach + 示例 chips + 输入（+ 若已有模式 chip 则并存，不删 chip）。
4. 只走 css token/alias；禁写死颜色；不改 send/confirm/domain。

文件优先：chatRender.js、chat.css；尽量避免与 F1 同改 chatPersona.js（若必须，先确认 F1 已合并 pathBias 导出）。

验收：空态首屏干净；顶部常驻 ≤1 条。不要 commit 除非用户要。
```

### 10.5 窗 W2 — F2（等 W1 合并后 · runtime prompt）

```text
你在 Leaf/cco 仓库。只做 docs/chat-dual-mode-empty-guard-2026-08-20.md 的 **F2**。前置：F1 已合并（§4.5 客户端 skip 钩子已在）。不要改 web JS 业务（除非 prompt 加载路径缺关键字需极小对齐）。

必读：§4.3 第 3 点、§4.5、§6 F2；现有 docs/runtime-prompts/chat-plan-writing.md 三入口表。

目标：
1. 三入口表增「快速出产品」行：entry=plan_only 且意图=快点看到东西时，**首条用户消息**（非点 chip）直接出 ```plan；槽位全记假设；计划开头写明「快速模式：以下按常见假设，可改」。
2. 与 §4.5 一致：点 chip 不出计划；禁 confirm_start/spawn；黄条/最小章节（目标·不做·验收）仍强制。
3. 人话第一句，无引擎名/VERDICT/run_id。
4. 若动覆盖序 → 同步 runtime-prompts/README；domain/Rust 零改。
5. **单改 prompt 若 F1 钩子不在，应在回报告知阻塞，不要假称 F2 完成。**

验收：md 含快速模式/常见假设关键字；与 F1 双轨描述一致。不要 commit 除非用户要。
```

### 10.6 窗 W5 — F5（收口 · 全员合并后）

```text
你在 Leaf/cco 仓库。做 docs/chat-dual-mode-empty-guard-2026-08-20.md 的 **F5 收口**。F1–F4 应已在工作树。

必读：§6 F5 断言清单、§9。

目标：
1. 扩 scripts/clarify-click-smoke.mjs：chatMode.js 存在；chip 文案；setMode/chip 路径无 claimBriefToPlan；selectClarifyEntry plan_only 自动 claim 仍在；send 路径有 fast 首 send skip 接线；chat-plan-writing 含快速模式行。
2. 按需扩 scripts/chat-fold-smoke.mjs（F4）。
3. tests/l2-interaction/tab-empty-guard.spec.js：无计划点拆分弹层；有 draft 不弹；同因二次不弹；CTA 去聊天。
4. cd web && node build.mjs；需要则 package-app.sh。
5. 全绿后：把 §6 F1–F5 勾成 ✅（若用户授权），更新 docs/CLAUDE.md 索引状态；**不要**新开阶段表。

跑通相关 smoke/playwright，失败贴日志。commit 仅当用户明确要求。
```

### 10.7 调度备忘（给人看）

| 顺序 | 动作 |
|------|------|
| 1 | 同时开 W1 / W3 / W4（W4 若怕 chatPersona 冲突可等 W1 后再开） |
| 2 | W1、W3 先 merge；W4 rebase 后再 merge |
| 3 | 开 W2（F2） |
| 4 | 开 W5（F5）全量验收 |
| 冲突热点 | `chat.css`（W1+W4）· `chatPersona.js`（W1+W4）· 勿让 W3 碰 chat/* |

---

> [PROTOCOL]: 本文为两模式 + 空态引导唯一勾选落点；改边界须同步 `docs/CLAUDE.md` 索引与涉及 L2（web/CLAUDE.md · runtime-prompts/README）；勾选只认 §6。F0 施工规格补丁（§4.5 / §5.3 / F1∥R4 / F5 清单 / §9）与多窗提示词（§10）已并入本文，不另开阶段表。
