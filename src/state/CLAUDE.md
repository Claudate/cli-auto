# src/state/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: RunState/RunStatus/TaskState · **RouteSource**（explicit/soft_fill/tag_routing/force/failover/**cost_auto**/**cost_escalate**）· **sqlite** · **cco_split_store** · **project_memory** · **project_ui**
sqlite.rs: **cco.db** — plan_jobs/plan_tasks + cco_split_* + project_last_summary/project_pins + **project_ui_prefs** · **split_graph_quality / latest_job_id_for_plan_path**（多步 AI ≫ confirmed direct 1 步）
cco_split_store.rs: **拆分 SoT** — cco_split_jobs/tasks 全字段读写
project_memory.rs: **P2-2 项目轻记忆** — last_summary + pins(≤3)
project_ui.rs: **项目 UI 偏好 SoT** — `dismissed_run_id` 等；结束本轮写 SQLite，**禁止**只放内存/localStorage

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
