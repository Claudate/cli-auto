# cco 非开发主路径 · 落地实施计划

> 日期：2026-07-21  
> 角色：**体验落地实施真源**（派工 / 勾选 / PR 边界）  
> 诊断依据：[`ux-nondev-mainpath-2026-07-21.md`](./ux-nondev-mainpath-2026-07-21.md)  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)  
> 架构边界（**不重开 A0–A5**）：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)  
> 交互意图参考（**不继承其 ✅**）：[`product-mainpath-optimize-2026-07-20.md`](./product-mainpath-optimize-2026-07-20.md)  
> 范围：`web/` 为主 · 设置文案/默认值 · 帮助页；**不**换栈 · **不**旁路 `confirm_start` · **不**改 Scheduler 内核  
> 状态：**波次 A–D ✅ 收口**（2026-07-21 · 体验主路径 MVP Ship；§5 仍建议本机目视）

[PROTOCOL]: **勾选只认本文件 §3 任务表**。诊断文 `ux-nondev-mainpath` 保留为背景，不再双轨勾选。落地后回写 PRODUCT / docs L2 / web L2。每波结束必须跑 §5 非开发脚本，不得以 `cargo test` 或 facade 行数代替。

---

## 0. 目标 / 非目标

### 0.1 一句话目标

让主受众（PM / 出海 / 非开发）在**不看帮助、不碰设置、不打开计划管理**的前提下，用最新打包 App 走完：

```text
加工作文件夹 → 一句话说目标（或模板）→ 直接进入拆分台 → 看懂波次 → 确认并开始 → 看懂进度 → 一页收口
```

### 0.2 非目标

| 不做 | 原因 |
|------|------|
| 换 React/Vue / 新组件库 | 架构已收口；成本与风险不对 |
| 重开 A0–A5 / workspace crates | 边界已 ✅ |
| 重写 Planner / Scheduler | 体验问题在壳与路由，不在内核 |
| 默认打开巡检 / push / PR | PRODUCT 高级默认关 |
| 删 multi-provider / stall / rework **能力** | 只藏进高级 |
| 像素抄 Codex | PRODUCT 禁止 |
| 平行第二套架构阶段表 | L1 硬规则 |

### 0.3 硬契约（落地时不可破）

1. **唯一业务开跑**：`confirm_start` / `gateway.confirmStart` → `split::confirm`。禁止 UI `start_run` 旁路。  
2. **optional**：有业务可选时仍须可勾选；**禁止** auto-start 跳过未勾选可选（既有逻辑保留）。  
3. **MVVM**：策略不进 View；默认值变更在 prefs + 设置文案，业务门禁仍在 Rust。  
4. **文件体量**：软 400 / 硬 600；禁止往 classic facade / `state.js` 堆业务（只改默认值或一行委托）。  
5. **IPC**：只经 `gateway`。

---

## 1. 体感拐点（必须先做到）

| # | 用户会说的话 | 代码落点 |
|---|--------------|----------|
| **T1** | 「拆完/重开项目，拆分台还在，不用找返回」 | `sessionEntry.js` `resolveEntryRoute` |
| **T2** | 「点拆成步骤不会先弹通道/并发」 | `state.js` `chatAssignDirect` 默认开；`projectPicker.js` |
| **T3** | 「拆分台顶上就一个开始按钮」 | `index.html` + `shellChrome` / split 顶栏 |
| **T4** | 「设置里还能找回高级」 | 设置文案翻转；能力保留 |

**最小可感知发布（MVP Ship）= 波次 A = T1+T2+T3+T4 文档**，约 2–3 人日。

---

## 2. 波次总览

```text
波次 A  入口与减法（体感拐点）     ~2–3 人日   ← 先 ship 打包
波次 B  写计划顺滑 + 拆分可读       ~3–4 人日
波次 C  执行/结果收口 + 冷启动      ~3–4 人日
波次 D  验收 · 文档 · 打包          ~1 人日
```

依赖：A → B/C 可部分并行（B 聊天 vs C 跑收不同树）；D 依赖 A 必过，B/C 至少主路径项过。

---

## 3. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> 每任务：文件 · 步骤 · 完成定义 · 自测 · 依赖

### 波次 A — 入口与减法（P0 · MVP Ship）

#### A1 · 待确认强制进拆分台

| 项 | 内容 |
|----|------|
| **ID** | **A1** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | 无 |
| **文件** | [`web/js/features/project/sessionEntry.js`](../web/js/features/project/sessionEntry.js)（`resolveEntryRoute` · `applyEntryRoute` · `tryRestorePersistedPlanJob` toast）<br>[`web/js/features/project/shellChrome.js`](../web/js/features/project/shellChrome.js)（`updateSplitPlanChip` / `btn-monitor-plan` 文案若需）<br>必要时 [`web/js/app/routes.js`](../web/js/app/routes.js) soft-sync 不抢 confirm |
| **现状** | `planned`/`confirmed` → `{ page: "chat" }`；仅 toast「顶栏返回确认」 |
| **改法** | 1. `resolveEntryRoute`：若 `planJobId` 且 status∈`planned\|confirmed` 或 `state.phase==="confirm"` → `{ page: "workspace", phaseHint: "confirm" }`（优先级：活动 run > planning > **confirm** > chat）<br>2. `applyEntryRoute`：`phaseHint==="confirm"` 时 `state.phase="confirm"`，`showPage("workspace")`，`renderPhasePanels` + `renderConfirmPanel`（走既有 `host.renderConfirmPanel`）<br>3. **必须改 `selectProject` 双保险**（约 L515–528）：今日在 `applyEntryRoute` 之后若 page=workspace 且非 planning 会 **强制 `openChatPage`**，只改 route 不删这段 = 半修好又被冲回聊天。放宽为：允许 `confirm` / `planned|confirmed` 停在 workspace<br>4. `tryRestorePersistedPlanJob` toast：去掉「顶栏返回确认」→「已回到拆分台，核对后可确认并开始」<br>5. 聊天页保留弱入口「回写计划」/ `btn-open-chat`；用户主动离开拆分台后可用 `btn-monitor-plan`「继续核对拆分」 |
| **完成定义** | 拆完停台；杀进程重开同项目仍落拆分台；无 job 时仍落聊天 |
| **自测** | fake：写计划→拆成步骤→见拆分台→回欢迎再选项目→仍见拆分台；无 job 项目→聊天；中途点聊天再点「继续核对」能回拆分台 |

#### A2 · 主路径默认跳过「执行选项」层

| 项 | 内容 |
|----|------|
| **ID** | **A2** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | 无（可与 A1 并行） |
| **文件** | [`web/js/state.js`](../web/js/state.js)（`CHAT_ASSIGN_DIRECT_KEY` / `chatAssignDirectEnabled`）<br>[`web/js/features/project/projectPicker.js`](../web/js/features/project/projectPicker.js)（`startExecuteFromSelection`）<br>[`web/index.html`](../web/index.html) 设置 `#s-chat-assign-direct` 文案<br>[`web/js/features/settings/settingsForm.js`](../web/js/features/settings/settingsForm.js) 绑定说明 |
| **现状** | 默认 `localStorage` 无键 → `chatAssignDirectEnabled()===false` → 每次弹 `#plan-chooser`（通道/并发/规划方式） |
| **改法** | 1. **默认开直拆**：`chatAssignDirectEnabled()`：无键或 ≠`"0"` → true；仅显式 `"0"` 关（**勿**与 `autoStartAfterPlan` 混：跳过选项层 ≠ 自动 confirm_start）<br>2. 设置项改名：**「拆成步骤前先确认选项」**（勾选 = 写 `"0"` = 关直拆）；默认不勾；`settingsForm` load 与新默认一致<br>3. `startExecuteFromSelection`：已有 planPath 时 `direct` 默认 true → `analyzePlanFromPicker`；无 path 仍 `openPlanChooser` **仅选文件**；`opts.direct===false` 可强制出选项<br>4. **直拆前并发种子**：确认 `#s-max-parallel` / 设置已写入 `#chooser-max-parallel`/`#pp-max-parallel`（settingsForm 已有 seed 则复测一遍，避免 DOM 未渲染导致并发丢默认）<br>5. toast 去掉「方案 B」→ 「正在拆成步骤…」；chooser DOM id **保留不删** |
| **完成定义** | 新装/清 localStorage 用户：聊天点「拆成步骤」**不出现**通道/并发表单，直接 planning→confirm；optional 仍挡 auto-start |
| **自测** | 清 `cco.chatAssignDirect` → 拆；勾选设置「先确认选项」→ 应出 chooser；无选中计划 → 仍可出列表选文件 |

#### A3 · 拆分台顶栏只留主路径控件

| 项 | 内容 |
|----|------|
| **ID** | **A3** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | 无 |
| **文件** | [`web/index.html`](../web/index.html) `#plan-phase-confirm` `.split-actions`<br>[`web/js/features/split/splitFillMeta.js`](../web/js/features/split/splitFillMeta.js) / [`SplitView.js`](../web/js/features/split/SplitView.js) 若控制按钮显隐<br>[`web/css/plan.css`](../web/css/plan.css) 顶栏布局（可薄） |
| **现状** | 同排：`让可并行的真正并行` · `重新拆分（保留你的修改）` · `写回步骤摘要` · `确认并开始` |
| **改法** | 1. **主路径可见**：`#btn-confirm-start`（primary）+ `#btn-replan`（ghost，文案缩为 **「重新拆分」**，title 保留「保留你的修改」）<br>2. **默认 hidden**，收入 `<details class="split-more-actions">` 或「调整…」菜单：`#btn-sanitize-deps` · `#btn-split-writeback` · `#btn-skip-confirm` · `#btn-confirm-back`<br>3. 质量区 `#split-quality`：默认只显示一条人话 summary；chips / 「开启巡检并重拆」仅在 details 展开后<br>4. 不删 API，只改默认可见性 |
| **完成定义** | 首屏拆分台可点主路径按钮 ≤ 2（重新拆分 + 确认并开始）；高级仍可达 |
| **自测** | 目视 + 点击「调整」仍能 sanitize / writeback |

#### A4 · 壳顶栏收敛（主路径）

| 项 | 内容 |
|----|------|
| **ID** | **A4** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | A1（confirm 落地后 chip 逻辑） |
| **文件** | [`web/js/features/project/shellChrome.js`](../web/js/features/project/shellChrome.js)<br>[`web/index.html`](../web/index.html) `#top-actions` / `#top-more` |
| **改法** | 1. 主路径同时最多 **1 个 primary**（情境：写计划页可无；workspace 非 confirm 时「拆成步骤」）<br>2. `#btn-plan-mgmt` · `#btn-plan-choose` · `#budget-chip` · `#btn-refresh` **默认只在「更多」内**（已有 details 则移入并保证 chat/workspace 主路径不裸露）<br>3. 有待确认且用户在 chat 时：显示一个清晰 **「继续核对拆分」**（可复用/改名 `#btn-monitor-plan`），点进 confirm（配合 A1 后较少需要）<br>4. 拆分 chip：仅非 confirm 的 workspace 显示（现状大体对，核对 A1 后不重复） |
| **完成定义** | 写计划首屏顶栏无按钮丛林；概念可指认 ≤3 |
| **自测** | chat / confirm / running 三态截顶栏 |

#### A5 · 设置与帮助默认句（波次 A 文档面）

| 项 | 内容 |
|----|------|
| **ID** | **A5** |
| **状态** | ✅ |
| **估时** | 0.3 d |
| **依赖** | A2 |
| **文件** | [`web/index.html`](../web/index.html) `#page-settings` · `#page-help` 主路径段落<br>[`web/js/flow.js`](../web/js/flow.js) 若有 chooser 文案键 |
| **改法** | 1. 设置「开始与拆分」首段：默认直拆 + 默认停拆分台；高级才「先确认选项 / 拆分后自动开始」<br>2. 帮助标题去掉「模式 B」→「三步上手」；删 R-S 清单出主帮助（可链到底部「给进阶」）<br>3. 去掉帮助里 TOML `providers.claude` 作第一屏内容 |
| **完成定义** | 帮助首页三步；无 Mode B 作标题 |
| **自测** | 目视帮助 / 设置 |

#### A6 · 波次 A 回归与打包

| 项 | 内容 |
|----|------|
| **ID** | **A6** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | A1–A5 |
| **步骤** | 1. `node --check` 改过的 js<br>2. §5 脚本项 1–5（至少）真人走<br>3. `scripts/package-app.sh` 出包目视<br>4. 本文件 A1–A5 勾 ✅ + 修订记录 |
| **完成定义** | MVP Ship 可对外：「拆分台找得着、选项层不挡路、按钮少了」 |

---

### 波次 B — 写计划顺滑 + 拆分可读（P0）

#### B1 · 聊天空态引导

| 项 | 内容 |
|----|------|
| **ID** | **B1** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **文件** | [`web/js/features/templates/catalog.js`](../web/js/features/templates/catalog.js) 或 `planTemplateChatEmptyHtml`<br>[`web/js/features/chat/chatRender.js`](../web/js/features/chat/chatRender.js)<br>[`web/css/chat.css`](../web/css/chat.css) 薄样式 |
| **改法** | 空态结构：一句教练文案 + 3 个示例目标（点填输入框）+ 模板两个；去掉工程师说明堆叠 |
| **完成定义** | 新项目打开聊天 5 秒内知道「输入目标点发送」 |
| **自测** | 空项目选中 → chat 空态 |

#### B2 · 主 CTA：保存与拆分意图合并（用户层）

| 项 | 内容 |
|----|------|
| **ID** | **B2** |
| **状态** | ✅ |
| **估时** | 0.5–1 d |
| **依赖** | A2 |
| **文件** | [`web/js/features/chat/chatPlanOps.js`](../web/js/features/chat/chatPlanOps.js) / `chatActions.js`<br>[`web/index.html`](../web/index.html) `#chat-ready-bar` 按钮<br>经现有 save + `startExecuteFromSelection` / assign |
| **改法** | 1. 已有 AI 计划草稿时，主按钮文案：**「拆成步骤」**（内部：未落盘则先 save，再 direct assign）<br>2. 「仅保存」降为 ghost 次按钮<br>3. 禁止在此路径调用 `start_run` |
| **完成定义** | 用户可不理解「计划文件」也能拆开；磁盘仍有 `plans/*.md` |
| **自测** | 聊天生成 → 一点主按钮 → planning（无 chooser） |

#### B3 · 会话切换 / 计划轨默认藏

| 项 | 内容 |
|----|------|
| **ID** | **B3** |
| **状态** | ✅ |
| **估时** | 0.3 d |
| **文件** | [`web/index.html`](../web/index.html) `#chat-session-switch` · `#btn-chat-rail-toggle`<br>chat install / render 默认 `hidden` 或仅「更多」 |
| **改法** | 会话 UI 默认 hidden；计划轨默认不展开；进阶用户设置或长按/更多可开（最小：设置一开关或保持 ☰ 但默认不教学） |
| **完成定义** | 首跑聊天首屏无「会话」「计划快览」概念 |
| **自测** | 目视 |

#### B4 · 计划管理降权

| 项 | 内容 |
|----|------|
| **ID** | **B4** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **依赖** | A4 |
| **文件** | shellChrome / index 顶栏；[`web/js/features/chat/plansMgmt.js`](../web/js/features/chat/plansMgmt.js) 仅保留能力 |
| **改法** | 主路径无「计划管理」实心入口；「更多 → 管理计划文件」；页内文案降工程师味（换夹等保留） |
| **完成定义** | §5 脚本全程可不进计划管理页 |
| **自测** | 脚本 §5 |

#### B5 · 拆分详情默认短读

| 项 | 内容 |
|----|------|
| **ID** | **B5** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **文件** | [`web/js/features/split/splitDetail.js`](../web/js/features/split/splitDetail.js)<br>[`web/js/features/split/splitRender.js`](../web/js/features/split/splitRender.js) |
| **改法** | 详情默认：标题 + 一句话 +「怎样算做完」；完整说明 `<details>`；高级路由 fold 保持默认关 |
| **完成定义** | 不展开不出现长 prompt 墙 / role 表单 |
| **自测** | 点选 3 张卡目视 |

#### B6 · 可选未勾底部条强化

| 项 | 内容 |
|----|------|
| **ID** | **B6** |
| **状态** | ✅ |
| **估时** | 0.3 d |
| **文件** | [`web/js/features/split/splitFillMeta.js`](../web/js/features/split/splitFillMeta.js) |
| **改法** | 有业务可选未勾时，confirm 区固定一条非 dismiss 提示（可沿用 meta 文案，视觉升一级） |
| **完成定义** | 不读分区标题也知道「确认后不会跑」 |
| **自测** | 含 optional 的计划 |

---

### 波次 C — 执行 / 结果 / 冷启动（P1）

#### C1 · 执行台日志工具降级

| 项 | 内容 |
|----|------|
| **ID** | **C1** |
| **状态** | ✅ |
| **估时** | 0.5–1 d |
| **文件** | [`web/index.html`](../web/index.html) `#monitor` toolbar<br>[`web/js/features/run/logPanel.js`](../web/js/features/run/logPanel.js) / `RunView.js` |
| **改法** | 默认折叠内：只保留步骤日志列表；`#log-event-filter` · raw 模式 · 导出 · handoff strip · 字号 收入「日志高级」details 或设置 |
| **完成定义** | 展开日志不出现调试矩阵第一眼 |
| **自测** | running 展开 fold |

#### C2 · 点步骤看人话进展（薄）

| 项 | 内容 |
|----|------|
| **ID** | **C2** |
| **状态** | ✅ |
| **估时** | 0.5–1 d |
| **依赖** | C1 |
| **文件** | [`web/js/features/run/RunView.js`](../web/js/features/run/RunView.js) · `logBoardCard.js` 等 |
| **改法** | 选中步骤时优先 3–5 行人话摘要（已有 status/stall 文案复用）；完整 term 次级 |
| **完成定义** | 不读 raw 日志知「在干什么/卡住」 |
| **自测** | fake running + stall 文案 |

#### C3 · 结果态一页报告

| 项 | 内容 |
|----|------|
| **ID** | **C3** |
| **状态** | ✅ |
| **估时** | 1 d |
| **文件** | [`web/js/features/result/ResultView.js`](../web/js/features/result/ResultView.js)<br>[`web/index.html`](../web/index.html) `#result-desk` / KPI<br>[`web/css/monitor.css`](../web/css/monitor.css) |
| **改法** | `finished`：弱化/折叠 KPI 墙；放大结果台；出口决策树——有遗漏：主「回补」、次「先这样结束」；无遗漏：主「完成并回写计划」、次「再写一份」 |
| **完成定义** | 终态像报告不像监控 |
| **自测** | fake 跑完 + 有/无 inspect_loop 两态 |

#### C4 · 欢迎模板闭环

| 项 | 内容 |
|----|------|
| **ID** | **C4** |
| **状态** | ✅ |
| **估时** | 0.5 d |
| **文件** | templates actions · welcome 按钮 · project add modal 串联 |
| **改法** | 无项目点模板 → 先选夹 → 落盘 → 进聊天或全文；有项目点模板直接落盘 |
| **完成定义** | 欢迎页模板不出现死按钮 |
| **自测** | 空列表点模板 |

#### C5 · 可选：演练全流程入口

| 项 | 内容 |
|----|------|
| **ID** | **C5** |
| **状态** | ⏭ 可砍 · 本轮不做 |
| **估时** | 1 d |
| **改法** | 欢迎或帮助「先看演示」：固定 fake provider + 示例计划 → 自动走到拆分台（仍 confirm 开跑） |
| **完成定义** | 5 分钟建立心智；不依赖本机 Claude |

---

### 波次 D — 收口

| ID | 任务 | 完成定义 | 估时 | 状态 |
|----|------|----------|------|------|
| **D1** | §5 脚本全项真人/录屏 | 工程侧包+标记绿；**本机目视仍建议**（见修订） | 0.5 | ✅ 半门（无交互自动化） |
| **D2** | `package-app` + 安装包目视 | 与脚本一致 | 0.3 | ✅ |
| **D3** | 回写 PRODUCT / docs L2 / web L2 / 本表 ✅ | 地图同构 | 0.2 | ✅ |
| **D4** | 诊断文状态改为「落地见本文件」 | 避免双真源勾选 | 0.1 | ✅ |

---

## 4. 推荐 PR 切片

| PR | 含任务 | 标题建议 | 合并门槛 |
|----|--------|----------|----------|
| **PR1** | A1 + A2 | `fix(web): pending split desk + skip chooser by default` | §5 项 3–5；无 start_run |

**PR1 推荐改序**（调研复核 · 降风险）：

```text
1. state.js          翻 chatAssignDirect 默认 + settings 文案同步
2. projectPicker.js  startExecuteFromSelection 默认 direct
3. sessionEntry.js   resolveEntryRoute + applyEntryRoute(confirm)
4. sessionEntry.js   删/放宽 selectProject 双保险 L515–528 + toast
5. 目视：无 job→chat；planned→拆分台；有 path 拆→无 chooser
```
| **PR2** | A3 + A4 + A5 + A6 | `fix(web): split/topbar density for non-dev main path` | 顶栏/拆分台目视；打包 |
| **PR3** | B1–B4 | `feat(web): chat empty guide + assign-without-file-anxiety` | 聊天主路径 |
| **PR4** | B5–B6 | `feat(web): split detail short-read + optional banner` | 拆分可读 |
| **PR5** | C1–C4 | `feat(web): run/result report mode + welcome template loop` | 跑收 |
| **PR6** | D* + 可选 C5 | `docs: non-dev UX landing closeout` | §5 全过 |

**禁止**：单 PR 同时大改 chat + run + 路由（难回滚）；禁止「先堆再还债」超 600 行文件。

---

## 5. 非开发验收脚本（门禁 · 每波 A 后必跑子集）

环境：最新 `package-app`；可用演练通道；**不看帮助**。

| # | 步骤 | 通过 | 波次门槛 |
|---|------|------|----------|
| 1 | 打开 App | 10s 内知要加工作文件夹 | A |
| 2 | 添加文件夹 | 进入写计划，无术语恐慌 | A |
| 3 | 一句话或模板 | **不出现**通道/并发表单 | A |
| 4 | 拆成步骤 | **自动停拆分台**；能指认并行 | A |
| 5 | 杀进程重开同项目 | 仍见拆分台（待确认） | A |
| 6 | 不勾可选，确认开始 | 知可选不跑 | B |
| 7 | 看执行 | 不打开高级日志知进度；卡住人话 | C |
| 8 | 结束 | 一页做完/遗漏；只点一个主下一步 | C |
| 9 | 全程 | **不进**计划管理 / 设置 / 独立监视窗 | A–C |

失败 = 该波未完成。

---

## 6. 成功标准（程序级）

| ID | 标准 |
|----|------|
| **L-A** | 待确认拆分不可被默认路由藏进聊天 |
| **L-B** | 新用户主路径零执行选项弹层 |
| **L-C** | 拆分台首屏主路径按钮 ≤ 2 |
| **L-D** | 写计划首屏概念 ≤ 3（项目 · 目标 · 拆开） |
| **L-E** | §5 脚本全过 |
| **L-F** | `confirm_start` 唯一业务开跑；optional 门禁不回退 |
| **L-G** | 帮助无「模式 B」标题 |

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| A1 改 H0 后老用户抱怨「总进拆分台」 | 仅 `planned/confirmed` 进；`done`/无 job 仍 chat；拆分台提供「回写计划」 |
| A2 默认直拆丢掉并发设置 | 设置「最多同时几步」仍生效；analyze 读 state 默认并发 |
| 藏 sanitize 后串行拆分难自救 | 「调整…」菜单保留；质量条人话可提示「可试试让步骤更并行」链到菜单 |
| 双 phase 不同步 | A1 改后用 `body.dataset` + 目视三态；不新造第三状态机 |
| 与 P2-16「全 ✅」叙事冲突 | 对外口径：骨架已落地；**体验验收以本文为准** |

---

## 8. 文件触达总表（按波次）

| 波次 | 主要路径 |
|------|----------|
| **A** | `sessionEntry.js` · `state.js` · `projectPicker.js` · `shellChrome.js` · `index.html` · `settingsForm.js` · `plan.css`（薄） |
| **B** | `chatRender.js` · `chatPlanOps.js` / `chatActions.js` · `templates/catalog.js` · `splitDetail.js` · `splitFillMeta.js` · `index.html` |
| **C** | `RunView.js` · `logPanel.js` · `ResultView.js` · `monitor.css` · templates/welcome · `index.html` |
| **D** | `docs/*` · `PRODUCT.md` · `web/CLAUDE.md` · `package-app` |

---

## 9. 与现有文档关系

| 文档 | 关系 |
|------|------|
| [`ux-nondev-mainpath-2026-07-21.md`](./ux-nondev-mainpath-2026-07-21.md) | **诊断 + 原则**；勾选迁移到**本文** |
| [`product-mainpath-optimize-2026-07-20.md`](./product-mainpath-optimize-2026-07-20.md) | 意图参考；其 ✅ = 骨架，不代替 §5 |
| [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md) | 边界已收口；本计划只消费 app/gateway |
| Mode B / optional 记忆 | 保留；A2 不削弱 optional 停台 |

---

## 10. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 首版落地计划：波次 A–D · 任务 A1–D4 · PR 切片 · §5 门禁 · 文件触达表 |
| 2026-07-21 | **planner 热修**：对本文件 `#### A1`… 拆分时，heuristic 原先认不出任务 ID，回落「读懂目标→拆包→落地→巡检」空壳四波；`extract_work_phases` 已优先识别 `#### A1/B2/U1-1` 与 `### 波次`（`src/plan/planner/heuristic.rs`）。**这与 UI 波次 A 无关**；重拆本计划应出 A1/A2… 真任务。 |
| 2026-07-21 | **原则落地**：识别失败不得静默换成空壳。路径改为 diagnose（规划日志写原因）→ recover（从文档真标题/可实施正文恢复任务）→ 仅无结构时 last-resort meta，且 meta 第一步 prompt 写明失败原因。 |
| 2026-07-21 | **A1 + 本轮状态收口**：`resolveEntryRoute` planned/confirmed → workspace 拆分台；`selectProject` 双保险放宽；历史 `project_live` completed 不抢 phase/KPI/结果台（`liveBelongsToOpenPlan`）；新开 plan job `supersede` 同项目 planning；`latest_plan_job` 跳过超时 planning 僵尸（>30min），其余仍按 `updated_at`。 |
| 2026-07-21 | **波次 A 收口 A2–A6**：`chatAssignDirect` 默认开（仅 `"0"` 关）；设置「拆成步骤前先确认选项」语义翻转；拆分台顶栏仅重新拆分+确认，sanitize/写回进「调整…」；chat 待确认顶栏「继续核对拆分」；计划管理仅更多且永不 primary；帮助「三步上手」去 Mode B/R-S 首屏；`node --check` + `package-app`。 |
| 2026-07-21 | **波次 B 收口 B1–B6**：聊天空态教练句+示例；计划卡主 CTA「拆成步骤」（未保存先 skipConfirm 落盘）；会话切换默认「会话…」；计划管理文案降权；详情默认短读+完整说明 fold；可选未勾 banner。 |
| 2026-07-21 | **波次 C+D 收口**：C1 日志高级折叠；C2 步骤卡人话进展；C3 结果态决策树 CTA；C4 欢迎模板无项目→选夹后套用；C5 砍；D 勾选/L2/PRODUCT/package。 |
