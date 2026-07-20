# src-tauri/
> L2 | 父级: /CLAUDE.md

成员清单
src/lib.rs: Tauri commands 薄壳 → cco::services（plan job/edit task/remove_plan_task_cmd P2-1/sanitize_plan_deps_cmd/live/runs/get_plan_meta H2/settings/chat · chat_list/new/delete_session_cmd C3 · chat_stream_partial_cmd C3 · read_plan_md_cmd · chat_save_plan_cmd(plansDir) · chat_save_attachment_cmd G4 · chat_normalize_plan_cmd G0b · start_rework/accept_residual P-loop）
src/main.rs: 桌面二进制入口 → cco_desktop_lib::run
Cargo.toml: cco-desktop crate · 依赖 cco 库
tauri.conf.json: 窗口/标识；frontendDist=../web
capabilities/default.json: 权限清单
build.rs: tauri build 钩子

法则: 成员完整·一行一文件·父级链接·技术词前置
注: P1-2 open_task_terminal_cmd 已接；chat_*_cmd 已接；**chat_send_cmd 必须 async+spawn_blocking**（同步会堵 UI）；业务逻辑禁止堆在本 crate

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
