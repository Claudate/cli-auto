# src/services/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: re-export live/projects/runs/settings/chat
live.rs: project_live_view（含 inspect_loop · handoff_board/handoff_md_path P2-6）· task_logs · open_task_terminal · stop_task
projects.rs: list/add/remove projects
runs.rs: list/start/stop/resume · plan job · confirm_start · sanitize_proposed_deps · update_proposed_task/remove_proposed_task（P2-1 depends_on/删任务）· list_plans · list_plan_meta（H2 ever_completed/last_run_*）· start_rework_from_run · accept_run_residual（P-loop）
settings.rs: get/set settings view（failover H4 · post_inspect/post_git_push 系统收尾 · planner_critic_enabled 可选 LLM 第二跳）
chat.rs: chat_session_get · chat_list/new/delete_session（C3 多会话）· chat_send(+attachments) · chat_stream_partial（C3 流式 partial）· chat_save_plan(plan_rel/plans_dir) · chat_save_attachment（G4）· read_plan_md · chat_normalize_plan/structure（G0b）· cleanup_expired 48h（G3）· 标题 sanitize（G0；无 max_turns；exit≠0 仍取 assistant 文本；复用 __chat__ 清 .done；extract_plan_fence 行首嵌套 depth + CJK 安全）
util.rs: kill_pid · log tail helpers

法则: 成员完整·一行一文件·父级链接·技术词前置
注: UI 细节禁止；Tauri/CLI 共用；Mode B 业务入口仍是 confirm_start

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
