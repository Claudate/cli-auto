# agentmemory 轻量记忆集成计划

**日期**: 2026-08-12  
**状态**: Draft (Revised - 轻量化方案)  
**对齐**: `PRODUCT.md` 主受众（PM/出海/非开发） · `docs/architecture-redesign-2026-07-20.md` 六边形架构

---

## 一、目标与动机

### 问题陈述

当前 cco 的记忆系统存在三大限制：

1. **3-pin 硬限制**（`src/state/project_memory.rs:MAX_PINS_PER_PROJECT`）：复杂项目无法记录所有关键决策
2. **全文注入**（`src/domain/chat/session_digest.rs:SESSION_DIGEST_SOFT_MAX_CHARS = 12,000`）：长对话后 token 成本线性增长
3. **无跨运行学习**：每次 run 独立，无法避免重复踩坑（如"task X + claude → 总是超时"）

### 轻量化集成策略

**核心原则**：
- ✅ **纯 Rust 实现**：无需 Node.js / Docker / 外部服务
- ✅ **本地优先**：SQLite + 本地 embedding，数据在 `~/.cco/memory/`
- ✅ **多平台共享**：通过文件同步（iCloud / Dropbox / Git）跨设备
- ❌ **不引入**：Docker Compose / agentmemory 服务端 / 云端 API

### 集成价值

| 能力 | 轻量方案 | cco 当前 | 产品收益 |
|------|----------|----------|----------|
| **语义检索** | 本地向量库 (tantivy) | 无 | 自然语言提问历史 |
| **Token 效率** | 检索式注入 (top-3) | 全文 12k 注入 | 成本减少 60%+ |
| **跨运行学习** | SQLite 持久化 | 独立运行 | 避免重复踩坑 |
| **多平台共享** | 文件同步 | 单机隔离 | Mac/Linux/Windows 通用 |

### 对齐产品方向

**PRODUCT.md 核心主张**: "不需要重复解释"  
→ 轻量记忆直接支撑 **主受众（PM/出海/非开发）** 需求，无需额外部署

---

## 二、轻量化技术选型

### 核心原则

**✅ 采用**:
- **纯 Rust 实现**（无外部服务依赖）
- **SQLite 本地存储**（`~/.cco/memory/memory.db`）
- **本地 embedding 模型**（ONNX Runtime + all-MiniLM-L6-v2，23MB）
- **BM25 + 向量混合检索**（tantivy 全文检索 + 余弦相似度）
- **文件同步支持**（iCloud / Dropbox / 自定义路径）

**❌ 明确不用**:
- ❌ agentmemory TypeScript 服务端（Node.js 依赖）
- ❌ Docker Compose 部署（桌面软件不应依赖容器）
- ❌ REST API 客户端（已实现的 `agentmemory_client.rs` 待评估）
- ❌ 云端 embedding API（本地优先原则）

### 技术栈对比

| 组件 | agentmemory 原方案 | cco 轻量方案 (本次) |
|------|-------------------|---------------------|
| **服务架构** | Node.js 服务 + HTTP API | 纯 Rust 库（无独立进程） |
| **存储** | iii-engine (Docker) | SQLite (5MB/10k 条) |
| **Embedding** | 云端 API / Ollama 服务 | ONNX Runtime 内嵌（23MB 模型） |
| **检索** | 向量库 + 知识图谱 | tantivy BM25 + 余弦相似度 |
| **部署** | Docker Compose / npm global | 无需部署，Rust 静态链接 |
| **跨平台** | HTTP 协议 (localhost:3000) | 文件同步 (iCloud/Dropbox) |
| **启动方式** | `docker-compose up` / `agentmemory` 命令 | Tauri 启动时自动初始化 |

---

## 三、分阶段路线图

### P2 阶段（当前）

**结论**: **不集成**  
**原因**: 
- P2-17 t58 已收口，聚焦核心架构（A0-A5 ✅）
- thin memory 满足当前功能验证需求
- 避免技术栈复杂度过早引入

### P3 阶段：轻量实现（3-4 周）

**里程碑**: 纯 Rust 记忆系统 + 两个场景试点

#### 3.1 核心存储层（Week 1）

**交付物**:
```rust
// src/state/memory_store.rs (新增)
pub struct MemoryStore {
    db: SqlitePool,
    embedder: EmbeddingModel, // ONNX Runtime
}

pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>, // 384-dim
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl MemoryStore {
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>>;
    pub async fn store(&self, content: &str, metadata: Metadata) -> Result<String>;
    pub async fn delete(&self, id: &str) -> Result<()>;
}
```

**技术细节**:
- **SQLite schema**:
  ```sql
  CREATE TABLE memory (
      id TEXT PRIMARY KEY,
      content TEXT NOT NULL,
      embedding BLOB NOT NULL,  -- 384 * 4 bytes = 1.5KB
      metadata TEXT,            -- JSON
      created_at INTEGER NOT NULL
  );
  CREATE INDEX idx_created ON memory(created_at);
  ```
- **Embedding 模型**: `all-MiniLM-L6-v2` (ONNX 格式, 23MB)
  - 下载地址: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
  - 加载库: `ort = "2.0"` (ONNX Runtime for Rust)
  - 推理速度: M1 Max ~50ms/条

**验收标准**:
- [x] `cargo test state::memory_store` 全过（9 passed 含真实模型 perf · 2026-08-12）
- [x] 存 100 条记忆 → 实测 216KB（≈2.2KB/条；原 200KB 估算未计 SQLite 页开销，纯 embedding 已 150KB；测试上限定 256KB）
- [x] 单次检索 < 100ms (含 embedding 生成) — 实测 **12.6ms**（M 系 Mac · fp32 model.onnx 90MB，非 23MB 量化版）

#### 3.2 试点场景 1：Mode B Planner Context（Week 3-4）

**位置**: `src/plan/planner/llm.rs`

**改动**:
```rust
// 在 run_short_claude_print 前注入检索结果
pub async fn run_short_claude_print(
    config: &Config,
    memory: Option<&dyn MemoryPort>, // ← 新增可选依赖
    job: &PlannerJob,
) -> Result<String> {
    let context_enhancement = if let Some(mem) = memory {
        // 检索："类似 {job.project_type} 项目的成功拆分案例"
        let hits = mem.search(&format!("successful split for {} project", job.project_type), 3).await?;
        format_context_from_hits(hits)
    } else {
        String::new()
    };
    
    let prompt = compose_planner_prompt(job, &context_enhancement);
    // ... 原有 claude print 流程
}
```

**验收标准**:
- [ ] 运行 5 个不同类型项目（Rust+Tauri / Node.js / Python），记录拆分方案
- [ ] 下次同类型项目提问："上次 Tauri 项目怎么拆的？" → 能检索到并注入 context
- [ ] Mode B 生成的任务数减少 20%+（对比无记忆 baseline）

#### 3.3 试点场景 2：Cost-Aware Router 历史记忆（Week 5-6）✅ 已落地（2026-08-12）

**实际落地**（与原草图不同：按硬规则 5/6，domain 保持纯决策、IO 留 runtime）：

- `src/domain/worker/memory_route.rs`（新）— 纯决策：`MemoryOutcomeStats`（success/timeout/failed 计数）+ `memory_failover_target(stats, current, candidates)`；阈值 **≥3 样本且失败率 >30%**；候选（去当前 / 去 unhealthy / 去未注册）由调用方过滤
- `src/state/memory_store/port.rs`（新）— `LocalMemory` 实现 `MemoryPort`（按次短开 store，避免调度器长占 tantivy 写锁；`relate` P3 no-op）
- `src/app/memory.rs` — `semantic_port(config)` 返回 `Option<Arc<dyn MemoryPort>>`（`config.memory.enabled` 门控）
- `src/runtime/scheduler/memory.rs`（新）— `record_task_outcome`（任务终态写 `outcome {provider} {role} {outcome}` + metadata/tags）+ `maybe_memory_failover`（spawn 前查历史 → 达阈值切 provider，落 `route_source=Failover` · `route_note=memory:…` · `provider_switched` 事件 · `[MEMORY]` 日志；**Explicit route 不动**，守硬规则 13）
- `tick.rs` 两条终态路径（reap + 首轮 poll 快路径）均接录入；CLI（foreground）与桌面（services/runs）构造 Scheduler 时经 `semantic_port` 注入

**验收标准**:
- [x] 单测：≥3 次失败样本且失败率>30% → 决策切换到 failover_order 首个可用 provider（`domain::worker::memory_route` 6 项）
- [x] 集成：fake 任务跑完 → 记忆库出现 `outcome=success` 条目，可被路由查询命中（`tests/scheduler_fake.rs::memory_pilot_records_task_outcome`）
- [x] 日志输出：`[MEMORY] 历史失败率 60%（3/5 次）→ 预防性切换 claude → codex`（tracing info）
- [x] 集成：切换动作本身 — 预置 3 条 (claude, implement) timeout 历史 → spawn 前切到 failover_order 首个可用 provider，断言 `route_source=Failover` · `route_note=memory:…` · `provider_switched` 事件（`tests/scheduler_fake.rs::memory_pilot_preventive_failover_switches_provider` · 2026-08-12）
- [ ] 真机 e2e：真实 claude 3 次 timeout 后自动切 codex（需真实 provider 环境；切换逻辑已由上条 fake 集成覆盖，本条验证真实 timeout 录入链路）

### P4-P5 阶段：产品化（8-12 周）

#### 4.1 全系统集成

**范围扩展**:
1. **Inspect 质量提升**（`src/domain/inspect/`）
   - 记录用户手动 rework 时的拒绝原因
   - 下次类似 output 自动触发 stricter acceptance

2. **Session Digest 智能压缩**（`src/domain/chat/session_digest.rs`）
   - 替换当前 12k 全文注入
   - 只检索当前对话相关的 3-5 条记忆

3. **Guide Store 迁移**（`src/state/guide_store.rs`）
   - 从 SQLite JSON 字段迁移到 agentmemory 知识图谱
   - 保留 SQLite 作为元数据存储

#### 4.2 部署方案（轻量化）

**唯一方案**: Rust 静态链接，无需独立部署

```rust
// src-tauri/src/lib.rs
use cco::state::memory_store::MemoryStore;

#[tauri::command]
async fn init_memory(app_handle: tauri::AppHandle) -> Result<(), String> {
    let data_dir = app_handle.path_resolver()
        .app_data_dir()
        .ok_or("无法获取数据目录")?;
    
    let memory_path = data_dir.join("memory");
    tokio::fs::create_dir_all(&memory_path).await
        .map_err(|e| format!("创建目录失败: {}", e))?;
    
    // 初始化 SQLite + 加载 ONNX 模型
    let store = MemoryStore::new(&memory_path).await
        .map_err(|e| format!("初始化记忆失败: {}", e))?;
    
    app_handle.manage(store);
    Ok(())
}
```

**配置**:
```toml
# ~/.cco/config.toml
[memory]
enabled = true
model_path = "~/.cco/models/all-MiniLM-L6-v2.onnx"  # 23MB，首次自动下载
ttl_days = 90
max_entries = 10000
sync_provider = "icloud"  # 可选: "dropbox" | "none"
```

**优势**:
- ✅ 一键启动，无需 Docker / Node.js
- ✅ 静态链接，Tauri 打包直接包含
- ✅ 跨平台二进制（Mac/Linux/Windows）

#### 4.3 隐私保护

**本地优先原则**:
- 默认 embedding: Ollama `nomic-embed-text`（CPU 可运行，3 秒/100 条）
- 禁止云端同步（除非用户显式开启）
- 敏感字段（API key / token）不入库

**数据生命周期**:
```rust
// src/state/memory_policy.rs
pub struct MemoryPolicy {
    pub ttl_days: u32,           // 默认 90 天
    pub max_entries: usize,      // 默认 10,000 条
    pub auto_archive: bool,      // 超期自动归档到 ~/.cco/archive/
}
```

### P6+ 阶段：长期演进

#### 高级特性

1. **多 Agent 共享记忆**
   - `CollabBus` 事件 → 自动入库
   - 分布式运行时的跨 agent 知识共享

2. **UI 可视化**
   - `web/js/features/memory/` 新模块
   - 知识图谱 D3.js 可视化：技术选型 → 原因 → 相关任务

3. **Active Learning**
   - 用户点赞/点踩检索结果 → 反馈模型
   - 逐步提升检索准确率至 98%+

---

## 三、技术细节

### 3.1 REST API 协议

基于 agentmemory 官方文档（2026-08-12 确认）：

```bash
# 存储
POST http://localhost:3000/memory
{
  "key": "task-browser-ui-verify-2026-08-12",
  "content": "browser_ui_verify 在 claude 上总是超时，改用 codex 成功",
  "metadata": {
    "task_role": "browser",
    "provider": "claude",
    "outcome": "timeout",
    "solution": "switch_to_codex"
  }
}

# 检索
GET http://localhost:3000/memory/search?q=browser%20verify%20timeout&limit=5
→ [
  {
    "key": "...",
    "content": "...",
    "score": 0.92,
    "metadata": {...}
  }
]

# 关系
POST http://localhost:3000/memory/relate
{
  "from": "task-browser-ui-verify",
  "to": "provider-codex",
  "relation": "solved_by"
}
```

### 3.2 Embedding 选型

| 模型 | 维度 | 速度 (M1 Max) | 质量 |
|------|------|---------------|------|
| **nomic-embed-text** (推荐) | 768 | 3s/100 条 | R@5 = 88% |
| text-embedding-3-small | 1536 | API 调用 | R@5 = 92% |
| BGE-M3 | 1024 | 5s/100 条 | R@5 = 90% |

**默认方案**: nomic-embed-text (Ollama) + 可选云端加速

### 3.3 数据模型映射

```rust
// src/ports/memory.rs
pub struct MemoryHit {
    pub key: String,
    pub content: String,
    pub score: f32,      // 0.0 - 1.0
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

pub struct Metadata {
    pub project_id: Option<String>,
    pub task_role: Option<String>,
    pub provider: Option<String>,
    pub outcome: Option<String>,  // "success" | "timeout" | "rejected"
    pub tags: Vec<String>,
}
```

---

## 四、风险控制

### 4.1 技术风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| agentmemory API 变更 | 中 | 中 | 锁定版本 + 每月 upstream 同步 |
| Embedding 质量不达标 | 低 | 高 | P3 Spike 先验证 R@5 > 90% |
| 本地部署门槛高 | 高 | 中 | Docker Compose 一键启动 + 详细文档 |
| 内存/存储开销 | 中 | 低 | 10k 条 ≈ 50MB，设置 TTL 自动清理 |

### 4.2 产品风险

| 风险 | 缓解 |
|------|------|
| 用户不理解"智能记忆"概念 | UI 文案：展示具体例子（"上次拆分方案"） |
| 隐私担忧 | 明确标注"本地存储" + 开源可审计 |
| 检索结果不准 | 点赞/点踩反馈 + 置信度阈值（< 0.7 不注入） |

### 4.3 回滚策略

```rust
// src/ports/memory.rs
pub struct NoOpMemory;  // 空实现，向后兼容

impl MemoryPort for NoOpMemory {
    async fn search(&self, _: &str, _: usize) -> Result<Vec<MemoryHit>> {
        Ok(vec![])  // 返回空结果，不影响主流程
    }
}
```

**配置开关**:
```toml
[memory]
enabled = false  # 一键关闭，降级到 P2 thin memory
```

---

## 五、验收标准

### P3 完成标志

- [x] 两个试点场景（Mode B + cost router）能独立运行（单测/fake 集成级 · 2026-08-12：场景 1 存取闭环单测，场景 2 outcome 录入 + 预防性切换两条集成测试；真实项目/真机验证见 3.2/3.3 残余项）
- [x] 集成测试覆盖率 > 80%（记忆相关模块行覆盖 85.9%–100% · cargo llvm-cov 2026-08-12；`scheduler/memory.rs` 91% · `memory_store` 93.8% · `memory_route` 98.5%）
- [x] 性能基准：10k 条存储 + 100 次检索 < 5 秒 — 实测 **2.31s**（存 1.67s + 检 0.64s · `store_batch` 单事务批量路径 · stub embedding 隔离存储层；真实模型 embedding ~12.6ms/条 已单独测。注：10k 实测库体积 ≈20.8MB，非早期估算 5MB）
- [x] 文档：`docs/memory-user-guide.md` + `docs/memory-dev-guide.md`（2026-08-12）

### P4-P5 完成标志

- [ ] Docker Compose 一键部署成功率 > 95%
- [ ] 新用户 onboarding：从空白到第一次检索 < 5 分钟
- [ ] 产品指标：检索使用率 > 30%（活跃用户/周）
- [ ] 成本下降：平均 token 消耗减少 40%+（对比 P2 baseline）

### P6+ 完成标志

- [ ] UI 知识图谱可视化上线
- [ ] 多 Agent 共享记忆在生产环境验证
- [ ] 社区贡献：至少 3 个外部 contributor 提交 memory 相关 PR

---

## 六、资源需求

### 开发工作量估算

| 阶段 | 工作量 | 关键路径 |
|------|--------|----------|
| P3 Spike | 2 周 | REST API 客户端 + 两个场景试点 |
| P4 产品化 | 4 周 | 全系统集成 + Docker 部署 |
| P5 抛光 | 4 周 | UI 可视化 + 性能优化 |
| **总计** | **10 周** | - |

### 依赖清单

**Rust 依赖**:
```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
```

**外部服务**:
- [agentmemory](https://github.com/rohitg00/agentmemory) 服务端（Docker / npm）
- [Ollama](https://ollama.ai) 本地 embedding（可选云端 OpenAI）

---

## 七、后续行动

### 立即行动（P2 完成后）

1. **Week 1**: 本地启动 agentmemory + Ollama，手动验证 API
2. **Week 2**: 实现 `src/ports/memory.rs` + 单元测试
3. **Week 3-4**: Mode B planner context 增强试点
4. **Week 5**: 评审决策：继续 P4 或暂停

### 长期对齐

- **月度同步**: agentmemory upstream 更新检查
- **季度回顾**: 检索质量指标 + 成本下降验证
- **年度**: 评估是否需要自研（当 agentmemory 不再满足需求时）

---

## 附录

### A. 参考资料

- agentmemory GitHub: https://github.com/rohitg00/agentmemory
- iii engine: https://github.com/iii-hq/iii
- Karpathy LLM Wiki: https://github.com/karpathy/llm.c/wiki
- Ollama: https://ollama.ai/library/nomic-embed-text

### B. 决策日志

| 日期 | 决策 | 原因 |
|------|------|------|
| 2026-08-12 | 选择 agentmemory 而非自研 | 26.8k stars，Apache-2.0，活跃维护，MCP 标准协议 |
| 2026-08-12 | P3 先试点再全面集成 | 降低风险，验证假设（92% token 减少） |
| 2026-08-12 | 本地 Ollama 优先于云端 | 对齐 cco 本地优先原则 + 隐私保护 |

### C. 联系人

- **架构决策**: 参考 `docs/architecture-redesign-2026-07-20.md`
- **产品方向**: 参考 `PRODUCT.md`
- **技术问题**: 提 issue 到本仓库或 agentmemory upstream

---

**最后更新**: 2026-08-12  
**下一次评审**: P2-17 完成后立即启动 P3 Spike
