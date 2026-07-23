# src/state/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: RunState/RunStatus/TaskState(attempt/last_retry_reason/failover_used/**route_source|route_previous|route_note**) · **RouteSource** · prepare_run_dir · prepare_for_resume / **prepare_task_retry** · events.jsonl · resolve_run_dir · **sqlite** · **cco_split_store** · **project_memory** (P2-2)
sqlite.rs: **cco.db** 连接 + schema — `plan_jobs`/`plan_tasks`（过渡索引 · **list_plan_split_index / latest_job_id_for_plan_path**（**优先 confirmed > planned > planning**，防残图盖住已开跑拆分）供计划列表回看）+ `cco_split_*` + **`project_last_summary` / `project_pins`**（轻记忆）
cco_split_store.rs: **拆分 SoT** — `cco_split_jobs` / `cco_split_tasks` 全字段读写；`save`/`load`/`try_*`；confirm 标 confirmed
project_memory.rs: **P2-2 项目轻记忆** — last_summary + pins(≤3) CRUD · `compose_last_summary` 规则模板 · `format_memory_context` 仅 prompt 上下文；best-effort try_*

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
