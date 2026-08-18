# 事件总线落地计划 · events.jsonl → 前端订阅

> **日期**：2026-08-18  
> **类型**：架构债落地计划（单独立项 · 不与 DSH UI 波次混跑）  
> **触发**：[`harness-inspired-roadmap-2026-08-14.md`](./harness-inspired-roadmap-2026-08-14.md) B1 · H1 Session Log 权威事件源  
> **角色**：**把 events.jsonl 从「写了没人读」升级为「前端唯一状态派生源」**——消除 2s 轮询 + RunState 文件双源不一致  
> **勾选**：**本文为 B1 唯一勾选落点**；落地后同步 `harness-inspired-roadmap` §二 B1 行 + `docs/CLAUDE.md` 业务规则参考  
> **不**替代 A1 checkpoint / A3bis PermissionTier / B3 压缩事件——它们是 B1 的**消费方**，B1 先通管道
> **UX 配套**：[`event-bus-ux-2026-08-18.md`](./event-bus-ux-2026-08-18.md) —— 用户操作流/易用性/易上手分析（U1-U6 + 落地优先级）

关联真源：

| 读这个 | 关系 |
|--------|------|
| 本文 | **B1 唯一实施勾选落点** |
| [`harness-inspired-roadmap-2026-08-14.md`](./harness-inspired-roadmap-2026-08-14.md) | H1 Session Log 主张 · B1 方向定义；**方向参考，非勾选** |
| [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md) | 六边形分层约束（规则 5–9）；改 services/runtime 须遵守 |
| [`run-resume-checkpoint-2026-08-14.md`](./run-resume-checkpoint-2026-08-14.md) | A1 checkpoint 事件（已写）——B1 通管道后才有前端消费方 |
| [`session-digest-event-2026-08-14.md`](./session-digest-event-2026-08-14.md) | B3 context_compressed 事件（已写）——同上 |
| [`permission-tier-audit-2026-08-14.md`](./permission-tier-audit-2026-08-14.md) | A3bis permission_tier 事件（已写）——同上 |

[PROTOCOL]: 变更时更新文首状态与 §勾选；落地后同步 `docs/CLAUDE.md` + `harness-inspired-roadmap` B1 行 + 相关 L2；**禁止**平行第二套「事件总览」阶段表

---

## 0. 一句话

**events.jsonl 已经写了 run_start / task_start / task_end / run_end / checkpoint / context_compressed / permission_tier——但前端不读它，仍用 2s setInterval 轮询 RunState 文件快照。本计划打通「Rust 写事件 → Tauri emit → 前端订阅」管道，让前端从事件流派生状态，2s 轮询降为降级兜底。**

---

## 1. 问题定性

### 1.1 现状（已核实代码）

| 层 | 现状 | 证据 |
|----|------|------|
| **写事件** | Scheduler 在 `task_end(Done)` 写 `checkpoint` 事件 | `scheduler/tick.rs:278` · `tick.rs:580` |
| **写事件** | Scheduler 分配任务时写 `permission_tier` 到 events | `scheduler/tick.rs:530-540` |
| **写事件** | Chat send 压缩时写 `context_compressed` 事件 | `services/chat/send.rs:237` |
| **写事件** | `run_start / task_start / task_end / run_end` 已写 | `state/mod.rs:294` `event()` |
| **读事件** | `last_checkpoint_task_id()` 只被 `prepare_for_resume` 调用 | `state/mod.rs:359` |
| **前端** | **完全不消费** events.jsonl | `loadLive.js` 只调 `gateway.getProjectLive` → Tauri `get_project_live` → 读 RunState 文件 |
| **前端** | 2s `setInterval` 轮询 | `shellBoot.js:46` `startPolling(intervalMs=2000)` → 每 tick 调 `window.loadLive()` |
| **前端** | `main.js` 另一个 2s `setInterval` 做 `softSyncFromLegacy` | `main.js:627` |

### 1.2 结构性后果

```text
events.jsonl（权威事件流）          RunState.json（文件快照）
  │                                     │
  │  ← Rust 侧已写                       │  ← Rust 侧已写
  │                                     │
  │  ✗ 前端不读                          │  ← 前端 2s 轮询读这里
  │                                     │
  └── 无人消费 ─────────────────────────  └── 唯一前端状态源
```

- **双源不一致**：events.jsonl 有 `checkpoint` 但前端看不到「从断点继续」按钮（因为前端读的是 RunState 快照，不读 events）；
- **延迟**：任务状态变化最长 2s 后前端才看到（轮询间隔）；`context_compressed` / `permission_tier` 完全不可见；
- **已落地的 A1/A3bis/B3 事件白写**：写了没人读 = 投资未回收。

### 1.3 为什么是 B1 先做

Harness 六条主张里 H1（Session Log 权威）是**基座**：

```
H1 Session Log 权威 ─┬─ H5 Resume（checkpoint 事件需被前端读才能显示「从断点继续」按钮）
                     ├─ H6 安全（permission_tier 事件需被前端读才能显示安全标签）
                     ├─ B3 压缩（context_compressed 事件需被前端读才能感知上下文截断）
                     └─ B2 Headless（headless JSON 输出可从事件流聚合，非读 RunState）
```

**B1 不通，其余四条的前端价值无法释放。**

### 1.4 非目标

- **不**重写 Scheduler 事件写入逻辑（已写，够用）；
- **不**替换 RunState.json（它仍是持久化真源；事件流是**派生**源，不是替代）；
- **不**做 Tauri 插件注册 / 微内核（Harness cordis 路线，Leaf 不做——见 roadmap §五）；
- **不**一次全删 2s 轮询（保留为降级兜底）；
- **不**在 JS 复制业务策略（规则 22）；
- **不**改 `state.js`（D9+ 桥/瘦 ~230 行，规则 18，禁止再堆）。

---

## 2. 目标模型

### 2.1 选定：事件派生 + 轮询降级（非 Harness 纯事件权威）

| 模型 | 是否采用 | 原因 |
|------|----------|------|
| M1 纯事件权威（Harness 式，RunState 只存磁盘不派生 UI） | ❌ | 改动面太大；RunState 是持久化真源，前端聚合成本高 |
| M2 纯轮询（现状） | ❌ | 双源不一致 + 2s 延迟 + 已写事件白费 |
| **M3 事件派生 + 轮询降级** | ✅ | 增量接入；事件通道优先，断开时退回轮询 |

### 2.2 M3 数据流

```text
Rust Scheduler / Chat send
  │
  ├─ state.event("task_start", {task_id, ...})     ← 已有
  │
  ▼
events.jsonl（磁盘真源 · 追加写）
  │
  ├─ RunState::append_event()                      ← 已有
  │
  ▼
Tauri emit("cco:run_event", payload)               ← 新增：Rust 写事件后 emit
  │
  ▼
前端 gateway.subscribe("cco:run_event", handler)    ← 新增：JS 订阅
  │
  ├─ 增量更新 ViewModel store                       ← 新增
  │
  ▼
RunViewModel → render                               ← 已有（改数据源）

降级：emit 未到达 → 2s setInterval loadLive() 兜底  ← 已有（保留）
```

### 2.3 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 事件传输 | Tauri `emit` / `listen` | 已有 AppHandle（`lib.rs:52`）；无需引入 WebSocket / SSE |
| 事件 payload | 从 events.jsonl 的 JSON 直透 | 不在 Rust 侧重新建模；前端按 `type` 字段分发 |
| 前端消费 | `gateway.subscribe` 返回 unsubscribe | 与现有 ViewModel `subscribe` 模式一致（`store.js:48`） |
| 降级 | 2s 轮询保留，频率不变 | 事件通道断开（emit 失败 / Tauri 未就绪）时无缝退回 |
| state.js | **不动** | 事件订阅走 `gateway`，不进 `state.js`（规则 18） |
| RunState.json | **不动** | 仍是磁盘持久化真源；事件流只负责**通知前端去刷新**，不替代数据 |

---

## 3. 落点图（改哪些文件）

> 法则：规则在 domain；用例在 app；IO 在 runtime/services；UI 只渲染 DTO。  
> **禁止**往 `services/live.rs` 厚文件堆策略（规则 18 已出榜）；新逻辑抽文件。

### 3.1 总览

| 能力 | 层 | 主路径 | 行为 |
|------|----|--------|------|
| 事件写入后 emit 通知 | runtime | `src/runtime/scheduler/tick.rs` | `state.event()` 后追加 `handle.emit()` |
| Scheduler 持有 AppHandle | runtime | `src/runtime/scheduler/mod.rs` | 新增字段 `app_handle: Option<AppHandle>` |
| 构造注入 AppHandle | app | `src/app/run/foreground.rs` | `prepare_scheduler` 签名加 `app_handle` 参数 |
| Tauri command 传 AppHandle | tauri | `src-tauri/src/lib.rs` | `start_run` 等命令参数加 `app: AppHandle` |
| 前端订阅 | web | `web/js/shared/gateway.js` | 新增 `subscribeRunEvents(handler)` |
| 前端消费 + page 分发 | web | `web/js/features/settings/shellBoot.js` | 全局订阅一次 + 按 `state.page` 分发 |
| patchTask 实现 | web | `web/js/features/run/RunViewModel.js` | `store.set(prev => ...)` 函数式更新（不改 store.js） |
| 降级保底 | web | `web/js/features/settings/shellBoot.js` | 保留 2s 轮询；事件到达时跳过当前 tick |

### 3.2 Rust 侧

> **关键纠正**：`AppHandle` 挂在 **Scheduler struct** 上，**不**进 `RunState`。  
> 原因：`RunState` 是 `#[derive(Clone, Serialize, Deserialize)]` 的磁盘模型（`state/mod.rs:210`），被 `RunState::load()` 从磁盘反序列化、被 Scheduler 值传递。`AppHandle` 不可 Serialize/Deserialize，塞进去会破坏 serde + 反序列化后变 None。  
> Scheduler struct（`scheduler/mod.rs:52`）已有 20+ 字段，加一个 `app_handle: Option<AppHandle>` 天然合理。

#### 3.2.1 `src/runtime/scheduler/mod.rs` — 加字段

```rust
pub struct Scheduler {
    // ... 现有 20+ 字段 ...
    /// B1: Tauri AppHandle for event emit (None on CLI/TUI path).
    pub app_handle: Option<tauri::AppHandle>,
}
```

#### 3.2.2 `src/runtime/scheduler/tick.rs` — emit 调用

```rust
// 在 state.event("checkpoint", ...) 之后追加：
// B1: emit to frontend (Tauri app handle, if available)
if let Some(handle) = &self.app_handle {
    use tauri::Emitter;  // Tauri 2: emit 在 Emitter trait 上
    let _ = handle.emit("cco:run_event", serde_json::json!({
        "run_id": self.state.run_id,
        "type": "checkpoint",
        "payload": payload,
    }));
}
```

- `emit_to_frontend` **不**放在 `RunState` 上（避免磁盘模型依赖 Tauri）；
- emit 逻辑直接在 `tick.rs` 调用点写（3–5 行），不抽 helper 函数（避免新建上帝 Manager，规则 8）；
- CLI/TUI 路径 `app_handle = None` → `if let Some` 跳过，行为不变。

#### 3.2.3 `src/app/run/foreground.rs` — 构造注入

```rust
// prepare_scheduler 签名加 app_handle 参数：
pub fn prepare_scheduler(
    config: &Config,
    run_id: &str,
    app_handle: Option<tauri::AppHandle>,  // ← 新增；CLI 传 None
) -> Result<Scheduler> {
    // ...
    Ok(Scheduler {
        // ... 现有字段 ...
        app_handle,
    })
}
```

**注入路径**（方案 A：Tauri command 参数）：
```
Tauri start_run command (lib.rs:263)
  参数加 app: AppHandle   ← Tauri 自动注入
  → prepare_scheduler(config, run_id, Some(app))
  → Scheduler { app_handle: Some(app), ... }

CLI cco run (cli/commands/run.rs)
  → prepare_scheduler(config, run_id, None)
  → Scheduler { app_handle: None, ... }   ← emit 静默跳过

CLI cco resume (cli/commands/resume.rs)
  → prepare_scheduler(config, run_id, None)
  → Scheduler { app_handle: None, ... }   ← 同上
```

- `resume` 路径也传 None（CLI 无 Tauri）；
- 桌面 resume 路径（`src-tauri/src/lib.rs` 的 resume command）传 `Some(app)`。

#### 3.2.4 `src-tauri/src/lib.rs` — command 加 AppHandle

```rust
#[tauri::command]
fn start_run(
    app: tauri::AppHandle,   // ← 新增；Tauri 自动注入
    state: tauri::State<'_, AppState>,
    req: StartRunRequest,
) -> Result<Value, String> {
    // ...
    let sched = prepare_scheduler(&config, &run_id, Some(app))?;
    // ...
}
```

- `AppState`（`lib.rs:54`）**不**加 `app_handle` 字段（方案 A，不选 B）；
- `setup` 中**无**需注入（AppHandle 直接从 command 参数拿）。

### 3.3 前端侧

> **关键纠正**：ViewModel **没有 destroy 生命周期**（`RunViewModel`/`ChatViewModel`/`SplitViewModel` 均无 destroy 方法，一次性创建挂在 window 上）。  
> 因此 Tauri listener 订阅**不**放在 `RunViewModel` 里（会泄漏——每次切回 run 台多注册一个 listener）。  
> 改为在 **`shellBoot.js` 全局 tick** 里做一次 `subscribeRunEvents` + 内部按 `state.page` 分发。

#### 3.3.1 `web/js/shared/gateway.js` — 新增订阅 API

```javascript
// 新增（不进 state.js）：
export function subscribeRunEvents(handler) {
  const unlisten = window.__TAURI__?.event?.listen("cco:run_event", (e) => {
    handler(e.payload);
  });
  return () => unlisten?.then((fn) => fn());
}
```

- 不按 `run_id` 过滤——handler 内部决定是否处理（shellBoot 已有 page/runId 上下文）。

#### 3.3.2 `web/js/features/settings/shellBoot.js` — 全局订阅 + page 分发

```javascript
// 在 startPolling 初始化时一次性订阅（替代 RunViewModel 订阅）：
let unsubscribeRunEvents = null;

export function startPolling(intervalMs = 2000) {
  const st = state();
  if (!st) return;
  clearInterval(st.pollTimer);

  // B1: 事件订阅（仅 Tauri 环境；CLI 无 __TAURI__ → 跳过）
  if (!unsubscribeRunEvents && window.__TAURI__?.event?.listen) {
    import("../../shared/gateway.js").then(({ subscribeRunEvents }) => {
      unsubscribeRunEvents = subscribeRunEvents((evt) => {
        // 只在 run 台 + 有 runId 时处理
        if (st.page !== "workspace" || !st.selectedPath) return;
        handleRunEvent(evt, st);
        st.eventStale = false; // 事件到达 → 跳过当前 poll tick
      });
    });
  }

  st.pollTimer = setInterval(() => {
    // B1: 事件正常时跳过轮询；事件超时未到达 → 退回 loadLive()
    if (st.eventSubscribed && !st.eventStale) return;
    st.eventStale = false; // 重置：下 tick 若无事件再轮询
    // ... 原 loadLive() 逻辑 ...
  }, intervalMs);
}
```

- `eventStale` 标记：事件到达时置 false；每 tick 置 true——如果下 tick 没有事件来，就退回轮询；
- 降级天然：`__TAURI__` 不存在（CLI/非 Tauri 环境）→ 不订阅 → 纯轮询。

#### 3.3.3 `web/js/features/run/RunViewModel.js` — patchTask 实现

```javascript
// 在 RunViewModel 内部实现 patchTask（不改 store.js）：
function patchTask(store, taskId, patch) {
  store.set((prev) => ({
    ...prev,
    tasks: prev.tasks.map((t) =>
      t.id === taskId ? { ...t, ...patch } : t
    ),
  }));
}

// handleRunEvent 分发函数（放在 shellBoot 或 RunViewModel 导出）：
function handleRunEvent(evt, st) {
  switch (evt.type) {
    case "task_start":
      patchTask(runVmStore, evt.payload.task_id, { status: "running" });
      break;
    case "task_end":
      patchTask(runVmStore, evt.payload.task_id, { status: evt.payload.status });
      break;
    case "checkpoint":
      // A1: 解锁「从断点继续」按钮（store 加 has_checkpoint flag）
      runVmStore.set((prev) => ({ ...prev, has_checkpoint: true }));
      break;
    case "permission_tier":
      // A3bis: 安全标签可见
      runVmStore.set((prev) => ({ ...prev, permission_tier: evt.payload.tier }));
      break;
    case "run_end":
      // 全量刷新保证一致性（事件只做增量提示，run_end 做对账）
      if (typeof window.loadLive === "function") window.loadLive();
      break;
    // context_compressed: chat 相事件，本期不做（见 §3.4）
  }
}
```

- `store.js` **不动**（规则 18）；patchTask 用 `store.set(prev => ...)` 函数式更新；
- `run_start` 不单独处理——`run_end` 后 `loadLive()` 全量刷新已覆盖。

---

## 4. 事件类型清单（前端消费映射）

> 扩展 `LogEvent` 不在 Rust 侧新增类型；直接用 `serde_json::Value` 透传。  
> 前端按 `type` 字段分发，**不**在 JS 重新定义枚举（规则 22）。

### 4.1 run 相事件（本期做）

| `type` | 写入位置 | 写入时机 | 前端消费 |
|--------|---------|---------|---------|
| `run_start` | Scheduler `run()` | Run 开始 | 跳过（loadLive 全量刷新覆盖） |
| `task_start` | Scheduler | 任务分配 | 增量更新 task status → running |
| `task_end` | Scheduler | 任务完成（含成功+失败） | 增量更新 task status → 按 `payload.status` 区分 done/failed |
| `checkpoint` | Scheduler | `task_end(Done)` | 解锁「从断点继续」按钮（A1） |
| `run_end` | Scheduler | Run 结束 | `loadLive()` 全量刷新 + 切 result 台 |
| `permission_tier` | Scheduler | 分配任务时 | 安全标签可见（A3bis） |

> **纠正**：原计划列了 `task_failed` 事件——代码里**不存在**。任务失败写的是 `task_end`（带 `status: "failed"` 字段），不是独立类型。已删除。

### 4.2 chat 相事件（本期不做 · 后置）

| `type` | 写入位置 | 写入时机 | 说明 |
|--------|---------|---------|------|
| `context_compressed` | `services/chat/send.rs` | 每轮压缩后 | 写在 `.cco/chat/{safe}.events.jsonl`，**不**在 run 的 `events.jsonl` |

> **关键纠正**：`context_compressed` 走 chat session 事件路径（`session.rs:366`），不是 run 事件路径。需要单独的 `cco:chat_event` channel。本期只做 run 相管道；chat 相事件后置（依赖 B3 文档已声明"chat 相非 run 相"）。

---

## 5. 验收标准

### 5.1 功能验收

- [ ] **E1 · Tauri emit 桥**：Scheduler 写事件后，前端 `subscribeRunEvents` 能收到；
- [ ] **E2 · 增量更新**：`task_start` 事件到达后，前端 task 卡片状态在 **< 100ms** 内变为 running（不再等 2s）；
- [ ] **E3 · 降级兜底**：关闭 Tauri emit（模拟 CLI 路径），前端退回 2s 轮询，不报错；
- [ ] **E4 · checkpoint 消费**：Run 失败且有 checkpoint 事件时，「从断点继续」按钮出现（A1 前端收口）；
- [ ] **E5 · 双源一致**：事件到达后 `loadLive()` 的 RunState 快照与事件流一致（无回退跳变）。

### 5.2 架构验收

- [ ] **A1 · state.js 不堆**：事件订阅走 `gateway.js`，`state.js` 行数不变（规则 18）；
- [ ] **A2 · services/live.rs 不堆**：emit 逻辑在 `state/mod.rs` 或 `lib.rs`，不进 `live.rs`（规则 18 出榜文件）；
- [ ] **A3 · IPC 唯一出口**：前端只从 `gateway.subscribeRunEvents` 订阅，feature 文件不散落 `__TAURI__.event.listen`（规则 20）；
- [ ] **A4 · CLI 不受影响**：CLI/TUI 路径 `AppHandle = None`，emit 静默跳过，行为不变。

### 5.3 UX 验收（详见 [`event-bus-ux-2026-08-18.md`](./event-bus-ux-2026-08-18.md)）

- [ ] **U1 · 防抖**：3 个并发 `task_start` 同时到达，看板无闪烁（CSS transition 平滑）
- [ ] **U2 · 失败可见性**：`task_end(failed)` 后 200ms 内有 `task_end(done)`，失败态不被覆盖
- [ ] **U3 · 降级体感**：事件断开后看板边缘出现"实时降级"小条（可关·不阻塞·停止/继续仍可用）
- [ ] **U4 · 安全标签**：默认不可见；设置开启后用人话文案（ReadOnly→"只读"等）
- [ ] **U5 · 从这里继续**：按钮文案 + tooltip 人话；切走再回来保持
- [ ] **U6 · 对账**：连续 3 tick（6s）无事件 → 强制 `loadLive()` 对账一次

### 5.4 验证口令

```bash
# 架构检查
bash scripts/check-arch.sh

# 单元测试（Rust 侧 emit 桥）
cargo test --lib -p cco event_bus

# 集成测试（前端订阅 → 增量更新）
cargo test -p cco --test event_bus_golden

# 手动验证
# 1. cco run --plan examples/hello.md --provider fake
# 2. 观察 web 桌面 Run 台：task 卡片应在 < 100ms 内变状态（非 2s 跳变）
# 3. kill -9 模拟 Worker 崩溃 → checkpoint 事件 → 「从这里继续」按钮出现
# 4. 并发任务防抖：多任务同时启动无闪烁
# 5. 降级体感：断开 emit → 降级小条出现 → 停止/继续仍可用
```

---

## 6. 落地波次

> 单独立项 · 不与 DSH UI 波次（`feat/ui-redesign-dsh`）混跑。  
> 建议分支：`feat/event-bus-b1`

### 波次 0 · Rust 侧 emit 桥（成本：低）

- [ ] `scheduler/mod.rs` 加 `app_handle: Option<tauri::AppHandle>` 字段
- [ ] `scheduler/tick.rs` 在 `state.event()` 后调 `handle.emit()`（含 `use tauri::Emitter`）
- [ ] `app/run/foreground.rs` `prepare_scheduler` 签名加 `app_handle` 参数
- [ ] `src-tauri/src/lib.rs` `start_run` / `resume` command 参数加 `app: AppHandle`
- [ ] CLI 路径 `app_handle = None` 验证（`cco run` / `cco resume` 行为不变）
- [ ] `RunState` **不**加字段（保持磁盘模型纯净）

### 波次 1 · 前端订阅 + 增量更新（成本：中）

- [ ] `gateway.js` 加 `subscribeRunEvents(handler)`
- [ ] `shellBoot.js` 全局订阅一次 + 按 `state.page` 分发（不 per-VM 订阅）
- [ ] `RunViewModel.js` 内实现 `patchTask`（用 `store.set(prev => ...)`，不改 `store.js`）
- [ ] `shellBoot.js` 2s 轮询加 `eventStale` 跳过逻辑
- [ ] 手动验证 < 100ms 状态更新

### 波次 2 · 降级 + 一致性（成本：低）

- [ ] 模拟 emit 失败（无 `__TAURI__` 环境）→ 退回轮询不报错
- [ ] `run_end` 事件后 `loadLive()` 全量刷新验证双源一致
- [ ] checkpoint 事件 → 「从断点继续」按钮出现（A1 前端收口）
- [ ] `main.js:627` `softSyncFromLegacy` interval **不动**（非 run 相，见 §10-C1）

### 波次 3 · 金样测试（成本：低）

- [ ] `tests/event_bus_golden.rs`：fake provider → 事件写入 → emit → 前端收到的端到端测
- [ ] `scripts/check-arch.sh` 加 B1 检查项（state.js 行数不增 + RunState 无 AppHandle 字段）

---

## 7. 与已有能力的关系（投资回收）

B1 通管道后，以下已落地但「写了没人读」的能力**价值释放**：

| 能力 | 事件已写 | B1 前前端效果 | 真源 |
|------|---------|-------------|------|
| A1 Run Resume | `checkpoint` ✅ | 「从断点继续」按钮出现 | [`run-resume-checkpoint`](./run-resume-checkpoint-2026-08-14.md) |
| A3bis PermissionTier | `permission_tier` ✅ | 安全标签可见 | [`permission-tier-audit`](./permission-tier-audit-2026-08-14.md) |
| B3 Session Digest | `context_compressed` ✅ | 压缩感知可见 | [`session-digest-event`](./session-digest-event-2026-08-14.md) |
| Headless JSON | events.jsonl ✅ | headless 输出可从事件聚合 | [`headless-mode`](./headless-mode-2026-08-14.md) |

**B1 是这四条的前端基座。**

> **注意**：B3 `context_compressed` 是 chat 相事件（写 `.cco/chat/{safe}.events.jsonl`，非 run 的 `events.jsonl`）。本期 B1 只通 run 相管道；chat 相事件后置，需另开 `cco:chat_event` channel。

---

## 8. 约束清单（落地时检查）

| 规则 | 约束 | 验证 |
|------|------|------|
| 规则 5 | 方向 `Presentation → Application → Domain` | emit 在 runtime/state，订阅在 gateway，不逆流 |
| 规则 7 | View/Tauri command 不写业务策略 | emit 只透传事件，不做策略判断 |
| 规则 8 | 不新建上帝 Manager | 不新增 `EventBusManager`；emit 在 `state/mod.rs` 方法 |
| 规则 18 | 不堆已知厚文件 | `live.rs` / `state.js` 不增行 |
| 规则 19 | MVVM | View 不写业务链；ViewModel 订阅 + store patch |
| 规则 20 | IPC 唯一出口 gateway | 前端只从 `gateway.subscribeRunEvents` 订阅 |
| 规则 22 | 不在 JS 复制策略 | 按 `type` 分发，不重新定义枚举 |
| 规则 25 | TUI = 观察 + 轻控制 | TUI 不做事件订阅（只桌面 web） |

---

## 9. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Tauri emit 在高频写入时性能问题 | 低 | 中 | events.jsonl 仍是磁盘真源；emit 只通知前端刷新，不传大数据 |
| 前端订阅泄漏（页面切换不 unsubscribe） | — | — | **已消除**：订阅在 shellBoot 全局做一次，不 per-VM 订阅，无泄漏风险 |
| 事件与 RunState 快照不一致 | 中 | 中 | `run_end` 事件后强制 `loadLive()` 全量刷新；事件只做增量提示 |
| CLI 路径 AppHandle 注入复杂 | — | — | **已消除**：方案 A——command 参数加 `app: AppHandle`，CLI 传 None |
| 多窗口（monitor window）emit 广播 | 低 | 低 | Tauri `emit` 广播到所有窗口——主窗 + monitor 窗都收到，符合预期；本期不做 per-window 过滤 |

---

## 10. 待澄清（落地前确认）

| # | 点 | 现状 | 建议处理 |
|---|---|---|---|
| C1 | `main.js:627` 的 `softSyncFromLegacy` 第二个 2s interval | 计划只改了 `shellBoot.js` 的轮询，`main.js` 的 `softSync` interval 也 2s | `softSync` 做的是 legacy state 镜像同步，非 run 状态——本期**不动**；B1 只管 run 相事件 |
| C2 | plan job 轮询（`jobPoll.js:420` 自调度） | AI 拆分进度走 `jobPoll.js` 自调度轮询（非 setInterval），不走 `loadLive()` | plan job 是**拆分相**事件，不是 run 相——本期**不动**；如未来需要，开 `cco:plan_event` channel |
| C3 | Tauri 2 的 `Emitter` trait import | `emit` 方法在 `Emitter` trait 上，需 `use tauri::Emitter` | 已在 §3.2.2 代码示例中加 `use tauri::Emitter` |

---

## 10. 状态

| 阶段 | 状态 | 说明 |
|------|------|------|
| B1-0 Rust emit 桥 | ☐ | `state/mod.rs` + `tick.rs` + `lib.rs` |
| B1-1 前端订阅 | ☐ | `gateway.js` + `RunViewModel.js` |
| B1-2 降级 + 一致性 | ☐ | `shellBoot.js` + `run_end` 全量刷新 |
| B1-3 金样测试 | ☐ | `tests/event_bus_golden.rs` + `check-arch.sh` |

---

## 修订

| 日期 | 说明 |
|------|------|
| 2026-08-18 | 初版立项；M3 事件派生 + 轮询降级；单独立项不与 DSH 混跑 |
| 2026-08-18 | 审查修订：AppHandle 挂 Scheduler 不挂 RunState；chat 相事件后置；task_failed 不存在；前端订阅放 shellBoot 不放 RunViewModel；加多窗口/softSync/jobPoll 待澄清项 |
| 2026-08-18 | UX 配套：新增 §5.3 UX 验收（U1-U6）+ §5.4 验证口令补充；详见 [`event-bus-ux-2026-08-18.md`](./event-bus-ux-2026-08-18.md) |
