# src/state/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: RunState/RunStatus/TaskState · **RouteSource**（explicit/soft_fill/tag_routing/force/failover/**cost_auto**/**cost_escalate**/**cost_budget**）· **TaskAutoCommitResult**（hash/files/push/branch/worktree）· **AutoCommitPolicySnapshot**（run_dir/auto_commit.json）· **sqlite** · **cco_split_store** · **project_memory** · **project_ui**
sqlite.rs: **cco.db** — plan_jobs/plan_tasks + cco_split_* + project_last_summary/project_pins + **project_ui_prefs** · **split_graph_quality / latest_job_id_for_plan_path**（多步 AI ≫ confirmed direct 1 步）
cco_split_store.rs: **拆分 SoT** — cco_split_jobs/tasks 全字段读写
guide_store.rs: **G0-2 引导存储** — user_profile / project_memory（富）/ guide_sessions/rounds/utterances schema · session CRUD + profile/memory get/upsert（rounds 读写待 G2）
memory_store/: **P3 轻量语义记忆**（`~/.cco/memory/`）— mod.rs SQLite+embedding BLOB · tantivy BM25 · ONNX all-MiniLM-L6-v2（模型缺失回退 stub 零向量）· TTL/max_entries 归档 · **store_batch**（单事务+单 commit 批量路径；10k+100 检索实测 2.31s）· port.rs LocalMemory（MemoryPort 适配 · 按次短开避免长占写锁）· tests.rs 单测+2 个 `--ignored` 基准（指南 `docs/memory-dev-guide.md`）
project_memory.rs: **P2-2 项目轻记忆** — last_summary + pins(≤3)
project_ui.rs: **项目 UI 偏好 SoT** — `dismissed_run_id` 等；结束本轮写 SQLite，**禁止**只放内存/localStorage

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
