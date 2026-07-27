# src/services/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export live/projects/runs/settings/chat/preview · **A1-7 deprecated facade**（Presentation 应调 `app::*`）
live.rs: project_live_view（含 inspect_loop · handoff_board/handoff_md_path P2-6 · **P1-3** task.route_source + App 拼 route_label · **P2-1** verification · **W3 browser_evidence** · **H1** `status_one_liner` · **H3** `merge_check`）· task_logs · open_task_terminal · stop_task（Pending 可停 · SIGTERM+KILL · 整 run 冻 pending→Aborted）
live_status.rs: **H1** 组装 `status_one_liner`（委托 app/domain；**禁止**再堆进 live.rs）
preview/: **可选本地预览 API**（`detect`/`http_ready` · HTTP 就绪后报 URL · stop/status）；**聊天短句不拦截**，起服由 CLI Bash 真执行；`annotate_false_preview_claims` 仅核验回复里的假 localhost
projects.rs: list/add/remove projects
runs.rs: list/start/stop/resume · plan job re-export · confirm_start（→ app::split::confirm）· start_run_from_plan（**materialize_selected_tasks 后写盘/spawn · A0-R4/D-T3-1**）· sanitize/update/remove proposed · list_plans · list_plan_meta · start_rework_from_run · accept_run_residual · **stop_run 含 Pending + meta.json pid + SIGKILL** · **新逻辑勿进本文件**
settings.rs: get/set settings view（failover H4 · post_* 系统收尾 · planner_critic · **effort** · **permission_mode** · **browser_enabled** 网页自动化）
chat/: **A1-6 多文件 IO 适配**（单文件 ≤600；出巨石榜）
  · mod.rs: facade re-export + domain pure re-export
  · types.rs: ChatSession/Message/Draft/Send/Stream/Normalize DTO
  · session.rs: get/list/new/rename/delete · save · cleanup 48h
  · send.rs: chat_send（fake/soft-fallback · draft from fence）
  · stream.rs: chat_stream_partial
  · plan_md.rs: chat_save_plan · read_plan_md
  · attachment.rs: chat_save_attachment
  · cli_call.rs: Claude print spawn for chat/normalize（chat：`permission_mode=bypassPermissions` + spawn allow 旗，可本项目内装依赖/起 dev；normalize 仍 dontAsk）
  · normalize.rs: chat_normalize_plan G0b
  · paths.rs: `.cco/chat` · plan path resolve
  · tests.rs: 集成测
  · 纯规则真源：`domain/chat`；用例面：`app/chat`
util.rs: kill_pid · log tail helpers

## 硬规则（A1-7）

1. **Presentation 入口**优先 `crate::app`；本目录 = IO 适配 + 过渡 re-export。  
2. **禁止**新增业务策略（soft-fill / confirm / Mode B 开跑）。  
3. `confirm_start` 必须保持一行委托 `app::split::confirm`。  
4. 本刀**不**删光 services 文件（A5 再收敛）。

法则: 成员完整·一行一文件·父级链接·技术词前置
注: UI 细节禁止；Tauri/CLI 共用 DTO；Mode B 业务入口仍是 confirm / app::split::confirm；chat 主产出散文，可本项目内启动验收

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
