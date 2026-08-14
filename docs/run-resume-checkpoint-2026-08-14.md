# A1 · Run Resume 检查点恢复

> 类型：**实施真源**（本文为 A1 唯一勾选落点）
> 日期：2026-08-14
> 来源：harness-inspired-roadmap-2026-08-14.md §A1 + §U-A1
> 约束：架构规则 10（confirm 唯一开跑）· 规则 12（CLI/桌面同一 app 路径）· 规则 4（地图与地形同构）

---

## 一、问题

PM 用户发起 10 任务的 Run，执行到第 6 条时崩溃。

**当前行为**：
- `cco resume` 存在，但 `prepare_for_resume()` 把**所有**非 Done/Skipped 任务重置为 Pending（全量重置）；
- `events.jsonl` 已有 `run_start/task_start/task_end/run_end`，但**无粒度 checkpoint 事件**——不知道哪条是确定的完成边界；
- 桌面 Run 台失败后**无"从断点继续"按钮**（只有"继续"绑到 `resumeRun`，在 `btn-log-resume` 上，但语义等同全量重置）；
- `session_resume = false` on all WorkerPort providers——Worker 不能恢复自己上一轮的会话。

---

## 二、实施范围

### R1 · Scheduler 写 checkpoint 事件 ☐

**改动文件**：`src/runtime/scheduler/finish.rs`（在 `apply_result` 调用后的 Done 路径）

`task_end(Done)` 时额外写一条 `checkpoint` 事件：

```rust
// 在 finish.rs 的 finish_or_retry Done 分支，apply_result 之后
self.state.event(
    "checkpoint",
    serde_json::json!({
        "task_id": id,
        "ts": chrono::Utc::now().to_rfc3339(),
    }),
)?;
```

同样在 `tick.rs` 的快速完成路径（fast-path）的 Done 分支加相同调用。

**不改**：`events.jsonl` 追加格式（`state.event` 已包含 `ts`/`type`）。

### R2 · `prepare_for_resume` 增量恢复 ☐

**改动文件**：`src/state/mod.rs`

新增 `last_checkpoint_task_id()` 方法，读取 `events.jsonl`，找到最后一条 `type = "checkpoint"` 的 `task_id`：

```rust
pub fn last_checkpoint_task_id(&self) -> Option<String> {
    let path = self.events_path();
    let text = std::fs::read_to_string(&path).ok()?;
    text.lines()
        .rev()
        .find_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            if v.get("type")?.as_str()? == "checkpoint" {
                v.get("task_id")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
}
```

修改 `prepare_for_resume()` 逻辑：

- 若 `last_checkpoint_task_id()` 返回 Some(id)：只把该任务**之后**未完成的任务重置（按 topo 顺序）；之前的 Done 任务不动；
- 若 None（无 checkpoint）：保持现有全量重置语义（向后兼容）。

**同时**在 `ProjectLiveView` 增加字段 `has_checkpoint: bool`（供桌面判断按钮显示），在 `services/live.rs` 的 `project_live_view` 中填充。

### R3 · `cco resume` CLI 透传（无需改代码） ☐

`cli/commands/resume.rs` 已调 `app::run::prepare_resume` → `state.prepare_for_resume()`；R2 改了 `prepare_for_resume()` 后 CLI 自动受益，**无需改 resume.rs**。

### R4 · 桌面 RunView "从这里继续"按钮 ☐

**改动文件**：`web/js/features/run/RunView.js`

在 `paintKpis()` 函数的"继续"逻辑段（`canResume` 判断区，约 L327 附近），增加：

```js
// A1: 有 checkpoint 时显示"从断点继续"，否则只显示普通继续
const hasCheckpoint = !!(live && live.has_checkpoint);
const logResume = $("btn-log-resume");
if (logResume) {
  logResume.hidden = !canResume;
  logResume.textContent = hasCheckpoint ? "从这里继续" : "继续";
  logResume.title = hasCheckpoint
    ? "从最后一个完成步骤后继续执行"
    : "重新执行未完成的步骤";
}
```

**设计约束**（§四 U-A1）：
- 只改按钮文案和 tooltip，不新增按钮；调用路径仍 `vm.resume()` → `runApi.resumeRun` → `gateway.resumeRun`；
- `has_checkpoint` 字段由 Rust 下发，JS 不重新解析 events.jsonl；
- 文案不出现 `checkpoint`/`run_id` 等技术词（规则 23）。

---

## 三、不做的部分（本轮）

| 条目 | 理由 |
|------|------|
| `session_resume` WorkerPort 恢复 | Worker 侧会话恢复复杂度高；独立评估 |
| `cco fork --from-task <id>` | 依赖 A1 先落地；C2 中长期 |
| 单独"重试这条"按钮（`retryTask`） | 已有 `retryTask` gateway 方法，现 logBoardCard 内已可触发；不重复 |

---

## 四、验收标准

1. `cco run` 执行中途 kill 后，`events.jsonl` 末尾有 `{"type":"checkpoint","task_id":"...","ts":"..."}` 记录；
2. `cco resume` 后只重入 checkpoint 之后的任务，已 Done 的不重跑；
3. 无 checkpoint 时（老格式 events.jsonl）`cco resume` 行为与之前**完全一致**；
4. 桌面 RunView：有 checkpoint 时按钮文案变为"从这里继续"；无时仍显示"继续"；
5. `cargo build` 通过，`scripts/check-arch.sh` 无新 violation。

---

## 五、勾选（改代码时在此更新）

- R1 Scheduler checkpoint 事件 ✅
- R2 state prepare_for_resume 增量 + has_checkpoint 字段 ✅
- R3 CLI 透传（自动，无改动需确认） ✅
- R4 RunView 按钮文案 ✅

---

> [PROTOCOL]: 改代码时先更新此文件勾选；完成后更新 docs/CLAUDE.md「还在做」区；
> 门禁：`scripts/check-arch.sh`；禁止平行第二套阶段表。
