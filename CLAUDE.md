# cco — CLI Orchestrator（项目任务控制台）
Rust + Tokio + Clap + ratatui + Tauri 2 + 原生 web（HTML/CSS/JS）

**规范根**：[`docs/`](./docs/CLAUDE.md)（本仓库**不使用** `.md/`；Agent/人读 **L1 → L2 `*/CLAUDE.md` → 源文件 L3 头部**）。

数据流：`计划文档 → PlanIR/Planner(Mode B) → Scheduler → WorkerProvider(claude|codex|fake) → state/report`；监视经 `log_events` → CLI/TUI/桌面 LogConsole。

<directory>
[`src/`](./src/CLAUDE.md) — 核心库与 CLI（plan·runtime·cli·services·tui·terminal·config·state·doctor·graph·report）
  · [`plan/`](./src/plan/CLAUDE.md) PlanIR + adapters + Planner(Mode B)
  · [`runtime/`](./src/runtime/CLAUDE.md) Scheduler · log_events · provider(claude/codex/fake) · worktree · acceptance
  · [`cli/`](./src/cli/CLAUDE.md) clap 命令面
  · [`terminal/`](./src/terminal/CLAUDE.md) TerminalManager + external launcher
  · [`tui/`](./src/tui/CLAUDE.md) ratatui 多页观察层
  · [`services/`](./src/services/) CLI/桌面共用服务层（D4 目录化）
[`src-tauri/`](./src-tauri/CLAUDE.md) — 桌面壳（Tauri 2 commands → cco::services）
[`web/`](./web/CLAUDE.md) — 桌面前端资源（打包进 App）
[`docs/`](./docs/CLAUDE.md) — **规范根** + 产品/UX/缺口总账计划
[`examples/`](./examples/CLAUDE.md) — 示例计划
[`tests/`](./tests/CLAUDE.md) — 集成与金样
[`scripts/`](./scripts/CLAUDE.md) — 打包与 smoke
dist/ — 已打包 CCO.app（生成物，无 L2）
</directory>

<config>
Cargo.toml — workspace（cco + cco-desktop）
[`claude-cli-orchestrator-plan.md`](./claude-cli-orchestrator-plan.md) — 编排器设计真源（**M0–M4 已落地**；**M5 → D5 池**；Codex 已实现；桌面与 Mode B 主线）
[`docs/gap-and-landing-plan-2026-07-18.md`](./docs/gap-and-landing-plan-2026-07-18.md) — 未完善唯一总账（§1.3/§2.1/§2.3/§3/§5 已冻结 · **§6 成功标准 t18 全绿** · **§7 非目标 t19 已冻** · **§8 开放确认 t20 按默认已冻** · **§9 修订历史 t21 已闭环** · **D0–D4 闭环** · **D5 池 t15** · **§5 序 t16** · **§5.4 Agent 策略 t17**：D0→D5，P2 不排期则不碰）
[`docs/desktop-ux-redesign-plan.md`](./docs/desktop-ux-redesign-plan.md) — 桌面壳 UX（0–4 已实施，勿再当缺口）
[`docs/product-mode-b-ai-planner.md`](./docs/product-mode-b-ai-planner.md) — Mode B（plan job / confirm_start；**B0–B3 已闭环** D1/D3）
[`docs/terminal-console-plan.md`](./docs/terminal-console-plan.md) — 监视日志（log_events A 路径 P0 + P1-1/2/3 **D2 已接**）
[`docs/ux-simple-mainpath-2026-07-17.md`](./docs/ux-simple-mainpath-2026-07-17.md) — 易用性主路径简化（已落地）
[`docs/chat-plan-builder-2026-07-18.md`](./docs/chat-plan-builder-2026-07-18.md) — 聊天共建计划 → 落盘 → 分配（**已落地** C0–C2 ✅ · 五指标全绿 · **§9 验证清单 t11 七绿** · **§10 t12 文档/GEB**；C3→D5/P2-9 不排期则不碰）
[`docs/chat-ux-focus-2026-07-19.md`](./docs/chat-ux-focus-2026-07-19.md) — 聊天页注意力收敛（后台降噪 · fake/故障可信 · CTA · **方案已定稿 · 未实施** U0–U2 → D5/P2-10；**不**回灌 P-chat）
[`docs/chat-utf8-fence-panic-2026-07-19.md`](./docs/chat-utf8-fence-panic-2026-07-19.md) — 聊天 plan fence UTF-8 panic 热修（**F0+F1 已闭环** · F2 可选不排期 · **P-chat-utf8**；**不**并入 P2-10）
[`docs/plan-execute-inspect-rework-2026-07-19.md`](./docs/plan-execute-inspect-rework-2026-07-19.md) — 计划驱动执行闭环（拆分·巡检·回补 · **L0–L2 已落地** · **P-loop / P2-11**）
[`docs/multi-cli-collaboration-2026-07-18.md`](./docs/multi-cli-collaboration-2026-07-18.md) — 多 CLI 协作（Claude+Codex · 声明/越界/检验员/handoff · **方案已定稿 · 未实施** · P0–P2 → D5）
</config>

能力要点: Mode B 规划相位 · providers claude/codex/fake · log_events 可读监视 · 预算分栏 · 上限 validate · 桌面 planSessions/auto-start

法则: 极简·稳定·导航·版本精确·地图与地形同构

[PROTOCOL]: 架构变更时更新此文件与对应 L2；规范根为 docs/，勿另建 .md/
