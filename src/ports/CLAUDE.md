# src/ports/
> L2 | 父级: /src/CLAUDE.md

成员清单
mod.rs: ports 根；re-export WorkerPort DTO · HandoffStore · **SplitAgentPort**；A0 marker
worker.rs: **A1-4** `WorkerPort` trait（start/poll/stop/collect/preflight/capabilities）+ TaskStatus/Capabilities/StartCtx/WorkerHandle/WorkerStatus/TaskResult
handoff.rs: **A1-5** `HandoffStore` trait（write_shell · on_task_start · on_task_end · on_run_end）；实现在 `runtime/handoff::FsHandoffStore`
split_agent.rs: **OpenHands 落地** `SplitAgentPort` + `SplitRequest` → `CcoSplitJob`（Plan Mode；实现 `plan/split_agent`）

目标（architecture-redesign 附录 B，未建勿假造）：
PlanJobStore · RunStore · ChatStore（A1-6 未建，chat 用 free-fn facade）· PlannerPort · ProcessPort · WorktreePort · Clock

## 硬规则

1. **trait + DTO only**；实现落在 `runtime/provider` / `runtime/handoff`（现）或未来 `adapters/*`。  
2. **禁止**第二总线 / 再发明 `XxxManager`。  
3. Domain/App 可依赖 ports；ports **不**依赖 UI/clap/tauri。  
4. 体积：软 400 / 硬 600。  
5. HandoffStore 调用方 **禁止** VERDICT 正文解析（解析在 domain/inspect）。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /src/CLAUDE.md
