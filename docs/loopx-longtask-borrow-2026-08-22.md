# LoopX 借鉴 · 长任务治理优化与实施计划

> 类型：**实施真源**（本文为「LoopX 借鉴」唯一勾选落点 · 代号 **LX0–LX3**）
> 日期：2026-08-22
> 来源：GitHub `huangruiteng/loopx`（5k★ · ByteDance AML · Apache-2.0 · v0.5.1）深度分析
> 上位参考：[`harness-inspired-roadmap-2026-08-14.md`](./harness-inspired-roadmap-2026-08-14.md)（方向参考 · 本文是其 §A 的一个具体落点）
> 约束：架构规则 3（禁止平行架构阶段表）· 规则 8（组合逻辑放 domain/app · 薄编排器）· 规则 10（confirm 唯一开跑）· 规则 14（optional 不静默 auto-start）· 规则 23/24（PM 轻量 · 高级默认关）
> 真源边界仍认：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（A0–A5 ✅）

[PROTOCOL]: 勾选只认本文 §四任务表；本文**不**改架构边界，只在既有 `domain/run` + `app/run` 内收敛决策；与上位 roadmap 冲突时以各功能真源为准。

---

## 〇、一句话

**LoopX 验证了 Leaf 的内核方向是对的**——同一道「长任务不跑飞、可复盘、可交接」的题，字节工程师独立收敛到了和 Leaf 几乎同构的边界。本文只借鉴**一个高价值机制**（把散落在 scheduler 里的「这一拍要不要动」判断，收敛成 `domain/run` 一个纯决策函数），外加两个轻量增强（gate 对象化、可选 safe-fallback），**明确拒绝**照搬 LoopX 的 headless 常驻 / peer-lease / control-plane 形态。

---

## 一、LoopX 是什么（深度定位）

| 维度 | LoopX | Leaf/cco |
|------|-------|----------|
| 本质 | **骑在** Codex/Claude Code/Cursor 之上的**状态治理层**（control plane），自己不干活；作者自称「a state kernel with a CLI」 | **自己就是**编排器：拆分→并行→巡检，直接调 provider 干活 |
| 时间尺度 | 跨天/跨重启/跨 Agent，heartbeat 自动唤醒，公开案例 200+ 小时 elapsed | 单次 run 为主（A1 Run Resume checkpoint 已落，但非「目标常驻」） |
| 交互入口 | CLI-first · headless · 心跳调度自动续跑 | 桌面 confirm · 人话 DTO · PM 目视 |
| 主受众 | 工程师 / 研究员 | **PM / 出海 / 非开发**（PRODUCT.md 真源） |
| 人机边界 | user gate = 一等结构化对象，人类拍板 | confirm 唯一开跑 + optional 必停 |
| 分层 | Kernel → Capability → Provider → Extension | Presentation → Application → Domain → Adapter（六边形，A0–A5 ✅） |

**LoopX 的长任务四大机制**（README + dev.to 评测 + 源码结构佐证）：

1. **Quota / should-run**：每 tick 先问「这个 Agent 现在该不该动」，返回 `deliver / ask / wait / self-repair / quiet`；**只有验证过 writeback 之后才记一次 spend**，quiet-skip / preflight 失败 / dry-run **不计费**——防 heartbeat 心跳烧 token 的核心闸。
2. **User Gate（一等对象）**：把「需要人拍板」做成带 id/理由的结构化对象，循环能「看到自己卡在哪个具体问题上」，而非模糊的「等 owner」。
3. **Safe Fallback（审计过的旁路）**：一条 lane 被 gate 卡住时，另一条**被审计的**旁路可继续推进而**不绕过** gate——避开「全停」与「让 Agent 自己决定」两个极端（dev.to 作者最推崇的机制）。
4. **Evidence + Handoff**：append-only run history（progress/validation/blocker/spend）+ 交接账本，任务可复盘、可重启、可交接。

---

## 二、逐条机制对照（LoopX ↔ Leaf 现状 · 附代码坐标）

| LoopX 机制 | Leaf 现状（已有） | 代码坐标 | 差距判定 |
|-----------|------------------|----------|----------|
| quota `should-run` 前置决策 | 「花超了才降档/停」+ stall/slot 判断散在 scheduler | [cost_budget.rs](../src/domain/worker/cost_budget.rs) `budget_tier_ceiling` · [status.rs](../src/domain/run/status.rs) `budget_exceeded`/`stall_triggered`/`provider_slot_open` · [tick.rs](../src/runtime/scheduler/tick.rs) + [patrol.rs](../src/runtime/scheduler/patrol.rs) | **有零件无总装**：纯谓词已备齐，但「本拍决策」逻辑散在 scheduler 循环里，无统一 `TickDecision`，无 quiet-skip 显式语义 → **LX1 收敛** |
| user gate 一等对象 | optional 必停 = 布尔勾选框 | `planNeedsOptionalConfirm`（web）· [status_line.rs](../src/domain/run/status_line.rs) `StatusOneLiner` | **半有**：能停，但没把「等你回答什么」做成人话对象 → **LX2** |
| safe fallback 旁路 | gate fail → 停 / auto-rework | [ensure_loop.rs](../src/app/run/ensure_loop.rs) `maybe_auto_rework` · [collab_gate.rs](../src/runtime/scheduler/collab_gate.rs) `WaitGate{Proceed,Defer,Fail}` | **无**：卡住即空等/停 → **LX3 可选后置** |
| evidence / run history | events.jsonl 权威 + Tauri emit | [event_bus.rs](../src/ports/event_bus.rs) · B1 事件总线全落 | **同构 ✅**（不需借鉴） |
| checkpoint / resume | R1–R4 已落 | [run-resume-checkpoint-2026-08-14.md](./run-resume-checkpoint-2026-08-14.md) | **已有 ✅** |
| handoff 交接账本 | HandoffStore + outputs | `runtime/handoff/*` · [ports/handoff.rs](../src/ports/handoff.rs) | **同构 ✅** |
| 上下文记忆 / reward memory | 纯 Rust agentmemory（P3 完成） | [ports/memory.rs](../src/ports/memory.rs) | **Leaf 更靠前**（LoopX 该能力仍 experimental/default-off） |
| inspect → transition | VERDICT 纯解析 + rework | `domain/inspect/*` | **同构 ✅** |

**结论**：真正值得动手的只有 **LX1**（should-run 收敛，高价值）；LX2 低成本高体感；LX3 复杂度高、非 PM 刚需，仅登记不排期。

---

## 三、设计（借鉴而非移植）

### LX1 · `TickDecision` 纯决策收敛（核心）

**动机**：现在 scheduler 每拍是否推进，取决于散在 [tick.rs](../src/runtime/scheduler/tick.rs) / [patrol.rs](../src/runtime/scheduler/patrol.rs) 的若干 `if`（`budget_exceeded()` → 插 `__budget__`、`stall_triggered`、`provider_slot_open`、ready 集）。这违反硬规则 8（组合逻辑应在 domain/app，编排器要薄）且难测。LoopX 的 should-run 把它收敛成一个**纯函数决策**。

**做法**：在 `src/domain/run/` 新增 `tick_decision.rs`，用已有纯谓词组合出一个决策枚举——**不新增策略，只搬家 + 命名**：

```rust
//! [POS]: domain/run — pure tick decision (borrowed from LoopX should-run)
//! [PROTOCOL]: 组合既有谓词(budget_exceeded/stall_triggered/provider_slot_open)；
//!   不新增策略；不 IO；scheduler 只消费本枚举（硬规则 8）
pub struct RunTickSnapshot {
    pub spent: f64,
    pub cap: Option<f64>,
    pub ready_ids: Vec<String>,
    pub running: usize,
    pub slot_cap: Option<usize>,
    pub any_stalled: bool,
}

pub enum TickDecision {
    /// 有就绪任务且额度/槽位允许 → 派生这些（≈ LoopX deliver）
    Spawn(Vec<String>),
    /// 槽位满或就绪集空但仍在跑 → 下一拍再看，**不计费**（≈ LoopX wait/quiet）
    Wait { reason: &'static str },
    /// 预算超顶 → 收口（≈ LoopX quota stop；对齐现 `__budget__`）
    Halt { reason: &'static str },
}

pub fn decide_tick(s: &RunTickSnapshot) -> TickDecision { /* 组合 status.rs 谓词 */ }
```

**收敛点**（行为等价，不改现有语义）：
- `budget_exceeded(spent,cap)` == true → `Halt`（替代现在裸插 `"__budget__"`）；
- `!provider_slot_open(running, slot_cap)` → `Wait{"slots_full"}`；
- `ready_ids` 空但仍有 running → `Wait{"awaiting_running"}`（**quiet skip 语义**：明确「这拍安静跳过」，为将来 heartbeat 续跑打底）；
- 否则 → `Spawn(ready_ids)`。
- scheduler [tick.rs](../src/runtime/scheduler/tick.rs) 改为：算 snapshot → `decide_tick` → match 执行副作用。**tick 更薄**。

**为何值得**：① 直接服务 [harness-inspired-roadmap](./harness-inspired-roadmap-2026-08-14.md) §A1 的续跑/heartbeat 方向——续跑最需要「这拍要不要动、还是安静跳过（不烧钱）」的前置闸；② 决策变纯函数 → 可单测（rule 4.3 domain 层）；③ 消灭 scheduler 里的裸 `__budget__` 魔法串。

### LX2 · `pending_user_gate` DTO 对象化（轻量高体感）

把 optional「必停」从布尔升级成人话对象，塞进 Run/Result DTO：

```rust
pub struct PendingUserGate {
    pub kind: GateKind,          // OptionalConfirm | InspectReview | ...
    pub question: String,        // 「是否执行可选任务：部署到预览环境？」
    pub why: String,             // 「该任务标了 optional，需你确认」
}
```

- 归属：`domain/run`（纯构造）+ `app/run` 填充 DTO；web 只渲染（rule 22）。
- 复用 [status_line.rs](../src/domain/run/status_line.rs) `StatusOneLiner` 的人话映射风格。
- 对 PM 受众价值高：「当前等你回答：X」比「有个可选任务」清楚得多，且**不违反**规则 14（仍是停住等确认）。

### LX3 · Safe Fallback 旁路（登记 · 不排期）

一条 lane 被 gate 卡住时，允许**声明过的、审计过的**旁路任务继续（不绕 gate）。复杂度高（需在 `collab_gate.rs` 的 `WaitGate` 之外引入「旁路可跑集」+ 审计事件），且非 PM 刚需。**仅登记为未来方向，本轮不做。**

---

## 四、任务表（勾选真源）

| ID | 任务 | 完成定义 | 状态 |
|----|------|----------|------|
| **LX0** | 本分析文档 + 对照表 + 索引接线 | 落 docs/ · docs L2 一行指针 | ✅ 2026-08-22 |
| **LX1-a** | `domain/run/tick_decision.rs`：`RunTickSnapshot`/`TickDecision`/`decide_tick`（组合既有谓词，无新策略）+ 单测 | `cargo test -p cco domain::run` 绿；覆盖 Halt/Wait/Spawn 三分支 | ✅ 2026-08-22 |
| **LX1-b** | [tick.rs](../src/runtime/scheduler/tick.rs) 改用 `decide_tick`；删裸 `__budget__` 插入点改由 `Halt` 驱动 | A0 金样 + scheduler_fake + mixed + mode_b 全绿；行为零漂移 | ✅ 2026-08-22 |
| **LX2** | `PendingUserGate` domain 构造 + `app/run` 填 DTO + web 只渲染 | 拆分台/Run 台显示「等你回答：X」；无 UI 业务策略（rule 22） | ✅ 2026-08-22 |
| **LX3** | Safe Fallback 旁路 | — | ☐ **登记不排期** |

**完成定义（LX1+LX2）**：scheduler tick 更薄；tick 决策可 domain 单测；PM 能在停住时看到人话待办；无行为回归红线；不改 run_dir/events schema/IPC 名。

---

## 五、非目标（明确拒绝照搬 · 否则违规）

| LoopX 形态 | 为何不抄 |
|-----------|----------|
| headless 心跳常驻自动续跑几天 | 违反规则 14（禁止静默 auto-start 跳过可选确认）+ PRODUCT PM 桌面 confirm 定位 |
| peer claim/lease 多 Agent 对等抢活（无 leader） | 你是「confirm 唯一开跑 + 薄编排器推进」，引入租约会把 Orchestrator 重新变肥（规则 8） |
| control-plane「骑在别人之上」只治理不干活 | 你自己就是编排器，多一层无收益 |
| CLI-first / 复杂 quota 分级（deliver/ask/self-repair 全套） | 只取 wait/quiet/halt 三态够用；全套是工程师工具，非 PM 心智（规则 26 同屏概念 ≤3） |

---

## 六、验证与风险

- **验证**：LX1 靠 domain 单测锁三分支 + 既有金样（A0/mode_b/scheduler_fake/mixed）证明行为零漂移；LX2 靠 fake 五步桌面冒烟目视。
- **风险**：LX1-b 触碰 scheduler 热路径 → 缓解：LX1-a 先纯函数 + 单测，LX1-b 只做「等价替换」（先跑金样 diff 再合），不改任何阈值常量。
- **回滚**：LX1 是内部重构，`TickDecision` 可随时退回内联 `if`；无对外契约变更。

---

*本文是「LoopX 借鉴」唯一实施真源。**LX0/LX1-a/LX1-b/LX2 ✅**（2026-08-22）；LX3 登记不排期。变更勾选时更新本头部与 §四，并同步 docs L2 指针（地图与地形同构 · 规则 4）。*

