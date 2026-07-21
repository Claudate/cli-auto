# src/tui/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export run_tui / TuiOptions
app.rs: 事件循环 · 轮询 reload · 快捷键 · **P2-5** selected_term_idx / term_zoom / open_term_panes · **A5-3** load/stop 经 `app::run`
pages.rs: Dashboard/Graph/Task/Logs/Terminals（多窗格 log 网格 · zoom · strip_ansi_lite）/Help
widgets.rs: 共用渲染小组件（TaskStatus 取自 `ports`，不碰 `runtime/provider`）

## 依赖表（A5-3）

| 触点 | 允许 | 禁止 |
|------|------|------|
| 读 run 状态 | `app::run::load_by_dir` / `load` | `RunState::load` 直读盘旁路 app |
| 读 resolved plan | `app::run::load_resolved_plan` | 硬编码 `plan.resolved.json` 路径布局 |
| 停任务 | `app::run::stop_task`（与 CLI/桌面同路径） | 自写 kill_pid / 写 `.done` / 改 `TaskStatus` |
| 状态着色 | `ports::TaskStatus`（wire DTO）· `state::RunStatus` | `runtime::provider::*` 适配器类型 |
| 终端会话 | `terminal::TerminalManager`（观察适配 · 保持） | 第二套拆分台 / Mode B confirm |
| 开跑 | **无** | `start_run` / Scheduler 装配 / `split::confirm` |

## 硬规则

1. **观察 + 轻控制** only；**不做**完整拆分台（架构 §8）。  
2. Presentation → **app 用例**；不写 stop/soft-fill/optional 策略。  
3. 体积：软 400 / 硬 600 行。  
4. Graph 可持 `PlanIR` 快照（app 查询返回），只用于展示 topo/title。

法则: 成员完整·一行一文件·父级链接·技术词前置
注: Terminals 网格 = stdout 只读伪 PTY；真交互 PTY write 仍走外部终端 (O)，不引入 portable-pty

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
