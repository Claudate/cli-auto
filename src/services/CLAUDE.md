# src/services/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export live/projects/runs/settings/chat
live.rs: project_live_view（含 inspect_loop）· task_logs · open_task_terminal · stop_task
projects.rs: list/add/remove projects
runs.rs: list/start/stop/resume · plan job · confirm_start · list_plans · start_rework_from_run · accept_run_residual（P-loop）
settings.rs: get/set settings view
chat.rs: chat_session_get · chat_send · chat_save_plan（聊天共建；无 max_turns 限制；exit≠0 仍取 assistant 文本；复用 __chat__ 清 .done；extract_plan_fence/历史截断 CJK 安全）
util.rs: kill_pid · log tail helpers

法则: 成员完整·一行一文件·父级链接·技术词前置
注: UI 细节禁止；Tauri/CLI 共用；Mode B 业务入口仍是 confirm_start

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
