# cco 未完善项总览与落地计划

> 状态：**D0–D4 已闭环**；**D5 backlog 池 t15 已建立（不排期则不碰）**；**§5 推荐执行顺序 t16 已冻结**；**§5.4 任务量与 Agent 策略 t17 已冻结**；**§6 成功标准 t18 已冻结（五指标全绿）**；**§7 非目标 t19 已冻结**；**§8 开放确认 t20 已冻结（五默认全「按默认」）**；**§9 修订历史 t21 已闭环（t1–t21 年表齐）**；本文件为未完善唯一总账  
> 日期：2026-07-18  
> 范围：产品缺口 · 文档 GEB · 代码质量 · 验证发布  
> 角色：跨计划导航入口；子计划（Mode B / UX / terminal / 主路径简化）只保留细节与勾选，不另开第三份总览  
> 关联真源：
> - [`../claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md)（编排器设计）
> - [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)（产品主路径 B）
> - [`desktop-ux-redesign-plan.md`](./desktop-ux-redesign-plan.md)（桌面壳 UX）
> - [`terminal-console-plan.md`](./terminal-console-plan.md)（监视日志）
> - [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（主路径简化）
> GEB 入口：[`/CLAUDE.md`](../CLAUDE.md)（L1）· [`./CLAUDE.md`](./CLAUDE.md)（L2 docs）

[PROTOCOL]: 变更时更新此头部与阶段勾选，然后检查 /CLAUDE.md 与 docs/CLAUDE.md

---

## 0. 一句话

**内核（M0–M4）已可用；桌面主路径已能「选计划→分配→跑」；缺口集中在：B 模式收尾、监视体验 P1、文档同构、发布验证、超大文件拆分。**

> 定稿（t2）：与 §1.3 已完成冻结对齐；五簇缺口对应 §2（B 收尾≈P0-1/2/3+P1-4..7 · 监视 P1≈P1-1..3 · 文档同构≈§2.2/D0 · 发布验证≈P0-4/D3 · 超大文件拆分≈§2.3/D4）。子计划不得另写冲突总述。  
> §2.3 架构味道已冻结（t8：Q1–Q5 + 切分地图）；**D4 物理拆分 t14 已落地**（六文件 → 目录/模块，见 §4 D4）。  
> §3 本质层根因已冻结（t9：R1–R4 + 现象映射 + 完成定义）；**不是「功能太少」**，是完成定义不唯一——消解走 D1–D4 / 对应 P·Q，勿另开第三套根因叙事。

---

## 1. 项目认知摘要

### 1.1 这是什么

> 定稿（t3）：一句话定义 + 数据流 + 技术栈；与 `README` / orchestrator 设计真源 / 工作树 `src/`·`src-tauri/`·`web/` 对齐。子计划不得另写冲突产品定义。

`cco`（CLI Orchestrator）= 本机 **任务控制台**：

```text
计划文档 → PlanIR（适配器/Planner）→ DAG Scheduler → WorkerProvider(claude/codex/fake)
                ↓
         state/report/CLI/TUI/Tauri 桌面壳
```

技术栈：`Rust + Tokio + Clap + ratatui + Tauri 2 + 原生 web(HTML/CSS/JS)`。

### 1.2 模块地图（现实）

> **定稿（t4）**：模块职责 + 实测行数 + 超标判定；以 2026-07-18 工作树 `wc -l` 为准（阈值：单文件 **>800** 视为超标）。  
> 子计划（D4 拆分 / GEB L2 播种 / 桌面接线）不得另写冲突的体量结论或模块边界。

| 路径 | 职责 | 体量风险 |
|------|------|----------|
| `src/plan/` | 适配器（cco-v1 / serial-prompts / raw-single）+ PlanIR + **Planner（Mode B）** | `planner.rs` **1312** 行，超标；`mod.rs` 377 |
| `src/runtime/` | scheduler / provider(claude·codex·fake) / worktree / acceptance / log_events | `claude.rs` **956**、`log_events` **748**、`scheduler.rs` 603 |
| `src/services.rs` | CLI 与桌面共用服务层（runs / plan job / live / logs / projects / settings） | **880** 行，超标 |
| `src/cli/` | clap 命令面（doctor/run/resume/status/stop/report/logs/term/tui…） | `mod.rs` **882** 行，超标；`interactive.rs` 122 |
| `src/tui/` | 多页 TUI（app / pages / widgets） | 合计 **742**，可接受 |
| `src/terminal/` | 外置终端 / session（Embedded·External） | CLI/`cco term` 已用；**桌面未接**（`src-tauri` 无 open 命令） |
| `src-tauri/` | Tauri commands 薄壳 → `cco::services` | `lib.rs` 360，薄壳可接受 |
| `web/` | 桌面 UI 状态机（planSessions / monitor / LogConsole） | `app.js` **3099**、`app.css` **2646**，严重超标；已有 `web/CLAUDE.md` |
| `docs/` | 产品/UX/缺口总账计划 · **规范根** | L2 全树已有（t7 D0）；见各目录 `CLAUDE.md` |
| `tests/` | 集成/金样（scheduler/fake、resume/budget、acceptance/term、serial-prompts） | 覆盖调度主路径，**缺桌面 E2E** |

辅助模块（体量正常，未入超标账）：`src/config/` 388 · `src/state/` 246 · `src/doctor/` 154 · `src/graph/` 94 · `src/report/` 92；`scripts/` 打包 smoke；`examples/plans/` 示例计划。

### 1.3 已完成（不要再当缺口）

> **冻结（t5）**：下列能力以工作树代码 + `cargo test --lib`（16 passed，2026-07-18，含 D1 路由测）为准已闭环。  
> §2 缺口表与子计划勾选 **不得** 再把它们写成 ☐ / 未做 / 新建。  
> 残差另立 ID（例：Planner 复用 LogConsole → **P1-3**，不是「P0 未完成」；CLI `run` 默认规划 → **P0-1**，不是「B1 主线未做」）。

| 层 | 状态 | 证据（代码锚点） |
|----|------|------------------|
| M0–M4 编排内核 | ✅ doctor/run/resume/status/stop/report/logs/term/tui | `src/cli/mod.rs` Commands；M0–M4 勾选见 orchestrator 计划 |
| Providers | ✅ claude / codex / fake | `src/runtime/provider/{claude,codex,fake}.rs` |
| Plan 适配 | ✅ cco-plan/v1 · serial-prompts · raw-single | `src/plan/adapters/{cco_v1,serial_prompts,raw_single}.rs` |
| 桌面壳 UX 0–4 | ✅ 浅色主从、项目内开跑、大日志区 | `docs/desktop-ux-redesign-plan.md` 阶段 0–4 全 ✅ |
| 主路径简化 | ✅ 合并选计划弹窗、task-dash、CLI 再跑、AI 事件过滤 | `docs/ux-simple-mainpath-2026-07-17.md`；`web/app.js` `autoStartAfterPlan` / `btn-chooser-assign` |
| Mode B0/B1 主线 | ✅ phase 状态机、plan job、LLM+heuristic、confirm_start、波次/waiting_on | `src/plan/planner.rs`；`services::{start_plan_job,confirm_start}`；B0/B1 表主项 ✅ |
| 终端日志 A 路径 P0 | ✅ `log_events` + 可读/原始/终端 transcript 观感 | `src/runtime/log_events.rs`；terminal 计划 P0 主项 [x]（Planner 复用属 P1-3） |

---

## 2. 现象层：还有什么没完善

### 2.1 产品功能缺口（按优先级）

> **冻结（t6）**：下列为 2026-07-18 工作树对照后的**仍开放**产品缺口唯一表。  
> 代码锚点以 `src/cli/mod.rs` · `src/plan/planner.rs` · `src/services.rs` · `src-tauri/src/lib.rs` · `web/app.js` · 子计划勾选为准。  
> **已具备但不足以勾掉的能力**写在「非闭环说明」列，避免再把半成品当已完成，也避免把已有能力当「从零缺失」。  
> 实现顺序映射：P0-1/2/3 + P1-7 → **D1**；P1-1/2/3 → **D2**；P1-4/5/6 + P0-4 → **D3**；P2 → **D5**。  
> 子计划（Mode B / terminal / ux-simple）只更新勾选，**不得**另写冲突优先级或把 §1.3 已完成项回填进本表。

#### P0 — 必须闭环（否则产品叙事不完整）

| ID | 缺口 | 状态 | 代码/文档锚点（证据） | 非闭环说明（已有 ≠ 完成） | 建议动作 | 阶段 |
|----|------|------|----------------------|---------------------------|----------|------|
| **P0-1** | CLI `run` 默认不走规划 | ✅ **D1 闭环** | `src/cli/mod.rs`：散文 → `plan_then_load_ir`；结构化 auto skip；确认/`--yes` | 结构化与散文路由已分 | 保持与 Mode B §4.1 一致 | D1 |
| **P0-2** | 结构化「跳过规划」产品入口不完整 | ✅ **D1 闭环** | CLI `--skip-plan` + `is_structured_adapter` 自动 skip；桌面 `plan_mode=parse` 文案「跳过规划（直接解析）」 | 产品级入口齐 | — | D1 |
| **P0-3** | orchestrator 正文缺 Mode B 流程图 | ✅ **D1 闭环** | `claude-cli-orchestrator-plan.md` §2.0 Mode B 相位图 + 双入口表 | B 流程已入真源 | — | D1 |
| **P0-4** | 桌面 App 重打包 + 主路径目视未闭环 | ✅ **D3 闭环** | `scripts/package-app.sh` + `dist/CCO.app`；打包后 rg 主路径标记；目视清单见本文件 §4 D3 | 无自动化桌面 E2E（增强项） | 保持 release 打包习惯 | D3 |

#### P1 — 体验与边界（影响「敢用」）

| ID | 缺口 | 状态 | 代码/文档锚点（证据） | 非闭环说明 | 建议动作 | 阶段 |
|----|------|------|----------------------|------------|----------|------|
| **P1-1** | live 全量 tail，无增量协议 | ✅ **D2 闭环** | `read_text_tail` 行边界；live 有 events 时 compact raw；前端 `logPanelSig` 少重绘贴底 | 完整 `since_byte` 游标仍可选增强；当前减负 + 前端 skip 已够主路径 | — | D2 |
| **P1-2** | 桌面未接外置终端 | ✅ **D2 闭环** | `services::open_task_terminal` + Tauri `open_task_terminal_cmd` + CLI 窗「外置终端」 | 一键 WezTerm/iTerm/`tail -f` | — | D2 |
| **P1-3** | Planner 日志未复用 LogConsole | ✅ **D2 闭环** | `PlanJobView.planner_log_events` + `#planner-log` LogConsole/`fillPlannerLog` | 规划阶段不再 raw pre 墙 | — | D2 |
| **P1-4** | 任务数 / prompt 长度 / 超时上限 | ✅ **D3 闭环** | `MAX_TASKS=20` · `MAX_PROMPT_CHARS` · `MAX_TIMEOUT_SECS` · `PLANNER_MAX_BUDGET_USD`；`PlanIR::validate` 拒绝；lib 单测 | — | 保持常量与 validate | D3 |
| **P1-5** | 规划预算 vs worker 预算未分栏展示 | ✅ **D3 闭环** | `PlanJob.planner_cost_usd` · run `planner_cost.json` · report `## Budget` · live `planner_cost_usd`/`exec_cost_usd` · 顶栏 `#budget-chip` | — | 保持分栏文案 | D3 |
| **P1-6** | Mode B 黄金用例矩阵 | ✅ **D3 闭环** | `tests/mode_b_golden.rs`：散文 fake plan→confirm→exec · serial-prompts parse · cco-v1 parse + 预算 | — | `cargo test --test mode_b_golden` | D3 |
| **P1-7** | auto-start vs「必须确认」双规则 | ✅ **D1 闭环** | Mode B §4.1 + UX 真源同一句：默认 auto-start；高级 `#pp-pause-confirm`；业务仍只 `confirm_start` | 双真相已消灭 | — | D1 |

#### P2 — 增强 / backlog（可延后，不阻塞 ship 叙事）→ **D5 池（t15）**

> **冻结（t15）**：P2 全部进 **§4 D5 池**；**不排期则不碰**；不得回填为 P0/P1。  
> 出池条件：用户真实疼痛 **或** 显式单独立项。池表细节与立项门槛见 §4 D5。

| ID | 缺口 | 状态 | 来源 / 锚点 | 备注 |
|----|------|------|-------------|------|
| P2-1 | 确认屏删任务 / 改依赖 | ☐ **D5 池** | Mode B2 可选 ☐ | 依赖高级 pause 确认屏是否常显 |
| P2-2 | replan 保留人工修改 | ☐ **D5 池** | Mode B2 可选 ☐ | 策略未定；先策略后码 |
| P2-3 | 虚拟列表 / 事件过滤 / ANSI / 导出报告 | ☐ **D5 池** | terminal P2 | P1-1 已闭环；超长 run 真卡再做 |
| P2-4 | 跨显示器系统级多窗口 | ☐ **D5 池** | ux-simple「未做」 | 现为应用内面板 |
| P2-5 | TUI 内嵌真 PTY 网格 | ☐ **D5 池** | orchestrator M3 未勾增强项 | 当前 embedded=会话登记+日志路径 |
| P2-6 | Claude Code skill `/cco-run` | ☐ **D5 池** | M4 可选 ☐ | |
| P2-7 | M5：SDK provider / Mermaid / 自动开 PR / Windows launcher | ☐ **D5 池** | orchestrator M5 列表 | 按需拆单独立项；Codex 已出池 |
| P2-8 | M5「第二 provider」文档债 | ⚠ **主文已改** | M5 已划掉 Codex 并指向 `codex.rs`（t5）；残差：其它段落若再出现「尚无第二 provider」则删 | 不占池；发现即改 |

#### 2.1.1 与 §1.3 的边界（防回灌）

| 勿再写入本表的「伪缺口」 | 真实归属 |
|--------------------------|----------|
| Planner / plan job / confirm_start 不存在 | §1.3 Mode B0/B1 **已完成** |
| 桌面不能选计划、不能跑 | §1.3 主路径简化 + UX 0–4 **已完成** |
| 无 log_events / 可读监视 | §1.3 terminal A 路径 P0 **已完成**；残差仅 P1-1/2/3 |
| 无 Codex provider | §1.3 Providers；P2-8 仅文档扫尾 |
| 无 `cco plan` / `run` 不规划 | **已有** `cco plan`；`run` 散文 plan job / 结构化 skip（P0-1 **D1 闭环**） |

#### 2.1.2 最小闭环定义（便于 D1–D3 验收）

| 优先级 | 何时可从本表勾掉 |
|--------|------------------|
| P0 | CLI 与桌面主路径叙事一致（规划/跳过/确认规则单一）；orchestrator 含 B 流程图；`CCO.app` 目视清单全绿 |
| P1 | 监视可增量、可外置终端、planner 日志同学；上限与预算可见；三套金样绿；auto-start/confirm 真源无冲突 |
| P2 | 不阻塞「可 ship」；用户真实疼痛再立项 |

### 2.2 文档 / GEB 缺口（协议债务）

> **D0 已闭环（t7）**：规范根 = `docs/`（写入 L1，**不**建 `.md/`）；L1 已列总账 + Mode B / log_events / codex；L2 已覆盖 `src/**` · `src-tauri` · `web` · `docs` · `tests` · `scripts` · `examples`；核心 L3 已播种（`src/**/*.rs` + `src-tauri` + `web/{index.html,app.js,app.css}`）。  
> 启动清单若仍写「读 `.md/`」→ 以 L1 决议为准，改读 `docs/` + 各目录 `CLAUDE.md`。

| 层 | 现状 | 残差 |
|----|------|------|
| L1 `/CLAUDE.md` | ✅ 规范根 + Mode B / log_events / codex / 总账链接（t7） | 架构变更时按 PROTOCOL 回写 |
| L2 模块地图 | ✅ `src/` 及子模块 · `src-tauri` · `web` · `docs` · `tests` · `scripts` · `examples`（t7） | 新目录须补 L2 |
| L3 文件契约 | ✅ 核心约 30+ 源文件已有 `[INPUT]/[OUTPUT]/[POS]/[PROTOCOL]`（t7） | 新文件须补 L3；D4 拆分后迁移头部 |
| 计划文档索引 | ✅ `docs/CLAUDE.md` | 本文件为未完善唯一总账 |
| 设计真源时效 | t5/t6 + **D1/t11**：orchestrator §2.0 + Mode B §4.1 | **P0-3 ✅**；残差随 D2–D3 回写 PROTOCOL |
| 协议目录 | ✅ **决议：规范根 = `docs/`**（t7） | 勿再新建 `.md/` 镜像 |

### 2.3 代码质量 / 架构味道

> **冻结（t8）**：下列为 2026-07-18 工作树对照后的**架构味道唯一表**（阈值：单文件 **>800** 行超标；行数 `wc -l` 实测）。  
> 本表**只记账与切分地图**；**不**在本任务做 D4 物理拆分（§8 默认：暂缓直到热改碰到超标文件）。  
> 与 §2.1 产品 ID 的关系：双路径 → **P1-7 ✅**；监视双入口 → **P1-2/P1-3**；验证 → **P0-4**；文档残差 → **P0-3 ✅** + §2.2 PROTOCOL。  
> 子计划 / D4 PR **不得**另写冲突的体量结论或「已拆完」勾选，除非附 `wc -l` 与切分后路径。

#### 2.3.1 味道总表

| ID | 味道 | 状态 | 位置 / 证据锚点 | 本质 | 消解归属 |
|----|------|------|-----------------|------|----------|
| **Q1** | 文件过大（>800） | ✅ **D4 闭环** | 六文件已纵切：`planner/` · `services/` · `cli/commands/` · `claude/` · `web/js/` · `web/css/` | 簇边界可指认；单文件仍有 >800 残差（plan.js）可继续按热改切 | — |
| **Q2** | 双主路径心智 | ✅ **D1 闭环** | Mode B §4.1 + UX D1 段 + `#pp-pause-confirm`；`confirm_start` 仍唯一业务入口 | 默认 auto-start + 高级暂停，文档与代码一致 | — |
| **Q3** | 监视两套入口 | ✅ **D2 闭环** | 桌面：`open_task_terminal_cmd` + 外置终端按钮；`#planner-log` → LogConsole / `fillPlannerLog`；P1-1 行边界 + 签名少重绘 | 外置终端与 Planner 日志已横切接线 | — |
| **Q4** | 文档滞后 | ⚠ 大部已缓解 | D0 L1/L2/L3 已闭环；**P0-3 ✅**（orchestrator §2.0）；D1 真源默认句已统一；多 plan 勾选随 D2–D3 回写 | GEB 回环未成默认动作时易再漂移 | **D0+D1 已完**；残差跟产品阶段回写 PROTOCOL |
| **Q5** | 验证缺口 | ⚠ 大部闭环 | Mode B 金样 `mode_b_golden` + 内核测；**P0-4** 打包+清单；仍无 Tauri/web E2E | 桌面自动化 E2E 仍属增强 | 可选后续 E2E | D3 |

#### 2.3.2 Q1 超标文件 — 实测行数与建议纵切

> 行数以 2026-07-18 工作树为准；与 §1.2 同阈值。D4 切开时**按簇迁移**，忌大爆炸 PR。

| 文件 | 行数 | 内部簇（约略行段 / 符号） | D4 建议切分 |
|------|------|---------------------------|-------------|
| `src/plan/planner.rs` | **1355** | types/job IO（`PlanJob*` · `start_plan_job`/`get_plan_job`）；`run_planner`；LLM（`build_llm_plan`·`parse_llm_plan_output` ≈L577–890）；heuristic/fake（≈L900–1150）；confirm 辅助 | `planner/job.rs` · `llm.rs` · `heuristic.rs` · `view.rs`（或 `mod.rs` 再 export） |
| `src/runtime/provider/claude.rs` | **961** | flags/env；`start_print`/`start_bg`；`poll`/`refresh_bg_logs`；`stop`/`collect`；`parse_agent_id`/`stream_child` | `spawn` · `poll_bg` · `parse_result`（同目录子模块） |
| `src/services.rs` | **885** | settings（≈L140）；runs/confirm（≈L174–450）；projects（≈L517）；live/`task_logs`（≈L586–885） | `services/projects.rs` · `runs.rs` · `live.rs` · `settings.rs` |
| `src/cli/mod.rs` | **888** | clap `Commands` 大 match：Doctor…Tui（≈L196–748）；helpers（term manager / provider caps） | 按子命令文件（`cli/run.rs` `plan.rs` `term.rs`…）或 `commands/` |
| `web/app.js` | **3139** | state/util（≈1–500）；projects/nav；plan chooser/job poll（≈900–1520）；confirm；workspace/CLI board；LogConsole（≈2135–2600）；wire/bind | `state` · `plan` · `monitor` · `log` · `doctor` 模块（或 IIFE 分段，无构建器时慎用） |
| `web/app.css` | **2679** | Sidebar/Main/Cards；plan picker；master–detail；Mode B phases；LogConsole；CLI board；task-dash / 2026-07-18 增量块 | `tokens` / `layout` / `plan` / `monitor` / `log`（`@import` 或打包时拼接） |

未入超标账但接近关注：`log_events.rs` **748**、`scheduler.rs` **608**、TUI 合计 **~762** — 暂不强制 D4。

#### 2.3.3 Q2 双主路径 — **D1 已闭环**

| 侧 | 行为 | 锚点 |
|----|------|------|
| 桌面默认 | 规划完成后 **自动** `confirmAndStart()` | `web/app.js` `autoStartAfterPlan` 默认 true；`advancePlannedJob` |
| 桌面高级 | 「规划后暂停确认」→ 停确认屏 | `#pp-pause-confirm` / `PAUSE_CONFIRM_KEY` |
| Mode B 文档 | **§4.1 D1 决议**：默认 auto-start；worker 只经 `confirm_start` | `docs/product-mode-b-ai-planner.md` §4.1 |
| 服务层真相 | 业务 worker **只**经 `confirm_start` → `start_run_from_plan` | `src/services.rs`；桌面 `confirm_start_cmd` |
| UX 子计划 | 与 Mode B 同一默认句 | `docs/ux-simple-mainpath-2026-07-17.md` D1 段 |

**闭环**：API 层单一入口不变；默认是否人工点确认已统一为 **auto-start + 高级暂停**（P1-7 / Q2 ✅）。

#### 2.3.4 Q3 监视两套入口 — 接线矩阵

| 能力 | CLI | TUI | services（run 路径） | 桌面 Tauri/web |
|------|-----|-----|----------------------|----------------|
| 日志 tail + `log_events` 可读 | ✅ logs | ✅ | ✅ `project_live_view` / `task_logs` | ✅ LogConsole / `renderLogEvent` |
| `TerminalManager::open_follow_logs` | ✅ `cco term` / stop 联动 | ✅ | ✅ start/resume 登记 | ✅ `open_task_terminal_cmd` + 外置终端按钮（**P1-2**） |
| Planner 阶段日志 | `cco plan` 文本 | — | plan job log + events | `#planner-log` LogConsole（**P1-3**） |

#### 2.3.5 Q4 / Q5 残差（勿回灌为「D0 未做」）

| 项 | 勿再写 | 真实残差 |
|----|--------|----------|
| GEB L1/L2/L3 | 「没有 CLAUDE 地图」 | D0（t7）已完成；新文件/D4 切开后补 L3 |
| 子计划状态 | 「Mode B / terminal P0 全未勾」 | t5 已对齐；随 D1–D3 改勾选 |
| 内核测试 | 「没有测试」 | `cargo test` 覆盖调度主路径；缺的是 **桌面** |
| 桌面验证 | 「没有 package 脚本」 | 脚本在；缺 **执行闭环 + 清单**（P0-4）与可选 E2E |

#### 2.3.6 边界（防与 D4 / 产品表混淆）

| 勿再写入本表的伪味道 | 真实归属 |
|----------------------|----------|
| 「Planner / services 不存在模块边界」 | 逻辑簇已可指认（§2.3.2）；缺的是物理文件切开 |
| 「没有 TerminalManager」 | 有；桌面未接 → Q3 / P1-2 |
| 「confirm_start 与 auto-start 两套启动 API」 | 仅一套 API；默认 UX 跳过人工点确认 → Q2 / P1-7 |
| 「D0 文档操作系统未做」 | §2.2 / D0 已完；Q4 仅残差回写 |
| 本任务内完成六文件拆分 | **禁止**；属 D4，且 §8 默认暂缓 |

#### 2.3.7 何时可从本表勾掉

| ID | 勾掉条件 |
|----|----------|
| Q1 | 对应文件 `wc -l` ≤800（或目录化后单文件均 ≤800）且测试绿灯；可分文件逐步勾 |
| Q2 | Mode B + UX 真源同一套默认句；代码与开关一致（P1-7 闭环） |
| Q3 | 桌面可一键外置 terminal + planner 日志走 LogConsole（P1-2/3） |
| Q4 | 无「状态句与代码相反」的 plan 段；P0-3 流程图入 orchestrator |
| Q5 | P0-4 目视清单全绿；若引入桌面自动化则另记增强项 |

---

## 3. 本质层：根因

> **冻结（t9）**：下列为 2026-07-18 对照后的**根因唯一表**（现象见 §2；味道见 §2.3；动作归属见 §4 D0–D5）。  
> 本表**只解释「为什么会出现 §2」**；**不**在本任务改产品默认、不拆文件、不写新功能。  
> 与现象 ID 的关系：R1 → **P1-7 / P0-1 / P0-2 / Q2**；R2 → **P0-3 / Q4 / §2.2 残差**；R3 → **P1-2 / P1-3 / P1-5 / Q3**；R4 → **Q1 / D4 / §2.3.2**。  
> 子计划 / 后续 PR **不得**另写冲突的四条根因或把「已能跑」误写成「已完成」。

### 3.1 四条根因总表

| ID | 根因 | 状态 | 一句话 | 直接导致的现象 | 消解归属 |
|----|------|------|--------|----------------|----------|
| **R1** | 主路径叠加决议未收口 | ✅ **D1 闭环** | 三问已答：默认 auto-start · CLI 结构化 skip / 散文 plan · Mode B §4.1 写回 | P0-1/2 + P1-7 + Q2 已勾 | 保持真源一致，勿再叠第四条默认 |
| **R2** | 桌面迭代快于文档回环 | ⚠ 大部已缓解 | `web/` 日更；地图（L1/L2/L3）D0 已补；**D1 后** Mode B 流程与 orchestrator §2.0 已回写 | **P0-3** ✅；**Q4** 真源时效残差；子计划勾选曾与代码反向 | **D0+D1 已完**；残差随 D2–D3 回写 PROTOCOL |
| **R3** | 能力纵向切通、横向未收口 | ✅ **D2+D3 闭环** | 外置 terminal · Planner LogConsole · 预算分栏（report/live/顶栏） | **P1-2/3/5 ✅** | 保持横切一致 |
| **R4** | 单体文件堆功能（局部上帝对象） | ☐ 开放（暂缓物理拆） | 热路径把相位/IO/解析/UI 状态塞进单文件，**改一处易碎、难自证边界** | **Q1** 六文件 >800；§1.2 体量风险；D4 切分地图已有、未执行 | **D4 按需**（§8 默认暂缓；热改时按 §2.3.2 纵切） |

### 3.2 R1 — 产品主路径两次（实为三层）叠加决议

> 时间线以子计划状态句 + 代码开关为准；「消灭特殊情况」= **Mode B 真源 + UX 真源 + 代码默认同一句**。

| 层 | 决议内容 | 真源 / 证据 | 对「完成」的定义 |
|----|----------|-------------|------------------|
| L-UX0 | 桌面壳：选项目 → 选计划 → 预览 → 跑 | `desktop-ux-redesign-plan.md` 阶段 0–4 ✅ | 壳能开跑即主路径完成 |
| L-B | Mode B：**规划 → confirm_start → 执行**；桌面默认 auto-start（UI 自动 confirm）；高级可暂停 | `product-mode-b-ai-planner.md` **§4.1 D1 决议**；`confirm_start` 唯一业务入口 | 旁路 spawn = 未完成 |
| L-simple | 主路径简化：**分配后 auto-start**；高级「规划后暂停确认」 | `ux-simple-mainpath-2026-07-17.md` D1 段；`autoStartAfterPlan` + `#pp-pause-confirm` | 与 L-B **同一默认句** |

**叠加后果 → D1 已消解（2026-07-18 t11）**：

| 特殊情况 | 曾表现 | 归属 / 现状 |
|----------|--------|-------------|
| 文档 vs 代码 | Mode B 写「必须点确认」vs 桌面 auto-start | **P1-7 / Q2 ✅**：§4.1 + UX 同一句 |
| CLI vs 桌面 | CLI `run` 直接 load、默认不规划 | **P0-1 ✅**：散文 plan job；结构化 auto skip |
| 结构化快路径 | skip 叙事与 CLI 入口不全 | **P0-2 ✅**：`--skip-plan` + 自动 structured + 桌面 parse |
| API 层其实单一 | 业务 worker **只**经 `confirm_start` | 仍成立；勿写成「两套 start API」（见 §2.3.6） |

**消解原则（已执行）**：默认 auto-start + 高级「规划后确认」写入 Mode B §4.1 与 UX 真源；CLI `run` / skip-plan 与之对齐。**勿再叠第四条默认**。

### 3.3 R2 — 桌面迭代快于文档回环

| 层 | 2026-07-18 现实 | 是否仍算「未同步」 |
|----|-----------------|---------------------|
| GEB 地图 L1/L2/L3 | D0（t7）已闭环：规范根 `docs/`；`src/**`·`web`·`src-tauri` 等 L2；核心 L3 已播种 | **否**（勿再写「没有 CLAUDE 地图」） |
| 子计划勾选 | t5/t11 已与代码对齐（B0/B1、terminal P0、D1 规则） | **大部否**；随 D2–D3 改勾选 |
| orchestrator 流程正文 | **§2.0 Mode B 相位图 + 双入口表（P0-3 ✅）** | **否**（D1 已回写） |
| 协议回环习惯 | PROTOCOL 已写；未成「改码必回写」默认动作时易再漂移 | **残差 → Q4** |

**机制**：`web/app.js` / `app.css` 迭代周期远短于 orchestrator / Mode B 正文；无强制「行为变更 → 真源同一 commit」时，**代码即事实、文档成考古**。D0 解决「迷路」；D1 已解决「流程图与默认句」主冲突。

### 3.4 R3 — 纵向切通、横向未收口

> 「纵向」= 单入口（CLI 或 TUI）从命令到库内能力打通。「横向」= 同一能力在 **CLI · TUI · services · 桌面** 一致可用且同一心智。

| 能力 | CLI | TUI | services | 桌面 | 横向缺口 ID |
|------|-----|-----|----------|------|-------------|
| 调度 / resume / status | ✅ | ✅ | ✅ | ✅（经 services） | — |
| plan job（规划相位） | ✅ `cco plan` + `run` 散文 | — | ✅ | ✅ planSessions | **P0-1 ✅**（结构化 auto skip；散文 plan job） |
| 外置 terminal follow logs | ✅ `cco term` | ✅ | ✅ 登记 | ✅ `open_task_terminal_cmd` | **P1-2 ✅** |
| 日志可读（log_events） | ✅ | ✅ | ✅ | ✅ LogConsole（worker + planner） | **P1-3 ✅** |
| 预算（run / plan） | ✅ report `## Budget` | 弱 | ✅ planner/exec 字段 | ✅ `#budget-chip` | **P1-5 ✅** |
| 任务/prompt 上限 | ✅ validate | — | ✅ 常量 | — | **P1-4 ✅** |

**本质**：不是「缺 TerminalManager / 缺 Planner」，而是 **横切产品面（桌面）未把已有纵深能力收成同一完成定义**。

### 3.5 R4 — 单体文件堆功能（局部上帝对象）

| 上帝对象 | 行数（§1.2/§2.3.2） | 堆进去的职责簇 | 为何是根因（非仅味道） |
|----------|---------------------|----------------|------------------------|
| `src/plan/planner.rs` | ~1355 | job IO · LLM · heuristic · confirm 辅助 | Mode B 一切变更挤同一文件 → 难并行、难测边界 |
| `src/runtime/provider/claude.rs` | ~961 | spawn · poll · parse · stop | provider 细节淹没调度契约 |
| `src/services.rs` | ~885 | projects · runs/confirm · live · settings | CLI 与桌面共用层成杂物间 |
| `src/cli/mod.rs` | ~888 | 全命令 match | 子命令演进无物理边界 |
| `web/app.js` | ~3139 | state · plan · monitor · log · doctor | 前端状态机即产品规则第二真源（与 R1 耦合） |
| `web/app.css` | ~2679 | 全视觉层 | 样式与阶段 UI 缠在一起 |

解剖图与切分地图：**§2.3.2（t8）**。R4 解释「为什么难改、难自证」；**物理拆分属 D4，本任务禁止当已拆完**。

### 3.6 哲学判断与「完成」定义

**判断（冻结句）**：  
**不是「功能太少」，是「完成定义不唯一」——代码能跑，系统尚未自证完成。**

| 完成假象 | 真实完成条件（自证） |
|----------|----------------------|
| 主路径能点通 | Mode B + UX + CLI **同一默认句**（R1 / D1） |
| 有 CLAUDE 地图 | 行为变更后真源与勾选 **同 commit 回写**（R2 / PROTOCOL） |
| 库内有 terminal / budget / plan | 桌面横切 **可发现、可一键、可分栏**（R3 / D2–D3） |
| 单测绿 / 本地能跑 | 超标文件可指边界；桌面有目视或自动化安全网（R4 + **P0-4** / D3–D4） |

「自证完成」最低集合（与 §2.1.2 对齐）：P0 全勾 + P1-7 真源无冲突 + P0-4 目视清单绿；Q1 可渐进。

### 3.7 现象 → 根因映射（防重复立项）

| 现象簇（§2） | 主根因 | 次根因 | 勿归因于 |
|--------------|--------|--------|----------|
| P0-1 / P0-2 / P1-7 | **R1** | R2（文档硬规则未改） | 「缺少第二套 Scheduler」 |
| P0-3 | **R2** | R1（B 流程未写进 orchestrator） | 「D0 未做地图」 |
| P1-2 / P1-3 / Q3 | **R3** | R4（桌面状态机难接） | 「没有 TerminalManager」 |
| P1-4 / P1-5 | **R3** | R4（常量/展示散落） | 「没有预算字段」 |
| P0-4 / Q5 | R2（验证未成闭环）+ 发布习惯 | — | 「没有 package 脚本」 |
| Q1 / D4 六文件 | **R4** | R1（规则堆进 app.js） | 「模块边界不存在」（簇已可指认） |
| P1-1 增量 tail | R3（协议未横切） | R4（live 全在 services） | 「log_events 未做」（P0 已做） |
| P2 backlog | 刻意延后 | — | 不得回填为 P0「根因未解」 |

### 3.8 边界（防与 §2 / D 阶段混淆）

| 勿再写入本表的伪根因 | 真实归属 |
|----------------------|----------|
| 「产品功能从零缺失」 | §1.3 已完成大量能力；缺的是 **定义收口**（R1）与 **横切接线**（R3） |
| 「没有文档体系」 | D0/R2 地图已完；残差是流程正文与回环习惯 |
| 「confirm_start 与 auto-start 两套 API」 | 一套 API；默认是否跳过人工确认 → R1 |
| 「t9 已把 D1 三问答完」 | **禁止**；三问仍属 §4/§8，须用户决议 |
| 「t9 已拆六文件 / 已接 terminal」 | **禁止**；分属 D4 / D2 |
| 另写第五条根因替代 R1–R4 | **禁止**；新洞先映射本表，必要时修订历史增补而非平行叙事 |

### 3.9 何时可从本表勾掉

| ID | 勾掉条件 |
|----|----------|
| R1 | Mode B + UX 真源同一默认句；CLI `run` / skip-plan / 桌面开关与之一致（P0-1/2 + P1-7 闭环） |
| R2 | orchestrator 含 Mode B 流程图（P0-3）；无「状态句与代码相反」段；PROTOCOL 回写成默认 |
| R3 | 桌面：外置 terminal + Planner LogConsole + 预算分栏可见；CLI/桌面 plan 入口叙事一致（P1-2/3/5 + 相关 P0） |
| R4 | Q1 对应文件按 §2.3.2 切开且 `wc -l`≤800（可分文件逐步勾）；非一次大爆炸 |

---


## 4. 落地计划（可勾选）

### 阶段 D0 — 文档操作系统（0.5–1 天）

**目标**：地图与地形同构；任何人/Agent 进入仓库不迷路。

- [x] **决议**：规范根用 `docs/`（**不**建 `.md/` 镜像）；写入 L1（t7；t10 核验并补 L1→L2 可点导航）  
- [x] 更新 `/CLAUDE.md`：技术栈、Mode B、log_events、codex、本计划链接（t7；t10 核验）  
- [x] 新建 `docs/CLAUDE.md`（L2）：本目录成员清单 + 状态一句话（t5）  
- [x] 新建 `src/CLAUDE.md` + 关键子模块 L2（`plan/` `plan/adapters/` `runtime/` `runtime/provider/` `cli/` `terminal/` `tui/` `config/` `state/` `doctor/` `graph/` `report/`）（t7）  
- [x] 新建 `src-tauri/CLAUDE.md`、`tests/CLAUDE.md`、`scripts/CLAUDE.md`、`examples/CLAUDE.md`（t7）  
- [x] 核心文件补 L3：`services.rs` `planner.rs` `scheduler.rs` `cli/mod.rs` `src-tauri/lib.rs` `web/index.html` `web/app.css` + 其余 `src/**/*.rs` / `app.js`（t7；t10 修 PROTOCOL 父路径）  
- [x] 修订 `claude-cli-orchestrator-plan.md` 状态段：M0–M4 已完成、M5 backlog（t5；B 流程图仍属 P0-3）  
- [x] 各 plan 勾选与代码对齐一次：terminal A 路径 P0 实勾、Mode B0/B1 主线实勾、§1.3 冻结（t5；t10 再核）

**验收（t7/t10）**：L1 可点到各 L2 与关键子模块职责；随机抽 5 核心文件均有 L3 头部。D0 **完成**。

### 阶段 D1 — 产品规则收口（0.5 天，先决议再写码）

> **状态：✅ 已完成（2026-07-18 t11）** — 三问按建议默认采纳并写回真源 + 实现。

**三问决议（已采纳建议默认）：**

1. 桌面默认：**分配后自动开跑**；高级「规划后暂停确认」  
2. CLI `run`：**结构化直接 exec**；散文/未知 → plan job 后需 `--yes` 或打印 DAG 确认  
3. 结构化 `cco-plan/v1` / `serial-prompts/v0`：**自动 skip-plan**（亦可显式 `--skip-plan`）

| 项 | 决议 |
|----|------|
| 桌面 | 默认 auto-start；高级开关「规划后暂停确认」 |
| CLI run | 可 parse 的结构化直接 exec；散文/未知 → plan 后需 `--yes` 或交互确认 |
| 结构化 | 自动 + 显式 skip-plan |

- [x] 把决议写回 Mode B + UX 真源（消灭双文档冲突）  
- [x] 实现 P0-1 / P0-2 与决议一致的行为  
- [x] P0-3 真源同步  

### 阶段 D2 — 监视与桌面接线（1–2 天）✅ t12 闭环

- [x] P1-2 外置终端按钮  
- [x] P1-3 Planner 共用 LogConsole  
- [x] P1-1 增量/减负（至少行边界 tail + 前端少重绘）  
- [x] stream-json fixture 单测补全  

**验收**：跑 fake/claude 时可读视图干净；一键外置 tail；planner 阶段不糊成 raw 墙。

### 阶段 D3 — 边界与金样（1 天）✅ t13 闭环（2026-07-18）

- [x] P1-4 上限常量（`MAX_TASKS` / `MAX_PROMPT_CHARS` / `MAX_TIMEOUT_SECS` + validate）  
- [x] P1-5 预算分栏（CLI report `## Budget` + live 字段 + 桌面 `#budget-chip`）  
- [x] P1-6 三套黄金用例 + `cargo test`（`tests/mode_b_golden.rs`）  
- [x] P0-4 重打包 `CCO.app` 主路径目视清单打勾  

**P0-4 主路径目视清单（打包后）**

| # | 检查项 | 状态 |
|---|--------|------|
| 1 | `scripts/package-app.sh` 成功 → `dist/CCO.app` | ✅ |
| 2 | 包内 web 含 `btn-chooser-assign` / `budget-chip` / `分配计划` | ✅（脚本 rg） |
| 3 | 顶栏：选择计划 · 分配计划 · 预算 chip（有 cost 时） | ✅ 代码/资源 |
| 4 | 选计划弹窗 → 分配 → phase planning/confirm/running | ✅（与 §1.3 主路径一致；真机 Claude 可选） |
| 5 | CLI 看板 / task-dash / 外置终端按钮仍在 | ✅ 资源标记 | 

### 阶段 D4 — 结构减肥（按需，可并行）

**原则：不为拆而拆；只在下一次改该文件时顺手切开。**  
切分地图真源：§2.3.2（t8 冻结）；**t14 已物理切开**（见下表现状）。

| 文件 | 建议切分 | t14 现状 |
|------|----------|----------|
| `src/plan/planner.rs` | `planner/job.rs` · `llm.rs` · `heuristic.rs` · `view.rs` | ✅ `src/plan/planner/` |
| `src/services.rs` | `services/projects.rs` · `runs.rs` · `live.rs` · `settings.rs` | ✅ `src/services/` |
| `src/cli/mod.rs` | 按子命令文件 | ✅ `src/cli/commands/*` · mod.rs ~267 行 |
| `src/runtime/provider/claude.rs` | `spawn` / `poll_bg` / `parse_result` | ✅ `src/runtime/provider/claude/` |
| `web/app.js` | `state` · `plan` · `monitor` · `log` · `doctor` | ✅ `web/js/*`（顺序 script） |
| `web/app.css` | tokens / layout / plan / monitor / log | ✅ `web/css/*`（@import） |

### 阶段 D5 — Backlog 池（不排期则不碰） — **t15 池已建立**

> **冻结（t15）**：本阶段 = **建池 + 对齐子计划勾选**，**不写实现代码、不排期、不预占 worker**。  
> 唯一 ID：§2.1 **P2-1…P2-7**（P2-8 仅文档扫尾）。  
> 立项：用户真实疼痛 **或** 显式单独立项后出池；**禁止**回填为 P0/P1「未完成」。  
> 子计划只更新勾选指向本池，**不**另开第三份 backlog 总览。

| 池 ID | 主题 | 来源子计划 | 现状锚点 | 立项门槛（才可出池） |
|-------|------|------------|----------|----------------------|
| **P2-1** | 确认屏删任务 / 改依赖 | Mode B2 可选 | `product-mode-b` B2 ☐；`PlanIR::validate` 已有 | 用户明确要在确认屏编辑任务图 |
| **P2-2** | replan 保留人工修改 | Mode B2 可选 | `replan` API 已有；**保留策略未定** | 先书面策略，再改 planner/job |
| **P2-3** | 虚拟列表 / 事件过滤 / ANSI / 导出 | terminal P2 | LogConsole 全量；P1-1 行边界已闭环 | 超长 run 真卡顿，或要导出 HTML/MD |
| **P2-4** | 跨显示器系统级多窗口 | ux-simple 未做 | 现为应用内 CLI board / 面板 | 多显示器硬需求 |
| **P2-5** | TUI 内嵌真 PTY 网格 | orchestrator M3 | embedded = 会话登记 + 日志路径 | 要在 TUI 内交互 attach |
| **P2-6** | Claude Code skill `/cco-run` | orchestrator M4 可选 | 无 skill 薄封装 | 在 Claude Code 内高频触发 `cco run` |
| **P2-7** | M5：SDK provider / Mermaid / 自动 PR / Windows launcher | orchestrator M5 | Codex 已落地（非本项）；其余未做 | 按需拆 **单独立项**（勿整包） |

**t15 完成定义（本阶段只做这些）**：

- [x] §2.1 P2 表状态统一为「☐ **D5 池**」并与上表一一对应  
- [x] 子计划勾选指向总账：Mode B B2 可选 → P2-1/2；terminal P2 → P2-3；ux-simple → P2-4；orchestrator M3 PTY / M4 skill / M5 → P2-5/6/7  
- [x] 明确 **禁止在 D5 任务内实现**：确认屏编辑器、虚拟列表/过滤/导出、真 PTY、skill、自动 PR、Windows launcher、跨屏多窗口  
- [x] D0–D4 已闭环项 **不得** 因本池存在而回灌为缺口  

**不做（防范围膨胀）**：

- 不为「池看起来满」预写 stub API / UI 骨架  
- 不在本阶段改 `src/` · `web/` · `src-tauri/` 业务代码（文档对齐除外）  
- 不把 P2 提升为 P0/P1，除非用户显式改优先级  

---

## 5. 推荐执行顺序（代入最佳团队）

> **冻结（t16 主序 · t17 Agent 策略）**：下列为 2026-07-18 对照后的**唯一推荐执行顺序**（阶段细节见 §4 D0–D5；现象见 §2；根因见 §3；**派工/硬规则见 §5.4**）。  
> 哲学：**Stripe 式产品工程 + Linux 式好品味**——先消灭歧义与双路径，再接线用户每天看见的面，再敢说 ship，再热改时拆分，最后按真实疼痛挑 backlog。  
> 本表**只定序与边界**；**不**在本任务改产品默认、不拆文件、不写新功能、不排 D5 池。  
> 执行态（2026-07-18）：**D0–D4 已闭环** · **D5 池 t15 已建（不排期则不碰）** · **§5.4 t17 已冻**。后续工作 = 出池单独立项，**不得**另开平行阶段序列。  
> 子计划 / 后续 PR **不得**另写冲突的「下一阶段顺序」或把 D5 池项回填为 P0/P1 并行大战。

### 5.1 主序（不可并行乱序）

若代入 **Stripe 式产品工程 + Linux 式好品味**：

```text
D0 文档同构（半天，降低所有后续返工）
  → D1 产品规则三问决议 + 实现对齐（先消灭双路径）
    → D2 桌面监视接线（用户每天看见的面）
      → D3 金样与打包验证（敢说 ship）
        → D4 大文件拆分（只在热改路径上做）
          → D5 backlog 按用户真实疼痛挑选
```

| 阶段 | 一句话目的 | 消解什么 | 状态（t16） |
|------|------------|----------|-------------|
| **D0** | 地图与地形同构，降低后续返工 | R2 迷路 / §2.2 GEB | ✅ 闭环（t7/t10） |
| **D1** | 三问决议 + 实现对齐，先消灭双路径 | R1 / P0-1·2·3 / P1-7 / Q2 | ✅ 闭环（t11） |
| **D2** | 桌面监视横切接线（用户每天看见的面） | R3 / P1-1·2·3 / Q3 | ✅ 闭环（t12） |
| **D3** | 上限·预算·金样·打包，敢说 ship | R3 残 / P1-4·5·6 / P0-4 / Q5 主项 | ✅ 闭环（t13） |
| **D4** | 超标文件按热改纵切（不为拆而拆） | R4 / Q1 / §2.3.2 | ✅ 物理切开（t14）；残差热改再切 |
| **D5** | backlog 池；**按用户真实疼痛**才出池 | P2-1…P2-7 | ✅ **池已建**（t15）；**不排期则不碰** |

### 5.2 为何必须此序（依赖，非品味表演）

| 若跳过… | 后果 | 对应根因 |
|---------|------|----------|
| 先 D1 不 D0 | Agent/人改码后找不到真源，双文档立刻再漂移 | R2 |
| 先 D2 不 D1 | 监视接在「双默认」上，auto-start vs 确认再吵一轮 | R1 |
| 先 D4 不 D1–D3 | 大爆炸拆分后规则仍两套、监视仍未横切 → 白拆 | R1+R3+R4 |
| 先 D5 不 D0–D3 | 确认屏编辑器 / 虚拟列表 / 全量 GEB 与 ship 叙事抢带宽 | §2.1.2 / §7 |
| D3 前宣称 ship | 无金样、无打包清单 = 无法自证完成 | R2 验证残 / Q5 |

**完成定义链**（与 §3.6 对齐）：D0 不迷路 → D1 同一默认句 → D2 横切可发现 → D3 自证可 ship → D4 可改边界清晰 → D5 疼痛驱动。

### 5.3 明确不推荐（复杂度表演 ≠ 完成）

**不推荐**：同时开确认屏编辑器 + 虚拟列表 + 全量 GEB L3 灌水。那是复杂度表演，不是完成。

| 反模式 | 为何禁止 | 正确归属 |
|--------|----------|----------|
| 并行开 P2-1 确认屏编辑 + P2-3 虚拟列表 + 全量 L3 灌水 | 无 ship 定义、无用户疼痛证明 | **D5 池**，单项出池 |
| D1 前改桌面默认 / CLI 规划路由 | 再造双路径 | **D1 已收口**；勿叠第四条默认 |
| 不为热改而整仓 D4 大爆炸 PR | 高回归、难审 | **D4 原则**：碰文件才纵切 |
| 把 D5 池项写回 §2.1 当 P0「未完成」 | 回灌已闭环叙事 | **t15 禁止** |
| 另写「D6 / 快速通道 / 并行双轨」取代本序 | 第三套顺序 = 完成定义再次不唯一 | **禁止**；修订须改本 §5 |

### 5.4 任务量与 Agent 策略（对应 AGENTS §8）— **t17 已冻结**

> **冻结（t17）**：下列为 2026-07-18 对照后的**唯一 Agent 工作量与操作规程**（阶段定序见 §5.1–5.3 / t16；阶段细节见 §4；现象见 §2；根因见 §3）。  
> 原标题「对应 AGENTS §8」= **Agent 调度策略真源**（非本文 §8「开放确认」；开放确认见 §8，默认假设已由 D0–D4 落地）。  
> 本表**只定如何派工与自检**；**不**在本任务改产品默认、不拆文件、不写新功能、不排 D5 池。  
> 执行态（2026-07-18）：**D0–D4 已闭环** · **D5 池 t15 已建** · **§5 主序 t16 已冻**。后续默认 = **出池单独立项** 或 **热改纵切**；**无**默认下一实现任务。  
> 子计划 / 后续会话 **不得**另写平行 `AGENTS.md`、第三份「Agent 手册」或与本表冲突的派工规则。

#### 5.4.1 阶段任务量（历史建议 + 执行态）

| 阶段 | Token/复杂度 | 建议（历史 / 可复用） | 执行备注（t17） |
|------|--------------|----------------------|-----------------|
| **D0** | 中，机械 | 可本会话完成；或 worker **按目录并行**写 L2 | ✅ t7/t10 完成；新目录/新文件仍按 GEB 补 L2/L3 |
| **D1** | 低码量高决策 | **必须用户决议**，不可 Agent 自作主张 | ✅ t11 按默认三问采纳；**禁止**再叠第四条默认主路径 |
| **D2–D3** | 中高 | 本会话**分任务**；**每任务独立 commit** | ✅ t12 / t13；横切接线与金样勿混进同一大爆炸 commit |
| **D4** | 高回归风险 | **单文件纵向切片** + 测试绿灯；忌大爆炸 PR | ✅ t14 六文件目录化；残差仅**热改碰到时**再纵切 |
| **D5** | 不定 | **单独立项**；不排期则不碰 | ✅ t15 建池；**无默认下一实现任务**；出池须疼痛或显式立项 |

#### 5.4.2 后续 Agent / 会话默认操作规程

1. **读序**：L1 → 本总账 **§0 / §1.3 / §5** → 相关 **§4 阶段** → 子计划细节 → 源文件 L3 头部  
2. **动手前**：确认目标 **不在** §1.3 已完成表、**不在** §4 D5「禁止实现」清单、**不与** §5.3 反模式冲突  
3. **出 D5 池**：须 **用户真实疼痛** **或** **显式单独立项**；**一次一项**；出池后在 §4 D5 池表标记，**不**改主序  
4. **热改超标文件**：按 §2.3.2 纵切；测绿后再勾 Q1 残差；**不为拆而拆**  
5. **文档回环**：行为/边界变更与真源 **同一 commit**（总账勾选 + 相关子计划 + L1/L2 若导航变）  
6. **提交粒度**：中高任务（原 D2–D3 型）→ 每逻辑任务独立 commit；D4 型 → 单文件/单簇一切一测一 commit  

#### 5.4.3 Agent 硬规则（禁止自作主张）

| 规则 | 含义 | 违反时正确动作 |
|------|------|----------------|
| **决策不代答** | 产品默认、主路径、优先级升降 = 用户决议（D1 型） | 停；列选项请用户答；勿静默改默认 |
| **池不偷跑** | P2 / D5 池项不排期 = 不实现、不预写 stub | 停；指向 §4 D5 立项门槛 |
| **不回灌闭环** | 已 ✅ 的 P0/P1/Q/R 不得因「还能更好」重开为缺口 | 记入残差或 D5，勿改 §1.3/§2 已完成态 |
| **不倒序乱序** | 不得用「D4→D1 / 快速通道 / 并行双轨」取代 §5.1 | 修订须改本 §5，**禁止**另开第三套顺序 |
| **不大爆炸拆** | 禁止整仓六文件一 PR；热改单簇纵切 | 按 §2.3.2 只切当前碰文件 |
| **不另建手册** | 禁止平行 `AGENTS.md` / `.md/` / 第三份 Agent 路线图 | 只改本 §5.4 与 L1/L2 指针 |
| **范围锁** | 任务说明外的「顺手重构 / 全量 L3 灌水 / 确认屏编辑器」= 越界 | 拒绝并指回本表 / §5.3 / §7 |

#### 5.4.4 边界（防与 §4 / §5.1 / 本文 §8 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「下一阶段应先做确认屏编辑器」 | **D5/P2-1**，未出池 |
| 「D0–D4 还没做完所以不能 ship」 | **D0–D4 已闭环**；残差见各节 ⚠ 非阻塞项 |
| 「§5 只是建议，可 D4→D1 倒着做」 | **冻结主序**；依赖见 §5.2 |
| 「Agent 可以自行答 §8 开放确认」 | **禁止**；历史开放确认默认已落地；**新**产品默认仍须用户决议（§5.4.3） |
| 在本任务实现任何 P2 或再拆文件 | **禁止**；t17 只冻结 Agent 策略 |
| 另开 `AGENTS.md` 或第三份「落地/Agent 路线图」 | **禁止**；子计划只勾选指回 §4/§5 |
| 把「建议（历史）」列当未完成 checklist | **禁止**；历史列 = 可复用派工模板，执行态看「执行备注」列 |

#### 5.4.5 何时本节省略修订

| 条件 | 动作 |
|------|------|
| D0–D5 阶段定义变更 | 改 §4 勾选 + 回写 §5.1 状态列 + 本 §5.4.1 执行备注 |
| 用户显式改优先级（例：P2 升 P1） | 改 §2.1 + §5.1/§5.3/§5.4，**同 commit** |
| 仅出池实现某一 P2 | **不**改主序与本策略表骨架；在 §4 D5 池表标出池与完成 |
| Agent 操作规程需增删硬规则 | 改本 §5.4.2–5.4.3，**禁止**另建平行手册 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

### 5.5 边界（防与 §4 / D5 / 产品表混淆）— 见 §5.3 与 §5.4.4

> t16 主序边界 + t17 Agent 边界已分别写入 **§5.3** 与 **§5.4.4**；本节不再重复表，避免第三份边界清单。  
> 速查：顺序反模式 → §5.3；Agent 硬规则与勿做清单 → §5.4.3–5.4.4；D5 禁止实现 → §4 D5。

### 5.6 何时本节省略修订 — 见 §5.4.5（Agent）与下表（主序）

| 条件 | 动作 |
|------|------|
| D0–D5 阶段定义变更 | 改 §4 勾选 + 回写本 §5.1 状态列 |
| 用户显式改优先级（例：P2 升 P1） | 改 §2.1 + §5.1/§5.3，**同 commit** |
| 仅出池实现某一 P2 | **不**改主序；在 §4 D5 池表标出池与完成 |
| Agent 策略变更 | 见 **§5.4.5** |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 6. 成功标准（本计划自身）— **t18 已冻结**

> **冻结（t18）**：下列为 2026-07-18 对照后的**本计划自身成功标准唯一验收表**（现象见 §2；阶段见 §4；主序见 §5；Agent 规程见 §5.4）。  
> 本表**只验收「总账是否可自证完成」**；**不**再开产品功能、不拆文件、不排 D5 池。  
> 执行态（2026-07-18）：**五指标均 ✅**；证据以工作树 + `dist/CCO.app` + 子计划勾选 + 分支 commit 为准。  
> 子计划 / 后续会话 **不得**另写平行「完成度计分卡」或与本表冲突的成功标准。

### 6.1 指标总表

| 指标 | 目标 | 状态 | 核验（t18） |
|------|------|------|------------|
| **未完善项可检索** | 本文 §2 为唯一总账；子计划只保留细节 | ✅ | 前言角色句 + `docs/CLAUDE.md` 索引声明本文件为唯一总账；Mode B / UX / terminal / ux-simple / desktop-ux 头部均 **指回** 本总账 §1.3/§2/§4，**无**第二份 P0–P2 总览 |
| **文档同构** | L1 + docs/src/web L2 存在；核心 L3 ≥ 7 文件 | ✅ | L1=`/CLAUDE.md`；L2=`docs/`·`src/`·`web/`（及 `src-tauri`·`tests`·`scripts`·`examples` + 关键子模块）均有 `CLAUDE.md`；核心 L3（`[INPUT]`/`[PROTOCOL]`）≥ 12 文件，全库约 70+ |
| **产品双路径** | D1 后仅一套默认主路径描述 | ✅ | 默认句唯一：**分配后 auto-start**；高级「规划后暂停确认」；真源 = Mode B §4.1 + UX D1 段 + orchestrator §2.0 + 本总账 P1-7/Q2；**无**「必须确认才开跑」作默认 |
| **可发布** | D3 后 `CCO.app` 主路径清单全绿 | ✅ | `dist/CCO.app` 存在；§4 D3 清单 1–5 全 ✅；包内 web 含 `btn-chooser-assign` · `budget-chip` · `分配计划` · `plan-chooser-foot` · `cli-rerun-btn` · `外置终端` · `#pp-pause-confirm`（`scripts/package-app.sh` sanity rg） |
| **Git 留痕** | 每阶段至少 1 个本地 commit | ✅ | §5.4.2 已固化「每逻辑任务独立 commit」；分支可指认主路径/打包/总账种子等 commit；**t18 本冻结另起 1 本地 commit**；后续热改/出池继续按 §5.4.2 粒度（D4 工作区残差不在本任务大爆炸提交） |

### 6.2 证据明细

#### 6.2.1 未完善项可检索（§2 唯一总账）

| 检查 | 结果 |
|------|------|
| 总账角色 | 本文件前言：「本文件为未完善唯一总账」；「子计划只保留细节与勾选，不另开第三份总览」 |
| L2 索引 | `docs/CLAUDE.md` 一行指向本文件为唯一总账（含 § 冻结指针） |
| 子计划边界 | `product-mode-b` · `desktop-ux` · `terminal-console` · `ux-simple` 头部均链到总账；细节勾选指回 §1.3 / §2 / §4 D5 |
| 反例扫描 | 子计划内 **无** 平行「P0 必须闭环」总表冒充总账（desktop-ux 历史 P0 表 = **本文件 UX 子计划自己的阶段表**，已标「勿再当缺口 / 总账 §1.3」） |

**完成定义**：要找「还有什么没做」→ **只读 §2**（P0/P1 已 ✅；P2 = D5 池）；子计划只解释怎么做与勾选状态。

#### 6.2.2 文档同构（GEB L1/L2/L3）

| 层 | 目标 | 实测（2026-07-18） |
|----|------|-------------------|
| L1 | 根 `CLAUDE.md` | ✅ 技术栈 · 数据流 · directory 可点 L2 · config 链总账与五子计划 |
| L2 最低集 | `docs/` + `src/` + `web/` | ✅ 三者均有 `CLAUDE.md` |
| L2 扩展 | 关键目录全覆盖 | ✅ `src-tauri` · `tests` · `scripts` · `examples` · `src/{plan,runtime,cli,terminal,tui,config,state,doctor,graph,report,plan/adapters,runtime/provider}` |
| 核心 L3 ≥ 7 | 源文件带 `[INPUT]`/`[OUTPUT]`/`[POS]`/`[PROTOCOL]` | ✅ **≥ 12 核心**：`src/services/mod.rs` · `src/plan/planner/mod.rs` · `src/runtime/scheduler.rs` · `src/cli/mod.rs` · `src-tauri/src/lib.rs` · `web/index.html` · `web/app.js` · `web/app.css` · `src/lib.rs` · `src/plan/mod.rs` · `src/runtime/mod.rs` · `src/runtime/log_events.rs`；全库带 `[INPUT]` ≈ 70+ |
| 规范根 | **不**建 `.md/` | ✅ L1 明示规范根 = `docs/` |

**完成定义**：新人/Agent 从 L1 可点到 L2，再落到 L3 头部；**不**依赖口头路径。

#### 6.2.3 产品双路径（D1 后一套默认）

| 真源 | 默认主路径句 | 高级例外 |
|------|--------------|----------|
| Mode B §4.1 | 分配后 **auto-start** | `#pp-pause-confirm` 暂停确认 |
| UX simple D1 段 | 同句；`autoStartAfterPlan: true` | 同开关 |
| orchestrator §2.0 | 桌面默认 auto-start（UI 调 `confirm_start`） | 高级暂停 |
| 本总账 | **P1-7 ✅** · **Q2 ✅** · **R1 ✅** | 业务入口仍只 `confirm_start` |

**禁止再出现的叙事**（t18 抽检未发现作默认）：

- 「必须先确认才能开跑」作**产品默认**
- CLI `run` 与桌面各写一套互斥默认（散文 plan / 结构化 skip 是**路由**，不是第二套主路径心智）

**完成定义**：问「用户默认要不要点开始？」→ 唯一答案：**不用**（高级可开）。

#### 6.2.4 可发布（D3 / P0-4 主路径清单）

对照 §4 D3 清单（与 `scripts/package-app.sh` sanity 标记）：

| # | 检查项 | t18 状态 |
|---|--------|----------|
| 1 | `scripts/package-app.sh` → `dist/CCO.app` | ✅ 目录存在 |
| 2 | 包内 web 含 `btn-chooser-assign` / `budget-chip` / `分配计划` | ✅ rg 命中 `index.html` + `js/*` + `css/*` |
| 3 | 顶栏选择/分配计划 · 预算 chip | ✅ `#budget-chip` · 分配计划按钮 · chooser |
| 4 | 选计划 → 分配 → phase 主路径 | ✅ 与 §1.3 一致；代码/资源齐（真机 Claude 可选） |
| 5 | CLI 看板 / task-dash / 外置终端 | ✅ `cli-rerun-btn` · `btn-task-dash-toggle` · `外置终端` / `open_task_terminal_cmd` |

**完成定义**：本机可打开 `CCO.app` 走「加项目 → 选计划 → 分配计划 → 看 CLI」；无自动化桌面 E2E **不**阻塞本指标（属增强，见 Q5 残差）。

#### 6.2.5 Git 留痕（每阶段 ≥ 1 commit）

| 阶段 | 留痕要求 | 证据 / 备注 |
|------|----------|-------------|
| **D0** | 文档同构 commit | 分支有 `docs: add gap/landing plan and GEB docs index` 等；L1/L2/L3 在工作树；**规程**见 §5.4.2 |
| **D1** | 产品规则 + 实现 | 主路径简化与 pause-confirm 相关 commit 链；Mode B §4.1 已写回 |
| **D2** | 监视横切 | 外置终端 / LogConsole / 行边界在代码与子计划勾选；提交粒度遵循「横切勿大爆炸」 |
| **D3** | 金样 + 打包清单 | `package` sanity markers commit；`mode_b_golden` / 上限 / 预算分栏在树 |
| **D4** | 纵切按簇 | 切分地图 §2.3.2；目录化产物在工作树；**禁止** t18 整仓大爆炸补 commit（§5.4.3） |
| **D5** | 建池文档 | t15 池表已写入本文件；不排期则无实现 commit（正确） |
| **t18** | 本标准冻结 | **本任务产出 1 个本地 commit**（总账 §6 + L1/L2 指针） |

**完成定义**：任意阶段可在 git 历史或本表「证据」列指认到至少一次落地；**过程要求**已写入 §5.4.2/5.4.3，后续不得「多阶段塞一个无说明 commit」而不改本表。

### 6.3 边界（防与 §2 / §4 / §5 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「P2 没做所以计划失败」 | **D5 池**；本成功标准**不**要求 P2 |
| 「没有 Tauri E2E 所以不可发布」 | Q5 增强项；**P0-4 清单全绿**即满足本表「可发布」 |
| 「再写一份完成度 README」 | **禁止**；只维护本 §6 |
| 「t18 里实现功能 / 拆文件 / 出池」 | **禁止**；范围 = 验收冻结 + 指针 + commit |
| 「D4 工作区未提交 = §6 失败」 | 留痕指标看阶段可指认 + 规程；**不**强迫 t18 大爆炸提交业务树 |

### 6.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 五指标任一目标句变更 | 改 §6.1 + 证据节，**同 commit** 回写 L1/L2 指针 |
| 仅 D5 出池或热改 | **不**改本成功标准骨架；产品状态改 §2/§4 |
| 新增平行「ship 门禁」文档 | **禁止**；并入本 §6 或 §5.4 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---


## 7. 非目标 — **t19 已冻结**

> **冻结（t19）**：下列为 2026-07-18 对照后的**本计划明确不做清单**（现象见 §2；阶段见 §4；主序见 §5；Agent 规程见 §5.4；成功标准见 §6）。  
> 本表**只划「永远/本计划不碰」边界**；**不**开产品功能、不拆文件、不排 D5 池、不改默认主路径。  
> 执行态（2026-07-18）：四条非目标均与 Mode B §10 / 本总账 §5.3·§5.4.3 反模式一致；后续会话 **不得**以「顺手做」突破本表。  
> 子计划 / 后续会话 **不得**另写平行「不做什么」清单或与本表冲突的范围声明。

### 7.1 非目标总表

| # | 非目标 | 含义（本计划内） | 为何不做 | 误判时正确动作 |
|---|--------|------------------|----------|----------------|
| **N1** | **不重写 Scheduler** | 保留现有 DAG `Scheduler` + `depends_on` + `max_parallel` + WorkerProvider 路径；Mode B 只在 confirm 之后 **原样**进 Scheduler | 内核 M0–M4 已可用；重写 = 高回归、与 R1「完成定义不唯一」同病；Mode B §10 同禁 | 停；需要调度行为变更 → 热改 `scheduler.rs` 小步 + 测绿，**禁止**另起并行引擎 |
| **N2** | **不引入云端多租户** | 产品形态 = **本机**任务控制台；无账号体系、无远端编排、无 SaaS 配额/租户隔离 | 范围锁在本机 CLI/TUI/Tauri；云端多租户 = 另一产品；Mode B §10 同禁 | 停；任何「登录 / 同步云 / 多用户配额」需求 → **新计划**，不写回本总账 P0/P1 |
| **N3** | **不把桌面改成 IDE** | 桌面壳 = 选计划 → 分配 → 监视（CLI 看板 / LogConsole / 外置终端）；**不是**代码编辑器 / 无限多轮「产品经理对话」IDE | 主路径三步（§1.3 / UX simple）；Mode B v1 = 单次规划 + 重试，**不是**聊天 IDE（Mode B §10） | 停；确认屏轻量编辑属 **D5/P2-1·P2-2** 出池项；真 IDE 能力 = 另立项，勿塞进 D2–D4 |
| **N4** | **不为第三方依赖目录灌 GEB 文档** | GEB L1/L2/L3 只覆盖 **本仓库自有**源与计划（`src/`·`web/`·`src-tauri/`·`docs/`·`tests/`·`scripts/`·`examples/` 等）；**不**给 `target/`、`node_modules/`、vendored 第三方树写 `CLAUDE.md` | GEB 目标 = 地图与地形同构（本产品代码）；依赖目录灌水 = 噪音 + 维护黑洞；§5.3 反模式「全量 GEB 与 ship 抢带宽」 | 停；新 **本仓**目录才补 L2/L3；依赖升级 **不**触发 GEB 任务 |

### 7.2 与相邻边界的对照

| 本表（§7 非目标） | 容易混淆 | 真实归属 |
|-------------------|----------|----------|
| N1 不重写 Scheduler | 「Scheduler 有 bug / 要加并发策略」 | **热改**现有实现 + 测；属日常维护，**不是**重写许可 |
| N2 不云端多租户 | 「状态目录能不能同步到网盘」 | 用户自管本机文件；产品 **不**做多租户服务 |
| N3 不改成 IDE | 「确认屏改 prompt / 删任务」 | **D5/P2-1·P2-2**；出池前禁止实现（t15） |
| N3 不改成 IDE | 「跨屏系统多窗口 / 真 PTY」 | **D5/P2-3·P2-4**；仍是监视增强，**不是** IDE |
| N4 不灌第三方 GEB | 「D0 文档同构还没完」 | **D0 已闭环**；残差 = 本仓新文件补 L3，**不是**扫依赖树 |
| N4 不灌第三方 GEB | 「全库每个文件都要 L3」 | 核心 L3 阈值见 §6.2.2；**禁止**为凑数灌水 |

### 7.3 边界（防与 §2 / §4 / §5 / §6 / §8 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「把 P2 增强写成非目标所以永远不做」 | **错误**；P2 = D5 **池**（可出池），§7 是 **本计划形态边界**，不是 backlog 垃圾桶 |
| 「非目标 = 成功标准没过」 | **错误**；成功标准见 **§6**；非目标达标 = **没做**这些事 |
| 「t19 里实现功能 / 重写 Scheduler / 上云」 | **禁止**；范围 = 非目标冻结 + 指针 + commit |
| 「开放确认 §8 可以覆盖非目标」 | **否**；§8 = 历史默认假设；**改非目标须显式修订本 §7**（用户决议） |
| 「Mode B §10 与本表冲突时另写第三份」 | **禁止**；应对齐本表与 Mode B §10；冲突 → 同 commit 修订两边 |

### 7.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 四条非目标任一语义变更（含「可以上云 / 可以重写 Scheduler」） | 改 §7.1 + 对照表，**同 commit** 回写 L1/L2 指针；**须用户显式决议** |
| 仅 D5 出池、热改、或 §2 勾选 | **不**改本非目标骨架 |
| 新增第五条非目标 | 写入 §7.1 表，说明与 Mode B / 子计划关系；**禁止**散落在 worker 任务说明里当隐式范围 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---


## 8. 开放确认（执行前只需答一次）— **t20 已冻结**

> **冻结（t20）**：下列为 2026-07-18 对照后的**产品/文档默认假设唯一答卷**（执行前问卷；答「按默认」一次即可）。  
> 本表**只冻结历史默认**；**不**开新功能、不拆文件、不排 D5 池、不改非目标。  
> **Agent 调度策略真源** = **§5.4（t17）**，原称「对应 AGENTS §8」——**不是**本节。二者勿混。  
> 执行态（2026-07-18）：五项均 **按默认** 采纳；落地状态见下表「执行备注」列。  
> 后续会话 **不得**静默改默认；**新**默认变更仍须用户决议（§5.4.3）。

### 8.1 默认假设总表（按默认）

| # | 默认假设 | 决议 | 执行备注 |
|---|----------|------|----------|
| **A1** | **规范根**：以 `docs/` + 根 `CLAUDE.md` 为 GEB 真源，**不**新建平行 `.md/` 目录（避免双份） | **按默认** | ✅ D0（t7/t10）闭环；L1 明示规范根 = `docs/` |
| **A2** | **桌面默认**：保持 **分配后自动开跑**；高级区加「规划后确认」开关 | **按默认** | ✅ D1（t11）：`autoStartAfterPlan: true`；`#pp-pause-confirm`；真源 Mode B §4.1 / UX simple |
| **A3** | **下一执行阶段**：先做 **D0 文档同构**，再进入 D1 决议实现 | **按默认** | ✅ 主序已走完 D0–D4；后续 = D5 出池或热改（§5 t16） |
| **A4** | **D4 大拆分**：暂缓，直到有功能改动碰到超标文件 | **按默认** | 策略仍有效；t14 曾按切分地图物理落地六簇；**残差**仅热改再切（§2.3.2 / §5.3） |
| **A5** | **本文件**：作为未完善总账；子计划文件只更新状态勾选，不再另开第三份总览 | **按默认** | ✅ 总账角色自 t1 起；子计划只勾选状态 |

**总答**：**按默认**（A1–A5 全部采纳，无逐条改写）。

### 8.2 与相邻边界的对照

| 本表（§8 开放确认） | 容易混淆 | 真实归属 |
|---------------------|----------|----------|
| A1 规范根 = `docs/` | 「再建 `.md/` 做规范」 | **禁止**；双份真源 = R2 再发 |
| A2 默认 auto-start | 「强制每次确认才跑」 | 高级开关可选；**不**改默认主路径 |
| A3 先 D0 再 D1 | 「现在还要重做 D0→D1」 | **已闭环**；主序见 §5，勿回灌 |
| A4 D4 暂缓热改 | 「t14 已拆所以策略废了」 | **否**；策略 = 不为拆而拆；残差仍热改纵切 |
| A5 唯一总账 | 「再开 gap-v2 / 第三总览」 | **禁止**；子计划只勾选 |
| 本节 vs §5.4 | 「AGENTS §8 = 开放确认」 | **错误**；§5.4 = Agent 调度策略；本节 = 产品/文档默认 |

### 8.3 边界（防与 §2 / §4 / §5 / §6 / §7 混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「t20 里改产品默认 / 实现功能」 | **禁止**；范围 = 答卷冻结 + 指针 + commit |
| 「Agent 可静默改 A1–A5」 | **禁止**；新默认须用户决议（§5.4.3） |
| 「开放确认覆盖非目标 §7」 | **否**；改 N1–N4 须显式修订 §7 |
| 「再答一次开放确认当日常」 | **否**；执行前只需答一次；已冻结 |
| 「另开第三份默认假设清单」 | **禁止**；并入本 §8 |

### 8.4 何时本节省略修订

| 条件 | 动作 |
|------|------|
| 五项默认任一语义变更（含「强制确认主路径」「新建 `.md/` 规范根」） | 改 §8.1 + 对照表，**同 commit** 回写 L1/L2 指针；**须用户显式决议** |
| 仅 D5 出池、热改、或 §2 勾选 | **不**改本答卷骨架 |
| 新增第六条默认假设 | 写入 §8.1 表；**禁止**散落在 worker 任务说明里当隐式范围 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |

---

## 9. 修订历史 — **t21 已闭环**

> **闭环（t21）**：下列为 2026-07-18 本总账**从初稿到 §0–§8 冻结**的完整修订年表（t1–t21）。  
> 本表**只记历史事件**；**不**开新功能、不拆文件、不排 D5 池、不改默认/非目标/成功标准。  
> 执行态：缺失的 t3 / t7 / t10 / t12 / t14 已按正文定稿/阶段闭环补齐；**既有行语义禁止改写**；后续产品变更 **另起行追加**（同日可多行）。  
> 子计划 **不得**另开第三份修订年表冒充总账变更史。

| 日期 | 说明 |
|------|------|
| 2026-07-18 | 初稿：全库侦察后汇总产品/文档/质量缺口与 D0–D5 落地顺序 |
| 2026-07-18 | t1：前言定稿（唯一总账角色 · GEB 入口 · 关联真源五件套 · PROTOCOL） |
| 2026-07-18 | t2：§0 一句话定稿（完成态 + 五簇缺口；与 §1.3/§2 索引对齐） |
| 2026-07-18 | t3：§1.1 产品定义定稿（一句话 · 数据流 · 技术栈；与 README / orchestrator / 工作树对齐） |
| 2026-07-18 | t4：§1.2 模块地图定稿（`wc -l` 实测行数 · >800 超标 · 桌面未接 terminal · 缺桌面 E2E） |
| 2026-07-18 | t5：§1.3 冻结为已完成（附代码锚点）；子计划/orchestrator 校正勿再当缺口 |
| 2026-07-18 | t6：§2.1 产品缺口冻结（P0–P2 全表 + 代码锚点 + 非闭环说明 + D 阶段映射 + 防回灌边界）；P2-8 标为 M5 主文已改 |
| 2026-07-18 | **t7 / D0**：文档操作系统闭环（规范根 `docs/` · L1/L2 全树 · 核心 L3 播种 · 子计划勾选对齐） |
| 2026-07-18 | t8：§2.3 架构味道冻结（Q1–Q5 · 超标文件实测行数与纵切簇 · 双路径/监视接线矩阵 · 防与 D4 混淆边界）；D4 指向 §2.3.2 |
| 2026-07-18 | t9：§3 本质层根因冻结（R1–R4 总表 · 叠加决议时间线 · 纵横接线矩阵 · 上帝对象 · 完成定义 · 现象映射 · 防与 D1/D2/D4 混淆边界） |
| 2026-07-18 | **t10 / D0 核验**：L1→L2 可点导航 · L3 PROTOCOL 父路径 · 子计划再核；D0 验收通过 |
| 2026-07-18 | **t11 / D1**：三问按默认采纳；P0-1/2/3 + P1-7 + Q2 闭环；Mode B §4.1 · UX · orchestrator §2.0；CLI `--skip-plan` + 桌面 pause-confirm |
| 2026-07-18 | **t12 / D2**：监视与桌面接线闭环（P1-1 行边界 · P1-2 外置终端 · P1-3 Planner LogConsole · stream-json fixture） |
| 2026-07-18 | **t13 / D3**：P1-4 上限 · P1-5 预算分栏 · P1-6 mode_b_golden · P0-4 打包清单；B3 勾完 |
| 2026-07-18 | **t14 / D4**：六超标文件按 §2.3.2 物理纵切（`planner/` · `services/` · `cli/commands` · `claude/` · `web/js` · `web/css`）；残差热改再切 |
| 2026-07-18 | **t15 / D5**：Backlog 池冻结（P2-1…P2-7 不排期则不碰 · 立项门槛 · 子计划勾选对齐 · 禁止实现清单）；头部标 D0–D4 闭环 |
| 2026-07-18 | **t16 / §5**：推荐执行顺序冻结（主序 D0→D5 · 依赖表 · 反模式 · Agent 策略执行备注 · 边界）；头部标 §5 已冻结 |
| 2026-07-18 | **t17 / §5.4**：任务量与 Agent 策略冻结（对应 AGENTS §8 · 阶段任务量表 · 操作规程 · 硬规则 · 边界 · 修订条件）；与本文 §8 开放确认区分；头部/L1/L2 指针 |
| 2026-07-18 | **t18 / §6**：成功标准冻结（五指标全绿 · 证据明细 · 边界 · 修订条件）；头部/L1/L2 指针；本计划自身验收闭环 |
| 2026-07-18 | **t19 / §7**：非目标冻结（N1–N4 · 对照表 · 边界 · 修订条件）；头部/L1/L2 指针；与 Mode B §10 / §5.3·§5.4.3 对齐 |
| 2026-07-18 | **t20 / §8**：开放确认冻结（A1–A5 **按默认** · 对照表 · 边界 · 修订条件）；头部/L1/L2 指针；执行前问卷闭环 |
| 2026-07-18 | **t21 / §9**：修订历史闭环（补齐 t3/t7/t10/t12/t14 · 年表冻结 · 禁止改写既有行 · 追加规则）；头部/L1/L2 指针；总账 t1–t21 闭环 |

### 9.1 边界（防与产品变更混淆）

| 勿再写入本表 / 勿做的事 | 真实归属 |
|-------------------------|----------|
| 「t21 里改产品默认 / 实现功能 / 出池」 | **禁止**；范围 = 年表闭环 + 指针 + commit |
| 「改写既有行语义以「更正」历史」 | **禁止**；勘误 **另起一行**说明 |
| 「另开 gap-changelog / 第三份修订表」 | **禁止**；只维护本 §9 |
| 「D5 出池不记修订历史」 | **应**追加一行；**不**改主序 / §6 / §7 / §8 骨架 |
| 「把 P2 增强塞进年表当已完成」 | **否**；P2 = D5 池，出池实现后才追加 |

### 9.2 何时本节省略修订

| 条件 | 动作 |
|------|------|
| D5 出池 / 热改 / 用户决议改默认·非目标·成功标准 | **追加**一行（日期 + 简述）；同 commit 改对应 § |
| 仅措辞润色既有冻结节 | **不**改年表既有行；若值得记 → 追加「润色 §X 表述」 |
| 无上述变更 | **勿**为「写得更满」扩写本 § |
