# src/
> L2 | 父级: /CLAUDE.md

成员清单（现状 → P2-17 目标见 [`docs/architecture-redesign-2026-07-20.md`](../docs/architecture-redesign-2026-07-20.md)）
lib.rs: 库根；re-export plan/runtime/services/state/terminal/tui；挂载 **domain/app/ports**
main.rs: cco 二进制入口；clap → cli::execute
domain/: **A1** 纯模型 — [`domain/CLAUDE.md`](./domain/CLAUDE.md) · `plan/`（A1-1 ✅）· `run/`（A1-3 ✅）· `worker/`（A1-4 ✅）· `inspect/`（A1-5 ✅）· `chat/`（fence/title/normalize；A1-6 ✅）
app/: **A1 ✅** 用例 — [`app/CLAUDE.md`](./app/CLAUDE.md) · `split`（confirm 唯一开跑；A1-2/A1-7）· `run/`（**S-run** 多文件 · list/stop/resume/materialize/foreground/route；A1-3/A1-7）· `chat`（会话/send/save_plan；A1-6/A1-7）
ports/: **WorkerPort ✅ A1-4 · HandoffStore ✅ A1-5** — [`ports/CLAUDE.md`](./ports/CLAUDE.md) · trait + DTO；ChatStore 未建（A1-6 free-fn）
services/: **deprecated facade**（A1-7；Presentation → app；`confirm_start` → `app::split::confirm`；IO 仍住此）
cli/: clap 命令面 + commands/（**A5-1** 1:1 表见 [`cli/CLAUDE.md`](./cli/CLAUDE.md)；Mode B `confirm_materialize`；ParseOnly `materialize_run`；soft-fill 真源 domain/worker）
plan/: adapters + load_plan IO + planner + system_post inject（**类型真源已迁 domain::plan**；mod 已瘦身；测 `plan_tests.rs`）
runtime/: **scheduler/** 多文件薄编排（A1-3 ✅ · 经 WorkerPort A1-4）· **handoff/** 多文件适配器（A1-5 ✅）· provider（port 适配器）· log_events · worktree · acceptance
terminal/: TerminalManager + external launcher
tui/: ratatui 多页**观察层**（**A5-3** load/stop 经 `app::run`；不做完整拆分台）
config/: ~/.cco/config.toml + AllowedProject
state/: run.json / events.jsonl / TaskState
doctor/: 环境门禁 DoctorReport
graph/: DAG ready_tasks / topo_layers / format_graph
report/: report.md+json

## 硬规则（继承 L1 · 本层加严）

1. 依赖方向：`cli|tui|services → app → domain`；`adapters` 实现 ports；**禁止** domain → UI/clap/tauri。  
2. **开跑**只经 Split 确认用例（现 `confirm_start`）；禁止第二业务入口。  
3. **Scheduler / 编排循环**不得内嵌 VERDICT 文本解析；inspect 规则在 domain/inspect + handoff adapter。  
4. **Worker**：claude/codex/fake 只实现 port；failover/isolation 策略对象化，不写死在调度 `if provider`。  
5. **体积**：业务文件软 400 / 硬 600 行；函数软 40 / 硬 80 行。  
6. **禁止**往厚文件**继续堆功能**（只拆/委托）。Rust A1 已出榜：`plan/mod` · `scheduler/*` · `handoff/*` · `services/chat/*`。前端 S8 facade 已出榜（plan/chat/log/doctor/monitor/result）；`state.js` D9 遗留见 web L2。  
7. 新模块优先 `domain/` `app/` `ports/` `adapters/`（A1 骨架起），避免再胀 `services/`。  
8. CLI 与桌面调用同一 app 路径；策略不写在 `cli/commands/*`。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
