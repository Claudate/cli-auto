# src-tauri/
> L2 | 父级: /CLAUDE.md

成员清单
src/lib.rs: Tauri commands **薄壳 → cco::app**（A1-7 ✅）· chat/split/run/plan-job/stop/resume/rework 无业务策略；live/projects/settings/doctor 仍 thin services 适配；open_monitor_window 系统窗
src/main.rs: 桌面二进制入口 → cco_desktop_lib::run
Cargo.toml: cco-desktop crate · 依赖 cco 库
tauri.conf.json: 窗口/标识；frontendDist=../web
capabilities/default.json: 权限清单（含 main + cco-monitor 窗）
build.rs: tauri build 钩子

## A1-7 command → app 表（IPC 名不变）

| Tauri command | Application |
|---------------|-------------|
| confirm_start_cmd | app::split::confirm |
| start_plan_job_cmd / get_plan_job_cmd / latest_plan_job_cmd | app::split::* |
| update/remove/sanitize plan task | app::split::* |
| stop_run_cmd / stop_task_cmd / resume_run_cmd / rework / residual | app::run::* |
| get_runs / get_run / plan meta / preview / start_run（ParseOnly） | app::run::* |
| chat_* / read_plan_md | app::chat::* |
| live / projects / settings / doctor | services thin（未建 app 模块） |

## 硬规则（继承 L1 · 本层加严）

1. **每个 command 无业务策略**：解析参数 → 调 `cco::app` → 映射错误；目标 **≤ ~30 行/命令**。  
2. **禁止**在本 crate 拼 prompt、改 DAG、实现调度/混跑策略。  
3. `chat_send_cmd` 必须 **async + spawn_blocking**（同步会堵 UI）。  
4. 开跑只暴露确认用例对应命令；**禁止**为 UI 方便新增旁路 `start_run` 业务语义（legacy `start_run` = ParseOnly，非 Mode B）。  
5. 新命令优先聚合进已有用例组，避免无 app 的「扁平命令袋」膨胀。  
6. **不**静默改 IPC 命令名 / JSON 字段（web 兼容）。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
