# A3bis · PermissionTier 安全层级审计

> 类型：**实施真源**（本文为 A3bis 唯一勾选落点）
> 日期：2026-08-14
> 来源：harness-inspired-roadmap-2026-08-14.md §A3bis + §U-A3bis
> 约束：架构规则 4（地图与地形同构）· 规则 12（CLI/桌面同一 app 路径）· 规则 13（provider 路由不动）· 规则 23（主路径不出现技术枚举）· 规则 24（高级默认折叠）

---

## 一、问题

Leaf 当前默认行为与 Harness 安全默认完全相反：

- `apply_permission_mode` 默认 soft-fill `bypassPermissions`（= Harness `danger-full-access` 的等价物），且无 per-call SandboxMode 概念；
- PM 用户无法知道 Worker 当前处于哪个安全层级——`bypassPermissions` 这个技术串对非开发受众不可读；
- 无 ApprovalPolicy 抽象，权限切换**不可审计**（events.jsonl 只记 `task_start` 的 mode 字符串，不记语义层级）。

---

## 二、设计

### D1 · Domain `PermissionTier` 枚举 ✅

**新增文件**：`src/domain/worker/permission.rs`（纯模型 · 无 IO）

```rust
pub enum PermissionTier {
    ReadOnly,        // 只读 · 不可写任何文件
    WorkspaceWrite,  // 可读写项目文件 · Harness 安全默认
    FullAccess,      // 完全访问 · 等价 bypassPermissions（Leaf 现默认）
}
```

方法（对齐 `CostTier` 模式）：
- `as_str()` → `"read-only" | "workspace-write" | "full-access"`（events.jsonl 持久化串）
- `parse(s)` → `Option<PermissionTier>`（从持久化串恢复）
- `from_permission_mode(mode: &str)` → `PermissionTier`：把现有 `permission_mode` 串映射到 tier
  - `bypassPermissions` → `FullAccess`
  - `acceptEdits` → `WorkspaceWrite`
  - `dontAsk` / `default` → `ReadOnly`（会拒写）
- `to_permission_mode()` → `&'static str`：反向映射回现有 mode 串（FullAccess→bypassPermissions 等），保持与 `apply_permission_mode` 的兼容
- `human_label()` → 人话标签（仅 Rust 侧用；UI 文案在 §U 落地，规则 23）：
  - `ReadOnly` → "受限只读"
  - `WorkspaceWrite` → "可读写项目文件"
  - `FullAccess` → "完全访问"

从 `src/domain/worker/mod.rs` 导出。

### D2 · WorkerPort `default_permission_tier()` ✅

**改动文件**：`src/ports/worker.rs`

trait 增加默认方法：
```rust
/// Default permission tier this provider declares (Harness-aligned default).
/// Override per provider when it natively supports a stricter tier.
fn default_permission_tier(&self) -> crate::domain::worker::PermissionTier {
    crate::domain::worker::PermissionTier::FullAccess // 向后兼容现有 bypassPermissions
}
```

**约束**：默认 `FullAccess` 保证现有 `apply_permission_mode` soft-fill `bypassPermissions` 行为不变（规则：不破坏现有行为、不静默变更已有项目）。各 provider 可 override 声明更严 tier，但**不**自动改运行时 mode——只声明能力，不强制路由（规则 13）。

### D3 · Scheduler 记录 tier 到 events.jsonl ✅

**改动文件**：`src/runtime/scheduler/tick.rs`（`spawn_ready` 的 `task_start` 事件处，L511 附近）

在 `task_start` 事件 extra 中加 `permission_tier` 字段：
```rust
let tier = provider.default_permission_tier();
self.state.event(
    "task_start",
    serde_json::json!({
        "task_id": id,
        "provider": task.provider,
        "mode": task.mode,
        "pid": handle.pid,
        "work_dir": work_dir,
        "attempt": attempt,
        "permission_tier": tier.as_str(),
    }),
)?;
```

权限层级变更可审计（每次任务分配都记 tier）。

### D4 · ProjectLiveView 暴露当前 tier ✅

**改动文件**：`src/services/live.rs` + `src/domain/worker/permission.rs`

`ProjectLiveView` 增加字段：
```rust
/// A3bis: 当前 Worker 安全层级（人话标签源 · UI 渲染 · 规则 23）
#[serde(default, skip_serializing_if = "Option::is_none")]
pub permission_tier_label: Option<String>,
```

在 `project_live_view` 中从 `config.default.permission_mode` 经 `PermissionTier::from_permission_mode(...).human_label()` 填充。**不**下发技术枚举串（规则 23）。

### U-A3bis · 桌面设置页人话安全标签 ✅

**改动文件**：`web/index.html` + `web/js/features/settings/settingsForm.js`

在现有「任务授权」分区（高级 · 默认折叠，规则 24）展示**当前生效层级**的人话标签（来自 Rust 下发 `permission_tier_label`），并在 `#s-permission-mode` 选择某 mode 时**同步刷新** tier 人话标签（不替换现有 select，只是加一句人话摘要）。

- 选 `bypassPermissions` → 显示"完全访问"（warn 色 · 当前默认偏宽）
- 选 `acceptEdits` → 显示"可读写项目文件"（推荐）
- 选 `dontAsk` / `default` → 显示"受限只读"（warn · 易假完成）

**约束**：
- 不引入技术枚举到主路径文案（规则 23）——现有 select 的技术值保留（设置页高级区允许），但**第一句摘要**用人话 tier 标签；
- 使用现有 `#s-permission-status` 元素承载 tier 人话，不新增大块 DOM；
- 规则 24 高级默认折叠不变；
- 规则 13 provider 路由不动——tier 只是 mode 的人话投影 + 审计，不改 soft-fill 逻辑。

---

## 三、不做的部分（本轮）

| 条目 | 理由 |
|------|------|
| per-call SandboxMode 切换运行时 | 需改 WorkerPort.start 签名 + 各 provider spawn，大改；本轮只做可观测/审计 |
| ApprovalPolicy 抽象（ask/never） | 审批逻辑仍在 CLI 自身；Leaf 不感知，本轮不引入 |
| 改默认 mode 为 `acceptEdits` | 约束明确：不静默变更已有项目行为；默认仍 bypassPermissions |
| 拆分台/Worker 选择 UI 显示 tier | §A3bis 原文提及，但本轮先在设置页落地（最小可观测面）；拆分台后续 |
| provider override `default_permission_tier` | trait 方法已加，本轮各 provider 用默认 FullAccess（不 override）；后续 provider 声明更严 tier 时再加 |

---

## 四、验收标准

1. `cargo build` 通过，`scripts/check-arch.sh` 无新 violation（规则 15/16 体积）；
2. `cco run` 执行时 `events.jsonl` 的 `task_start` 事件含 `permission_tier` 字段（`full-access` / `workspace-write` / `read-only`）；
3. 现有 `bypassPermissions` 默认行为**完全不变**（tier=FullAccess 映射回 bypassPermissions，soft-fill 不动）；
4. `PermissionTier::from_permission_mode` / `to_permission_mode` 往返一致（单测）；
5. `ProjectLiveView` 下发 `permission_tier_label` 人话字段（非技术枚举）；
6. 桌面设置页「任务授权」区显示当前 tier 人话标签（选不同 mode 同步刷新）。

---

## 五、勾选（改代码时在此更新）

- D1 Domain PermissionTier 枚举 ✅
- D2 WorkerPort default_permission_tier() ✅
- D3 Scheduler 记录 tier 到 events.jsonl ✅
- D4 ProjectLiveView permission_tier_label ✅
- U-A3bis 桌面设置页人话安全标签 ✅

---

> [PROTOCOL]: 改代码时先更新此文件勾选；完成后更新 docs/CLAUDE.md「还在做」区；
> 门禁：`scripts/check-arch.sh`；禁止平行第二套阶段表；规则 13 provider 路由不动。
