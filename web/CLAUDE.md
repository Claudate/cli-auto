# web/
> L2 | 父级: /CLAUDE.md

成员清单
index.html: 桌面壳结构；经典 `js/*.js` 顺序加载 + **A2–A5** `type=module` → `js/main.js`（构建后改引 `dist/`）；**shell-chrome** 顶栏无阶段条/编辑任务 · 三 icon（聊天/计划/刷新）· 拆分台仅「重新规划」+ **P4-3 底部确认 dock**（`#split-confirm-hint` + `#btn-confirm-start`「执行规划」primary）· 设置含 **GitHub 发布** 大区。**P4-4 执行台**：`#monitor` 包 `.run-flow-row`（flex row）+ 右次级列 `#run-detail-column`（320px · 可折叠）· 看板工具条 `#btn-run-detail-toggle`。**P4-2 两栏壳**：侧栏 = `.sidebar-head`（brand + `#btn-sidebar-collapse` rail）+ `.sidebar-actions`（`#btn-add-plus` 新建项目 + `#btn-sidebar-search-toggle`）+ `.sidebar-search`（`#sidebar-search-input`/`#sidebar-search-clear`）+ `#project-list`（行内 `name-text` + `.project-hover-card` 路径复制卡）+ 底部图标设置；顶栏 `#view-ring` 段控 `拆分|执行|结果|聊天`（`body[data-cco-app-phase]` 驱动高亮 · 仅 workspace/chat 页显示）。**防逆向 dist/ 前置**：产物引用 gitignored `web/dist/`（`dist/app.js` + `dist/classic/*.js`），干净 clone / 冒烟先 `cd web && node build.mjs`，否则页面全 404
app.js: 入口说明（逻辑在 js/）
app.css: @import 聚合 css/*（含 components · tokens CTA）
js/: **A5 S8 facade** state（**D9 桥/瘦 ~230**）· flow · **templates≤80**（**P-ship-D D7 ✅**）· plan≤200 · monitor≤200 · result≤80 · split 空壳 · log≤200 · chat≤80 · doctor≤80（**禁止堆新功能**；**A5-0/2 清单**见下文 · **A5-4 收口**）
js/main.js: **ESM 入口** — AppViewModel + gateway + **installStatusUi/markdown/shellUi（D9）** + **installSelectUi** + **chat/settings/project/templates desk** + split + run + result · `window.ccoChat` / **`ccoProject`** / `ccoRun` / `ccoResult` / **`ccoSettings`** / `ccoLog` / `ccoSplit` / **`ccoTemplates`** / **`ccoSelectUi`** · **P4-2**：wireShellNav 委托 `#view-ring` 段控（`dataset.ccoA2Wired` 守卫）+ project-list 点击守卫（hover 卡/复制路径不选中）
js/shared/: **gateway.js**（IPC 唯一出口 · 含 **chatReadImageDataUrl** / **gitDoctor** 发布状态）· **store.js**（可订阅薄 store）· **statusUi.js**（D9 人话/badge/elapsed）· **markdown.js**（D9 确认屏/计划说明/**聊天气泡** md · **`![alt](path)` 本地图占位**）· **shellUi.js**（D9 pages/projects/run-lock · **B1 侧栏 × 移除项目** · **P4-2 侧栏 chrome**：折叠 56px rail · 搜索过滤 · 只列项目不嵌计划）· **clickOutside.js**（**B2** 展开 details/菜单/设置高级点空白收）· **confirmDialog.js**（应用内确认层 `confirmDialog()`→Promise&lt;bool&gt; · 替代 WKWebView 不可靠的 window.confirm/prompt · Esc/背板=取消 · danger 焦点在取消；main install `ccoConfirm`）· **selectUi.js**（原生 select 增强为 macOS 风下拉；保留 `.value`/`change`）· **icons.js**（Lucide 风格开源线条图标 · `data-icon` / `ccoIcon` · **禁止 emoji 作按钮图标**）· **workStyle.js**（方案 C 工作习惯四选一 · 可跳过 · 并发/模板/grain 种子 · 项目级覆盖 W4-2 · **不**改 plan_mode 默认 ai）· **thinkingOrb.js**（canvas 2D 思考 orb 渲染引擎：九状态点阵球 · 减弱动态效果时单帧静止 · IO 防泄漏；无 IPC · 纯展示）
js/app/: **AppViewModel.js** · **routes.js**（phase author|split|run|result ↔ page）· **wireRunResult.js**（A4 壳接线）
js/features/chat/: **A5-2a ✅ · P-ship-D 软超纵切** chatApi · ChatViewModel · chatState · chatSessions · **chatSessionRename**（历史面板重命名）· chatActions · **chatRender** · **chatThinkingOrb**（等待气泡=思考 orb：canvas 持久节点 + 文案；场景映射 澄清/整合→composing@36 · 思考→weaving@32 · 状态切换先 stop 再 start 防泄漏；渲染引擎 `shared/thinkingOrb`）· **chatImageHydrate**（`![alt](本地图)` / 附件 path → data URL 内联）· **chatMsgEnhance**（AI 编号题 A/B/C **可点选** · 历史消息默认折叠自展开）· **chatPlanOps**（**直接执行** = `plan_mode=direct` + auto-confirm；**无** start_run）· chatFormat · chatAttachments · planDir · planRail（**数据/meta 加载 · 聊天右栏 DOM 已撤**）· planFull · **plansMgmt**（计划列表 UI 唯一入口 · 顶栏「计划管理」）· **chatSlash**（`/` 命令联想下拉 · per-CLI 目录来自 Rust `slash_catalog` · 本地+透传+保留标灰 · 无策略复制）· installChat · legacy/host · index（经 gateway；**无** confirm/start_run）
js/features/project/: **A5-2b-fin D5 ✅** projectApi · ProjectViewModel · sessionEntry（**tryRestorePlanJobForPlan / loadPlanSplitIndex** 按路径回看拆分 · SQLite）· shellChrome · projectCrud · planMeta（**已拆分 badge**）· projectPicker · planSelect · jobPoll · confirmActions · loadLiveBridge · installProject · legacy/host · index（picker/H0/job 轮询/optional 门；confirm→ccoSplit；**无** invoke/start_run）
js/features/split/: **A3 ✅ · A5-2b · S-role · P1-4 · 双受众 S0–S3 · shell-chrome A2/A5 · P4-3 dsh 语言** splitApi · SplitViewModel · **splitRender**（**P4-3 任务卡 dsh**：StateDot + 标题 + route pill 簇（默认通道 chip / 角色 / 范围）+ optional 徽标 + 展开 chevron · 卡片无 provider 下拉——P2-17 不回退，通道只在详头可改）· splitDetail（要做什么/怎样算做完 · **本步说明**在下 · 无技术/完整说明壳 · paintChrome「重新规划」）· **splitTaskBody**（【做什么】…→可读 md）· SplitView · **splitFillMeta**（顶栏人话 + 来源 · acceptance 黄条 · **P4-3 底部 dock hint** optional 停住 · 只读 DTO 不写策略）· index
js/features/run/: **A4 ✅ · A5-2b · A5-2c · P-ship-D · ux-C · P1-3** runApi · runBuckets · RunViewModel · RunView（失败摘要含执行方式 · **计划级自动提交状态**）· logPanel · **loadLive** · **log\*** · **logBoardCard**（人话进展 · 失败卡 route_label · **任务级自动提交 hash/files/push/失败**）· **logBoardEvents** · **runDetail**（**P4-4 右次级列**：Terminal/Diff/Read 卡片 · wait/stall 琥珀条 · 日志 DisclosureRow 折叠；几何瞬态不入 localStorage）· index（进度·stall·停/续；**运行端 CLI 看板始终可见** · 卡内详细日志按需；日志栏右侧 **高度 + 详情 + 继续 + 结束计划**；workspace 轮询壳）
js/features/result/: **A4 ✅ · ux-C3 · P0-1/P0-4 · P1-3 · P2-1 · W3 · P4-5** resultApi · inspectCopy · **resultSummary** · **browserEvidence**（`live.browser_evidence` 截图 data URL / 摘录）· ResultViewModel · ResultView（miss 行执行方式 · verification · **网页验收证据** · CTA 仅「结束」· **#result-cost-chip** · **P4-5 dsh 化**：完成/遗漏列表卡片化 StateDot + route_label · 浏览器证据网格 · honest footer 提示条 · 验收面板保持 details 可折叠）· index
js/features/settings/: **A5-2d ✅ · P-ship-D · A3bis · P4-7** settingsApi · settingsForm（表单加载/保存）· **permissionControls**（授权 preset · 完全访问危险确认 · 不保存）· Git · 版本发布大区 · doctorPage · shellBoot · uiActions · bindUi · **bindUiClick** · settingsNav（settings/doctor/gitDoctor/meta/open_monitor 经 gateway；事件表只绑意图）
js/features/templates/: **P-ship-D D7 ✅ · ux-C4** catalog · splitSummary · templatesApi · templatesActions（无项目 pending 模板）· installTemplates · index（冷启动模板落盘 · S14 拆分摘要写回；经 chatApi/gateway；**无** confirm/start_run）
css/: **tokens**（P4-1 三层 `--leaf-*`：static 色阶 → body alias → 暗色 `body[data-leaf-theme="dark"]` 覆写；legacy `--bg/--accent` 等为别名过渡；**组件禁写死颜色**）· **components**（P4-1 dsh 原语：`.dot.run/pending/danger` + `.live/.ok/.warn/.err/.muted` 别名 StateDot · `.leaf-card` · `.pill` · `.disclosure-row` · `.terminal-block`/`.diff-block`/`.read-block` · `.toast`；≤200 行 · 只引 alias/static）· layout（P4-2：`#view-ring` 段控 · `.sidebar-*` · `.project-hover-card` · `body.cco-sidebar-collapsed` rail 56px ≥901px）· **select**（统一下拉 closed/open）· plan（含 split-route-advanced · **result-cost-chip** · `.top-actions` flex 1:0:1 · **P4-3** `.split-channel-chip`/`.split-role-pill`/`.split-scope-pill`/`.split-chevron` · `.opt-badge` · `.confirm-detail` 卡片化 · `.confirm-dock` 底部确认 dock）· monitor（board-toolbar-side · is-result）· **run**（**P4-4 执行台**：`.cli-window` dsh 化 · `.is-running` 蓝追光 · `.run-flow-row`/`#run-detail-column` 次级列 · wait/stall 琥珀条；只走 alias token）· **result**（**P4-5 结果台**：`.result-desk-*` 卡片化 · `.result-browser-*` 证据网格 · `.result-desk-honest` 提示条；只走 alias token）· log · chat（**#chat-effort** 推理深度选择器）
设置：高级 → **#s-effort**（low…max|ultracode）；聊天 composer 可按次覆盖 → `chat_send_cmd.effort`

## 硬规则（继承 L1 · 本层加严）

1. **MVVM**：View 不写 Mode B / 开跑 / 混跑 / stall-failover / inspect 门禁策略；业务在 Rust Application。
2. **IPC 唯一出口**：新建代码必须经 `js/shared/gateway.js`；禁止在 `features/` 内直接 `invoke` / `__TAURI__`。
3. **主区 phase**：`author | split | run | result`（`AppViewModel`）；一屏主焦点；日志次级。
4. **禁止**在 JS 复制 `confirm_start` / optional / provider soft-fill / stall 策略。
5. **禁止** UI 旁路开跑（`start_run` 不得替代 Split 确认）；回补只经 `start_rework` / `resultApi.startRework`。
6. **禁止**往 classic facade（plan/chat/monitor/result/log/doctor）与 `state.js` **继续堆功能**（只抽离/删除/一行委托）；新功能进 `features/*`。S8 facade 已出业务巨石榜；`state.js` = **D9 桥/瘦**（~230 · 展示→`shared/statusUi`+`markdown` · 壳导航→`shared/shellUi`）。
7. 主路径文案人话；`VERDICT` / 引擎名 / run_id **不进第一句**。
8. 文件体量：软 400 / 硬 600 行（与 L1 同；`check-arch.sh` GIANTS 业务榜已空 · LEGACY_THICK 对 state.js **D9 已 ≤400** 仅 info）。

## A2–A4 模块图（源码边界 · 非第二套阶段表）

```text
index.html
  classic scripts (strangler globals; chat.js / plan.js = thin facade)
  type=module js/main.js
    → app/AppViewModel + routes
    → shared/gateway + store + selectUi
    → features/chat/{api,VM,sessions,actions,rail,full,installChat}  ← A5-2a
    → features/project/{api,VM,sessionEntry,picker,jobPoll,confirmActions,installProject}  ← A5-2b-fin D5
    → features/split/{api,VM,Render,Detail,View}
    → features/run/{api,buckets,VM,View,logPanel,log*} ← A4 + A5-2c
    → features/result/{api,inspectCopy,VM,View}       ← A4
    → features/settings/{api,form,doctor,boot,ui}     ← A5-2d
    → features/templates/{catalog,splitSummary,api,actions,install} ← P-ship-D D7
window.ccoGateway / ccoApp / ccoChat / ccoProject / ccoSplit / ccoRun / ccoResult / ccoLog / ccoSettings / ccoTemplates
```

### gateway 已有 Run / Result 方法表（A1-7 命令名 1:1）

| gateway | Tauri | app |
|---------|-------|-----|
| `stopRun` | `stop_run_cmd` | `run::stop` |
| `stopTask` | `stop_task_cmd` | `run::stop_task` |
| `resumeRun` | `resume_run_cmd` | `run::resume` |
| **`retryTask`** | **`retry_task_cmd`** | **`run::retry_task`（单任务再跑，非 re-split）** |
| **`startRework`** | **`start_rework_cmd`** | **`run::rework`（非 confirm 旁路）** |
| `acceptResidual` | `accept_residual_cmd` | handoff accept residual |
| `openTaskTerminal` | `open_task_terminal_cmd` | terminal |
| `getProjectLive` | `get_project_live` | live 查询 DTO（含 `inspect_loop`） |
| `openMonitorWindow` | `open_monitor_window_cmd` | P2-4 独立窗 |

### Split 方法表（A3 · 唯一开跑）

| gateway | Tauri | app |
|---------|-------|-----|
| **`confirmStart`** | **`confirm_start_cmd`** | **`split::confirm`（唯一开跑）** |
| `startPlanJob` / `getPlanJob` / `updatePlanTask` / … | 同名 `*_cmd` | `split::*` |

### monitor / result 职责 → features

| 旧（classic） | 新（A4） |
|---------------|----------|
| `renderTaskStrip` / KPI / stall 横幅 | `features/run` RunView + runBuckets |
| 停 / 续 / 停步 | `ccoRun` → runApi → gateway |
| `syncMonitorLogsFold` / 日志次级 | `features/run/logPanel`；虚拟列表 `logVirtual`（A5-2c）· `log.js` facade |
| `renderResultDesk` / 完成·遗漏 | `features/result` ResultView |
| `renderInspectLoopStrip` / 人话 | `inspectCopy`（读 `inspect_loop` DTO；无裸 VERDICT） |
| rework / accept residual | `ccoResult` → resultApi → gateway |
| 终态 phase | `ccoRun` onFinished → `AppViewModel.goResult` |
| 回补开跑 | `startRework` → `goRun`（**非** `start_run` / **非** confirm） |

invoke 散落 → gateway 方法表：见 `js/shared/gateway.js`（命令名 1:1 A1-7）。

## A5-0 清单（调研 · 2026-07-21 · 不删代码）

> 真源勾选：[`docs/architecture-redesign-2026-07-20.md`](../docs/architecture-redesign-2026-07-20.md) §11 A5-0 + **§16 附录 C**。
> 起点：A4 tip 工作树（分支名仍 `feat/arch-a3-split-desk`；A5-1+ 建议 `feat/arch-a5-…`）。
> **本刀零行为 diff**。

### 1) classic `js/*.js` 清单

| 文件 | 行数 | 职责（一行） | 仍直接 `invoke` 的命令 | 已有 feature 委托点 |
|------|------|--------------|------------------------|---------------------|
| [`state.js`](./js/state.js) | ~~820~~ → ~~503~~ → **~230**（**D9+**） | 全局 state · `$` · **invoke 桥** · prefs · toast | 仅桥：`getInvoke`/`invoke`；dialog pre-main 兜底 | 展示→`shared/statusUi`+`markdown`；pages/projects/run-lock→`shared/shellUi`（main install）；`requireGateway()`；`loadProjects` 单路径 gateway优先 |
| [`flow.js`](./js/flow.js) | ~340 | 主路径流程文案 / 趣味旁白 | **无** | 无（纯文案 helper，可长期保留） |
| [`split.js`](./js/split.js) | ~~305~~ → **≤50 空壳**（**A5-2f D3 ✅**；index **已去 script**） | 无逻辑；三栏真源 `ccoSplit` | **无** | `window.ccoSplit`（A3/A5-2b）；禁止双轨 |
| [`templates.js`](./js/templates.js) | ~~389~~ → **≤80 facade**（**P-ship-D D7 ✅**） | classic 全局名 → `window.ccoTemplates` | **无** | `ccoTemplates.*` · 真源 `features/templates/*`（catalog/summary/api/actions） |
| [`plan.js`](./js/plan.js) | ~~3020~~ → ~~2550~~ → **≤200 facade**（**A5-2b-fin D5** · ~108 行） | classic 全局名 → `window.ccoProject` | **无** | `ccoProject.*` · confirm→`ccoSplit` · loadLive→`ccoLoadLive`；真源 `features/project/*` |
| [`monitor.js`](./js/monitor.js) | ~~549~~ → **≤200 facade**（**A5-2f D2 ✅**） | workspace 壳 · phase/body · doctor/picker；进度只 `ccoRun` | **无** | `ccoRun.renderProgress`；无 KPI/stall/tile 副本 |
| [`result.js`](./js/result.js) | ~~207~~ → **≤80 facade**（**A5-2f D1 ✅**） | classic 名 → `ccoResult` | **无** | `ccoResult.renderResultDesk` · `finishRound` |
| [`log.js`](./js/log.js) | ~~1476~~ → **≤200 facade**（**A5-2c**） | classic 全局 → `ccoLog` | **无** | `features/run/log*` · stop/resume 只 `ccoRun`/`ccoResult`；虚拟列表迁出 |
| [`chat.js`](./js/chat.js) | ~~3050~~ → **≤80 facade**（**A5-2a**） | classic 全局名 → `window.ccoChat` | **无**（全量经 gateway/chatApi） | `ccoChat.*`（list/send/save/session/stream/rail/full/mgmt）；真源 `features/chat/*` |
| [`doctor.js`](./js/doctor.js) | ~~1242~~ → **≤80 facade**（**A5-2d**） | classic 全局名 → `window.ccoSettings` | **无** | `ccoSettings.*`（load/save settings · doctor · meta · open_monitor · boot · UI 意图表）；真源 `features/settings/*` |
| [`main.js`](./js/main.js) | ~560 | **ESM 入口**（非 classic 巨石） | **无** | 装配 `ccoApp/Gateway/Chat/Project/Split/Run/Result/Settings/Log` |
| [`app.js`](./app.js) | ~10 | 入口说明占位 | 无 | 无 |

**features 散落 invoke**：`rg` 仅注释声明「禁止 invoke」——**无真实 `__TAURI__`/invoke 调用**（门禁绿）。

**A5-2e 业务 invoke 清扫**：classic 业务路径统一 `requireGateway()` / `ccoChat` / `ccoRun` / `ccoResult`；`rg 'invoke\(' web/js` 除 `shared/gateway.js` + `state.js` 桥外 **无** 业务 `invoke("…_cmd")`。

**UI `start_run` 旁路**：classic + features **无** `start_run` 调用；gateway **不**暴露 `startRun`。Tauri 仍注册 legacy `start_run`（ParseOnly → `app::run::start_from_request`）——A5-2 候选「藏/弃用文档化」，**非**桌面主路径。

### 2) 删除顺序建议（风险升序 · 仅建议）

| 序 | 目标 | 前置 | 风险 |
|----|------|------|------|
| **D1** | `result.js` → **≤80 facade**（`renderResultDesk`/`finishRunRound` → `ccoResult`） | — | **✅ A5-2f** |
| **D2** | `monitor.js` → **≤200** workspace 壳 + `ccoRun`；删 KPI/stall/tile 副本 | — | **✅ A5-2f** |
| **D3** | `split.js` 空壳 + **index 去 script**（`ccoSplit` 单轨） | — | **✅ A5-2f** |
| **D4** | `log.js` 停/续/rework 死代码删；虚拟列表抽 `features/run/log*` | **A5-2c ✅**：facade ≤200 · `ccoLog` · 无 invoke fallback | 中（✅） |
| **D5** | `plan.js` 项目/计划列表/入口路由 → `features/project`+`split` 全量；**A5-2b-fin ✅**：facade ≤200；picker/H0/job 轮询/optional 门在 `features/project`；confirm 仅 `ccoSplit` | 入口 H0 目视：选项目→选计划→拆分台停留/optional→confirm→run | **高**（✅ 2b-fin） |
| **D6** | `chat.js` 全量 → `features/chat`（session/stream/rail/assign） | **A5-2a ✅**：facade ≤80；IPC 经 gateway；**无** confirm/start_run | **高**（✅ 2a） |
| **D7** | `templates.js` → `features/templates` | **P-ship-D D7 ✅**：facade ≤80；`ccoTemplates`；IPC 经 chatApi/gateway；**无** confirm/start_run | 中（✅） |
| **D8** | `doctor.js` settings/轮询 → `features/settings` + app 轮询收口 | **A5-2d ✅** facade ≤80；IPC 经 gateway；事件表只绑意图 | 中（✅ 2d） |
| **D9** | `state.js` 桥/瘦身（展示 helper → `shared/statusUi`+`markdown`） | **P-ship-C ✅** · ~820→~503（硬≤600） | 已做；不一次清全局 |
| **D9+** | pages/projects/run-lock → `shared/shellUi`；state ≤400 | **P-ship-C ✅** · ~503→~230；`installShellUi`；loadProjects gateway 单路径 | 仍留：state 对象 · invoke 桥 · toast · prefs；**不**一次清全局 |
| **D10** | `flow.js` 可迁 `shared/flowCopy` 或保留（无 invoke） | 文案键 | 低 |

**禁止**：一次 PR 清空 plan+chat；未先迁 invoke→gateway 就删 classic fallback。

### 3) 并发建议（A5 后续）

| 轨 | 任务 | 并行？ |
|----|------|--------|
| **A5-0** | 本清单 | 串行 · 已完成 |
| **A5-1** | CLI 子命令 → app 1:1（尤其 `run`/`resume` 去手搓 Scheduler） | 可与 A5-2 前端 **分 agent 并行**（不同树） |
| **A5-2** | 删旧 JS / 迁剩余 invoke→gateway→feature | 内串：D1→D4 可并行薄删；D5/D6 **各 1 agent 串行** |
| **A5-3** | TUI 只读 app 查询 + stop 经 `app::run` | 可与 A5-1 并行（同 Rust 时注意冲突 `app/run`） |
| **A5-4** | L1/L2/门禁/总账 GEB | **✅ 2026-07-21** · 串行收口 |
| **A5-5** | workspace crates（可选） | **本轮不做** · 评估 docs only |

P4-6：composer 无模型选择器；「本轮上下文」summary 右侧只读徽标显示会话模型（无覆盖显示「默认」· 不可点/不可编辑），切换唯一入口 = `/model <名称>` 斜杠命令（resp.model 回流同步徽标与 state.chatSession.model）；CLI/effort 既有语义不变。

P4-7：`shared/themePreference.js` + `css/theme.css` 提供浅色/深色/跟随系统三态与全局 reduced-motion；设置页以 preset chip 展示权限层级，完全访问需危险确认；仅持久化本机展示偏好。

P4-8：`ResultView` 将 `live.verification` 置入结果局部巡检列（约 320px，窄窗默认收起），不复制 DTO 或业务规则。侧栏只列项目，不嵌计划树。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
