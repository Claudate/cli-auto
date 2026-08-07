# src/runtime/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: 子模块与 re-export（Scheduler · LogEvent · ProviderRegistry · WorkerPort · handoff）
scheduler/: **A1-3 多文件编排**（经 **A1-4 WorkerPort + domain/worker 策略**；读取 `auto_commit.json` 执行 host 自动提交）
  · mod.rs: Scheduler 结构 + `run()` 主循环；**per_task worktree 从已提交依赖分支 fork**（后置任务可见前置产物）
  · tick.rs: external_stop · reap · spawn_ready · exit 谓词 · **noop guard**（implement Done 前零产出→Failed）
  · start.rs: start_task · **fork_base_for**（取最新已提交依赖分支）· isolation_on_fail → worktree · terminal open · WorkerPort slot
  · finish.rs: finish_or_retry（FailoverPolicy.classify）· apply_result · archive logs · **auto_commit_task / auto_commit_plan**（记录 hash/files/push 状态，不改变 worker 成败）
  · start.rs: start_task · isolation_on_fail → worktree · terminal open · WorkerPort slot
  · patrol.rs: stall / budget / FailoverPolicy target+preflight
  · gates.rs: outputs · inspect VERDICT **经 handoff facade + domain inspect_gate_fail_reason**（无正文解析）· handoff_task_end
  · active.rs: --only/--from → domain::run::resolve_active_ids
  · types.rs: ProgressWatch · StallAction · mirror_run
  · 行为：并行上限 · 预算 · acceptance · **sys-post-git-push 先巡检 PASS** · 卡死巡检/重试 · H4 failover · **P1 cost escalate（先升档再 walk failover_order）** · **P2 spawn 前 budget downgrade（soft/cost_auto）** · **每轮 reload disk：Aborted/Paused → 杀 worker、冻 pending** · Stopped 不进 failed、run 终态 Aborted
handoff/: **A1-5 多文件适配器**（单文件 ≤600；实现 `ports::HandoffStore`）
  · mod.rs: facade re-export（稳定 `crate::runtime::handoff::*`）
  · model.rs: Handoff/BoardRow/Fragment · load/save · render_md
  · paths.rs: resolve_output_path · missing_outputs · write_task_diff
  · inspect_io.rs: read_inspect_verdict/issues · collect · system_push_inspect_gate（调 domain 纯规则）
  · lifecycle.rs: write_shell · on_task_start/end · on_run_end
  · prefix.rs: build_prompt_prefix · with_handoff_prefix（**H3** integrate/inspect 拼验纪律 3 条 · 无自动 merge · **CCO_STEP** implement 中步观察标记）
  · rework.rs: build_rework_plan（prompt 补 commit）· accept_residual · inspect_loop_view（**auto_rework_run_id / ensure_phase / docs_closeout_only**）· count_rework_rounds
  · store.rs: FsHandoffStore
  · tests.rs: 单元测（原 monolith 迁入）
log_events.rs: worker stdout/stderr → LogEvent · compact_text_tail/floor_char_boundary（CJK 安全）
provider/: **A1-4** WorkerPort（claude + **shell_print** 多 CLI：codex/gemini/qwen/kimi/deepseek/copilot/codebuddy + fake）· **P2-7** `sdk`（默认关）· ProviderRegistry · DTO re-export；`WorkerProvider` 别名
worktree.rs: git worktree 隔离创建/清理 · on_fail 映射 domain IsolationOnFail（混跑 FailClosed）
acceptance.rs: 任务后软验收命令（仅 `is_runnable_verify` 为真时 `sh -c`；人话跳过写 `skipped_not_shell`，≠ PASS）
browser_mcp.rs: **W1/W3 浏览器 MCP** — tags∋`browser` 且 enabled → `mcp-browser.json` · env · Claude `--mcp-config`；**collect_browser_evidence**（shot data URL / report 摘录 → live 结果台）；默认 kitewright（`docs/browser-automation-cco.md`）

## 硬规则（继承 L1）

1. 编排循环**禁止**内嵌 VERDICT 正文解析（gates 只调 handoff/domain API）。  
2. handoff 已切开：**禁止**再合并成单文件巨石；纯解析只进 `domain/inspect`。  
3. 纯状态/重试/路由/隔离/inspect 规则放 `domain/{run,worker,inspect}`；本目录只做 IO 与循环。  
4. 单文件软 400 / 硬 600。  
5. scheduler **只经 WorkerPort** 启停轮询；failover 目标名纯决策、preflight 留本层。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 src/CLAUDE.md
