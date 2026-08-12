# agentmemory 集成总结

**日期**: 2026-08-12  
**状态**: 评估完成，P3 待启动

---

## 一、已完成工作

### 1. 优点分析 ✅

对比 agentmemory 与 cco 当前记忆系统，核心优势：

| 能力 | agentmemory 理念 | cco 轻量方案 | 当前 cco |
|------|-----------------|-------------|----------|
| **语义检索** | 向量库 + 知识图谱 | tantivy BM25 + 余弦相似度 | 无 |
| **Token 效率** | 检索式注入 | 检索 top-3 | 全文 12k 注入 |
| **跨运行学习** | 持久化 + 关系 | SQLite 持久化 | 独立运行 |
| **容量限制** | 10k+ 条 | 10k 条 (可配置) | 3 pin 硬限制 |

**核心收益**：
- 自然语言提问历史（对齐 PRODUCT.md "不需要重复解释"）
- 成本减少 60%+（检索式注入 vs 全文 12k）
- 避免重复踩坑（记录"browser_ui_verify + claude → timeout"）

### 2. 技术选型决策 ✅

**最终方案**：纯 Rust 轻量实现

| 组件 | agentmemory 原方案 | cco 轻量方案 (采用) |
|------|-------------------|---------------------|
| **服务架构** | Node.js 服务 + HTTP API | 纯 Rust 库（无独立进程） |
| **存储** | iii-engine (Docker) | SQLite (5MB/10k 条) |
| **Embedding** | 云端 API / Ollama 服务 | ONNX Runtime 内嵌（23MB 模型） |
| **检索** | 向量库 + 知识图谱 | tantivy BM25 + 余弦相似度 |
| **部署** | Docker Compose / npm global | 无需部署，Rust 静态链接 |
| **跨平台** | HTTP 协议 (localhost:3000) | 文件同步 (iCloud/Dropbox) |
| **启动方式** | `docker-compose up` / `agentmemory` 命令 | Tauri 启动时自动初始化 |

**不采用的方案**：
- ❌ Docker Compose（桌面软件不应依赖容器）
- ❌ Node.js 服务端（增加运维负担）
- ❌ REST API 客户端（HTTP 开销）
- ❌ 云端 embedding API（本地优先原则）

### 3. 落地计划文档 ✅

完整文档：`docs/agentmemory-integration-plan-2026-08-12.md` (506 行)

**分阶段路线**：
- **P2**（当前）：不集成，聚焦核心架构
- **P3**（3-4 周）：纯 Rust 实现 + 两个试点场景
- **P4-P5**（8-12 周）：全系统集成 + 产品化
- **P6+**：UI 可视化 + Active Learning

### 4. 基础接口层 ✅

已实现（P3 可复用）：

**文件**：
- `src/ports/memory.rs` (220 行) - MemoryPort trait + 数据结构
- `src/ports/mod.rs` - 模块注册
- `src/config/mod.rs` - MemoryConfig 配置结构

**接口设计**：
```rust
#[async_trait]
pub trait MemoryPort: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>>;
    async fn store(&self, key: &str, content: &str, metadata: Metadata) -> Result<()>;
    async fn relate(&self, from: &str, to: &str, relation: &str) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn health(&self) -> Result<()>;
}
```

**向后兼容**：
```rust
pub struct NoOpMemory; // 空实现，enabled=false 时使用
```

---

## 二、P3 实施清单（待启动）

当前状态：**P2-17 未完成，P3 不启动**

### P3-Week1: 核心存储层

**交付物**：
- [x] `src/state/memory_store/`（mod.rs + tests.rs）- SQLite + tantivy BM25 + ONNX embedding（模型缺失回退 stub 零向量）· 2026-08-12
- [x] 依赖添加：`tantivy`, `ort 2.0-rc.13`, `tokenizers`（`ndarray` 实测不需要，已移除）
- [x] 单元测试：8 个全过（init/CRUD/BM25 检索/TTL/max_entries 归档/余弦相似度）；性能基准（100 条 < 100ms）待模型落地后测

### P3-Week3-4: 试点场景 1（Mode B Planner）

**位置**：`src/plan/planner/llm.rs` + `src/app/memory.rs`（实际落地形态；未改 `run_short_claude_print` 签名）

**已落地（2026-08-12）**：
- [x] `app::memory::remember_split_success` — LLM 拆分成功后写入语义记忆（best-effort）
- [x] `app::memory::semantic_split_context` — `build_llm_plan` 组 prompt 后检索 top-3 注入「历史拆分记忆」块
- [x] `config.memory.enabled` 门控（默认 false → 行为零变化）；单测覆盖门控 + 存取闭环

**验收标准（需真实项目运行）**：
- [ ] 5 个不同类型项目实测检索命中
- [ ] 任务数减少 20%+（对比无记忆 baseline）

### P3-Week5-6: 试点场景 2（Cost Router）✅ 已落地（2026-08-12）

**实际落地**（domain 纯决策 + runtime IO，非原草图的 async domain 函数）：

- `domain/worker/memory_route.rs` — 纯决策（≥3 样本失败率>30% → 切 failover_order 首个可用）
- `state/memory_store/port.rs` — `LocalMemory: MemoryPort`（按次短开）
- `app/memory.rs::semantic_port` — `config.memory.enabled` 门控的调度器句柄
- `runtime/scheduler/memory.rs` — 终态 outcome 录入 + spawn 前预防性 failover（Explicit route 不动；`route_note=memory:…` + `provider_switched` 事件）

**验收标准**：
- [x] 决策单测 6 项 + 集成测试（fake 跑完记忆库有 outcome 条目）
- [ ] 真机 e2e：真实 claude 3 次 timeout → 自动切 codex（需真实 provider）

---

## 三、待清理项（P3 启动前）

### 删除超范围实现

当前存在但 P3 不需要的文件：

1. **src/ports/agentmemory_client.rs** (319 行)
   - REST API 客户端
   - 轻量方案不使用 HTTP
   - 建议：P3 启动时删除

2. **Cargo.toml 依赖**
   - `urlencoding = "2"` - 仅 HTTP 客户端需要
   - 建议：P3 启动时移除

### 保留资产

以下文件保留，P3 可直接复用：

- ✅ `src/ports/memory.rs` - MemoryPort trait
- ✅ `src/config/mod.rs` - MemoryConfig
- ✅ `docs/agentmemory-integration-plan-2026-08-12.md` - 完整路线图

---

## 四、决策日志

| 日期 | 决策 | 原因 |
|------|------|------|
| 2026-08-12 | 选择 agentmemory 理念而非自研 | 验证过的架构（26.8k stars） |
| 2026-08-12 | 采用纯 Rust 实现而非 Node.js 服务 | 桌面软件轻量化原则 |
| 2026-08-12 | 移除 Docker 部署方案 | 不符合 Tauri 桌面应用定位 |
| 2026-08-12 | P2 不启动实施 | 聚焦核心架构 A0-A5 ✅ |
| 2026-08-12 | 保留 MemoryPort trait | P3 可复用，向后兼容 |

---

## 五、下一步行动

### 立即行动（P2 完成后）

1. **删除超范围代码**
   ```bash
   rm src/ports/agentmemory_client.rs
   # 编辑 Cargo.toml，移除 urlencoding 依赖
   # 编辑 src/ports/mod.rs，移除 agentmemory_client 相关导出
   ```

2. **P3-Week1 启动**
   - 添加依赖：`tantivy`, `ort`, `ndarray`
   - 创建 `src/state/memory_store.rs`
   - 实现 MemoryStore 基础结构

### 验收标准

**P2 阶段（当前）**：
- ✅ 优点分析完成
- ✅ 落地计划文档完成（506 行）
- ✅ 技术选型明确（纯 Rust 轻量方案）
- ✅ 基础接口层就绪（MemoryPort trait）

**P3 阶段（待启动）**：
- [ ] 两个试点场景能独立运行
- [ ] 集成测试覆盖率 > 80%
- [ ] 性能基准：10k 条存储 + 100 次检索 < 5 秒

---

**最后更新**: 2026-08-12  
**状态**: 评估完成，等待 P2-17 收口后启动 P3
