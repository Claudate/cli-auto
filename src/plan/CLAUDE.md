# src/plan/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: PlanIR/TaskIR(optional/include · role/scope/outputs/tags · require_inspect) · TaskRole · TaskScope · apply_tag_routing(P2-4) · normalize_optional_title · title_is_meta_heading · materialize_selected_tasks · materialize_role_defaults(P2-1 inspect) · inject_system_post_tasks re-export · load_plan · list_plans · validate · MAX_TASKS(22)/PLANNER_MAX_TASKS(20)/MAX_PROMPT_CHARS/MAX_TIMEOUT_SECS · INSPECT_DEFAULT_*
system_post.rs: 系统收尾可选任务（sys-post-inspect / sys-post-git-push · 不参与拆解 · 设置总开关默认关 · 开启后 include 默认 true）
planner/: Mode B plan job（job 写 proposed 前 inject_system_post · llm/heuristic 用 PLANNER_MAX_TASKS · digest 状态行优先 · critic_summary/chips/notes/critic_llm_used/cost/ms · 规则 critic + 可选 LLM 第二跳（设置或 env） · sanitize_proposed_deps · update_proposed_task(depends_on) / remove_proposed_task · plan.user_edits.json + preserve_from_job_id（P2-1/P2-2） · LLM 解析 provider/role/scope/tags/outputs（P2-5） · 确认屏一键重拆/开巡检/开智能校对 · 真实 docs 金样 · 可选任务确认屏勾选 · **落地计划优先 extract_work_phases(W0/W1…) 真任务图**；仅无工作窗的产品 chrome 才回落 meta 四波 + inspect 尾波）
adapters/: cco_v1 · serial_prompts · raw_single（serial 跳过 meta 标题；cco_v1 解析 role/scope/outputs/tags/require_inspect）

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
