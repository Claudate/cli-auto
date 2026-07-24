# src/cli/
> L2 | 父级: /CLAUDE.md

成员清单
mod.rs: clap Commands / TermCommands 枚举 · execute dispatch（D4 已瘦身；**parse --mermaid** P2-7）
commands/: 子命令实现（doctor/init/plans/parse/plan/run/resume/status/stop/report/logs/term/tui）· common helpers
commands/run.rs: **A5-1** Mode B → `app::split::confirm_materialize`；ParseOnly → `app::run::materialize_run`；soft-fill → `apply_provider_override`；loop → `prepare_scheduler`
commands/resume.rs: **A5-1** → `app::run::{prepare_resume,prepare_scheduler}`
commands/stop.rs: **A1-7** 整 run / 单 task → `app::run::{stop,stop_task}`
commands/plan_cmd.rs: **A1-7** → `app::split::{start_job,get_job}`
commands/status.rs: **A5-1** → `app::run::{load_by_dir,handoff_paths,format_status_by_provider}`；**H0-5** 首行 `report_summary_line`
commands/plans.rs: **A5-1** → `app::run::plans`
commands/common.rs: plan_then_load_ir → app::split；`run_scheduler_loop`（**H0-4** 结束先 `report_summary_line` 再 status 枚举 + finish_with_reports）；term path helper
interactive.rs: 交互选 project/plan · confirm · 非交互硬要求 --project

## A5-1 CLI ↔ app 1:1 表（真源）

> 规则：handler = argv 解析 → **Application 用例** → 打印 DTO。  
> **禁止**第二套 soft-fill / confirm / 手搓 `new_run_id`+`mark_confirmed`+Scheduler 策略字段。  
> 桌面 IPC 名**不动**（本表只收敛 CLI 侧调用路径）。

### Mode B / Run / Split

| CLI 子命令 | 场景 | app 用例 | 备注 |
|------------|------|----------|------|
| `cco plan` | 只规划 | `app::split::{start_job,get_job}` | 不 spawn worker；开跑指引桌面或 `run` |
| `cco run`（散文 / 非 structured） | 规划 + **开跑** | plan: `start_job`/`get_job`/`load_proposed_plan`；开跑: **`split::confirm_materialize`**；loop: `run::prepare_scheduler` + `preflight_plan` + `finish_with_reports` | TTY `proceed?` = 人确认；契约同桌面 confirm（optional drop · soft defaults）；**`--effort`** 会话级覆盖 config（ultracode→xhigh+多 Agent 提示） |
| `cco plan` | 只规划 | `app::split::{start_job,get_job}` | **`--effort`** 同步写入 job / config 默认 |
| `cco run`（structured / `--skip-plan` / `--adapter`） | ParseOnly 开跑 | `run::apply_provider_override` → **`run::materialize_run`**（**返回已 drop optional 的 IR** · A0-R4/D-T3-1）→ `prepare_scheduler(… returned ir …)` | **非** Mode B；文档化 ParseOnly；**禁止**当主路径旁路 Mode B；**禁止**调度未 materialize 的原 IR |
| `cco run --provider` | soft-fill | `run::apply_provider_override` → domain `RouteFillMode::Soft` | 不盖显式 route |
| `cco run --force-provider` | 全量覆盖 | 同上 · `Force` | force 优先于 soft |
| `cco resume` | 恢复 | `run::prepare_resume` + `prepare_scheduler` … | 与桌面 `resume`（后台）同准备语义 |
| `cco stop` | 停 run/task | `run::{stop,stop_task}` | Pending 冻结在 app/services |
| `cco status` | 观察 | `run::{load_by_dir,handoff_paths,format_status_by_provider}` | **不** import handoff 内部类型 |
| `cco plans` | 列计划 | `run::plans` | 与桌面 list 同路径串 |
| `cco report` | 打印报告 | `report::*`（观察面） | 可后收 app 查询 |
| `cco logs` | 跟日志 | `state` + log 路径（观察面） | 非业务策略 |
| `cco parse` | 图/Mermaid | `plan::load_plan` + graph（观察面） | 不写 run |
| `cco doctor` / `init` / `term` | 环境/终端 | doctor · config · terminal | 非 Mode B 开跑 |
| `cco tui` | 观察 + 轻 stop | tui → `app::run::{load_by_dir,load_resolved_plan,stop_task}` | **A5-3**；不做拆分台 |
| （无）chat | 桌面-only | `app::chat` | CLI 不必对称 |

### 红线（handler 内禁止）

| 禁止 | 正确路径 |
|------|----------|
| 手搓 `state::new_run_id` + `mark_confirmed` 当 Mode B 开跑 | `split::confirm_materialize` 或桌面 `split::confirm` |
| 第二套 soft-fill 循环改 `task.provider` | `run::apply_provider_override` |
| CLI 直构 `Scheduler { … }` 填策略字段 | `run::prepare_scheduler(ForegroundOpts)` |
| `runtime::handoff::Handoff::path_*` | `run::handoff_paths` |
| `start_run` / `start_from_request` 冒充 Mode B | 仅 legacy ParseOnly IPC；CLI 用 `materialize_run` |

### 已删/标死旁路（A5-1）

- ~~`commands/run.rs` 手搓 `new_run_id` + `RunState::new` + `mark_confirmed`~~ → `confirm_materialize` / `materialize_run`
- ~~`commands/run.rs` / `resume.rs` 手搓 `Scheduler {…}`~~ → `prepare_scheduler`
- ~~`commands/status.rs` `Handoff::path_*`~~ → `handoff_paths`
- soft-fill 真源仍仅 `app::run` + `domain::worker`（无 CLI 副本）

## 硬规则（继承 L1 · 本层加严）

1. Handler **薄壳**：argv → **Application 用例** → 打印 DTO；目标 **≤ 30 行**业务编排（解析/打印除外可略长）。  
2. **禁止**在 `commands/*` 实现调度循环、inspect 解析、混跑 failover 策略。  
3. 与桌面**同一 app 路径**；CLI 不是第二产品内核。  
4. `--provider` = soft-fill；`--force-provider` = 全量覆盖；**不得**用 soft 静默擦掉任务显式 route（真源 domain/worker + app::run）。  
5. Mode B：`plan` 只规划；开跑经 **`split::confirm` / `confirm_materialize`**，禁止 silent 旁路。  
6. **A5-1**：上表为 CLI↔app 1:1 真源；改命令须同步本表。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
