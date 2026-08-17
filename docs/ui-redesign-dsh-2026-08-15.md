# 桌面端 UI 重构计划 — 借鉴 DeepSeek Harness Web UI

> 阶段：**实施中**（P4-0 ✅ · P4-1 ✅ · P4-2 ✅ · P4-3 ✅ · **P4-4 ✅** · **P4-5 ✅** · **P4-6 ✅** · **P4-7 ✅** · **P4-8 实现完成，视觉终验待授权通道恢复**）｜真源候选：本文件 + 各页面 L2
> 参考对象：[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)（`dsh`，MIT）Web UI
> 已决策：品牌主色 = **dsh DeepSeek 蓝 #4176E6**｜范围 = **全量页面一次规划**｜暗色 = **纳入本次**
> 已定（2026-08-15 确认）：① 暗色默认 = **跟随系统**；② 次级列（执行/结果台局部）宽约 **320px · 默认折叠可开**；③ 状态色 = **运行沿用蓝**（与成功绿区分）；④ **不做全局第三栏 details**（拆分台已三栏，四栏拥挤）
> 原则：**借视觉语言与交互范式，不借架构与技术栈**；主路径仍 Plan-First（Split 确认唯一开跑）

---

## 0. 结论先行

dsh 的 webui 是一套「**三栏壳 + 会话流 + 细节栏**」的现代 Agent 界面：设计 token 分三层（static 色阶 → 语义别名 → 组件），组件用 CSS Modules + `clsx`（本项目等价物 = 手写 `css/*` + 类名约定），核心观感 = **中性偏蓝的表面体系 + 单一品牌蓝 + 卡片化工具呈现 + 状态点语言 + 停靠式 composer dock**。

Leaf 与 dsh 的产品形态不同（**Plan-First 五步闭环 vs Chat-First 自由会话**），所以不是照搬 dsh 的「会话即主界面」，而是：

1. **把 dsh 的三栏壳（sidebar｜conversation｜details）映射为 Leaf 的（项目侧栏｜phase 主区｜phase 局部次级面板）**——不做全局第三栏（拆分台已三栏，见 §4 修正）；
2. **把 dsh 的 view-ring 标签页（Chat｜Trajectory）映射为 Leaf 的 phase 段（拆分｜执行｜结果｜聊天）**；
3. **把 dsh 的卡片/状态点/停靠 dock/披露行等语言全面引入**；
4. **执行台/结果台引入「phase 局部次级面板」**（详情 / 日志 / 巡检对照）——本轮主要结构增量，仅限真正需要详情/对照的 phase。

> 工程硬规则（L1 §1–26）全部保留：唯一开跑 = `confirm_start`；MVVM；IPC 只经 `gateway`；facade 体积守门；PM/出海人话文案；新概念 ≤3。本计划只动 **Presentation 层**，Domain/Application/Ports 不改。

---

## 1. dsh Web UI 设计语言提炼（借鉴什么）

### 1.1 结构：三栏 AppFrame
- `ui-layout` 的 `AppFrame`：**sidebar（默认 ~260px，可折叠为 56px rail）｜conversation（1fr）｜details（默认 ~320px，可收起为 0）**。
- 行为契约：details 优先让步（concession chain），窄窗自动收起；collapsed 侧栏保留 56px 控制 rail（含设置入口）；几何状态**不入 localStorage**（瞬态）。
- 会话头 = 标题 + view-ring 标签页（Chat｜Trajectory）+ 右缘工具。

### 1.2 色彩：三层 token（`--dsw-static-*` → `--dsw-alias-*` → 组件）
- static：`neutral-bluish` 全灰阶 + `deepseek` 品牌蓝阶 + 状态 amber/green/red 阶（`design-platform.css`）。
- alias：`bg-base/layer-1/2/3`、`border-l1…l4`、`label-primary/secondary/tertiary/caption`、`button-*`、`interactive-bg-*`、`markdown-*`、`scrollbar-*` 等语义层。
- 主题：`html{color-scheme}` + `body[data-ds-dark-theme]` + body 内联 alias token；浅色/暗色各一整套，`system` 跟随 `prefers-color-scheme`。

### 1.3 字体与密度
- 字体栈：`-apple-system … 'PingFang SC' 'Microsoft YaHei'`；代码栈 `'SF Mono','JetBrains Mono','Fira Code',Consolas`。
- 密度：紧凑（dsh 卡片、气泡、状态点都很小），行高 ~1.45，圆角 ~8–12px，层级用 1px 低对比边框 + 微妙底纹而非重阴影。

### 1.4 组件原语（`ui-primitives`）
- **StateDot**：运行=蓝追光、成功=绿、失败=红、等待=琥珀，单点无动效渲染（aria-hidden + 隐藏文字标签）。
- **DisclosureRow**：默认折叠的披露行（上下文注入 / 历史 / 系统消息），展开有高度上限 + 内部滚动。
- **卡片族**：`TerminalBlock`（命令+输出+状态 pill+复制，pre 保持对齐）、`DiffBlock`（逐文件 `-`/`+`、`└ +A -R · N file(s)` 页脚）、`ReadBlock`（带行号 gutter + `showing N of M`）、`SearchBlock`、`WebBlock`——**头尾截断 + 「显示 X / 共 N」诚实计数**。
- **HoverCard**（可复制）、**Toast** 顶部横幅（120px 处、随 anchor 居中、3s 保持）、**Menu/Modal/Input/Pill**、**MarkdownText**（增量流式渲染、KaTeX、安全链接）。
- 滚动条主题化 + 预留 gutter（`--dsh-scrollbar-width` 镜像）。

### 1.5 交互语言
- **PendingInteraction**：侧栏行琥珀点优先于运行点（等待审批 / 等待计划评审 / 等待回答）。
- **Composer dock 链**：TodoDock(计划条) → GoalBar(目标) → QueueDock(排队)；model seat、plan chip、permission chip、发送/停止；ApprovalPanel 接管 composer。
- 折叠动效：collapse 150ms fade + 300ms slide；`prefers-reduced-motion` 关闭；键盘 focus-visible 保留。

---

## 2. 不借什么（红线）

| # | 不借 | 原因 |
|---|------|------|
| 1 | React / Cordis / 插件系统 / 组件库 | Leaf = 原生 HTML/CSS/JS + esbuild bundle；禁止引框架 |
| 2 | Chat-First 主路径 | Leaf 产品 = 生成→核对→拆分→并行→巡检；聊天只负责「生成/核对」 |
| 3 | Trajectory 时间轴/回放深度 | 只需要执行台「日志次级」级别的对照，不建第二套巡检台 |
| 4 | Settings 配置树（plugin inventory） | Leaf 设置保持平铺分区；高级能力默认折叠 |
| 5 | dsh 的引擎/协议文案入主路径 | `VERDICT`/run_id/provider 名不进第一句（L1 §23） |
| 6 | 把状态机/策略复制进 JS | 状态/路由/波次策略仍由 Rust 下发 DTO（L1 §22） |

---

## 3. 设计系统层（本轮落点：`web/css/tokens.css` 重写 + `web/css/base.css`）

### 3.1 命名：`--leaf-*` 三层
- **static 色阶**（取自 dsh bluish-neutral + deepseek blue，accent 收敛为单一 `--leaf-static-deepseek-*`）：

| 层 | 值（浅色） | 用途 |
|----|-----------|------|
| `--leaf-static-bg-00/50/75/100/150/200/300` | `#FFFFFF/#F9FAFB/#F1F3F5/#EBEEF2/#E9ECF2/#E1E5EE/#CFD3D6` | 表面/面板/卡片 |
| `--leaf-static-bg-400…1000` | `#ADB2B8/#979DA6/#81858C/#61666B/#43454A/#353638/#2C2C2E/#1B1B1C/#0F1115` | 文字/描边/深面 |
| `--leaf-static-deepseek-50/100/300/450/500/600` | `#EDF3FE/#E4EDFD/#B7C8FE/#5686FE/#4176E6/#4868B2` | **品牌蓝（accent=500）** |
| `--leaf-static-ok-500/warn-500/danger-500` | `#22C55E/#F59E0B/#EF4444` | 状态三色 |

- **语义别名**（body 上）：`--leaf-alias-bg-base/layer-1/2/3`、`--leaf-alias-border-l1…l4`、`--leaf-alias-label-primary/secondary/tertiary/caption`、`--leaf-alias-brand-primary`(=accent)、`--leaf-alias-button-*`、`--leaf-alias-interactive-bg-hover/active`、`--leaf-alias-status-*`、`--leaf-alias-scrollbar-*`、`--leaf-alias-markdown-*`。
- **暗色**：`body[data-leaf-theme="dark"]` 内整体覆写 static 阶（bluish 取 dsh 暗色表：`--leaf-static-bg-1000:#F5F5F7` 反转为文字、`--leaf-static-bg-875:#232324` 为表底面等），语义别名全部由 static 重映射，**组件 CSS 禁止写死颜色**。

### 3.2 字体
- 沿用现有 `--font/--mono` 栈（与 dsh 基本一致），新增字号/行高角色：`--leaf-font-caption/secondary/body/title/display` 五级。

### 3.3 组件 CSS（本轮新增 `web/css/components.css`）
- `StateDot`（`.dot.ok/.run/.warn/.danger` + 无动效 + aria-hidden）、`DisclosureRow`、`Pill`（provider/role/scope chip）、`Card`（任务卡基类）、`HoverCard`、`Toast`、`Menu/Modal/Input` 微调、`TerminalBlock/DiffBlock/ReadBlock` 三张卡片原语、`SplitChip`（计划/波次 chip）。
- 体积守门：单组件文件软 200 行；`check-arch.sh` 新增「components.css ≤ 上限」检查项。

---

## 4. 壳层：两栏壳 + phase 局部次级面板（结构修正版）

> **结构修正（2026-08-15 自检）**：初稿把 dsh 的 details 栏映射为**全局第三栏**。验证发现拆分台已是三栏（`SplitView.js`「三栏绑定」，plan.css 有 `.confirm-layout`/`.task-list-pane` 多列），外层再套全局 details = 拆分台四栏，1280px 窗口主区被压到 <700px，PM/非开发用户可操作性明显变差。**修正：壳层保持两栏（sidebar｜main），details 语义下沉为「phase 局部次级面板」**——只有确实需要详情/日志/对照的 phase（执行台、结果台）在自己的布局里开一个次级列；拆分台保持自身三栏不动。

### 4.1 布局
```
┌─────────┬──────────────────────────────────────────┐
│ sidebar  │  main                                   │
│ 200px   │  topbar                                 │
│ ─(56px) │   ├ 项目名 · 计划名    [拆分|执行|结果|聊天] │
│ 品牌+新建│   └ 主区 phase 内容（phase 自管次级面板）   │
│ 项目列表│   · 拆分台：自身三栏（不动）              │
│ (状态点)│   · 执行台：任务流 + 右次级日志/详情列     │
│ ───────│   · 结果台：完成/遗漏 + 右次级巡检对照列   │
│ 设置/环境│                                        │
└─────────┴──────────────────────────────────────────┘
```

### 4.2 侧栏（dsh `ui-sidebar` + `ui-workspace` 映射）
- **两层级**：项目（= dsh Workspace）→ 计划（= dsh Session，在项目内分组展示）。
- 项目行：**StateDot**（运行=蓝 / 等待确认=琥珀 / 已完成=绿 / 失败=红）+ 名称 + hover 卡（完整路径 + 一键复制）；「显示更多」超过 5 个计划时出现（P4-8 已实现，展开态不持久化）。
- 顶栏：**新建项目**（dsh New Session）+ **搜索**（折叠展开，250ms 防抖）+ 折叠控制。
- 底部 pin：**设置** + **环境检查**（dsh `sidebar.settings` 座）。
- 折叠 → 56px rail（150ms fade + 300ms slide）；rail 保留新建/搜索/设置三个 36px 控制。

### 4.3 顶栏（dsh 会话头映射）
- 左：项目名 + 当前计划名 + 拆分 badge（已拆分/未拆分）。
- 中：**view-ring 段控** `拆分｜执行｜结果｜聊天`（替代原 phase 隐藏条；`routes.js` 语义不变）。
- 右：聊天快捷、刷新、计划管理、budget/cost chip（结果台显示）、暗色切换（设置里）。

### 4.4 phase 局部次级面板（替代全局第三栏）
- **不做全局 details 容器**；由各 phase 在自身布局内实现：
  - 执行台 → 右次级列（选中任务详情 + 日志卡 TerminalBlock 风格）。
  - 结果台 → 右次级列（巡检对照勾选）。
  - 拆分台 → **不动自身三栏**，任务详情仍在其中一栏（现状即如此）。
- 次级列可折叠（本地 view 状态）；窄窗优先折叠次级列；几何瞬态（不入 localStorage）。

---

## 5. 页面级改动

### 5.1 欢迎页
- 居中 Hero 卡（dsh Hero/OnboardingSurface 风格），背景 `layer-1`；保留「三步」+「添加项目文件夹」主 CTA。
- 模板行 → 卡片（出海落地页 / 通用需求大纲）。
- 工作习惯四选一 → 保留在流内，卡片化，可跳过。
- 概念数：添加项目 / 模板 / 帮助 = 3，守 L1 §26。

### 5.2 拆分台（S0–S3 语义不动，视觉全换）
- 布局**保持现有三栏不动**（顶栏 S1 人话条 → 波次依赖并行图 S2 → 任务列表）；只换任务卡视觉语言：每任务卡 = **StateDot + 标题 + route pill（provider/role/scope）+ optional 徽标 + 默认通道 chip**，可展开（DisclosureRow 式）。
- 任务详情（要做什么 / 怎样算做完 / acceptance 黄条 / 默认通道下拉）留在现有一栏，不新增全局右栏（S0 双受众保持）。
- 底部确认条：`执行规划` primary + optional 勾选停住（记忆：**never auto-start past optionals**）。confirm 仍唯一开跑。

### 5.3 执行台
- 主栏：**任务流程卡**——每卡 = StateDot + 人话进展（`logBoardCard` 既有语义）+ 提交状态（auto-commit hash/files/push/失败，失败卡 route_label）+ 停/续/重跑。运行中蓝追光。
- **右次级列**：选中任务 → 命令+输出（`TerminalBlock`：状态 pill + 复制 + pre 对齐）+ 文件读写（`ReadBlock`/`DiffBlock`）+ 出错详情；等待审批 → 琥珀接管条。次级列可折叠。
- 日志次级：dsh Trajectory 的「日志次级」概念 —— 默认折叠在右次级列或独立次级面板，`logVirtual` 虚拟列表保留。

### 5.4 结果台
- 主栏：完成 / 遗漏（miss）行（带执行方式）+ 验证卡片 + **网页验收证据**（`browserEvidence` 截图卡）+ 回补入口（`startRework` 非 confirm 旁路）+ 结束 CTA。
- **右次级列**：巡检对照详情（`inspectCopy` 人话 + 对照勾选树），可折叠。
- `#result-cost-chip` 保留。

### 5.5 聊天（计划生成/核对）
- 会话流：dsh 气泡语言 + **composer dock 链**：
  - TodoDock（order 0）= 当前计划/波次计划条；
  - GoalBar（order 10）= 当前计划目标（可编辑/暂停/恢复/清除）；
  - **QueueDock（order 20）先做语义核对再定**：现代码 `queue/pending` 是「待发送附件 + 待渲染气泡」语义，**不是多消息排队队列**（`chatState.js`）。若 Leaf 无多消息排队，则不造 QueueDock 空壳——改为只保留「发送中/待发送」状态行，避免为对齐 dsh 加概念（L1 §26）。
- 输入座：**model/effort 两级菜单**（dsh `ui-model-selection`：provider 分组 → 具体模型 → effort），复用现有 `#s-effort` 语义；plan chip（当前计划 on/off）；发送/停止。
- 上下文注入 / 召回 → **DisclosureRow 默认折叠**（`上下文注入` / `跨会话召回` 人话标签）。
- 图片粘贴/拖放 → `DropOverlay` + 限制横幅（复用 `chatImageHydrate`）。
- `/` 命令联想保留，改为 dsh 命令菜单样式。

### 5.6 设置
- 两栏布局保留（左图标菜单 + 右滚动区），改 dsh settings 视觉：常规 / 模型 / 发布 / 环境（doctor）。
- **模型页**：provider 分组（claude/codex/fake）→ 每 provider 卡片 + effort 行（dsh `ui-settings-models`）。
- **权限 tier**（A3bis）：dsh permission presets —— preset chip 列表 + 当前值高亮 + `danger`（Full access）需显式风险确认（P4-7 已实现；保存链路不变）。
- 新增**外观**分区：暗色/浅色/跟随系统。
- 高级能力默认折叠（L1 §24）。

---

## 6. 交互行为改动汇总

| 行为 | 现状 | 目标（dsh 范式） |
|------|------|-----------------|
| 状态指示 | 文字/颜色混杂 | **StateDot 统一**（蓝运行/绿成功/红失败/琥珀等待） |
| 待确认 | 普通提示 | **PendingInteraction 琥珀点**优先于运行点（拆分确认/执行审批） |
| 长文本卡片 | 平铺 | 默认折叠 + 「显示 X / 共 N」诚实计数 |
| hover 路径 | 无 | HoverCard 可复制 |
| 错误/成功反馈 | 行内文字 | Toast 顶部横幅（3s） |
| 滚动条 | 系统默认 | 主题化 + 预留 gutter |
| 动效 | 少量 | 150ms/300ms 折叠；reduced-motion 全关 |
| 焦点 | 弱 | focus-visible 环 + 键盘可达 |
| 暗色 | 无 | `data-leaf-theme` 双主题 + 跟随系统 |

---

## 7. 实施阶段（P4-x · 每阶段独立可验收）

> **每阶段通用验证**（不重复列出）：① `cd web && node build.mjs` 产物就绪；② `scripts/check-arch.sh`（STRICT=1 时也要绿）；③ Tauri 窗口实测 1280px 与 1024px 双宽度走查 + 明暗双主题 + `prefers-reduced-motion`；④ facade 体积门禁（不新增堆叠）；⑤ 改动只落 Presentation 层、gateway 零新增命令。任一阶段失败可独立回滚。

- **P4-0 分支与地基确认（本轮已做）**：设计真源提交 = `a66c204`；建议开工前切独立分支 `feat/ui-redesign-dsh`（与当前 `complete-fix-split-table-terminal-cli` WIP 解耦）。`--accent` 现被 5 CSS + 2 JS 引用（`chat.css/layout.css/monitor.css/plan.css/select.css` + `chatClarify.js/chatMsgEnhance.js`）。
- **P4-1 设计系统地基 ✅（零行为 diff · 2026-08-15）**：`tokens.css` 三层 `--leaf-*`（static 色阶 → body alias → `body[data-leaf-theme="dark"]` 暗色覆写）+ 字体五级；**过渡期保留旧变量为别名指向新值**（`--accent: var(--leaf-alias-brand-primary)` 等），CSS 零 break；JS 侧 2 文件（chatClarify / chatMsgEnhance）`var(--accent, …)` 改读 `var(--leaf-alias-brand-primary, #4176E6)`。加 `components.css` **191 行**原语（StateDot / Card / Pill / DisclosureRow / Terminal / Diff / Read / Toast）≤200 守门（check-arch 新增 1c 检查项）。验收：`node build.mjs` ✅ · esbuild CSS @import 链解析 ✅ · token 交叉校验（light+dark static 完整、全部 var() 可解析）✅ · `check-arch.sh` STRICT=1 绿（WARN=4 为既有 Rust 文件 soft 超限，非本轮引入）✅ · **明暗双主题目视走查 ☐**（需 P4-2 壳后人工走查）。
- **P4-2 两栏壳 + 侧栏 ✅（2026-08-15）**：`index.html` 保持两栏 grid；侧栏重做（StateDot 状态点已 P4-1 落地 / 折叠 56px rail 几何瞬态不入 localStorage / 搜索 / hover 复制卡 / 底部设置带图标）；顶栏加 view-ring 段控 `拆分|执行|结果|聊天`（`body[data-cco-app-phase]` 现有属性驱动高亮，`routes.js` 语义不变）。**不做全局第三栏**。新增 `scripts/p42-visual-smoke.mjs`（一次性目视冒烟，stub invoke，不进 CI）。验收：DOM id 全部保留（`#project-list`/`#page-*` 等），脚本无空引用；窄窗下侧栏折叠正常；`node build.mjs` ✅ · `check-arch.sh` STRICT=1 绿（WARN=4 为既有 Rust soft 超限，未新增；components.css 186 ≤200）✅ · `p42-visual-smoke.mjs` **14/14 pass** ✅（含 rail 折叠宽度 <70px · 搜索 1/3 · hover 复制 · 暗色非白 sidebar）。
- **P4-3 拆分台（视觉换，三栏不动）✅（2026-08-15）**：任务卡 dsh 语言——StateDot + 标题 + route pill 簇（**默认通道 chip** / 角色 / 范围）+ optional 徽标 + 展开 chevron，卡片无 provider 下拉（P2-17 57ab9d6 不回退，通道只在详头可改）；现有任务详情栏换卡片样式（`confirm-detail` 卡片底/描边/圆角）；**底部确认 dock**（`#split-confirm-hint` 涂装 + `执行规划` primary 移入，`#btn-confirm-start` 语义与唯一开跑不变）；optional 勾选停住与 dock hint 在 `splitFillMeta` 只读 DTO 渲染、不写策略。验收：`node build.mjs` ✅ · `check-arch.sh` STRICT=1 绿（**WARN=4 为既有 Rust soft 超限，未新增** · components.css 186 ≤200）✅ · `clarify-split-visual-smoke.mjs` **12/12** ✅ · `provider-control-smoke.mjs` **ALL PASS**（详头下拉 8 选项 · 卡片无下拉复活）✅ · 新增 `scripts/p43-visual-smoke.mjs` **27/27** ✅（StateDot 颜色 · 通道 chip 跟随 provider · optional 徽标 · runLocked dock 禁用 · 明暗 dock 非白）。
- **P4-4 执行台 ✅（2026-08-15）**：主栏任务流程卡 dsh 化（`.cli-window` 走 alias token · 运行中蓝追光 `.is-running` · 卡片语义不变：StateDot / 人话进展 / auto-commit hash·files·push / 失败卡 route_label / 停·续·重跑）· **右次级列 `#run-detail-column`（约 320px · 默认展开、可折叠 · 几何瞬态不入 localStorage · 窄窗优先折叠）**：选中任务 → TerminalBlock（工具命令+输出 · 状态 pill · 复制）· ReadBlock/DiffBlock（读/写文件 · auto-commit files 诚实计数）· 失败错误详情 · 等待琥珀条（DTO 无「等待审批」字段 → 用既有 `wait` 排队 / `stall` 卡住语义承接，不造假概念）· 日志 DisclosureRow 默认折叠（`fillPanelLogBody` 保留 logVirtual 虚拟列表）。CSS 落 **`css/run.css`**（alias token；components.css 186 ≤200 不动）· 新增 `features/run/runDetail.js`（渲染幂等，签名不变不重绘）· `RunView` `renderProgress` 尾部渲染 · 停/续/重跑仍经 `ccoRun`→`runApi`→gateway（stop_run / stop_task / resume_run / retry_task 1:1）· `logBoardCard` `.is-running` 追光 · `logBoardEvents` 聚焦分发同步详情列。验收：`node build.mjs` ✅（FACADE_OK 253/253）· `check-arch.sh` STRICT=1 绿（**WARN=4 为既有 Rust soft 超限，未新增** · components.css 186 ≤200）✅ · 既有三冒烟全绿（`p43-visual-smoke.mjs` **27/27** · `provider-control-smoke.mjs` **ALL PASS** · `clarify-split-visual-smoke.mjs` **12/12**）✅ · 新增 `scripts/p44-visual-smoke.mjs` **38/38** ✅（流程卡 dsh · is-running 追光动画 cco-run-chase · 失败卡执行方式 · 自动提交状态 · 右次级列 Terminal/Diff/Read · wait/stall 琥珀条 · 日志折叠 · 详情列折叠 aria-pressed · 停→stop_task_cmd / 续→resume_run_cmd / 重跑→retry_task_cmd · 明暗非白无页面错误）。
- **P4-5 结果台 ✅（2026-08-16）**：主栏完成/遗漏列表卡片化（`.result-desk-item` + StateDot check/x 图标 + 标题 + route_label 执行方式 + 失败原因）· issue_preview 卡（icons.js 暂无 alert-triangle → fallback `!`）· 浏览器证据网格 dsh 化（截图卡放大 lightbox + 文本摘录 pre + 打开文件）· honest footer 提示条样式（border-left warn · 巡检对照计划结论）· 验收面板保持现有 `<details>` 可折叠（plan_items 勾选 ☑/☐）· **CSS 走 alias token 落 `css/result.css`**（components.css 186 ≤200 不动；旧 `.result-desk-*` 规则从 `css/plan.css` 清除只留 `[hidden]` 守卫）。验收：`node build.mjs` ✅（FACADE_OK 253/253）· `check-arch.sh` STRICT=1 绿（**WARN=4 为既有 Rust soft 超限，未新增** · components.css 186 ≤200）✅ · 既有四冒烟全绿（`p43-visual-smoke.mjs` **27/27** · `p44-visual-smoke.mjs` **38/38** · `provider-control-smoke.mjs` **ALL PASS** · `clarify-click-smoke.mjs` **12/12**）✅ · 新增 `scripts/p45-visual-smoke.mjs` **32/32** ✅（完成 2 卡 check 图标 · miss 卡 x 图标 + route_label 执行方式 + 网络超时 · issue 卡 `!` + 内容 · honest footer 巡检结论 · 验收面板默认收起→点击展开 · plan_items 3 项 ☑/☐ · 浏览器证据 2 卡截图 img + 文本摘录 · rework 按钮文案「回补并再巡检（第 2/3 轮）」· 明暗非白无页面错误）。
- **P4-6 聊天 ✅（2026-08-17）**：composer 保持 dock，补 Claude 模型 + 推理深度两级选择；模型以会话覆盖真实下传，非 Claude 通道自动禁用；本轮上下文（项目/计划/附件）以 DisclosureRow 默认收口。QueueDock 语义仍不符，未造空壳；无策略复制进 JS。验收：web build ✅ · cco chat tests 33/33 ✅ · model 覆盖/清空 test ✅ · desktop cargo check ✅。
- **P4-7 设置 ✅（2026-08-17）**：设置页新增浅色 / 深色 / 跟随系统三态；`themePreference.js` 持久化仅本机的展示偏好，启动前同步解析主题以免闪白；`tokens.css` 与 `theme.css` 覆盖全页明暗 alias 与 reduced-motion，`thinkingOrb` 在减弱动效时静帧。补齐权限 preset chip，切入完全访问必须显式危险确认，仍复用既有保存链路。验收：web build ✅ · theme/orb/main 语法检查 ✅ · chat tests 33/33 ✅ · desktop cargo check ✅。
- **P4-8 打磨（实现完成，视觉终验待受控执行通道恢复）**：补齐侧栏项目→计划二级树（`sidebarPlans.js` 只读 `getPlanMeta/getPlans` 缓存，默认五项、超过后瞬态展开，选项先选项目再选计划）与结果局部巡检列（约 320px、默认展开、1024px 以下默认收起但可手动展开，继续只读 `live.verification`）；全局 reduced-motion 与明暗主题已由 P4-7 落地。`p45-visual-smoke.mjs` 已扩展为 1280/1024、明暗、二级树、计划展开、权限确认与巡检列开合契约，但本轮先因沙箱拒绝本地监听（`EPERM`），再因受控执行调度通道 `503` 无可用通道而无法运行；未绕过限制。已验：web build ✅ · JS 语法检查 ✅ · `STRICT=1 ./scripts/check-arch.sh` `FAIL=0`（5 条既有警告）✅。

**执行顺序依赖**：P4-1 → P4-2 → P4-3/4/5（三桌，可并行拆人）→ P4-6 → P4-7 → P4-8。每阶段完成即提交（记忆：plan done → commit）。

---

## 8. 门禁与硬规则对齐

- **L1 §4**：先更新本计划 → 改代码时同步 `web/CLAUDE.md`（L2）与 `docs/architecture-redesign-2026-07-20.md` 附录 C 对应条目。
- **L1 §5–9**：只动 Presentation；`gateway` 保持唯一 IPC 出口，本计划**不新增任何 Rust 命令**（只复用现 command）。
- **L1 §10–11**：confirm 唯一开跑不变；无 `start_run` 旁路。
- **L1 §15–18**：单文件软 400/硬 600；组件 CSS ≤200；不往 classic facade 堆功能。
- **L1 §19–22**：MVVM；策略只在 Rust；JS 只渲染 DTO。
- **L1 §23–26**：主路径人话；高级折叠；TUI 不加第二套拆分台；新概念 ≤3。
- **构建链路**：每阶段 `cd web && node build.mjs`（`web/dist/` 是 gitignored 产物，不构建则冒烟 404）+ `scripts/check-arch.sh` + 打包 smoke（`scripts/`）。
- **分支**：开工前切 `feat/ui-redesign-dsh`，不混入当前 WIP 分支。
- **tokens 过渡**：`--leaf-*` 落地期间旧变量（`--accent` 等）保留为别名，单阶段一次性切引用，禁止跨阶段半替换。
- **记忆**：optional 勾选停住、拆分台默认停、不静默覆盖 route、icons 用 Lucide 风 SVG、双受众 S0。

---

## 9. 决策点与风险

| 决策点 | 状态 | 决定 | 说明 |
|--------|------|------|------|
| 暗色默认值 | ✅ 已定 | **跟随系统** | dsh `system` 行为；桌面值钱 |
| **全局第三栏 details** | ✅ 已定 | **不做**（改 phase 局部次级面板） | 拆分台已三栏，再套全局第三栏 = 四栏拥挤；见 §4 修正 |
| 次级列宽度/收起 | ✅ 已定 | 约 320px · **默认展开、可折叠**（几何瞬态不入 localStorage） | 执行/结果台局部；窄窗优先折叠 |
| 侧栏「计划」二级树 | ⬜ 待定 | 项目内展开分组 | 对应 dsh Workspace→Sessions |
| 状态色语义 | ✅ 已定 | **运行=蓝**（沿用 dsh 约定） | 与「ok 绿 / 成功绿」区分开 |
| 品牌蓝全量替换 #0071E3 | ✅ 已定 | 是（走 §7 过渡策略） | 含图标 hover / focus-ring / CTA |
| QueueDock（排队 dock） | ⬜ 待定 | **先核对 Leaf 队列语义** | 现代码是附件/气泡 pending，非多消息排队；语义不符则不实现 |
| 风险 | — | dsh 是 chat-first，直接照搬会把主路径带偏 | 由 §2 红线 + §8 门禁兜底；每阶段回归拆分台「唯一开跑」 |

---

## 附：dsh 组件 → Leaf 等价物映射表

| dsh | Leaf 等价 |
|-----|-----------|
| `AppFrame` 三栏 | `#app` grid 保持两栏（sidebar/main）；details 语义 → **phase 局部次级面板** |
| `ui-sidebar`（56px rail） | 项目侧栏 + 折叠 |
| `ui-workspace`（分组行/搜索/Show more） | 项目→计划分组列表 |
| `ui-conversation` view-ring | phase 段控 拆分/执行/结果/聊天 |
| `ui-plan` chip | 当前计划 chip（拆分/聊天） |
| `ui-jobs` 列表 | 执行台任务卡 + 后台任务 |
| `ui-trajectory` | 执行台「日志次级」 |
| `ui-agent-preset` / `ui-model-selection` | 聊天 model/effort 两级选择 |
| `ui-permission-presets` | 权限 tier（A3bis） |
| `TodoDock`/`GoalBar`/`QueueDock` | 计划条/目标/排队 |
| `TerminalBlock/DiffBlock/ReadBlock` | 执行台/结果台卡片原语 |
| `StateDot`/`DisclosureRow`/`Toast`/`HoverCard` | 同名原语 |
| `--dsw-*` token 三层 | `--leaf-*` 三层 |
