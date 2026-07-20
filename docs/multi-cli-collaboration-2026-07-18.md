# cco 多 CLI 协作（Claude + Codex 并跑）

> 状态：**P0–P1 全绿 · P2 主线已落地**（P2-1/2/3/4/5/6 ✅ · **t33** tags 路由 + planner provider/role/scope；**不阻塞** D0–D4）  
> 日期：2026-07-18  
> 范围：同 run 内多 `WorkerProvider` 混部 · 任务级 provider/role/scope · 越界约束 · 终闸检验员 · 事中 handoff 账本  
> 角色：编排主路径**增强**子计划——在已有 claude/codex/fake 总线上把「能混跑」收成「可控协作」；**不**另开第二套 Scheduler；**不**替代 Mode B / 分配主路径  
> 关联真源：
> - 编排器 → [`../claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md)（M0–M4 已落地；Codex 已出池；M5 → D5）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md)（未完善唯一总账；本计划增强项入 **D5 池**，不排期则不碰）
> - Mode B → [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)（confirm_start 唯一业务 worker 入口）
> - 聊天共建 → [`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md)（产出散文计划；本计划约束结构化混部字段）
> - 执行闭环 → [`plan-execute-inspect-rework-2026-07-19.md`](./plan-execute-inspect-rework-2026-07-19.md)（计划对照巡检 · 遗漏分级 · 回补波 · **P-loop/P2-11 已落地**；扩展本计划 inspect，**不**合并阶段勾选）
> GEB 入口：[`/CLAUDE.md`](../CLAUDE.md)（L1）· [`./CLAUDE.md`](./CLAUDE.md)（L2 docs）

> **定稿（t1）**：本前言 + §0–§9 冻结角色、契约、字段、阶段与非目标。  
> 实施勾选真源 = **§6**（P0/P1/P2）；**禁止**第二份 P0–P2 总览；**禁止**回灌已冻 D0–D4。  
> 与总账边界：本增强 → D5；发现即改文档债可顺手，不占主排期。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**拆任务时写清：谁跑（CLI）、能碰什么（边界）、产出写哪（账本）；并行互不越界；最后固定一家做检验员做整合验收；全程一份可接力的执行账本，下一棒只靠它调整。**

```text
拆分声明              越界约束             进度账本              检验员
(provider+role+scope) → (worktree+锁) → (handoff.md 接力) → (inspect 终闸)
```

---

## 1. 产品结论

| 问题 | 结论 |
|------|------|
| Claude CLI 与 Codex CLI 能否同一 run 并跑？ | **能**。架构已支持 per-task `provider` + 并行调度 + per-provider 上限 |
| 要不要新引擎？ | **不要**。沿用 `WorkerProvider` + Scheduler + PlanIR |
| 现在卡在哪？ | 产品协议未钉死：声明不全、越界靠自觉、无专用检验员、无事中接力账本 |
| 主推模式 | **模式 A**：同 run 混 provider（不同任务并行） |

### 1.1 三种「并跑」语义

| 模式 | 含义 | 态度 |
|------|------|------|
| **A. 同 run 混 provider** | 一份计划里 t1=claude、t2=codex 并行 | ★ 主推；本计划范围 |
| **B. 同项目多 run** | 两个 `cco run` 盯同一仓库 | △ 慎用；状态隔离但工作区易撞 |
| **C. 竞赛 / 双写后选** | 两家各做一版再 merge | 后期模板；不进 P0 |

---

## 2. 现状锚点（2026-07-18 代码）

| 能力 | 位置 | 状态 |
|------|------|------|
| 多 provider 注册 | `src/runtime/provider/mod.rs` · claude / codex / fake | ✅ |
| 任务级 `provider` | `TaskIR.provider` · `src/plan/mod.rs` | ✅ |
| 计划解析 per-task provider | `src/plan/adapters/cco_v1.rs` | ✅ |
| 全局并行 + per-provider 上限 | `max_parallel` · `providers.*.max_parallel` · scheduler | ✅ |
| 仅 preflight 用到的 provider | `cli/commands/common.rs` `preflight_providers` | ✅ |
| git worktree 隔离 | `src/runtime/worktree.rs`（失败会 **warn 回退** project_root） | ⚠ 混跑需 fail-closed |
| Codex 第二真实 provider | `src/runtime/provider/codex.rs`（print/exec；**无 bg**） | ✅ |
| Claude 项目范围锁文案 | `src/runtime/provider/claude/spawn.rs` `append-system-prompt` | ✅ 仅 Claude |
| host `acceptance` | `src/runtime/acceptance.rs`（任务后 shell 门禁） | ✅ 非 LLM 检验员 |
| 终态报告 | `src/report/mod.rs` · `report.md` / `report.json` | ✅ 事后，非事中接力 |
| 事件流 | `events.jsonl` · `run.json` | ✅ 系统态，非 worker 工作记忆 |
| `role` / `scope` / `outputs` | — | ❌ 本计划新增契约 |
| `handoff.md` host 归并 | — | ❌ 本计划新增 |
| `role: inspect` 终闸 | — | ❌ 本计划新增 |
| CLI `--provider` 覆盖语义 | `cli/commands/run.rs` **全量覆盖**所有 task | ⚠ 混跑陷阱 |

调度语义（已实现）：

```text
ready 任务 → max_parallel
           → providers.<name>.max_parallel
           → registry.get(task.provider).start(...)
           → poll / collect → DAG 推进
```

---

## 3. 协作契约（四条硬规则）

### 3.1 拆分时必须声明

每个任务在进 Scheduler 前必须可解析出：

| 字段 | 含义 | 必填条件 |
|------|------|----------|
| `provider` | `claude` \| `codex` \| `fake` | 混跑计划：**每任务必填**（禁止只靠 default 隐式） |
| `role` | `scout` \| `implement` \| `integrate` \| `inspect` | P1 起结构化；P0 可先写在 title/prompt 约定 |
| `scope.paths` | 可写路径白名单（glob） | `implement` / `integrate` 必填 |
| `scope.readonly` | 可读范围 | 默认 worktree/project；scout 默认可读全库 |
| `scope.forbid` | 硬禁区 | 默认含家目录与跨任务私有树 |
| `outputs[]` | 必须落盘的产物 | 至少 1 个 handoff 片段路径 |
| `depends_on` | DAG | 已有 |
| `acceptance` | host 侧命令（客观） | 建议 inspect 波叠加 |
| `provider_opts` | 私有上限 | Claude: tools/budget；Codex: full_auto/model |

**拆分 = 任命 + 授权**，不允许 runtime 再猜 provider。

### 3.2 默认角色 × CLI 路由

| role | 默认 provider | 典型限制 | 禁止 |
|------|---------------|----------|------|
| `scout` | claude | 只读 tools；worktree 可关 | 改业务代码 |
| `implement` | claude **或** codex | 写死 `scope.paths`；worktree 开 | 改他人 scope、直接合 main |
| `integrate` | **固定一家**（默认 claude） | 可读各产物；写集成分支 | 新开大功能 |
| `inspect` | **固定一家**（建议 claude） | 默认只读业务 + 写报告；跑测 | 静默大改后宣称通过 |

### 3.3 越界定义（可操作）

对 worker，**越界** = 任一：

1. 写入路径 ∉ `scope.paths`
2. 读取/执行触及 `scope.forbid`
3. 改了未声明共享的依赖方文件
4. 在 depends 未完成时假定下游状态
5. 跳过 `CCO_DONE` 或不写约定 `outputs`

**三层边界：**

```text
L1 物理  worktree / cwd；混 provider 并行 → 强制 worktree，失败不回退 project_root
L2 能力  Claude: allowed_tools + permission_mode + scope system prompt
         Codex: 同等 cwd 前缀注入；无 tool allowlist → 更依赖 L1 + 窄 scope
L3 契约  plan scope + host 产物检查 +（P1+）diff 白名单 + inspect 语义审查
```

**validate 硬规则（P1）：**

| 规则 | 动作 |
|------|------|
| 任务 provider 集合 size>1 且存在并行波 | 未全开 worktree → **硬错误** |
| 同波 `implement` 的 `scope.paths` 相交 | **硬错误** |
| `inspect` 未在终闸（仍有未完成业务依赖） | 图校验失败 |
| codex + `mode=bg` | validate 失败 |
| 约定 `outputs` 文件缺失 | collect 后 → Failed |
| 混跑时 `cco run --provider X` | **不得**抹掉已写 provider 的任务（改语义见 §6.P1） |

### 3.4 专门检验员（终闸）

- **不是**又一个 feature worker，而是 run 质量门。
- 与 host `acceptance` **分工**：
  - `acceptance`：host 跑 shell（test/lint），客观 exit code
  - `role: inspect`：专用 CLI 会话，语义审查、越界判断、整合质量、写 VERDICT
- 默认：**只写** `.cco-out/inspect/**`；业务树只读（tools 无 Edit 业务路径）。
- 失败 → 任务 Failed → `on_failure: pause`；账本写 ISSUES，供 rework 波消费（P2）。

```text
implement* (+ integrate) done
        ↓
   inspect（专用 provider 会话）
        ↓
  读 handoff + 代码 + acceptance
        ↓
  PASS → run completed
  FAIL → pause + ISSUES → 可选 rework
```

### 3.5 专门账本（handoff，事中真源）

| 文件 | 读者 | 时机 | 重心 |
|------|------|------|------|
| `events.jsonl` | 系统/调试 | 实时 | 原始事件 |
| `run.json` | 系统 | 实时 | 状态机 |
| **`handoff.md` + `handoff.json`** | **下一棒 CLI + 人** | **每任务终态更新** | 进度、边界、产物、风险 |
| `report.md` | 人/归档 | 结束 | 成本与终态摘要 |

**硬原则：**

- worker **只写**自己的 fragment（scope 内，如 `.cco-out/<task_id>/SUMMARY.md`）
- **仅 host** 归并全局 `~/.cco/runs/<run_id>/handoff.md`（避免并行写冲突）
- 下一任务 **start 前**，host 将最新 handoff 摘要 **注入 prompt 前缀**
- **禁止**用 `report.md` 冒充事中账本

#### handoff.md 固定章节

```markdown
# CCO Handoff · run_id=<id>
updated: <rfc3339>
project: <path>
plan: <path>
status: running|completed|failed

## Board
| id | provider | role | status | scope | outputs | cost | notes |
|----|----------|------|--------|-------|---------|------|-------|

## Timeline
- …

## Fragments
### <task_id>
- status / provider / work_dir / branch
- summary（或链到 SUMMARY.md）
- artifacts
- risks / followups

## Open risks
- …

## Instructions for next worker
- 当前可启动 / 你的 scope / 禁止 / 必读 Fragments
```

#### 启动注入包（host 拼进 prompt）

```text
[CCO_HANDOFF]
你是 task={id} provider={p} role={r}
scope.paths=…
scope.forbid=…
必读: Board + Fragments({depends_on})
全局账本: {run_dir}/handoff.md
你的 outputs: …
完成后最后一行: CCO_DONE ok
[/CCO_HANDOFF]

（业务 prompt …）
```

#### 生命周期

```text
run start     → handoff 空壳（Board = 全部 pending）
task start    → Board → running；注入 handoff 摘要
task done/fail→ 检查 outputs → 归并 fragment → 更新 Board/Timeline/risks
next start    → 再读最新 handoff 注入
inspect done  → VERDICT 写入账本；report 可链 handoff 路径
resume        → 从 handoff + run.json 恢复
```

---

## 4. 端到端主路径

```text
1. 拆分（人 / Mode B / 聊天落盘后结构化）
   → 每任务: provider + role + scope + outputs
   → 尾波必须有 role=inspect（或 plan.require_inspect=true）
   → validate: 路径不交、混跑 worktree、inspect 终闸、codex 无 bg

2. confirm_start（唯一业务 worker 入口）
   → 写 handoff 空壳
   → preflight 仅用到的 CLI

3. 并行 implement（claude ‖ codex）
   → 各 worktree + scope 注入
   → 禁止 --provider 抹掉已声明引擎
   → 每任务结束归并 handoff

4. integrate（单 provider）
   → 读 handoff 全量 + fragments
   → 合流 / 解冲突
   → 更新 handoff

5. inspect（专门 CLI）
   → 只读业务 + 写 VERDICT/ISSUES
   → host acceptance 叠加
   → FAIL → pause + Open risks

6. report
   → report.md 链接 handoff.md
   → 归档
```

### 4.1 推荐波次形状（安全默认）

```text
Wave 0  scout（claude，只读）
Wave 1  implement* 并行（claude ‖ codex，各 worktree，scope 不相交）
Wave 2  integrate（固定一家）
Wave 3  inspect（专门 CLI 终闸）+ host acceptance
```

---

## 5. 计划与配置示例

### 5.1 混合计划（P0 目标形态；P1 起 role/scope 结构化）

```yaml
# docs/plans/mixed-claude-codex-inspect.cco.yaml
schema: cco-plan/v1
name: mixed-claude-codex-inspect
# P1: require_inspect: true
defaults:
  mode: print
  worktree: true
  providers:
    claude:
      max_turns: 40
      max_budget_usd: 8
      permission_mode: dontAsk
      allowed_tools: [Read, Edit, Bash, Glob, Grep, Write]
    codex:
      full_auto: true
      json: true
max_parallel: 4
on_failure: pause
retry_max: 1

tasks:
  - id: inventory
    title: 只读摸底
    provider: claude
    # role: scout
    prompt: |
      只读梳理仓库结构。产物：.cco-out/inventory/SUMMARY.md
      最后一行：CCO_DONE ok

  - id: feat-a
    title: 模块 A（Claude）
    provider: claude
    # role: implement
    # scope.paths: [src/module_a/**, .cco-out/feat-a/**]
    depends_on: [inventory]
    prompt: |
      仅在约定 scope 内实现模块 A。
      产物：.cco-out/feat-a/SUMMARY.md 与 CHANGED.md
      CCO_DONE ok

  - id: feat-b
    title: 模块 B（Codex）
    provider: codex
    # role: implement
    # scope.paths: [src/module_b/**, .cco-out/feat-b/**]
    depends_on: [inventory]
    prompt: |
      仅在约定 scope 内实现模块 B。
      产物：.cco-out/feat-b/SUMMARY.md 与 CHANGED.md
      CCO_DONE ok

  - id: integrate
    title: 汇合
    provider: claude
    # role: integrate
    depends_on: [feat-a, feat-b]
    prompt: |
      读 handoff 与 .cco-out/feat-*/，做整合与冲突消解。
      产物：.cco-out/integrate/MERGE.md
      CCO_DONE ok

  - id: inspect
    title: 代码检验员
    provider: claude
    # role: inspect
    depends_on: [integrate]
    provider_opts:
      allowed_tools: [Read, Glob, Grep, Bash, Write]
      max_budget_usd: 4
    acceptance: "cargo test"
    prompt: |
      你是代码检验员，不是实现者。
      1. 读 HANDOFF / 各 SUMMARY
      2. 核对声明 scope vs 实际变更
      3. 解读 acceptance；检查整合一致性
      4. 写 .cco-out/inspect/VERDICT.md（PASS|FAIL）与 ISSUES.md
      默认不改业务代码。
      CCO_DONE ok
```

### 5.2 配置

```toml
# ~/.cco/config.toml
[default]
default_provider = "claude"
max_parallel = 4
worktree = true

[providers.claude]
enabled = true
bin = "claude"
max_parallel = 2

[providers.codex]
enabled = true
bin = "codex"
max_parallel = 2
```

### 5.3 运行注意

```bash
cco doctor
cco parse --project /path/to/repo \
  --plan examples/plans/mixed-claude-codex-inspect.cco.yaml
cco run --project /path/to/repo \
  --plan examples/plans/mixed-claude-codex-inspect.cco.yaml --yes

# 混跑时不要使用会抹掉 per-task provider 的全局覆盖
# --provider：仅软覆盖 default / 未声明任务（混部已声明引擎保留）
# --force-provider：硬抹全部任务引擎 —— 混跑禁止
```

> **索引（追加）**：可复制样例真源 = `examples/plans/mixed-claude-codex-inspect.cco.yaml`（P0-2）；文件头含 handoff 章节样板（P0-3）与混跑红线（P0-4）；§5.1 内嵌骨架仍为形态说明，以 examples 路径跑通为准。

### 5.4 Codex vs Claude 能力落差（拆分时必须写清）

| 控制项 | Claude | Codex | 对策 |
|--------|--------|-------|------|
| tool allowlist | ✅ | 弱/无 | Codex 任务 scope 更窄 |
| budget_usd | ✅ | 口径不同 | 报告分栏；总预算严格侧偏 Claude |
| scope system prompt | ✅ 已有 | 需同等注入 | P1 codex start 拼 cwd 锁 |
| mode=bg | ✅ | ❌ | validate 禁 bg |

---

## 6. 阶段切分与勾选

> 实施勾选真源 = 本节。总账仅记「D5 增强：多 CLI 协作」，细节以本文件为准。

### P0 — 协议与示例（文档 / 示例为主）

| # | 项 | 状态 |
|---|----|------|
| P0-1 | 本计划定稿并挂 L1/L2 索引 | ✅ t1 |
| P0-2 | `examples/plans/mixed-claude-codex-inspect.cco.yaml`（或 `docs/plans` 样例） | ✅ |
| P0-3 | handoff.md 章节规范 + 启动注入文案（本 §3.5）可复制 | ✅（§3.5 真源 + 示例 YAML 头注释样板） |
| P0-4 | 文档标明：混跑禁止依赖 `--provider` 全覆盖；必须 worktree | ✅（§5.3 + 示例文件头） |
| P0-5 | 本地 smoke：同 run events 出现 `provider:claude` 与 `provider:codex` 的 `task_start` | ✅ `tests/mixed_provider_smoke.rs` |

**P0 成功标准**：有人能按示例跑通「双 implement + integrate + inspect」形状（inspect 可先纯 prompt 约定）。

### P1 — host 硬保障（代码）

| # | 项 | 状态 |
|---|----|------|
| P1-1 | `PlanIR`/`TaskIR`：`role`、`scope`、`outputs`（或等价 JSON 字段）+ adapter 解析 | ✅ `TaskIR` + `cco_v1` · 旧计划 `role/scope/outputs` 缺省兼容 |
| P1-2 | `validate`：混 provider → worktree 必开；并行 scope 不相交；codex 禁 bg；可选 `require_inspect` | ✅ `PlanIR::validate_collab_rules` · lib 单测 p1_2_* |
| P1-3 | `resolve_work_dir`：混跑路径 **fail-closed**（禁止 silent 回退 project_root） | ✅ |
| P1-4 | 维护 `run_dir/handoff.md` + `handoff.json`；task 终态归并 fragment；检查 outputs 存在 | ✅ |
| P1-5 | task start 注入 `[CCO_HANDOFF]…` 前缀 | ✅ `handoff::with_handoff_prefix` · scheduler `start_task` · fake `prompt.md` 断言 |
| P1-6 | Codex 启动注入与 Claude 同级的 cwd/scope 文案前缀 | ✅（`build_scope_prefix` / `with_scope_prefix`；unit tests） |
| P1-7 | `cco run --provider`：仅覆盖 default / 未声明任务；全覆盖改 `--force-provider` | ✅ |
| P1-8 | report/status 按 provider 分栏（running 数、cost）；report 链 handoff 路径 | ✅ `report::summarize_providers` · by_provider · handoff 路径；status 分栏 |

**P1 成功标准**：非法混部计划在 validate 阶段失败；合法混部自动写 handoff 且下一任务 prompt 含账本摘要。

### P2 — 检验员与分配体验（按需）

| # | 项 | 状态 |
|---|----|------|
| P2-1 | `role: inspect` 默认 opts（只读业务 tools + 仅写 inspect 目录） | ✅ `materialize_role_defaults` · load_plan |
| P2-2 | host 预生成 per-task diff 列表供 inspect 消费 | ✅ `write_task_diff` → `.cco-out/<id>/CHANGED.md` · on_task_end · unit |
| P2-3 | VERDICT=FAIL → pause + Open risks 写 ISSUES 摘要；轻量 REWORK_HOOK（不自动 merge/PR） | ✅ `enforce_inspect_verdict` · handoff `ISSUES[task]` / `REWORK_HOOK` · tests |
| P2-4 | 任务 `tags` + 简单 routing 表（L1 分配） | ✅ **已落地**（t33）：`TaskIR.tags` · cco_v1 解析 · `apply_tag_routing`（codex/claude/fake 标签软路由，不覆盖显式 provider） |
| P2-5 | Mode B planner / 聊天结构化输出带 provider+role+scope | ✅ **已落地**（t33）：LLM schema + `LlmTask` 解析 provider/role/scope/outputs/tags；标题/tag 可推断 inspect |
| P2-6 | 桌面确认屏任务表可改引擎；Board = handoff 总览 | ✅ 确认屏 provider 可见 + `handoff_board` strip · 打开 handoff.md |

**P2 成功标准**：默认混跑模板含 inspect 终闸；FAIL 可暂停并留下可消费 ISSUES。

### 分配策略档位（与阶段对应）

| 档 | 内容 | 阶段 |
|----|------|------|
| L0 手写 per-task provider | 已有 cco-plan/v1 | P0 |
| L1 规则路由 / 确认屏改引擎 | tags、桌面列 | P2 |
| L2 planner 智能建议 provider | Mode B JSON | P2 |

---

## 7. 架构落点（改哪里）

不引入新大模块：

| 层 | 变更 |
|----|------|
| `plan/` | TaskIR 扩展 role/scope/outputs；validate 硬规则；`require_inspect` |
| `plan/adapters/cco_v1` | 解析新字段；示例 plan |
| `runtime/scheduler` | handoff 归并；start 注入；混跑 worktree fail-closed；outputs 检查 |
| `runtime/worktree` | 混跑不静默回退 |
| `runtime/provider/claude` | 已有 scope lock；可拼 role 段 |
| `runtime/provider/codex` | cwd/scope 前缀；validate 禁 bg |
| `cli/run` | `--provider` vs `--force-provider` |
| `state` / `report` | handoff 路径；分栏；链接 |
| `doctor` | 对 plan 用到的多家分别检查（已有雏形） |
| 桌面/TUI | P2：引擎列、Board |

**刻意不做（非目标）→ 见 §8。**

---

## 8. 非目标

| ID | 非目标 |
|----|--------|
| N1 | 跨 provider 共享同一会话 / 统一 tool schema |
| N2 | core 内自动 3-way merge / 自动开 PR（仍属 M5/D5 其它项） |
| N3 | 把 Codex 伪装成 Claude `bg` agent |
| N4 | 用 SDK 进程内双嵌替代 CLI `WorkerProvider` |
| N5 | 用 `report.md` 替代事中 `handoff.md` |
| N6 | 把 inspect 做成默认可大改业务代码的超级 worker |
| N7 | 阻塞或回灌已冻 D0–D4 / 另起第二套总账 |

---

## 9. 风险与决策默认

| 风险 | 默认对策 |
|------|----------|
| 同目录写冲突 | 混写强制 worktree；并行 scope 不相交 |
| `--provider` 抹掉混部 | P1 改语义；文档 P0 标红 |
| Mode B 一刀切 provider | 混部用结构化 plan；P2 planner 输出 per-task |
| Codex 无 bg / 弱 tool 锁 | 禁 bg；窄 scope；同等 system 前缀 |
| 预算口径不齐 | 分栏展示；run 总预算对有 cost 的 provider 累加 |
| 无自动 git merge | integrate 任务 + 可选 acceptance；不进 core 魔法 |
| auth/额度独立 | doctor 分家；`on_failure: continue` 可选 |
| worker 互不可见 | **handoff 真源** + start 注入，不靠翻兄弟 stdout |

### 9.1 拍板默认值

| 项 | 默认 |
|----|------|
| 混跑是否必写 per-task provider | 是 |
| implement 是否必写 scope.paths | 是（P1） |
| 检验员默认 provider | `claude`（可配 `inspect_provider`） |
| 检验员能否改业务代码 | 默认否 |
| 账本路径 | `~/.cco/runs/<run_id>/handoff.md`（可选镜像 project） |
| 谁写全局账本 | **仅 host** |
| 下一 CLI 如何知情 | start 时 host 注入 handoff |
| 越界发现 | host（路径/产物）+ inspect（语义）双保险 |
| 与总账关系 | **D5 增强**；不排期则不碰 |

---

## 10. 成功标准（总览）

| # | 标准 | 对应 |
|---|------|------|
| S1 | 同 run 可同时出现 claude 与 codex 的 running/done 任务 | P0 |
| S2 | 拆分文档/计划每任务可见 provider + 限制摘要 | P0/P1 |
| S3 | 越界（无 worktree 混写 / scope 相交 / 缺 outputs）在 host 层失败 | P1 |
| S4 | 每 run 有持续更新的 handoff，且下一任务 prompt 含摘要 | P1 |
| S5 | 存在专门 inspect 终闸；PASS/FAIL 落盘；与 acceptance 分工清晰 | P0 约定 / P2 产品化 |
| S6 | 不破坏 Mode B `confirm_start` 唯一入口与单 provider 旧计划 | 全程 |

---

## 11. 决策树

```text
要两家一起跑？
├─ 同一计划、不同任务并行 → 模式 A（本计划）
│    ├─ 会改代码？ → worktree=true（必须）
│    ├─ 每任务写清 provider + scope + outputs
│    └─ 尾波 inspect（专门 CLI）
├─ 两个 cco 进程盯同一 repo → 模式 B（不推荐写冲突场景）
└─ 两家比一比 → 模式 C（后期模板）
```

---

## 12. 修订历史

| 版本 | 日期 | 说明 |
|------|------|------|
| t1 | 2026-07-18 | 初稿定稿：可行性结论 + 四条契约（声明/越界/检验员/账本）+ P0–P2 + 非目标 + 示例计划骨架 |
| t33 | 2026-07-20 | P2-4/P2-5 落地：`TaskIR.tags` + `apply_tag_routing`；LLM planner 解析 provider/role/scope/outputs/tags；cco_v1 解析 tags；确认屏引擎列仍可手改 |

> **修订规则**：既有行语义禁止改写；后续变更 **另起行追加**。阶段勾选只改 §6 状态列。

---

## 附录 A · 检验员检查清单（prompt 模板要点）

1. **完整性**：账本每个 done 任务是否有 SUMMARY / 约定 outputs  
2. **越界**：实际 diff 路径是否 ⊆ 声明 scope  
3. **整合**：跨模块接口、重复定义、半截 merge  
4. **验收**：acceptance / 测试结果  
5. **可接力**：FAIL 时 ISSUES 含文件 + 症状 + 建议，可供 rework 直接消费  

## 附录 B · 代码锚点速查

| 主题 | 路径 |
|------|------|
| Provider 总线 | `src/runtime/provider/mod.rs` |
| Claude spawn / scope lock | `src/runtime/provider/claude/spawn.rs` |
| Codex | `src/runtime/provider/codex.rs` |
| Scheduler / provider slot | `src/runtime/scheduler.rs` |
| Worktree | `src/runtime/worktree.rs` |
| PlanIR / TaskIR / tags 路由 | `src/plan/mod.rs`（`apply_tag_routing`） |
| cco-plan/v1（role/scope/tags） | `src/plan/adapters/cco_v1.rs` |
| Mode B LLM collab 字段 | `src/plan/planner/llm.rs`（`LlmTask` provider/role/scope/tags） |
| run --provider 覆盖 | `src/cli/commands/run.rs` |
| acceptance | `src/runtime/acceptance.rs` |
| report | `src/report/mod.rs` |
| state / events | `src/state/mod.rs` |
