# A5-5 评估：workspace 拆 `cco-domain` / `cco-app`（2026-07-21）

> 状态：**评估完成 · 本刀不落代码**（建议延期）  
> 角色：P2-17 / A5-5 可选 crate 边界决策真源  
> 父计划：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md) §2.4 阶段 B · §11 A5-5  
> 红线：保持行为金样；禁止大改 API；无把握则只出评估文档  

[PROTOCOL]: 结论变更时先改本文件与架构 §11 A5-5 勾选说明，再动 Cargo workspace。

---

## 0. 一句话结论

**现在不要拆 workspace crate。**  
单 crate 模块边界（阶段 A）已足够支撑六边形与金样；`cco-app` **尚未**可独立编译，`cco-domain` 虽可抽出但 **ROI 低、回归面大**。待 A5-2 巨石收口、A5-4 GEB、以及 Store 端口 + 用例 DI 齐备后再开 **单独 PR**。

---

## 1. 门禁条件（用户任务原文）

| 条件 | 现状 | 是否满足 |
|------|------|----------|
| A5-1…4 绿后才评估/实施 | A5-1 ✅ · A5-3 ✅ · A5-2 部分 ✅（plan 巨石/余量未尽）· **A5-4 ☐** | **A5-4 未绿** → 本刀只评估 |
| 保持行为金样 | 可保持 | 拆 crate 需全量重跑 |
| 禁止大改 API | 外层 `cco::*` re-export 可兼容 | 内路径 `crate::domain` 变 `cco_domain` 有摩擦 |
| 无把握则只出评估文档 | 见 §6 | **本刀 = 文档 only** |

架构计划原文：阶段 A（模块切开）优先；阶段 B 仅当「单 crate 编译/边界仍糊」；**A 完成即达标，B 不阻塞 UI**；A5-5 = 「高编译风险 · 单独 PR」。

---

## 2. 现状地图（2026-07-21 工作树）

### 2.1 体量

| 单元 | 文件数 | 约行数 | 角色 |
|------|--------|--------|------|
| `src/domain/` | 28 | ~3.4k | 纯/近纯模型 |
| `src/app/` | 4 | ~0.9k | 用例面 |
| `src/ports/` | 3 | ~0.2k | WorkerPort · HandoffStore |
| `src/plan/` | 13 | ~7.7k | adapters + planner + facade |
| `src/runtime/` | 28 | ~7.4k | scheduler · handoff · provider |
| `src/services/` | 17 | ~3.6k | deprecated facade + IO |
| workspace | — | — | 仅 `cco` + `cco-desktop`；**无** `crates/` |

### 2.2 依赖方向（实测 import）

```text
domain/*
  → domain 内部（plan/run/worker/inspect/chat）
  → 外部：serde · serde_json · anyhow · std
  ✗ 无 tauri / clap / provider / services / fs IO   ✅ 阶段 A 达标

ports/*
  → domain::plan::{PlanIR, TaskIR}
  → state::{RunState, RunStatus}     ⚠ 跨「wire 层」
  → async_trait · anyhow · serde

app/*
  → domain::{run, worker}（部分）
  → config::Config
  → plan::{load_plan, PlanIR, planner::*}
  → runtime::{Scheduler, provider::*}
  → services::{runs, chat_*, …}
  → state · report · terminal
  ✗ 仍是「自由函数 + 具体适配器」组合，非注入端口   ❌ 不可独立 crate
```

### 2.3 Presentation 消费面

- CLI / TUI / Tauri：主要 `cco::app::{split,run,chat}`  
- `lib.rs` 仍 re-export 大量 `services::*` 与 `plan::{PlanIR,…}`（类型真源已是 `domain::plan` 再 export）  
- 桌面：`cco = { path = ".." }`；拆 crate 后需 workspace members + path 依赖链

---

## 3. 分 crate 可行性

### 3.1 `cco-domain` — **技术可行 · 收益不足**

| 项 | 评估 |
|----|------|
| 可编译性 | 高：几乎无 `crate::` 逃逸到 domain 外（除内部 `crate::domain::*`） |
| 纯净度 | 良：无 fs/tokio/provider；`PlanIR.source_path: PathBuf` 为轻度宿主气味 |
| 测试 | domain 内 unit 可随 crate 走；`cargo test -p cco-domain` 有意义 |
| 金样/集成 | 仍链 `cco`；不直接受益 |
| API 稳定 | 经 `cco::domain` 或 `cco` re-export 可无破坏外 API |
| 成本 | Cargo workspace、path 改写、CI 矩阵、IDE 索引、`check-arch` 路径 |

**结论**：能拆，但不解决 app 耦合；**单拆 domain 对贡献者心智几乎无新增**（已有 `src/domain/` + L2 硬规则）。

### 3.2 `cco-ports` — **半就绪**

| 阻塞 | 说明 |
|------|------|
| `HandoffStore` → `state::RunState` | wire 模型在 `state/`，不在 domain |
| 未建端口 | PlanJobStore · RunStore · ChatStore · PlannerPort · ProcessPort · WorktreePort · Clock（附录 B） |
| `WorkerPort` DTO | 可随 domain 或 ports crate；与 provider 实现仍在 `runtime/provider` |

拆 ports 而不迁 `RunState`/补 Store → **假边界**（ports crate 仍依赖 cco 主体）。

### 3.3 `cco-app` — **未就绪（否决本刀）**

`app/run.rs` / `app/split.rs` / `app/chat.rs` 直接依赖：

| 依赖 | 问题 |
|------|------|
| `config::Config` | 非 domain；无 SettingsPort |
| `services::*` | 厚 facade；拆 crate 会循环或拖进整个 services |
| `plan::planner` | Mode B job 状态机仍在 plan，未进 `domain/split` |
| `runtime::Scheduler` | 编排循环具体类型 |
| `runtime::provider::ProviderRegistry` | 适配器细节 |
| `terminal::TerminalManager` | Presentation 适配 |
| `report` | 输出适配 |

目标形态应是：

```text
struct RunUseCase<R: RunStore, W: WorkerPort, H: HandoffStore, …> { … }
// Presentation 组装 adapters，注入 app
```

现状是 **free-fn + 具体模块调用**。在 DI 化之前拆 `cco-app` = 要么拖入半个 monorepo 当依赖，要么大改 API（违反本刀红线）。

### 3.4 目标 workspace（架构 §2.4）对照

| 目标 crate | 本评估 |
|------------|--------|
| `cco-domain` | 可，建议等前置 |
| `cco-app` | 否，待 DI + Store 端口 |
| `cco-providers` | 否，非本刀；provider 已在 runtime |
| `cco-store` | 否，ports 未齐 |
| `cco` bin / `cco-desktop` | 保持 |

---

## 4. 风险（若强行本刀落地）

| 风险 | 级别 | 说明 |
|------|------|------|
| 行为静默漂移 | 高 | 集成路径/ re-export 漏一层即可让 Tauri/CLI 链到旧路径 |
| 循环依赖 | 高 | app→services→app 已存在 facade 环；跨 crate 会编译失败或强迫合并 |
| API 面膨胀 | 中 | 双路径 `cco::domain` vs `cco_domain::`；文档/GEB 未收口（A5-4）时更乱 |
| CI / 桌面构建 | 中 | `src-tauri` path、`package-app.sh`、集成测 `-p cco` 全要改 |
| 编译时间 | 不确定 | 无实测「单 crate 过慢」证据；拆 crate **不保证** 变快（incremental 边界利弊并存） |
| 与 A5-2 并行冲突 | 高 | plan/chat 巨石与 facade 仍在绞杀；crate 搬家与前端收口抢同一后端面 |

---

## 5. 建议路径（延期条件，不是排期承诺）

### 5.1 必须先完成（门槛）

1. **A5-2** 余量：`web/js/plan.js` 巨石 → facade ≤200 或删除（S8）。  
2. **A5-4** GEB：L1/L2/PRODUCT 链接与「实施真源」身份收口。  
3. **行为金样**持续绿：`a0_behavior_golden` · `mode_b_golden` · lib。

### 5.2 建议再完成（使 `cco-app` 可拆）

| 序 | 工作 | 目的 |
|----|------|------|
| P1 | `RunStatus` / 必要快照字段迁 `domain/run` 或 `domain/run_wire` 纯类型；`state` 只做 IO | 解开 ports→state |
| P2 | 落地 `RunStore` · `PlanJobStore` · `ChatStore`（最小 read/write） | app 不再 `use services` |
| P3 | `app::{Run,Split,Chat}` 改为结构体 + 端口字段；Presentation 组装 | 可测、可 crate |
| P4 | `plan/planner` job 状态机 → `domain/split` + store 适配 | split 边界闭合 |
| P5 | **单独 PR**：`crates/cco-domain` →（可选）`cco-ports` → `cco-app`；`cco` 变 facade 包 | 一次搬家、金样全绿 |

### 5.3 可选捷径（仍 **不** 推荐抢 A5 收口）

仅抽 `cco-domain`，`ports`/`app` 留在 `cco`：

- 优点：编译防火墙验证 domain 无宿主依赖  
- 缺点：双包维护、app 仍糊；与「阶段 B 在边界仍糊时」动机不完全对齐  

若未来只为 **CI 强制 domain 纯净**，优先加强 `scripts/check-arch.sh`（禁止 domain 下 `use crate::(services|runtime|cli|tui)`）而非拆 crate。

---

## 6. 决策记录

| 字段 | 值 |
|------|----|
| 决策 | **Defer** — 不创建 `crates/`，不改 `Cargo.toml` members（除既有 desktop） |
| 代码 diff | **零**（本评估） |
| A5-5 勾选 | ☐ 保持未完成；备注「2026-07-21 评估：延期，见本文件」 |
| 重开条件 | §5.1 全绿 + §5.2 P1–P4 实质完成，或出现可度量的单 crate 编译/边界痛点 |
| 不做 | FE · 调度重写 · React · 上云 · 大改 IPC/run_dir/job 路径 |

---

## 7. 验证（本刀）

本刀无 Rust/JS 行为变更。仍跑回归确认工作树基线：

```bash
cargo test --lib -p cco
cargo test -p cco --test a0_behavior_golden --test mode_b_golden
bash scripts/check-arch.sh
```

（A5-5 无新增 `.js`；features invoke / start_run 旁路检查属 A5-2e 已绿基线。）

---

## 8. 与总账 / GEB

- 总账 **P2-17**：A5-5 仍为可选未实施；**不**回灌 D0–D4。  
- 架构计划 §11 A5-5：状态写「评估延期」而非假 ✅。  
- A5-4 仍应独立完成 GEB；**不要**把本评估冒充 A5-4。

---

*评估日期：2026-07-21 · 分支语境 `feat/arch-a5-…` · 只读架构/依赖；无 crate 落地。*
