# src/app/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 用例层根 · A1-7 presentation map 注释
split.rs: **A1-2/A1-7/A5-1** Mode B — `confirm`（后台唯一业务开跑）· **`confirm_materialize`**（CLI 前台同契约）· start_job/get_job/**latest_job_for_plan_path**/**list_plan_split_index**（计划列表回看拆分 · SQLite 索引）· edit_task(**role/scope_paths S-role**)/remove_task/sanitize_deps
run/: **A1-3/A1-7/A5-1/A5-3 · S-run 多文件** Run 用例面（单文件 ≤400）
  · mod.rs: lifecycle facade（list/load/stop/resume/**retry_task**/rework）· domain maps · observe · re-export
  · materialize.rs: materialize_run / **materialize_run_with_route**（**返回 (run_id,state,ir)** · drop optional · **Ensure inject closeout + plan.checklist.json** · stamp route_source · A0-R4/D-T3-1）· **apply_effort** · **apply_permission_mode**（默认 bypassPermissions · 无人 worker 可写）· materialize_parse_only
  · foreground.rs: ForegroundOpts · prepare_scheduler · preflight_plan · prepare_resume · finish_with_reports（**Ensure auto_rework 钩子**）
  · **ensure_loop.rs（E3）**：`maybe_auto_rework` / quiet — docs-closeout FAIL → `start_rework`（非 Mode B 旁路）
  · route.rs: apply_provider_override（soft/force · 返回 RouteFillReport · A0-R3）
  · provenance.rs: **P1-2** stamp_route_fill / stamp_route_inferred / stamp_failover → TaskState.route_* · **P1-3** compose_route_label / provider_product_label（live 人话）
  · **status_line.rs（H1）**：`from_run_state` / `from_job_view` / `resolve` → `StatusOneLiner`；CLI/live 共用
  · 编排循环仍在 `runtime/scheduler`；**不**旁路 Mode B；TUI 只经本面
chat.rs: **A1-6/A1-7** Chat 用例面 — session list/get/new/**rename**/delete · send · stream_partial · **preview_start/stop/status**（本地 dev 独立进程）· save_plan · read_plan_md · normalize_plan · save_attachment · cleanup_expired；委托 `services::chat_*` / `preview`；**禁止** confirm/start_run
memory.rs: **P2-2** 项目轻记忆 — get/last_summary/list_pins/upsert_pin/delete_pin · writeback_from_run · prompt_context
project_ui.rs: **项目 UI 偏好** — dismiss_run / clear_dismissed_run（SQLite `project_ui_prefs`；结束本轮 SoT）
（A1-5 **未**加 `app/inspect` 用例面：inspect 纯规则在 `domain/inspect`，IO 在 `runtime/handoff`；桌面 rework 经 `app::run::start_rework` → services。A4 再做人话 DTO 用例。）

## 硬规则

1. Presentation（CLI/Tauri/TUI）只调 app 用例，不写业务策略。  
2. **开跑**只经 [`split::confirm`](./split.rs) / [`confirm_materialize`](./split.rs)（`services::confirm_start` 为后台 facade）；ParseOnly 走 `run::materialize_run`（文档化，非 Mode B；**仍** drop `optional && !include` · A0-R4/D-T3-1；调度须用返回 IR）。  
3. 禁止新建上帝 Manager；组合逻辑写在用例内。  
4. 体积：软 400 / 硬 600 行。  
5. `app/run` **不**旁路 Mode B；stop 冻 Pending 语义与 `services::stop_run` 一致。  
6. **禁止**在 app 内重写 VERDICT 正文解析（domain/inspect 真源）。  
7. **`app/chat` 只写散文 plan.md / 会话 JSON**；不得旁路 confirm 或 spawn 执行 worker。  
8. **`app/run/` 已纵切**（S-run）：禁止再合并成单文件；新逻辑按 materialize / foreground / route / facade 归类。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /src/CLAUDE.md
