# cco 壳层减法 · 拆分台与项目入口 · 落地计划

> 日期：2026-07-22  
> 角色：**体验落地实施真源**（派工 / 勾选 / PR 边界）  
> 触发：用户对照拆分台截图提出的壳层减法 + 非开发可读性  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)  
> 架构边界（**不重开 A0–A5**）：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)  
> 体验前序（**不继承勾选**）：[`ux-nondev-landing-2026-07-21.md`](./ux-nondev-landing-2026-07-21.md)  
> 范围：`web/` 为主（index · css · features/{project,split,settings} · shared）；gateway 已有 `removeProject`；**不**改 Scheduler / confirm 语义 / Mode B  
> 状态：**A+B+C+D 全量 ✅ · 2026-07-22**（A5 纠偏：完整说明**不**强制默认 open；D1 目视用户验收通过）

[PROTOCOL]: **勾选只认本文件 §3 任务表**。历史 UX 波次 A–D 已收口，本文件是新一轮壳层减法，**不**回灌旧表。落地后回写 `docs/CLAUDE.md` + `web/CLAUDE.md`。每波结束须跑 §5 非开发脚本（目视），不得以 `cargo test` 代替。

---

## 0. 目标 / 非目标

### 0.1 一句话目标

让非开发用户在拆分台**一眼只看到「重新规划 / 执行规划」**，侧栏能**安全移除项目**，点计划能**回看拆分结果与历史**，步骤说明**默认完整可读**，顶栏只留**图标化的聊天 / 计划管理 / 刷新**，去掉顶栏阶段条与多余文案噪音。

### 0.2 用户原话 → 落地映射

| 用户说 | 落地 |
|--------|------|
| 增加项目移除功能，从软件中移除项目 | 侧栏项目行悬停 × · 二次确认 · 调已有 `remove_project_cmd`（**不删磁盘文件夹**） |
| 拆分的计划，点击计划信息，增加查看拆分结果 | 强化「计划信息 / 拆分 chip / 计划列表」→ 回拆分台只读或可编辑；文案「查看拆分结果」 |
| 拆分后的计划依旧可以重新拆分 | 保留 replan 路径；按钮改名 **重新规划** |
| 之前执行过的拆分计划还可以看到 | 计划列表 / rail 默认可露「已有拆分」；点入仍可看上次 steps；不静默丢 session |
| 拆出来的独立任务可以继续优化 · 给不懂的用户详细讲解 | **现有编辑** + **完整说明默认展开** + 白话 meta；**不**新增「AI 讲解/重写」 |
| 所有展开的功能按钮，点其他地方自动收缩 | `details` / 下拉菜单统一 click-outside 关闭 |
| 图 1 右侧「重新拆分」等按钮都清理掉，改为：重新规划 · 执行规划 | 拆分台主 CTA 只留两键；删/藏「调整…」第一屏 |
| 右上角「编辑任务」去掉不合理 | 删除顶栏 `btn-edit-plan`；编辑入口留在步骤详情内 |
| 其他 3 个按钮改为 icon + hover 说明 | 聊天 / 管理计划文件 / 刷新 → icon + `title`/`aria-label` |
| 刷新好像没啥用 | **保留**（用户确认三键全 icon）；title 写清「刷新项目与运行状态」 |
| 头部「写计划→拆分→执行→结果」去掉 | 去掉顶栏 `#flow-strip-global`（页面内局部条若有可保留或一并收） |
| 右下角底部文字清理掉 | 清残留 ghost CTA / 无意义底角文案（见 §1.4） |
| 完整说明可读 | 白话 meta + 可展开完整说明（**纠偏**：不强制默认 open，对齐双受众 S0 短读） |

### 0.3 非目标

| 不做 | 原因 |
|------|------|
| AI「讲解/细化」步骤（调模型重写） | 用户确认本轮不做；成本与失败态未设计 |
| 换 React / 新组件库 | 架构已收口 |
| 旁路 `confirm_start` / 改 Mode B / optional 规则 | L1 硬契约 |
| 删除 replan / 多 provider / 写回摘要 **能力** | 只减第一屏；高级仍可进设置或折叠 |
| 真删磁盘项目文件夹 | 只从 cco 项目列表移除 |
| 重开 A0–A5 / workspace crates | 边界已 ✅ |
| 平行第二套架构阶段表 | L1 |

### 0.4 硬契约（落地时不可破）

1. **唯一业务开跑**：`confirm_start` / `gateway.confirmStart` → `split::confirm`。按钮可改名「执行规划」，**不得**改走 `start_run`。  
2. **optional**：有业务可选时仍须可勾选；禁止 auto-start 跳过未勾选。  
3. **MVVM**：策略不进 View；文案/默认展开可在 web；业务门禁仍在 Rust。  
4. **IPC**：只经 `gateway`；`removeProject` 已存在，禁止 feature 内直接 invoke。  
5. **文件体量**：软 400 / 硬 600；禁止往 classic facade / `state.js` 堆业务。  
6. **图标**：Lucide 线标 via `shared/icons.js`；**禁止 emoji 作按钮图标**（记忆 open-source-icons）。

### 0.5 已确认的产品选择（AskUser 2026-07-22）

| 议题 | 选择 |
|------|------|
| 顶栏 3 按钮 | 聊天 + 管理计划 + 刷新 **全部 icon + 悬停说明**（保留刷新） |
| 拆分台主 CTA | **只留两个**：重新规划 · 执行规划（「调整…」退出第一屏） |
| 项目移除入口 | 侧栏项目行悬停 × + 二次确认 |
| 任务「优化/讲解」 | 现有编辑 + 白话 meta + 完整说明可展开（不强制 open）· 无新 AI |

---

## 1. 体感拐点（必须先做到）

| # | 用户会说的话 | 代码落点（初判） |
|---|--------------|------------------|
| **T1** | 「顶栏没有写计划→拆分那一串了」 | `index.html` `#flow-strip-global` · `shellChrome.refreshFlowStrips` · `flow.js` |
| **T2** | 「拆分台右边只有重新规划 / 执行规划」 | `index.html` `.split-actions` · `splitDetail.paintChrome` · `confirmActions` · `flow.js` 文案 |
| **T3** | 「项目上有 ×，确认后从列表消失」 | `shellUi.renderProjectList` · `projectCrud.removeSelectedProject` · 确认 dialog |
| **T4** | 「点计划/chip 能看上次拆分」 | `split-plan-chip` · `showSplitPlanConfirm` · 计划列表/rail 入口与 badge |
| **T5** | 「完整说明打开就有，不用再点」 | `splitDetail.js` details 默认 open |
| **T6** | 「顶栏三个小图标，鼠标放上才知道」 | `index.html` top-actions · icons · `projectPicker` 显隐 |
| **T7** | 「点外面，下拉/展开自己收」 | 统一 click-outside（split-more 若还在高级、selectUi、其它 details 菜单） |

**最小可感知发布（MVP Ship）= 波次 A = T1+T2+T5+T6**，约 1 人日。  
**完整本轮 = A+B+C**，约 2–3 人日。

---

## 1.4 截图噪音清单（图 1 对照）

当前拆分台第一屏噪音（要减）：

1. 顶栏阶段条：`写计划 → 拆分 → 执行 → 结果`  
2. 顶栏文字按钮：`聊天` · `管理计划文件` · `编辑任务` · `刷新`  
3. 拆分台右上：`重新拆分（保留你的修改）` · `仍要开始（验收未写清）` · `调整…` 菜单（含「让可并行的真正并行」「写回步骤摘要」）  
4. 步骤详情：完整说明默认折叠；meta「需要时展开完整说明」偏引擎腔  
5. 侧栏：无移除；底栏 `桌面应用 · v0.x` 可保留（用户说的「右下角底部文字」优先清主区残留 ghost，如淡出的「拆成步骤」等）

保留（能力不删）：

- 确认开跑、optional 勾选、验收 stub 黄条、replan 保人工修改  
- 高级 · 执行通道与路由（折叠）  
- 步骤内编辑（编辑/删除）  
- 计划管理页 / 聊天

---

## 2. 波次总览

```text
波次 A  拆分台减法 + 顶栏图标 + 完整说明默认展开   ~1 人日   ← 先 ship 体感
波次 B  项目移除 + click-outside + 底角清理         ~0.5–1 人日
波次 C  计划→拆分结果可达 + 历史可见 + 白话文案     ~1 人日
波次 D  验收 · 文档回写 · 打包目视                    ~0.5 人日
```

依赖：A 可独立；B 与 C 可并行（侧栏 vs 计划入口）；D 依赖 A 必过。

---

## 3. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> 每任务：文件 · 步骤 · 完成定义 · 自测 · 依赖

### 波次 A — 拆分台减法 + 顶栏图标（P0 · MVP Ship）

#### A1 · 去掉顶栏全局阶段条 ✅

- **文件**：[`web/index.html`](../web/index.html)（`#flow-strip-global`）· [`web/js/features/project/shellChrome.js`](../web/js/features/project/shellChrome.js) · 可选 [`web/js/flow.js`](../web/js/flow.js)  
- **步骤**：  
  1. 顶栏不再渲染 `flow-strip-global`（hidden 或移除节点；JS 写 strip 时 no-op）。  
  2. **页面内**局部阶段条（若 monitor/confirm 页内仍有）默认也去掉，避免双轨；若某页强依赖再单独开例外。  
  3. CSS 清理仅服务顶栏条的多余间距。  
- **完成定义**：任意业务页顶栏标题下**看不到**「写计划→拆分→执行→结果」。  
- **自测**：欢迎 / 聊天 / 拆分台 / 运行 / 结果各看一眼。  
- **依赖**：无。

#### A2 · 拆分台主 CTA 只留「重新规划 / 执行规划」 ✅

- **文件**：[`web/index.html`](../web/index.html) `.split-actions` · [`web/js/features/split/splitDetail.js`](../web/js/features/split/splitDetail.js) `paintChrome` · [`web/js/features/project/confirmActions.js`](../web/js/features/project/confirmActions.js) · [`web/js/flow.js`](../web/js/flow.js) 文案 helper  
- **步骤**：  
  1. `btn-replan` 文案固定 **重新规划**（运行中可 toast「请先停止」；不改 handler 语义）。  
  2. `btn-confirm-start` 文案固定 **执行规划**；验收 stub 时**不**改成「仍要开始…」——可 `title`/黄条提示，按钮字仍「执行规划」。  
  3. 第一屏移除 `#split-more-actions`（「调整…」）；`btn-sanitize-deps` / `btn-split-writeback` **能力保留**：进设置「高级」或帮助说明，**或**进步骤详情高级折叠（本波优先：**DOM 仍在但 `hidden`，设置页链过去**；避免死代码）。  
  4. 禁止在 JS 再写「重新拆分（保留你的修改）」「仍要开始（验收未写清）」作主按钮 label。  
- **完成定义**：拆分台右上只见两键：重新规划 · 执行规划；点执行仍走 `confirmStart`。  
- **自测**：有/无 optional · 验收 stub · 运行中 replan toast。  
- **依赖**：无。

#### A3 · 去掉顶栏「编辑任务」 ✅

- **文件**：[`web/index.html`](../web/index.html) `#btn-edit-plan` · [`web/js/features/project/projectPicker.js`](../web/js/features/project/projectPicker.js) · [`web/js/features/settings/uiActions.js`](../web/js/features/settings/uiActions.js)  
- **步骤**：删除或永久 hidden 顶栏编辑按钮；去掉显隐逻辑；编辑仍只在拆分台步骤详情「编辑」。  
- **完成定义**：顶栏无「编辑任务」。  
- **自测**：confirm 相位顶栏无该键；点步骤仍可编辑。  
- **依赖**：无。

#### A4 · 顶栏三键 → icon + tooltip ✅

- **文件**：[`web/index.html`](../web/index.html) `#btn-open-chat` / `#btn-plan-mgmt` / `#btn-refresh` · [`web/js/shared/icons.js`](../web/js/shared/icons.js) · [`web/js/features/project/projectPicker.js`](../web/js/features/project/projectPicker.js) · CSS  
- **步骤**：  
  1. 三键改为 `icon-btn` + `data-icon`（建议：`message-square` / `list` 或 `file` / `refresh`）。  
  2. `title` + `aria-label`：聊天 · 管理计划文件 · 刷新项目与运行状态。  
  3. 显隐逻辑保留（系统页 / 欢迎页规则不变）。  
  4. 视觉对齐侧栏 `+` 的 icon-btn 尺寸。  
- **完成定义**：顶栏业务区只见图标；悬停有中文说明；点击行为不变。  
- **自测**：hover · 键盘 focus 有 label · 窄宽顶栏不挤。  
- **依赖**：A3（少一个文字按钮后再排图标）。

#### A5 · 白话 meta + 完整说明可展开 ✅（**产品纠偏**）

- **文件**：[`web/js/features/split/splitDetail.js`](../web/js/features/split/splitDetail.js) · 可选 [`web/js/flow.js`](../web/js/flow.js)  
- **步骤（纠偏 · 对齐双受众 S0）**：  
  1. `details.split-detail-full` **不**强制默认 `open`（短读优先）；同任务 re-paint 仍尊重用户 `wasOpen`。  
  2. meta 白话：「左侧选步骤 · 下方可展开完整说明 · 可编辑」。  
  3. 空态友好引导看正文 / 展开说明。  
- **完成定义**：非开发读得懂 meta；完整说明按需展开，不与默认短读对着干。  
- **自测**：切换步骤默认短读；手动展开后同任务 poll 保持。  
- **依赖**：无。

---

### 波次 B — 项目移除 + 展开自动收 + 底角清理

#### B1 · 侧栏项目行「移除」 ✅

- **文件**：[`web/js/shared/shellUi.js`](../web/js/shared/shellUi.js) `renderProjectList` · [`web/js/features/project/projectCrud.js`](../web/js/features/project/projectCrud.js) · CSS layout · 可选 confirm modal 复用  
- **步骤**：  
  1. 每行悬停显示 ×（`icon-btn` + `trash` 或 `x`）；`stopPropagation`，避免点 × 同时选中。  
  2. 二次确认文案：**「从 cco 列表移除「{name}」？不会删除电脑上的文件夹。」**  
  3. 确认后 `gateway.removeProject(path)`（已有）；清 `planSessions`、停 poll、若是当前项目则 `goHome`。  
  4. 运行中：toast 锁（沿用 `toastRunLocked`），不可移除。  
  5. 可选：保留/对齐设置里旧 `btn-remove-project` 若仍存在则同路径。  
- **完成定义**：非运行项目可移除；磁盘目录仍在；列表与状态一致。  
- **自测**：移除当前 / 非当前 / 运行中 / 取消确认。  
- **依赖**：无。

#### B2 · 展开控件 click-outside 收起 ✅

- **文件**：优先拆分台相关 `details`（若 A2 后第一屏已无 more-menu，则覆盖：**高级·执行通道**、步骤详情内其它下拉、顶栏若有菜单、计划 rail 等）· 可抽 [`web/js/shared/`](../web/js/shared/) 小 helper  
- **步骤**：  
  1. 统一：打开状态的 `details.split-*` / 菜单，在 `document` capture 阶段点击外部 → `open=false`。  
  2. 不误伤：点击控件内部、select 下拉、modal。  
  3. 与现有 `selectUi` 不冲突。  
- **完成定义**：展开后点主内容空白处，展开层收起。  
- **自测**：高级折叠 ·（若有）菜单 · select。  
- **依赖**：A2（菜单是否还在第一屏）。

#### B3 · 右下角/残留底角文字清理 ✅

- **文件**：拆分台/workspace 内残留 ghost（如淡「拆成步骤」占位、无绑定底角文案）· CSS  
- **步骤**：对照截图与运行态，删除或 `hidden` 无意义残留；**侧栏** `桌面应用 · v*` 默认**保留**（非主路径噪音；若用户后续要藏再单开）。  
- **完成定义**：拆分台主区右下无游离按钮/残字。  
- **自测**：confirm 相位全屏截图。  
- **依赖**：A2。

---

### 波次 C — 计划信息 → 拆分结果 + 历史可见

#### C1 · 点击计划信息 / chip → 查看拆分结果 ✅

- **文件**：[`web/js/features/project/shellChrome.js`](../web/js/features/project/shellChrome.js) `updateSplitPlanChip` · [`web/js/features/project/planSelect.js`](../web/js/features/project/planSelect.js) `showSplitPlanConfirm` · 计划管理/rail 条目  
- **步骤**：  
  1. chip 文案/title 统一「查看拆分结果」；点击进拆分台（已有路径）。  
  2. 计划列表/rail：对已有 `planJob` 或 executed 元数据的计划，提供显式入口「查看拆分结果」（不只「拆成步骤」）。  
  3. 已执行计划：进入拆分台 **可看**；重新规划仍允许（现有 replan 规则）；步骤编辑规则不变（已执行步只读）。  
- **完成定义**：非开发不用猜「拆分」chip 含义；从计划信息能回到 steps。  
- **自测**：跑完一轮后从 chat/plans/workspace 回看。  
- **依赖**：A2 文案一致。

#### C2 · 历史拆分仍可见 ✅（沿用 session/job；审计 clear 路径）

- **文件**：[`web/js/features/project/sessionEntry.js`](../web/js/features/project/sessionEntry.js) · [`web/js/features/project/planMeta.js`](../web/js/features/project/planMeta.js) · plan rail / plansMgmt  
- **步骤**：  
  1. 沿用现有 SQLite/session；重开项目经 `restorePlanSession` / job 查询回显。  
  2. 「显示已执行」hint 已在 rail/chooser/plansMgmt；默认不全开历史。  
  3. `clearPlanSession` **仅**允许：取消规划 · 新开拆分 supersede · 移除项目（注释锁死；非切页/管理页旁路）。  
- **完成定义**：执行过的拆分计划，用户能再次打开看到步骤列表。  
- **自测**：拆分→执行→结果→回计划→再进拆分台。  
- **依赖**：C1。

#### C3 · 步骤可读性小抄（无 AI） ✅

- **文件**：`splitDetail` / `splitRender` 文案 · 帮助页一句  
- **步骤**：  
  1. 详情头「必做 · 无依赖…」保留结构化，旁注可更白话。  
  2. 帮助/拆分台空态加一句：「点左侧步骤看完整说明；可编辑后再执行规划。」  
- **完成定义**：不增加新概念；帮助与台面一致。  
- **依赖**：A5。

---

### 波次 D — 验收 · 文档 · 打包

#### D1 · 非开发目视脚本 ✅

按 §5 清单本机点一遍；**2026-07-22 用户验收通过**。

#### D2 · 文档回写 ✅

- [`docs/CLAUDE.md`](./CLAUDE.md)：本文件状态已更新。  
- [`web/CLAUDE.md`](../web/CLAUDE.md)：顶栏/拆分 CTA 变化一句。  
- 本文件任务表勾选 ✅。

#### D3 · 回归硬契约 ✅（warn 级）

- `scripts/check-arch.sh`：FAIL=0（既有 soft/hard WARN 与本波无关）。  
- 无 UI `start_run`；执行规划仍 `confirmStart`。  
- `rg` 主路径无「仍要开始」「重新拆分（保留」主按钮文案。

---

## 4. 文案表（主路径统一）

| 位置 | 旧 | 新 |
|------|----|----|
| 拆分台次 CTA | 重新拆分（保留你的修改） | **重新规划** |
| 拆分台主 CTA | 确认并开始 / 仍要开始（验收未写清） | **执行规划**（stub 用 title/黄条） |
| 顶栏 | 聊天 / 管理计划文件 / 刷新 文字 | icon · title 同上义 |
| 顶栏 | 编辑任务 | **删除** |
| 顶栏 | 写计划→拆分→执行→结果 | **删除** |
| chip | （隐晦） | 查看拆分结果 |
| 完整说明 | 默认折叠 / 原计划默认 open | **可展开 · 不强制默认 open**（短读优先） |
| 移除确认 | — | 从 cco 列表移除「…」？不会删除电脑上的文件夹。 |

---

## 5. 非开发验收脚本（每波结束）

1. 添加项目 → 侧栏见项目 → 悬停见 × → 取消确认仍在 → 再确认移除 → 列表无、文件夹还在。  
2. 进入拆分台：顶栏**无**阶段条、**无**编辑任务；三 icon 悬停有字。  
3. 拆分台右上**仅**「重新规划」「执行规划」。  
4. 点步骤：默认短读（要做什么 / 怎样算做完）；完整说明**按需展开**（不强制 open）；可编辑保存。  
5. 点「执行规划」仍开跑（optional 未勾选仍停住）。  
6. 跑完后点计划信息/chip → 能看拆分结果；可再「重新规划」。  
7. 展开高级/菜单后点空白 → 收起。  
8. 主区右下无残字。

---

## 6. 风险与回滚

| 风险 | 缓解 |
|------|------|
| 高级「写回摘要 / 可并行」难发现 | A2 放设置高级或帮助；不删 handler |
| 默认展开完整说明过长 | 详情栏可滚；同任务记住用户折叠 |
| 误点移除项目 | 二次确认 + 明确「不删文件夹」 |
| 去掉阶段条后新手迷路 | 欢迎页步骤 + 拆分台内 hint 保留；不做顶栏双轨 |
| icon 无障碍 | 必须 `aria-label` + `title` |

回滚：各波次独立 commit；文案/ hidden 优先，易回退。

---

## 7. 建议提交切片（落地时）

```text
1) feat(web): split desk CTA 重新规划/执行规划 + 完整说明默认展开
2) feat(web): topbar icon 化 · 去阶段条 · 去编辑任务
3) feat(web): sidebar remove project + click-outside
4) feat(web): plan chip/list 查看拆分结果 · 历史可见
5) docs: shell-chrome-simplify 勾选 + L2 回写
```

---

## 8. 与用户确认纪要（已答 · 2026-07-22）

- 顶栏：三键全 icon + 悬停说明，**保留刷新**。  
- 拆分台：只留 **重新规划 · 执行规划**。  
- 移除项目：侧栏行悬停 × + 确认。  
- 任务优化：编辑 + 完整说明**可展开不强制 open** + 白话，**无新 AI**（对齐双受众 S0）。

---

## 9. 状态

| 波次 | 状态 |
|------|------|
| A 拆分台+顶栏+白话 meta | ✅（A5 纠偏：不强制默认展开完整说明） |
| B 移除+outside+底角 | ✅ |
| C 计划→拆分结果+历史 | ✅（C1 chip+计划管理+rail · C2 clear 审计 · C3） |
| D 验收文档 | ✅（D1 目视通过 · D2 文档 · D3 门禁） |

**已收口 2026-07-22**（窗口 B shell-chrome · 用户 §5 目视验收通过）。  
**窗口 C 汇合（2026-07-22）：** 与 multi-window / 双受众对齐 — CTA 固定「重新规划/执行规划」· 完整说明短读优先 · critic 次级按钮文案统一「重新规划」· `confirmStart` 未旁路 · `plan_mode` 默认 ai。
