# src/domain/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 领域根；A0 骨架 marker；挂 `plan` · `run` · `worker` · `inspect` · `chat`
plan/: **A1-1** PlanIR/TaskIR/TaskRole（+**Closeout**）/TaskScope/OnFailure · MAX_* · optional 标题 · materialize_selected/role（**空 inspect depends_on → 业务叶子**）· **closeout**（`inject_closeout_task` · 剥离 inspect 关账文案 · `sys-closeout`）· **checklist**（`HostChecklist` / `plan.checklist.json` 结构）· validate+collab · **soften_plan_for_accept**（LLM 出图软接受：scope 重叠串行/补 scope，避免整图丢弃）· **cco_split**（CcoSplitJob/Task · soft_accept（**scope_paths 重叠串行** · H3 中文排队提示）· **sanitize_cco_split_deps** · from/to PlanIR · run_gate · waves · **humanize** summary/done_when/依赖列 · 禁 worker 首行）· **verify**（`is_runnable_verify` · H0 人话≠shell）· **merge_check**（H3 拼完怎么验 · 禁默认 MERGE.md）· apply_tag_routing（返回 rewritten ids）· **tag_implied_provider** · system post ids · **TaskRole::parse/as_str · parse_role_input**
run/: **A1-3** 纯 run 规则 — status（终态/external-stop/slot/budget/stall）· **status_line**（**H1** `StatusOneLiner` · 双源 job∥run · 禁 web 复制）· retry（SameProvider/TryFailover/Permanent · **next_failover_target(order,tried)** · 默认 claude,codex；fake/sdk 永不自动）· active（--only/--from 下游展开）；**无**路径拼接 / provider IO / VERDICT 解析
worker/: **A1-4** 纯 worker 策略 — ProviderId（claude/codex/fake/sdk + gemini/qwen/kimi/deepseek/copilot/codebuddy）· WorkerRoute/CapabilityFlags · soft-fill Soft/Force · **FailoverPolicy{order}** · IsolationOnFail / is_multi_provider；**无** spawn / worktree 路径 / RunState
inspect/: **A1-5** 纯 VERDICT/ISSUES — types（InspectVerdict/IssueSeverity/ParsedIssue · REWORK_MAX · MAP 白名单）· parse（parse_verdict_text/parse_issues_text）· gate（candidate paths · task_has_verdict_gate · count_blocking · inspect_gate_fail_reason · push_inspect_gate_decision · can_start_rework）· **classify**（`is_docs_closeout_issue` / `all_blocking_are_docs_closeout` · Ensure E0）；**无**路径拼接 / fs / git
chat/: **A1-6** 纯 chat 规则 — fence（extract_plan_fence 嵌套 depth + CJK）· title（sanitize/extract H1）· normalize/structure_plan_markdown · **acceptance_quality / AcceptanceQuality / acceptance_hint / acceptance_is_stub（P1-4；`## 成功标准`=验收别名）** · **parse_acceptance_checklist / collect_task_acceptance_items / build_verification · VerificationView（P2-1 清单 vs 巡检）** · stream_parse（extract_assistant_text）· id（sanitize_session_id）· text（truncate_chars）· **plan_writing_guidance**（从 `docs/runtime-prompts/*.md` 加载 · 覆盖序同 README · `include_str` 回落；聊天/拆分注入：chat-plan-writing + **ui-delivery-recipes** + backend + layout + color + type + **copy** + motion；planner greenfield 用 planner-greenfield-stack）

## 硬规则

1. **禁止**依赖 tauri / clap / UI / 具体 provider 实现（L1 #6）。  
2. **禁止**拼 run_dir / plan_jobs / `.cco/chat` 路径（Store / services 适配器的事）。  
3. 纯函数优先；需要 IO 的放 `plan` facade、`runtime/handoff` 或 `services/chat` 适配器。  
4. 体积：业务文件软 400 / 硬 600 行。  
5. **run.json schema `cco-run/v1`** 落盘类型仍在 `state/`；domain 只持决策规则，不改 wire 形状。  
6. soft-fill **不得**静默覆盖任务上已显式声明的 route（全量覆盖须 Force）。  
7. VERDICT 正文解析只在 `inspect/parse`；scheduler **禁止**内嵌解析。  
8. chat 纯规则 **不**开跑、**不** spawn worker；只服务「生成计划」步。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /src/CLAUDE.md
