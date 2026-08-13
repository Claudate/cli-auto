# 语义记忆 · 开发指南

> 面向开发者。使用者视角见 [`memory-user-guide.md`](./memory-user-guide.md)；勾选真源 [`agentmemory-integration-plan-2026-08-12.md`](./agentmemory-integration-plan-2026-08-12.md)。

## 架构摆位（六边形）

```
Presentation（CLI foreground / services::runs）
    └─ app/memory.rs          semantic_port · semantic_split_context · remember_split_success
        ├─ ports/memory.rs    MemoryPort trait + MemoryHit/Metadata DTO + NoOpMemory
        ├─ domain/worker/memory_route.rs   纯决策：MemoryOutcomeStats · memory_failover_target
        └─ state/memory_store/
            ├─ mod.rs         MemoryStore（SQLite + tantivy BM25 + ONNX embedding）
            └─ port.rs        LocalMemory: MemoryPort（按次短开，避免长占 tantivy 写锁）
runtime/scheduler/memory.rs   IO 胶水：record_task_outcome · maybe_memory_failover
```

依赖方向守 L1 硬规则 5/6：domain 只有纯决策（无 IO / 无 async）；检索与写入 IO 在 runtime / state。

## 两条数据流

### 1. Mode B 拆分记忆（`src/plan/planner/llm.rs` → `app/memory.rs`）

- 拆分成功 → `remember_split_success` 写入 `split-{job_id}` 条目（best-effort，失败只 warn）
- `build_llm_plan` 组 prompt 后 → `semantic_split_context` 检索 top-3，注入「历史拆分记忆（仅参考）」块
- 同项目记忆标 `[本项目]`，跨项目标 `[其他项目]`

### 2. Cost Router 历史 failover（`runtime/scheduler/memory.rs`）

- 任务终态（tick.rs 的 reap 与首轮 poll 快路径均接入）→ `record_task_outcome` 写 `outcome {provider} {role} {outcome}` 条目
- spawn 前 → `maybe_memory_failover` 检索同 (provider, role) 历史 → `domain::memory_failover_target` 判定（**≥3 样本且失败率 >30%** → 切 failover_order 首个可用）
- 切换落 `route_source=Failover` · `route_note=memory:…` · `provider_switched` 事件 · `[MEMORY]` tracing
- **硬规则 13**：`RouteSource::Explicit` 的任务永不改写；候选已过滤 当前 / unhealthy / 未注册

## 存储层（`state/memory_store`）

- **SQLite**：`memory(id, content, embedding BLOB, metadata JSON, created_at)`；embedding 384 维 f32 LE 编码
- **tantivy**：BM25 全文预筛（over-fetch 50 候选）→ SQLite 载入 → 余弦相似度重排
- **embedding**：`~/.cco/models/all-MiniLM-L6-v2.onnx` + `tokenizer.json` 存在则 ONNX 推理（mean pooling）；缺失回退 stub 零向量（此时排序退化为 BM25 序）
- **生命周期**：每次写后 `check_and_archive`（TTL 过期删除 + 超 `max_entries` 删最旧）
- **写入两条路**：`store`（单条，每次 commit 索引）；`store_batch`（单事务 + 单次 tantivy commit，批量导入/基准用——单条路径在万级会被逐条 commit 支配）
- `delete` 只删 SQLite；tantivy 陈旧条目 P4 再清（检索时 SQLite 查不到即跳过）

## 配置门控

`~/.cco/config.toml` `[memory]`：`enabled`（默认 false → 零行为变化）· `ttl_days` · `max_entries`。
`app::memory::semantic_store_cfg` 是唯一门控点；调度器经 `semantic_port` 拿 `Option<Arc<dyn MemoryPort>>`，None 即无记忆。
存储根：`<state_root>/memory`（生产即 `~/.cco/memory`）。

## 测试与基准

常规（每次 `cargo test` 跑）：

- `state::memory_store::tests` — init / CRUD / BM25 / TTL / max_entries / batch roundtrip / 余弦
- `state::memory_store::port::tests` — LocalMemory 经 port 的存取闭环
- `domain::worker::memory_route::tests` — 决策 6 项（阈值边界 / 无候选 / stopped 不算失败）
- `app::memory::tests` — 门控（默认关不注入）+ 拆分记忆存取闭环
- `tests/scheduler_fake.rs::memory_pilot_records_task_outcome` — fake 跑完记忆库出现 outcome 条目

基准（`--ignored` 显式跑，实测 2026-08-12 · M 系 Mac）：

| 测试 | 标准 | 实测 |
|------|------|------|
| `memory_store_perf_real_model`（需真实模型） | 单次检索含 embedding < 100ms；100 条 < 256KB | 检索 ~12.6ms；218KB |
| `memory_store_perf_10k_bulk`（stub embedding，隔离存储路径） | 10k 存储 + 100 次检索 < 5s | 2.31s（存 1.67s + 检 0.64s）|

注意：10k 条实测库体积约 20.8MB（≈2.1KB/条，含 tantivy 索引前的 SQLite 部分），计划文档早期估算 5MB/10k 偏乐观。真实模型下批量写入会被 embedding 推理支配（~12.6ms/条 → 10k ≈ 2 分钟），基准故意用 stub 隔离存储路径。

## 硬边界（改动时必守）

1. 记忆一律 **best-effort**：任何 store/search 失败不得影响拆分、开跑、finish 主路径
2. 检索结果**只作 context 注入**：不改 route（Explicit 不动）、不改 confirm 语义、不改任务图
3. 开跑入口不变：记忆不构成第二开跑入口（L1 硬规则 10）
4. domain 保持纯函数；新 IO 只进 runtime/state 适配器
