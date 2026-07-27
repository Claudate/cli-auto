# src/plan/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: **A1 facade** — re-export `domain::plan` 类型/纯函数；`load_plan`/`list_plans`/`peek_adapter` IO；`#[cfg(test)] include plan_tests.rs`
plan_tests.rs: PlanIR/load_plan 单测（从 mod 抽出压行数；fixture 缩进保持）
system_post.rs: 系统收尾注入（Config 开关 · inspect/push/**open-pr S-PR**）；ids/predicate re-export from domain
**split_agent/**: **OpenHands Plan Mode** — `ModelSplitAgent`（prompt/**extract**/parse/model）· `FixtureSplitAgent` · `build_split_agent_plan`；输出 **cco-split/v1** → soft_accept → SQLite SoT；adapter `cco-split/llm+split-agent-llm`；env `CCO_SPLIT_AGENT_JSON`/`CCO_SPLIT_AGENT_FIXTURE`/`CCO_SPLIT_AGENT=off`；**Q2** prompt 要求 **scope_paths** + body【做什么/改哪里/…】模板 · 禁 worker 腔 · parse 写入 scope · extract 认 stream-json · **W4** 可选 `grain_hint` 进 user_prompt · **revision_notes** 重拆反馈（可空 · 不碰 confirm）
planner/: Mode B plan job（job 写 proposed 前 inject_system_post · llm/heuristic 用 PLANNER_MAX_TASKS · digest 状态行优先 · critic_summary/chips/notes/critic_llm_used/cost/ms · 规则 critic + 可选 LLM 第二跳（设置或 env） · **sanitize.rs**（P3-4 CcoSplit depends 优先 · 回落 PlanIR）· update_proposed_task(depends_on/**role/scope_paths S-role**) / remove_proposed_task · plan.user_edits.json + preserve_from_job_id（P2-1/P2-2） · **task_edit.rs** role/scope 补丁纯辅助 · LLM 解析 provider/role/scope/tags/outputs（P2-5） · 确认屏一键重拆/开巡检/开智能校对 · 真实 docs 金样 · 可选任务确认屏勾选 · **extract_work_phases 优先 #### A1/B2/U1-1 → W0/W1 → 波次**；失败则 **diagnose + recover 文档真标题**（禁止静默空壳）；仅用任何可恢复工作结构才 last-resort meta 四波（prompt 内写失败原因） · **CcoSplit SoT**（write_proposed→SQLite cco_split_*；load_proposed 优先 SoT→PlanIR；import plan.proposed；job_view 全字段 desk DTO · **P1-4 acceptance_is_stub + acceptance_hint**） · **plan_mode=fast** 本地启发式 · **plan_mode=direct** 整份计划单任务（raw-single；聊天「直接执行」）· **plan_mode=ai 优先 ModelSplitAgent** → legacy LLM PlanIR → heuristic · **ai 不完整：禁止 heuristic 残图入库/展示（plan_failed；有上次成功则恢复之；无则只显示失败）** · **prior/restore 排序：图质量（多步 AI ≫ direct/raw-single 1 步）> status > updated_at** · **cco-split parse 容错 string↔array** · **fast/heuristic/parse 永不跑 LLM critic**（设置开也跳过）· 僵尸 planning 扫 `llm_work/tasks/*/meta.json`（含 `__critic__`）kill + 5min hard timeout · **`.done`/exit0 或 `cco_split_agent.json` 有任务时不 reap** · `get_plan_job`/`finish` 可从假 zombie 抢救 proposed · critic 超时 stop+kill · supersede kill pid · **heuristic 优先 #### 任务 id（P0-1/A1…）** · 剥 `cco-split-summary` 写回垃圾 · `looks_like_work_task_id` 域真源（不把 `P0 —` 当任务 id）· **S2 #### 不 force_serial**：信计划「依赖」列；无列则 max_parallel 切批 · acceptance←完成定义
adapters/: cco_v1 · serial_prompts · raw_single（serial 跳过 meta 标题；cco_v1 解析 role/scope/outputs/tags/require_inspect）

注: 类型/validate/materialize/tag routing **真源** = [`../domain/plan/`](../domain/plan/)；本目录勿再堆纯模型

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
