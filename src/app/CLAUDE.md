# src/app/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 用例层根 · A1-7 presentation map 注释
split.rs: **A1-2/A1-7/A5-1** Mode B — `confirm`（后台唯一业务开跑）· **`confirm_materialize`**（CLI 前台同契约）· start_job/get_job/edit_task(**role/scope_paths S-role**)/remove_task/sanitize_deps
run/: **A1-3/A1-7/A5-1/A5-3 · S-run 多文件** Run 用例面（单文件 ≤400）
  · mod.rs: lifecycle facade（list/load/stop/resume/rework）· domain maps · observe · re-export
  · materialize.rs: materialize_run · materialize_parse_only
  · foreground.rs: ForegroundOpts · prepare_scheduler · preflight_plan · prepare_resume · finish_with_reports
  · route.rs: apply_provider_override（soft/force · A0-R3）
  · 编排循环仍在 `runtime/scheduler`；**不**旁路 Mode B；TUI 只经本面
chat.rs: **A1-6/A1-7** Chat 用例面 — session list/get/new/delete · send · stream_partial · save_plan · read_plan_md · normalize_plan · save_attachment · cleanup_expired；现委托 `services::chat_*` thin facade；**禁止** confirm/start_run
（A1-5 **未**加 `app/inspect` 用例面：inspect 纯规则在 `domain/inspect`，IO 在 `runtime/handoff`；桌面 rework 经 `app::run::start_rework` → services。A4 再做人话 DTO 用例。）

## 硬规则

1. Presentation（CLI/Tauri/TUI）只调 app 用例，不写业务策略。  
2. **开跑**只经 [`split::confirm`](./split.rs) / [`confirm_materialize`](./split.rs)（`services::confirm_start` 为后台 facade）；ParseOnly 走 `run::materialize_run`（文档化，非 Mode B）。  
3. 禁止新建上帝 Manager；组合逻辑写在用例内。  
4. 体积：软 400 / 硬 600 行。  
5. `app/run` **不**旁路 Mode B；stop 冻 Pending 语义与 `services::stop_run` 一致。  
6. **禁止**在 app 内重写 VERDICT 正文解析（domain/inspect 真源）。  
7. **`app/chat` 只写散文 plan.md / 会话 JSON**；不得旁路 confirm 或 spawn 执行 worker。  
8. **`app/run/` 已纵切**（S-run）：禁止再合并成单文件；新逻辑按 materialize / foreground / route / facade 归类。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /src/CLAUDE.md
