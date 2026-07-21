# cco 计划管理 → 执行任务：操作流收敛

> 状态：**E0–E4 已落地**（入口止血 · 统一执行 · 拆完回跳 · 文案双轨 · plans_dir 过滤 · 选项薄层；桌面需重打包目视）  
> 日期：2026-07-19  
> 范围：从用户点「计划管理」起，到任务真正在跑（含拆分确认 / 自动开跑 / 执行看板）的**整条操作链**；不重做聊天共建、不改 Mode B 内核契约  
> 角色：体验收敛子计划——把「管计划」与「跑计划」拆清，砍掉重复列表/重复「分配」/页面乱跳，让用户永远知道**下一步只点哪一个钮**  
> 关联真源：
> - 计划管理主线 → [`ux-plan-mgmt-attach-ttl-2026-07-19.md`](./ux-plan-mgmt-attach-ttl-2026-07-19.md)（G0–G6 **已落地**；本计划**不**回勾其 ☐）
> - 聊天主窗 → [`chat-home-plan-cli-2026-07-19.md`](./chat-home-plan-cli-2026-07-19.md)（H0–H4 路由/已执行/stall；本计划吃其路由，**不**重做）
> - 主路径三步 → [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（「分配后默认 auto-start」仍有效）
> - Mode B → [`product-mode-b-ai-planner.md`](../product-mode-b-ai-planner.md)（`confirm_start` 唯一业务入口，**不改**）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md)（本计划 → **D5 / P2-14 · P-plan-exec-flow**；**勿**回灌 D0–D4 / P2-12 / P2-13）
> GEB 入口：[`/CLAUDE.md`](../../CLAUDE.md)（L1）· [`./CLAUDE.md`](../CLAUDE.md)（L2 docs）

> **定稿（t1）**：对照现网代码 + 用户反馈「从计划管理到执行任务后流程乱」；冻结问题根因、目标心智、规格、阶段、非目标与成功标准。  
> **实施（t2）**：E0–E2 + E3 大部：`openPlanManagement` 不弹层；`startExecuteFromSelection`；`advancePlannedJob` 强制回 workspace；文案「执行此计划 / 开始拆分 / 编辑文档」。  
> **实施（t3）**：E1 薄层折叠列表 · E3 编辑任务 · E4 plans_dir 过滤 + 换夹刷新 +「显示其它位置」。  
> 实施勾选真源 = **§5**（E0–E4）；**禁止**第二份「执行流总览」；**禁止**把 G0–G6 / H0–H4 勾回未完成。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**「计划管理」只负责选中/看/改；点一次「执行」就进工作区完成拆分与开跑；全程只有一份计划列表真相、一个主 CTA、一个当前阶段。**

```text
【管】  计划管理页 = 本夹计划列表 + 详情预览 + 编辑 md
【跑】  详情主钮「执行此计划」→ 工作区（可选轻量选项）→ Mode B 拆分 → 自动开跑（默认）
【看】  有活动 run / 待确认拆分 → 顶栏只留「返回执行 / 继续确认」
【禁】  同屏三套列表、同名两颗「分配计划」、管理页上再盖一层选计划弹窗
```

---

## 1. 用户视角：现在为什么乱

### 1.1 用户以为的流程（心智）

```text
写好计划 → 打开计划管理 → 点那份计划 → 点执行 → 看任务跑
```

期望步数：**3 次有意义点击**（开管理 · 选计划 · 执行）。中间可以有一次「确认 CLI/并发」，但不应再出现第二套列表或第二颗同名按钮。

### 1.2 现网真实路径（代码行为 · 2026-07-19）

```text
① 聊天保存 .md
② 顶栏「计划管理」
   · 首次 confirm 默认目录
   · 若已有 selected/draft → showPage(plans) 且**立刻** openPlanChooser
   · 若无选中 → 进管理页 + toast「请先选中」
③ 管理页左侧再选一次 / 右侧「分配计划」
④ 浮层 chooser 又是一份列表 + 再点一次「分配计划」
⑤ showPage(workspace) · phase=planning 拆分
⑥ 默认 auto-start → running；若暂停确认 / 有可选任务 / 人不在 workspace
   → 停在 confirm 或 toast「请返回确认」→ 用户找「继续确认」
⑦ 执行看板；中途回聊天后要找「返回执行」
```

用户感受：

| 感受 | 实际发生 |
|------|----------|
| 「我不是已经在计划管理了吗，为什么还要选计划？」 | 管理页列表 ≠ chooser 列表，两套 UI |
| 「分配计划点了怎么又出来一个分配计划？」 | 详情钮只**打开选项**；chooser 脚钮才**真拆分** |
| 「点计划管理怎么直接弹分配？」 | `openPlanManagement` 有选中时强制 `openPlanChooser(true)` |
| 「弹层闪一下没了」 | `renderPlanPicker`：**非 workspace 且非 chat 会关 chooser** → 在 `page=plans` 上再渲染会关弹层 |
| 「拆完了人在哪？」 | 不在 workspace 时不 auto-start，只 toast，阶段按钮文案在变 |
| 「编辑是改 md 还是改任务？」 | 管理页「全文/编辑」= 散文；工作区「编辑计划」= 拆分后任务图 |

### 1.3 根因总表（按用户痛感排序）

| ID | 根因 | 代码锚点 | 用户后果 |
|----|------|----------|----------|
| **R1** | **三套计划列表** | `page-plans` · `#plan-rail` · `#plan-chooser` | 不知道以谁为准 |
| **R2** | **「分配计划」一名三义** | 管理详情 `#btn-plans-assign` · chooser `#btn-chooser-assign` · 顶栏 `#btn-pp-analyze` | 点了以为跑了，其实只开了选项 |
| **R3** | **管理入口越权** | `openPlanManagement` 有选中即 `openPlanChooser` | 「管理」变成「半执行」 |
| **R4** | **管理页与 chooser 互斥 bug** | `renderPlanPicker`：`!inChat` 关 chooser | 管理页上选项层不稳 |
| **R5** | **阶段跳转不闭环** | `advancePlannedJob` 人在 chat/plans 时只 toast | 拆完找不到「下一步」 |
| **R6** | **页面过多且职责重叠** | chat / plans / workspace + 浮层 + modal | 心智地图碎 |
| **R7** | **编辑语义双轨未标注** | 全文 modal vs 确认屏编辑 | 改完不知是否已「重分配」 |
| **R8** | **列表数据源与保存目录可不一致** | 保存 `plans_dir`；扫描 `list_plans(project)` 固定逻辑 | 换夹后「管理页空 / 扫到别处」 |

> R1–R5 为本计划 **P0**；R6–R8 为 **P1**（同迭代可做，不阻塞主路径收敛）。

### 1.4 与已落地计划的边界（勿当缺口）

| 已完成 | 本计划只修 |
|--------|------------|
| G0–G6：右栏默认藏、标题、单击双击、换夹、附图、TTL | **不**重做；只收敛「管理 → 执行」衔接 |
| H0–H4：有跑进 workspace、已执行 badge、stall/failover | 沿用；补「拆完不在场」回跳 |
| 主路径 auto-start | **保持**默认；只让入口别绕路 |
| Mode B `confirm_start` | **不改**契约；UI 仍最终调它 |

---

## 2. 需求拆解（用户语言 → 产品语义）

| # | 用户语言 | 产品语义 | 优先级 |
|---|----------|----------|--------|
| **U1** | 计划管理里操作别乱 | 管理页职责冻结：列表 · 预览 · 改 md · **一个**主 CTA「执行此计划」 | P0 |
| **U2** | 选完就要能跑 | 主 CTA 直接进入执行流水线；**不再**要求用户在第二套列表里重选同一文件 | P0 |
| **U3** | 别让我猜下一步点哪 | 任意时刻顶栏/主区 **最多 1 个 primary** 表示「当前该做的事」 | P0 |
| **U4** | 分配就是执行，不要绕 | 文案统一：「执行此计划」= 进入 Mode B；选项用「执行选项」不叫第二份分配 | P0 |
| **U5** | 拆完了告诉我去哪看 | 人不在 workspace 时：自动切到确认/运行相，或强 primary「继续确认 / 查看运行」 | P0 |
| **U6** | 跑起来后别和管计划搅在一起 | 运行/确认相：隐藏「计划管理」主入口或降为 ghost；突出停止/监视 | P1 |
| **U7** | 改计划 vs 改任务说清楚 | 散文编辑 =「编辑文档」；拆分图 =「编辑任务」；已执行仍另存副本 | P1 |
| **U8** | 流程短 | 有选中时：管理 → 执行 →（可选选项一次）→ 跑；目标 ≤4 次点击到 running | 贯穿 |

---

## 3. 设计原则（硬约束）

1. **单列表真相**：用户可见的「计划文件列表」在管理场景只认 **计划管理页**；聊天右栏 = 可选快览（次要）；chooser **禁止**再铺第三套全量列表当主交互。  
2. **单主 CTA**：同一阶段只有一个 primary。  
3. **管理 ≠ 执行**：点「计划管理」**只**进管理页，**绝不**自动弹分配层。  
4. **执行带走选中**：从管理页点执行时，`selectedPlan` 已定 → 直接 workspace 流水线；选项层只调 CLI/并发/暂停，**不**重选文件（除非用户主动「换一份」）。  
5. **阶段 entrapment 禁止**：拆分结束 / 开跑后，若用户不在执行面，必须**自动带回**或给出不可忽视的「继续」primary。  
6. **不改内核**：`start_plan_job` / `confirm_start` / auto-start 默认 / stall·failover 语义保持。

**不改：**

- Mode B 业务入口仍是 `confirm_start`  
- 默认 `autoStartAfterPlan = true`（可选任务仍强制人工确认，见 memory 约定）  
- 已执行计划不原地当新交付（仍另存副本）

---

## 4. 修后主路径（唯一心智）

### 4.1 主路径（推荐 · 有已保存计划）

```text
① 侧栏选项目 · 无活动 run → 聊天（写/改）或直接「计划管理」
② 「计划管理」→ page=plans
     · 左：本夹计划（标题 + badge）
     · 右：预览正文 + 【执行此计划】primary + 【编辑文档】ghost
③ 单击选中（高亮 + 右侧预览）；双击 = 编辑文档 modal
④ 点「执行此计划」
     · 关闭管理页心智，showPage(workspace)
     · 若需改 CLI/并发：打开【执行选项】薄层（无全量列表；顶部显示已选计划名；主钮「开始拆分」）
     · 若用户从不改选项：可设置「记住上次选项，直接开始拆分」（默认：仍显示薄层一次，可勾「下次跳过」）
⑤ phase=planning → 完成
     · auto-start 且无 optional → 直接 running
     · 暂停确认 / 有 optional → phase=confirm，主钮「开始运行」
⑥ running：看板 + 时长 + stall；顶栏「聊天」ghost、「计划管理」弱化
⑦ done：完成卡「回聊天」/「再跑」/ 有 ISSUES 时「回补」
```

### 4.2 从聊天捷径（不经管理页）

```text
保存成功后就绪条：
  · primary「执行此计划」（等同管理页主 CTA，带上 chatDraftPlan）
  · ghost「打开计划管理」
禁止：保存后只写「请用顶栏计划管理」却不给执行入口。
```

### 4.3 顶栏按钮矩阵（冻结）

| 页面 / 阶段 | Primary | Ghost / 次要 | 隐藏 |
|-------------|---------|--------------|------|
| chat · 无活动 | **计划管理**（进管理） | 有草稿时就绪条「执行」 | 选择计划 / 分配（顶栏） |
| plans · 有选中 | **执行此计划**（页内） | 回聊天 · 换夹 · 显示已执行 | 再弹 chooser 全量列表 |
| workspace · pick | **执行此计划** 或打开执行选项 | 选择计划（仅换文件时） | — |
| workspace · planning | （无抢戏 primary） | 取消规划 | 计划管理可弱显 |
| workspace · confirm | **开始运行** | 重新规划 · 返回监视 | 计划管理 |
| workspace · running | （停止等运行控件） | **返回执行** 仅在他页 | 分配/选择计划 |
| 任意 · 他页有活动 | **返回执行 / 继续确认** | 聊天 · 计划管理 ghost | 第二套执行入口 |

### 4.4 「执行选项」薄层（取代滥用的全量 chooser）

| 项 | 规格 |
|----|------|
| 标题 | **执行选项**（禁止再叫「选择并分配计划」当主标题） |
| 顶部 | 固定展示：`将执行：{短标题}` + 路径 tooltip；链「换一份计划…」才展开列表 |
| 主体默认 | CLI · 并发 · 「规划后暂停确认」；高级折叠 |
| 主钮 | **开始拆分**（调用现有 `analyzePlanFromPicker` / 等价） |
| 关闭 | Esc / 关闭；**不**清 `selectedPlan` |
| 打开条件 | 用户点「执行此计划」且（首次 / 未勾「记住并直接开始」/ 显式要改选项） |
| 与旧 chooser | 旧全量列表模式降级为「换一份计划」子态；扫描/手动选文件只在子态 |

### 4.5 管理入口行为（覆盖 G1 越权）

| 动作 | 新行为 |
|------|--------|
| 点顶栏「计划管理」 | **只** `showPage("plans")` + 扫列表；**禁止** `openPlanChooser` |
| 已有 draft/选中 | 自动高亮该项并拉详情；**不**弹执行选项 |
| 首次目录确认 | 可保留一次 confirm；文案缩短 |
| 管理页「分配计划」文案 | 改为 **执行此计划** |

### 4.6 拆分完成人不在场（修 R5）

| 条件 | 行为 |
|------|------|
| `advancePlannedJob` 且 `page !== workspace` | **默认 `showPage("workspace")`** 进入 confirm（或 auto-start 路径）；toast 说明「拆分完成」 |
| 用户明确在聊天输入中（可选侦测 focus） | 可不抢焦点，但顶栏 **primary「继续确认」** + banner 不可关到无入口 |
| auto-start 将开跑 | 同样切到 workspace · running，避免后台静默起 worker 而用户还在管理页 |

### 4.7 聊天右栏角色（降级为快览）

| 规则 | 冻结 |
|------|------|
| 名称 | **计划快览**（已有） |
| 能力 | 单击选中 · 双击预览；hint 改为「选中后可顶栏「计划管理」细看，或就绪条执行」 |
| **不做** | 右栏不承担「换夹 / 完整管理 / 分配主入口」 |
| 与管理页 | 共享 `selectedPlan` / meta；**数据一套** |

### 4.8 列表与 `plans_dir`（R8 最小修）

| 项 | 规格 |
|----|------|
| 管理页扫描 | 优先列 `{plans_dir}/**/*.md`（及用户绑定目录）；与保存一致 |
| 兼容 | 若项目根其它历史 `.md` 计划，可「显示其它位置」折叠，默认关 |
| 空态 | 「此夹暂无计划 · 回聊天保存」+ 显示当前 `plans_dir` |

---

## 5. 阶段切分与勾选（实施真源）

### E0 — 止血：入口与同名（体感最大 · 1 小步）

- [x] `openPlanManagement`：**删除**有选中即 `openPlanChooser`；只进 `page=plans` 并选中高亮  
- [x] 管理页 / 就绪条文案：**分配计划** → **执行此计划**（详情脚 + 聊天就绪条 primary）  
- [x] `renderPlanPicker`：允许 `page=plans` **不**误关执行层（chat/plans 保留 chooser）  
- [ ] 目视：点计划管理不弹层；有选中只高亮（需重打包桌面）

### E1 — 执行主路径：一带走选中进 workspace

- [x] `startExecuteFromSelection(planPath)` 统一入口：set selected → workspace → 执行选项  
- [x] 管理页「执行此计划」· 聊天就绪条 · 全文 modal 全部走统一入口  
- [x] 薄层默认**不**渲染全量列表；「换一份计划…」才展开  
- [x] 禁止：执行前强迫用户在第三套列表点同一文件（选中已带走）  

### E2 — 阶段闭环：拆完/开跑必回执行面

- [x] `advancePlannedJob`：非 workspace 默认切 workspace（confirm 或 auto-start）  
- [x] 顶栏矩阵：running/confirm 时计划管理弱化为 ghost  
- [x] 完成后完成卡下一步保持（沿用 G6，不重复造）  

### E3 — 文案与编辑双轨

- [x] 「全文/编辑」→ **编辑文档**；chooser → **执行选项** / **开始拆分**  
- [x] 欢迎 / 帮助 / 空态主路径文案对齐「执行此计划」  
- [x] 工作区拆分编辑文案统一为「编辑任务」  

### E4 — 列表与目录对齐（P1）

- [x] 管理页 / 右栏列表默认按 `plans_dir` 过滤；「显示其它位置」展开  
- [x] 换夹后立刻 `loadPlanRail` + 管理页重绘  
- [ ] 目视：自定义 `docs/plans` 保存后管理页可见（需重打包）  

### 建议落地序

```text
E0（入口止血 + 改名）          ← 当天可验
 → E1（统一执行入口 + 薄层）   ← 主路径变短
 → E2（阶段闭环回跳）          ← 消灭「拆完失踪」
 → E3（文案/编辑双轨）
 → E4（plans_dir 对齐）
```

---

## 6. 非目标

| ID | 不做 | 原因 |
|----|------|------|
| N1 | 取消 Mode B / 聊天内直接 spawn worker | 破坏 `confirm_start` |
| N2 | 删掉聊天右栏 | 快览仍有价值；只降权 |
| N3 | 重做 Scheduler / stall / failover | H3/H4 已有 |
| N4 | 回勾 G0–G6 / H0–H4 为未完成 | 总账纪律；本计划是衔接层 |
| N5 | 多会话 tab / 流式（C3） | P2-9 |
| N6 | 全量 multi-cli 协作 UI | 另册 |
| N7 | 移动端 / 新信息架构大改版 | 超范围 |

---

## 7. 成功标准

| ID | 标准 | 验证 |
|----|------|------|
| S1 | 点「计划管理」**不会**自动弹出选计划/执行浮层 | 目视 |
| S2 | 有选中计划：管理页 **1 次**点「执行此计划」进入拆分或执行选项，**无需**在第二列表重选同一文件 | 目视路径 |
| S3 | 任意阶段顶栏+主区 primary ≤ 1 个「当前该做的事」 | 对照 §4.3 |
| S4 | 拆分完成时用户若在 chat/plans，会进入 workspace 确认/运行，或有不可丢的「继续确认」 | 打断路径测 |
| S5 | 全文里不再出现两颗同名「分配计划」串联 | 文案 diff |
| S6 | `plans_dir` 换夹后保存与列表一致 | 手测 + 可选单测 |
| S7 | `node --check web/js/*` 绿；相关桌面路径不回退 auto-start 默认 | CI 本地 |
| S8 | 新用户口述路径可复述：管理→选→执行→看跑；无需解释 chooser/phase 术语 | 录屏清单 |

---

## 8. 风险与默认决策

| Q | 议题 | 默认 |
|---|------|------|
| Q1 | 执行前是否还要选项层？ | **要**，但是薄层；可「记住并下次直接开始」 |
| Q2 | 是否合并 page-plans 进 chat？ | **本迭代不合并**；先理顺职责，避免大改路由 |
| Q3 | workspace 顶栏「选择计划」保留吗？ | 保留为「换文件」次要入口；主路径不依赖它 |
| Q4 | auto-start 与「拆完必回 workspace」是否冲突？ | 不冲突：回 workspace 后仍可立即 auto `confirm_start` |
| Q5 | 可选任务强制确认是否保留？ | **保留**（用户偏好 / memory）；仅 UI 说清「含可选任务，请确认勾选」 |
| Q6 | 旧 chooser 全删？ | **不删**；降为「换一份计划」子态，避免回归扫描/手动选文件 |

---

## 9. 关键文件地图

| 区域 | 文件 |
|------|------|
| 管理入口 / 管理页 | [`web/js/chat.js`](../../web/js/chat.js) `openPlanManagement` · `assignFromPlansMgmt` · `renderPlansMgmtPage` |
| 执行/chooser/阶段 | [`web/js/plan.js`](../../web/js/plan.js) `openPlanChooser` · `renderPlanPicker` · `analyzePlanFromPicker` · `advancePlannedJob` · `confirmAndStart` |
| 壳结构 / 文案 | [`web/index.html`](../../web/index.html) `#page-plans` · `#plan-chooser` · 顶栏钮 |
| 样式 | [`web/css/chat.css`](../../web/css/chat.css) · [`web/css/plan.css`](../../web/css/plan.css) |
| 列表扫描 | [`src/services/runs.rs`](../../src/services/runs.rs) `list_plans` / `list_plan_meta`（E4 若要按 dir 滤） |
| 保存目录 | [`src/services/chat.rs`](../../src/services/chat.rs) `chat_save_plan` `plans_dir` |

---

## 10. 修订历史

| 时点 | 内容 |
|------|------|
| **t1 · 2026-07-19** | 初稿定稿：用户反馈「计划管理到执行任务流程乱」；对照 `openPlanManagement` / 三列表 / 双「分配」/ `renderPlanPicker` 关层 / `advancePlannedJob` 不在场；冻结 E0–E4、非目标、成功标准；总账 ID **P2-14 / P-plan-exec-flow**；明确基于 G0–G6 + H0–H4 **衔接层**而非从零 |
| **t2 · 2026-07-19** | 实施 E0–E2 + E3 大部：`web/js/chat.js` · `plan.js` · `doctor.js` · `index.html`；`node --check` 三文件绿；E4 与 chooser 列表默认折叠未做；桌面需重打包目视 |
| **t3 · 2026-07-19** | E1 选项薄层折叠 · E3 编辑任务 · E4 plans_dir 过滤/换夹刷新/显示其它位置；打包 CCO.app |

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`
