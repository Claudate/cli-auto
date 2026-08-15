# B3 · Session Digest 事件化

> 类型：**实施真源**（本文为 B3 唯一勾选落点）
> 日期：2026-08-14
> 来源：harness-inspired-roadmap-2026-08-14.md §B3（lines 195–204）
> 约束：规则 4（地图与地形同构）· 规则 18（厚文件只抽不堆）· 规则 23（主路径第一句人话，不出现 run_id/digest_hash）· 规则 24（高级能力默认关）
> 邻接（**不**吞并勾选）：[`context-digest-compress-landing-2026-07-27.md`](./context-digest-compress-landing-2026-07-27.md) C0–C2 ✅（会话 digest 本体）；B3 只做**事件化**，不改 digest schema/抽取/注入。

---

## 一、问题

会话语义压缩（C0–C2 ✅）已做，但压缩动作本身未落盘成事件，事后无法回答"当时模型看到了多少上下文、压缩成了什么"。

roadmap §B3 原文：

> 每次压缩后写一条 `LogEvent::ContextCompressed { tokens_before, tokens_after, digest_hash }` 到 log_events；这让 Inspect 阶段能感知到"本次执行中 Worker 的上下文是否被截断过"。

### 地形校准（探索结论）

1. **压缩实际发生在 chat 相，不在 run 相**。`session-digest` 在 `services/chat/send.rs::chat_send` 抽取/浅检/存入 `ChatSession.session_digest`。run 相 worker 提示（`runtime/scheduler/start.rs:130-131`）只拼 handoff 前缀，**不**携带 `session_digest`。即 roadmap 说的"Worker 上下文被截断"在本仓当前架构里不存在——run worker 不读 digest。

2. **`LogEvent` 不是 typed enum**。`src/runtime/log_events.rs` 的 `LogEvent` 是平铺字符串 kind 的展示 DTO（从 worker stdout/stderr 解析而来），`LogEvent::ContextCompressed` 在此无处落点——roadmap 的变体记法是示意。

3. **真正的事件持久化 API 是 `RunState::event(type_name, extra_json)`**（`src/state/mod.rs:294`）→ 追加到 `{run_dir}/events.jsonl`。A1 `checkpoint`、A3bis `task_start.permission_tier` 都是用这个 pattern 写新 type 字符串。B3 沿用：加一个**新事件 type 字符串**（`context_compressed`），不是枚举变体。

4. **chat 相没有 run_dir / 没有 events.jsonl**。`ChatSession`（`types.rs:39-70`）无 `run_dir` 字段；chat 只写 `.cco/chat/{safe}.json`。压缩发生时 run 可能尚未存在（clarify 相在 confirm 之前）。

5. **`tokens_before`/`tokens_after`/`digest_hash` 当前都不存在**。digest 代码（`domain/chat/session_digest.rs`）只做字符级浅检与 `truncate_session_digest`（char cap 12_000），**无 token 计数**；chat 路径也无 tokenizer，只有 `truncate_chars`。`Cargo.toml` 无直接 sha2（仅 transitive）。

---

## 二、设计

### 2.1 事件落点：chat 相会话事件日志（非 run events.jsonl）

压缩发生在 chat 相、ChatSession 无 run_dir。因此 B3 事件**不**写 run 的 `events.jsonl`（那是 run scheduler 的领域，且 run 可能未建）。

新增**会话级事件日志**：`.cco/chat/{safe}.events.jsonl`，与 `{safe}.json` 同目录。复用 `RunState::event` 的**形状**（`{"ts": rfc3339, "type": type_name, …extra}`），但独立实现一个薄 append 函数（chat 无 `RunState`，且 `state/mod.rs` 已 603 行近硬上限，**不**往里堆——规则 18）。

落点：`src/services/chat/session.rs`（364 行，软上限内）加 `append_session_event(project, session_id, type_name, extra)` + 读回 `last_session_event_type`。`session.rs` 已持 `chat_dir`/`save_session`，事件 append 与 save 同属会话 IO。

### 2.2 触发点

`send.rs:194-195` 压缩成立分支（`session_digest_looks_valid` 通过且非 soft-fallback-empty）后，在 `sess.session_digest = Some(...)` 赋值**之后**写一条 `context_compressed` 事件。压缩**未发生**（无 fence / 被拒 / soft-fallback）时**不**写、**不**创建 events 文件。

### 2.3 事件字段（按可用数据，诚实标注近似）

```jsonl
{"ts":"2026-08-14T12:00:00Z","type":"context_compressed","session_id":"default","chars_before":N,"chars_after":M,"digest_hash":"sha256:…前12位"}
```

| 字段 | 来源 | 说明 |
|------|------|------|
| `ts` | `Utc::now().to_rfc3339()` | 与 RunState::event 同 |
| `type` | `"context_compressed"` | 新 type 字符串 |
| `session_id` | `sess.session_id` | 会话标识 |
| `chars_before` | push assistant 前 `sess.messages` 字符和 | **非 token**：chat 路径无 tokenizer。用历史消息总字符数作 `tokens_before` 的近似代理，字段名诚实写 `chars_before`（**不**谎称 token）。roadmap 的 `tokens_before/after` 是示意，落地以现有数据为准。 |
| `chars_after` | `sess.session_digest.chars().count()`（存储后） | digest 存储字符数；近似"保留摘要规模" |
| `digest_hash` | 存储后 digest 内容的 sha256 前 12 位 | 让 Inspect/回看能对"哪一份 digest"去重对照，非密码学用途 |

### 2.4 digest_hash 算法

`sha2` 在 Cargo.lock 是 transitive（reqwest/tantivy 引入），src/ 无直接 `use sha2`。直接加 `sha2 = "0.10"` 为 direct dep（版本对齐 lock），用 `sha256` 更稳且语义清晰；sha2 本就在依赖树内，加为 direct dep 不增传递体积。doc 注明它是**去重/对照用**，非安全用途。

### 2.5 chars_before 取值（诚实近似）

取**历史规模**（push assistant 前 `sess.messages` 字符和）作 `chars_before`——它是压缩**输入**的直接代理，且无需改 CLI 调用顺序。`chars_after` = 存储后 digest 字符数。doc 注明"近似字符，非 token；主路径不展示给非开发用户"。

### 2.6 表层（规则 23/24）

- 事件日志 `.cco/chat/{safe}.events.jsonl` 是**高级/诊断**产物，**默认不**在桌面主路径展示（规则 24）。本轮**不**加 UI、**不**加 `ChatSendResponse` 字段（reply/messages 不变）。
- 若将来 Inspect/巡检要感知"Worker 上下文是否截断"，那是 **run worker 的 context 事件**（不同于 chat digest）；本轮只做 chat digest 压缩事件，doc 明确边界。
- 桌面/CLI 均不暴露 `digest_hash`/`chars_*` 作主路径第一句（规则 23）。

---

## 三、不做的部分（本轮）

| 条目 | 理由 |
|------|------|
| `LogEvent::ContextCompressed` 枚举变体 | `LogEvent` 是平铺展示 DTO 非 enum；事件走 `RunState::event` 形状的会话级 append |
| run `events.jsonl` 里的 worker context 压缩事件 | 压缩在 chat 相；run worker 不读 digest（start.rs 只拼 handoff）。run worker token 截断事件属另一能力 |
| 真 token 计数（tokenizer） | chat 路径无 tokenizer；加 tokenizer 是独立工作。用 chars 近似并诚实命名字段 |
| `ChatSendResponse` 新字段 / 桌面 UI | 事件日志是诊断产物，规则 24 默认关；本轮只落盘 |
| 把 `session_digest` 注入 run worker | 违反现状（worker prompt 不带 digest）；超出 B3 范围 |
| 改 digest schema / 抽取 / 注入 | 属 C0–C2（已 ✅）；B3 只事件化 |
| 往 `state/mod.rs`(603) 加 chat 事件 API | 近硬上限（规则 18）；chat 事件 API 放 `services/chat/session.rs` |

---

## 四、验收标准

1. `cargo build` 通过；`cargo test -p cco` 绿（含新增/既有 chat digest 测）；`STRICT=1 scripts/check-arch.sh` 无新 violation。
2. `chat_send` 在压缩成立时向 `.cco/chat/{safe}.events.jsonl` 追加一条 `{"ts","type":"context_compressed","session_id","chars_before","chars_after","digest_hash"}`。
3. 压缩未发生（无 fence / soft-fallback-empty）时**不**写事件、**不**创建 events 文件。
4. `digest_hash` 为 `sha256:` 前缀 + 12 位 hex（直接 dep sha2）；可对同一 digest 复算一致。
5. 事件写入**不**改变 `ChatSendResponse` 任何字段（reply/messages/draft_plan 不变）；桌面/CLI 表层无变化（规则 23/24）。
6. 既有 `fake_send_produces_plan_and_digest` 等测试仍绿；新增一测：fake 路径压缩后 events.jsonl 含一条 `context_compressed` 且 `chars_after` ≤ `chars_before`。
7. 单文件不破硬 600：`session.rs` 加 append/read ≤ 软 400 目标；`send.rs` 触发 ≤ 几行（≤ 硬 80 函数）。

---

## 五、勾选（改代码时在此更新）

- B1 doc `docs/session-digest-event-2026-08-14.md`（问题/设计/不做/验收/勾选）✅
- B2 `session.rs::append_session_event` 会话级事件 append + 读回 ✅
- B3 `send.rs` 压缩成立时写 `context_compressed`（chars_before/after + sha256 digest_hash）✅
- B4 测试 + L2/doc 索引 + `check-arch.sh STRICT=1` 绿 ✅

---

> [PROTOCOL]: 改代码时先更新此文件勾选；完成后更新 docs/CLAUDE.md「还在做」区；
> 门禁：`scripts/check-arch.sh`；规则 18 厚文件只抽不堆；规则 23/24 诊断产物默认不进主路径。
