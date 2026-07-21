# cco — CLI Orchestrator（项目任务控制台）
Rust + Tokio + Clap + ratatui + Tauri 2 + 原生 web（HTML/CSS/JS）

**产品方向**（非计划）：[`PRODUCT.md`](./PRODUCT.md) — 轻量 Codex 式 · 主受众 PM/出海/非开发 · 生成→核对→拆分→并行→巡检  
**架构大改**（实施真源 · **已收口**）：[`docs/architecture-redesign-2026-07-20.md`](./docs/architecture-redesign-2026-07-20.md) — 六边形+用例 · 前端 MVVM · Split/Worker/Run/Inspect · **A0–A5 ✅** · **P2-17 t58** · **A5-5 可选 ☐ 本轮不做**（评估 [`docs/a5-5-workspace-crates-eval-2026-07-21.md`](./docs/a5-5-workspace-crates-eval-2026-07-21.md)）  
**规范根**（工程/落地）：[`docs/`](./docs/CLAUDE.md)（本仓库**不使用** `.md/`；Agent/人读 **L1 → L2 `*/CLAUDE.md` → 源文件 L3 头部**）。

数据流（目标）：`PlanDoc → Split(ProposedGraph) → confirm → Run/Orchestrator → WorkerPort(claude|codex|fake) → Handoff/Inspect → report`；入口 CLI/Tauri/TUI 只调 Application 用例。

<directory>
[`src/`](./src/CLAUDE.md) — 核心库与 CLI（plan·runtime·cli·services·tui·terminal·config·state·doctor·graph·report）
  · [`domain/`](./src/domain/CLAUDE.md) 纯模型 plan/run/worker/inspect/chat（A1-1…A1-6）
  · [`app/`](./src/app/CLAUDE.md) 用例 split/run/chat
  · [`ports/`](./src/ports/CLAUDE.md) WorkerPort（A1-4）· HandoffStore（A1-5）+ 未来 Store
  · [`plan/`](./src/plan/CLAUDE.md) PlanIR facade + adapters + Planner(Mode B)
  · [`runtime/`](./src/runtime/CLAUDE.md) Scheduler · handoff/* 适配器 · log_events · provider(WorkerPort 适配) · worktree · acceptance
  · [`cli/`](./src/cli/CLAUDE.md) clap 命令面
  · [`terminal/`](./src/terminal/CLAUDE.md) TerminalManager + external launcher
  · [`tui/`](./src/tui/CLAUDE.md) ratatui 多页观察层
  · [`services/`](./src/services/) CLI/桌面共用服务层（迁移期 facade）
[`src-tauri/`](./src-tauri/CLAUDE.md) — 桌面壳（Tauri 2 commands → cco::app / services）
[`web/`](./web/CLAUDE.md) — 桌面前端（**A2–A5 ✅** `js/main.js` module · `shared/gateway` · `app/AppViewModel` · `features/{chat,project,split,run,result,settings}`；classic **S8 facade** chat/doctor/result ≤80 · plan/log/monitor ≤200；**state.js D9 遗留**；IPC 只 gateway）
[`docs/`](./docs/CLAUDE.md) — **规范根**（真源 · 业务规则参考 · 历史计划索引）
[`examples/`](./examples/CLAUDE.md) — 示例计划
[`tests/`](./tests/CLAUDE.md) — 集成与金样
[`scripts/`](./scripts/CLAUDE.md) — 打包与 smoke
[`.claude/skills/cco-run/`](./.claude/skills/cco-run/CLAUDE.md) — Claude Code skill `/cco-run`（**P2-6** 薄封装 `cco run`）
dist/ — 已打包 CCO.app（生成物，无 L2）
</directory>

<config>
Cargo.toml — workspace（cco + cco-desktop）

### 真源（改边界 / 勾选只认这些）
[`PRODUCT.md`](./PRODUCT.md) — 产品方向（受众 · 五步 · 轻量；**非**落地勾选）
[`docs/architecture-redesign-2026-07-20.md`](./docs/architecture-redesign-2026-07-20.md) — **本轮架构/实施真源 · A0–A5 ✅**（P2-17；A5-5 可选不做）
[`docs/a5-5-workspace-crates-eval-2026-07-21.md`](./docs/a5-5-workspace-crates-eval-2026-07-21.md) — A5-5 crate 边界评估（本轮不落）
[`docs/contracts/`](./docs/contracts/) — A0 行为/run-dir/plan-job 契约

### 业务规则参考（改 confirm / 混跑 / 巡检 / 拆分台时读；**不**继承阶段勾选）
[`docs/product-mode-b-ai-planner.md`](./docs/product-mode-b-ai-planner.md) — Mode B · confirm 唯一开跑 · optional
[`docs/multi-cli-collaboration-2026-07-18.md`](./docs/multi-cli-collaboration-2026-07-18.md) — provider/role/scope · handoff · tags 路由
[`docs/plan-execute-inspect-rework-2026-07-19.md`](./docs/plan-execute-inspect-rework-2026-07-19.md) — 巡检对照勾选 · 回补波（P-loop）
[`docs/product-mainpath-optimize-2026-07-20.md`](./docs/product-mainpath-optimize-2026-07-20.md) — 拆分台三栏/结果台交互意图（P2-16 UI 已闭环）
[`docs/gap-and-landing-plan-2026-07-18.md`](./docs/gap-and-landing-plan-2026-07-18.md) — 历史总账 + D5 池导航（D0–D4 已闭环；**不**新开阶段表）
[`claude-cli-orchestrator-plan.md`](./claude-cli-orchestrator-plan.md) — 编排器设计（M0–M4 已落地；M5 → D5）

### 历史参考（主线已 ✅ · 文件仍在 docs/ · **勿当缺口 · 勿继承勾选**）
清单与一行摘要见 [`docs/CLAUDE.md`](./docs/CLAUDE.md)「历史参考」——chat / 桌面 UX / terminal / 计划管理 / sys-post 等已闭环子计划。
</config>

能力要点: 轻量任务控制台 · Mode B 规划相位 · providers claude/codex/fake · log_events 可读监视 · 计划闭环五步 · 桌面 planSessions/auto-start

法则: 极简·稳定·导航·版本精确·地图与地形同构

## 工程硬规则（P2-17 起 · 防复发）

> 细则展开：[`docs/architecture-redesign-2026-07-20.md`](./docs/architecture-redesign-2026-07-20.md) §2–§4。  
> 门禁脚本：[`scripts/check-arch.sh`](./scripts/check-arch.sh)（`warn` 默认 / `STRICT=1` 可失败）。  
> **违反下列任一条 = 先改设计或拆文件，禁止「先堆上再还债」。**

### 真源与文档

1. **产品方向**只认 [`PRODUCT.md`](./PRODUCT.md)（受众 · 五步 · 轻量）；工程计划不得改定位。  
2. **本轮架构/实施勾选**只认 [`docs/architecture-redesign-2026-07-20.md`](./docs/architecture-redesign-2026-07-20.md)（**P2-17**）。  
3. 历史落地计划（含 P2-16 与各已 ✅ P2）= **参考**；**不**继承勾选、**不**平行第二套阶段表、**不**回灌 D0–D4。  
4. 改架构边界：**先更新计划/本法则/对应 L2，再改代码**（地图与地形同构）。

### 分层与依赖

5. 方向仅允许：`Presentation → Application → Domain`；`Adapters` 实现 `Ports`。  
6. **禁止** Domain 依赖 app / UI / Tauri / clap / 具体 provider 实现。  
7. **禁止** View、Tauri command、CLI handler 写业务策略；只做「解析 → 调 app 用例 → 返回 DTO」。  
8. **禁止** 新建上帝 `*Manager` 或继续堆厚 `services`；组合逻辑放 **Application 用例**。  
9. 跨限界上下文（Chat / Split / Run / Worker / Handoff / Inspect）只经 app 命令/查询，禁止 UI 直读调度内部字段。

### 业务硬契约

10. **唯一业务开跑入口**：Split 确认（现 `confirm_start` 语义 → 目标 `SplitUseCase.confirm`）。**禁止** UI/`start_run` 旁路 Mode B。  
11. 结构化计划可用 ParseOnly，仍走**同一用例**，不是第二开跑入口。  
12. **CLI 与桌面共用同一 Application API**；禁止 CLI 再写一套调度/混跑策略。  
13. 多 CLI：`provider` / `role` / `scope` 是 **Worker 路由**；soft-fill **不得**静默覆盖任务上已显式声明的 route（全量覆盖须 force 语义）。  
14. 计划中带 **optional** 的业务步骤：确认屏必须可勾选停住，**禁止**静默 auto-start 跳过可选确认（见记忆/拆分契约）。

### 体积与深度

15. 业务源文件：软上限 **400** 行，硬上限 **600** 行；超硬上限必须先拆再加功能。  
16. 单函数：软 **40** 行，硬 **80** 行。  
17. 业务路径自定义封装深度 **≤ 4**（Presentation→App→Domain→Adapter）。  
18. **禁止**往已知厚文件继续堆功能（只删/抽/一行委托）。**S8 已出榜**（facade ≤200）：`plan.js` · `chat.js` · `log.js` · `doctor.js` · `monitor.js` · `result.js`（真源 `features/*`）。**仍厚待后收**：`state.js`（D9 · invoke 桥，非业务策略）。Rust 侧 A1 已出榜：`plan/mod` · `scheduler/*` · `handoff/*` · `services/chat/*`。

### 前端（桌面）

19. **MVVM**：View 不写业务链；ViewModel 只发意图并调 gateway；规则在 Rust Application。  
20. IPC **唯一出口** `gateway`（或等价模块）；禁止 feature 文件散落 `__TAURI__`/`invoke`。  
21. 主区 phase：`author | split | run | result`（一屏主焦点）；日志默认次级。  
22. **禁止**在 JS 复制 Mode B / optional / confirm / 混跑策略；只渲染 app 下发的人话 DTO。

### 产品护栏

23. 主路径文案服务 PM/出海；引擎名、`VERDICT`、schema、run_id **不得**作主路径第一句。  
24. 高级能力与系统收尾**默认折叠/默认关**；不为「像 IDE」堆第一屏。  
25. **TUI = 观察 + 轻控制**；**不**做第二套完整拆分台（除非单独立项）。  
26. 同一屏新概念 **≤ 3**（对齐 PRODUCT 心智三问）。

[PROTOCOL]: 架构变更时更新此文件与对应 L2；规范根为 docs/，勿另建 .md/；硬规则变更须同步 `docs/architecture-redesign-2026-07-20.md` 与 `scripts/check-arch.sh` 可检查项
