# src/state/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: RunState/RunStatus/TaskState(attempt/last_retry_reason/failover_used) · prepare_run_dir · events.jsonl · resolve_run_dir · **sqlite** · **cco_split_store**
sqlite.rs: **cco.db** 连接 + schema — `plan_jobs`/`plan_tasks`（过渡索引）+ `cco_split_*` 表定义；best-effort dual-write PlanIR 索引
cco_split_store.rs: **拆分 SoT** — `cco_split_jobs` / `cco_split_tasks` 全字段读写；`save`/`load`/`try_*`；confirm 标 confirmed

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
