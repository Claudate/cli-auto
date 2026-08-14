# Harness 启示 · Leaf 优化与扩展方向

> 类型：**方向参考**（非实施阶段表）  
> 日期：2026-08-14  
> 来源：DeepSeek Harness 开发者预览版分析  
> 角色：对照 Harness 架构设计的 Leaf 可优化方向与扩展机会；**不是**新阶段序；勾选只认各功能真源文档

---

## 一、Harness 核心主张（与 Leaf 相关的部分）

DeepSeek Harness 是一套以 Cordis 微内核为底、"一切皆插件"的 Agent SDK 框架。  
与 Leaf/cco 高度对位的设计决策有六条：

| # | Harness 设计主张 | 一句话总结 |
|---|-----------------|-----------|
| H1 | **Session Log 是权威事件源** | 所有状态（工具调用/权限切换/压缩/取消）进追加日志，UI 从日志派生，不各自维护 |
| H2 | **工具调用有流水线** | 前置策略 → 安全守卫 → 执行 → 后置处理；守卫拒绝不可绕过 |
| H3 | **能力集可声明组合** | 不同运行预设（Minimal/Standard/Creative）从同一内核装配不同能力 |
| H4 | **多 Agent 有作用域隔离** | 每个子 Agent 拥有自己的工具集和上下文层，随生命周期清理 |
| H5 | **Session Resume / Fork** | 从历史检查点恢复，或从确定边界派生新会话 |
| H6 | **安全是系统约束，不是弹窗** | workspace-write 默认，danger 模式须显式选；失败关闭原则 |

---

## 二、Leaf 当前状态与 Harness 的对照

| 能力面 | Leaf/cco 现状 | Harness 对应 | 差距 |
|--------|--------------|-------------|------|
| 事件日志 | `log_events.rs` 监控流 | Session Log 全量权威 | Leaf 的日志是监控面板，不是 UI/状态的唯一派生源 |
| 工具安全 | WorkerPort 接任务就运行 | 前置策略+守卫+审批链 | 无正式工具执行流水线；浏览器自动化 W0-W3 已落但无统一门控 |
| 能力预设 | claude/codex/fake 三个硬编码 provider | 多预设同一内核装配 | 无"只读巡检模式" vs "读写执行模式"区分 |
| 多 Worker 隔离 | role/scope 路由（已落地） | 每 Agent 独立能力作用域 | 有路由无正式能力声明；Worker 工具集不透明 |
| 会话恢复 | `cco resume` CLI 已实现（resets non-Done → Pending）；无粒度 checkpoint 事件；桌面无恢复按钮 | Resume/Fork 从事件检查点 | 恢复粒度粗（全量重置）；无"从第 N 任务继续"语义；`session_resume = false` 全 provider |
| 安全模型 | `bypassPermissions` = 默认（Claude/fake worker soft-fill）；无工作区边界层 | workspace-write 默认；danger 须显式选；失败关闭 | Leaf 默认是 Harness 最宽松模式的反向——默认绕过；无 per-call SandboxMode；无 ApprovalPolicy |
| 无头模式 | CLI 可运行，无结构化一次性输出 | Headless：接任务→完成→退出 | CI/自动化集成体验缺失 |
| 压缩事件化 | Session Digest C0-C2 已落（每轮内置） | 压缩事件进 Session Log | 压缩已做但未作为事件进 log_events |

---

## 三、优化与扩展方向（按 Leaf 受众优先级排序）

### A 类：高价值 · 直接服务 PM/非开发主受众

---

#### A1 · Run Resume 检查点恢复

**问题**：PM 用户发起一个 10 任务的 Run，执行到第 6 条时 Worker 超时或进程崩溃。  
桌面没有"从断点继续"按钮；当前恢复语义粗（全量重置非 Done 任务，不区分"第 N 条之后"）。  
Harness 启示：Session 是追加事件流，从最后一个已完成边界可以准确 fork/resume。

**已有**：  
- `cco resume` CLI 已实现（`src/cli/commands/resume.rs`）——`prepare_for_resume()` 把所有非 Done/非 Skipped 任务重置为 Pending，再走 `prepare_scheduler`；  
- `handoff/store.rs` 有 SQLite 任务状态持久化；  
- events.jsonl 已记录 `run_start / task_start / task_end / run_end`。

**真正的差距**：  
1. events.jsonl 缺粒度 checkpoint——无法知道"第 N 条完成时的确定边界"，恢复只能全量重置；  
2. 桌面 Run 台失败后没有"从断点继续"按钮（只有重新开始）；  
3. `session_resume = false` on all WorkerPort providers——Worker 侧无法恢复自己上一轮的会话上下文；  
4. 无 `cco fork --from-task <id>` 语义（A/B 对比不同 Worker 的前置）。

**Leaf 方向**：  
- Scheduler tick 每个 `task_end(Done)` 写一条 `checkpoint` 事件到 events.jsonl（`type = "checkpoint"`, `task_id`, `ts`）；  
- `cco resume` 读最新 checkpoint，只重入 checkpoint 之后未完成的任务（替代当前全量重置）；  
- 桌面 RunView 失败时显示"从断点继续"按钮，调 `gateway.resumeRun()`；  
- `session_resume` 升级为可选实现（先 Claude provider，再其他）。

**约束**：改 Scheduler 须遵守架构 §3；不破坏 `confirm_start` 唯一开跑入口（规则 10）；`cco resume` 仍走同一 Application API（规则 12）。

---

#### A2 · Worker 能力预设（Safe / Full / Inspect-only）

**问题**：对 PM 受众，Worker 能做什么是个黑盒。Harness 的 Minimal（只有 2 个工具）vs Standard（全量）设计表明：限制工具集 = 降低风险感知，也真的降低风险。

**Leaf 方向**：
```toml
# plan.toml / run config 里声明
[worker_profile]
preset = "safe"   # safe | full | inspect

# safe  = 只读文件 + 网络搜索 + 浏览器截图；禁止写盘/Shell
# full  = 现有全量能力
# inspect = 只读 + diff + 无写盘；专用于巡检阶段
```
- Domain 层新增 `WorkerCapabilitySet` 枚举，WorkerPort trait 加 `capability_set() -> &WorkerCapabilitySet`；  
- Scheduler 在分配任务前校验任务类型与 Worker 能力集匹配；  
- 桌面拆分台：Worker 角色选择 UI 可显示"读写"/"只读"标记（对应 PRODUCT 主路径"不暴露工程黑话"——用人话而非 preset 名）。

**约束**：现有 role/scope 路由不动（规则 13）；preset 是能力边界，不是路由策略。

---

#### A3 · 工具安全门控统一层

**问题**：浏览器自动化（W0-W3）、Shell 执行、文件写入分散在各 adapter 实现里，没有统一的"允许/拒绝/需用户审批"策略。

**Harness 启示**：工具调用 = 前置策略 → 安全守卫 → 执行 → 后置；守卫失败不能被后续插件绕过；权限切换进 Session Log。

**Leaf 方向**：  
在 `ports/worker.rs` 的 WorkerPort trait 上增加执行钩子：
```rust
trait WorkerPort {
    // 现有
    fn assign(&self, task: &TaskRef) -> ...;
    
    // 新增（可选默认 impl = 透传）
    fn pre_execute(&self, action: &WorkerAction) -> PolicyDecision;
    fn post_execute(&self, action: &WorkerAction, result: &ActionResult);
}

enum PolicyDecision { Allow, Deny(reason), RequireApproval(prompt) }
```
- 默认实现 = Allow（向后兼容）；  
- 未来可注入 workspace-write guard：文件操作检查是否在 run_dir 边界内；  
- `RequireApproval` 路径在桌面弹确认，CLI 打印并阻塞等待输入。

**约束**：本条是基础设施，不改现有任务执行语义；不为 A2-5 可选任务 confirm 逻辑（已有规则 14）做第二入口。

---

#### A3bis · 安全模型：SandboxMode / ApprovalPolicy

**问题**：Leaf 当前默认行为与 Harness 安全默认完全相反。

具体代码现状（已核实）：
- `src/app/run/materialize/mod.rs` `apply_permission_mode`：默认 soft-fill `bypassPermissions`（Claude + fake worker）；
- 无 per-call SandboxMode 概念；
- 无 ApprovalPolicy 抽象——审批逻辑硬编码在 Claude CLI 自身，Leaf 不感知。

Harness 设计：
- `SandboxMode` per-call：`read-only` | `workspace-write` | `danger-full-access`
- `ApprovalPolicy`：`ask`（交互审批）| `never`（失败关闭）；`ask` 是安全默认
- 权限切换动作写入 Session Log（可审计）

**差距**：Leaf 的 `bypassPermissions` 是 Harness `danger-full-access` 的等价物，且是默认值。PM 受众无法知道 Worker 当前处于哪个安全层级。

**Leaf 方向**：
- Domain 层引入 `PermissionTier` 枚举：`ReadOnly | WorkspaceWrite | FullAccess`（对应 bypassPermissions/allowedTools 策略组合）；
- WorkerPort 增加 `default_permission_tier() -> PermissionTier`，默认 `WorkspaceWrite`（对齐 Harness 默认，比现有 bypassPermissions 更保守）；
- Scheduler 每次分配任务前记录 tier 到 events.jsonl（权限变更可审计）；
- 桌面拆分台/Worker 选择 UI 显示人话安全标签（"可读写项目文件" / "受限只读" 而非技术枚举）。

**约束**：不破坏现有 `bypassPermissions` 行为（通过 `default_permission_tier = FullAccess` 向后兼容）；改 default 须显式配置，不静默变更已有项目行为；规则 13 provider 路由不动。

---

### B 类：中等价值 · 提升工程侧体验与可观测性

---

#### B1 · log_events 升级为统一事件总线

**优先级说明**：架构分析后这条实为 A 级架构债（归 B 是沿用初稿；改动中长期，故暂保留 B 标签，但应在 A1/A3 后优先）。

**问题**：`log_events.rs` 目前是 TUI 的监控流。Web UI 的进度状态来源是 `setInterval(2000ms)` 轮询 `get_project_live`，后者读 RunState 文件而非 events.jsonl——两套源，结构性不一致。

具体代码现状（已核实）：
- `shellBoot.js:startPolling` — `setInterval(intervalMs=2000)` 每 tick 调 `window.loadLive()`；
- `loadLive.js` — 调 `gateway.getProjectLive` → Tauri `get_project_live` → 从磁盘读 RunState；
- events.jsonl 存在，写入 `run_start/task_start/task_end/run_end`，但**前端完全没有消费**；
- 导致：session digest 压缩、handoff verdict、permission escalation 等业务事件不进任何可观察流。

**Harness 启示**：Session Log 是权威来源，所有界面从同一事件流派生状态。

**Leaf 方向**：  
- 把 `log_events` 扩展成带 `event_type` 分类的结构化事件（已有 `LogEvent`，需加字段）；  
- 关键业务事件（task_started / task_done / checkpoint / compress / handoff_verdict / permission_escalation）都进同一流；  
- 前端 `run/RunViewModel.js` 订阅 `gateway.subscribe('run_events')` 代替分别轮询 handoff 和 log；  
- TUI 观察层复用同一流（现已接近这个方向，补齐语义类型）。

**约束**：`state.js` 已是桥/瘦 ~230 行，禁止再堆（规则 18）；事件类型扩展走 Rust 侧 DTO，不在 JS 重新定义；2s 轮询可作降级兜底，不需要一次全部替换。

---

#### B2 · 无头（Headless）输出模式

**问题**：非开发用户可能通过脚本、其他 AI 工具调用 cco。目前 CLI 有交互态，无"接任务→完成→结构化输出→退出"的一次性模式。

**Leaf 方向**：
```bash
cco run --headless --plan plan.md --output json
# 输出: {"run_id":"...","status":"completed","tasks":[...]}
```
- `cli/` 增加 `--headless` flag，进入无 TUI、无交互确认的静默执行路径；  
- 等价于 `confirm_start` 直接触发，Run 完成后打印结构化 JSON 到 stdout；  
- stderr 保留 log_events 流（可 `2>/dev/null` 丢弃）。

**约束**：`--headless` 仍走同一 Application API（规则 12），不是第二套调度。

---

#### B3 · Session Digest 事件化

**问题**：会话语义压缩（C0-C2 ✅）已做，但压缩动作本身未进 log_events，所以事后无法知道"当时模型看到了多少上下文"。

**Harness 启示**：压缩事件（context_compressed）进 Session Log，包含压缩前 token 数、保留摘要 hash。

**Leaf 方向**：  
`context-digest-compress-landing-2026-07-27.md` C3（可选 pin）配套：  
每次压缩后写一条 `LogEvent::ContextCompressed { tokens_before, tokens_after, digest_hash }` 到 log_events；  
这让 Inspect 阶段能感知到"本次执行中 Worker 的上下文是否被截断过"。

---

### C 类：探索方向 · 中长期 / 可选

---

#### C1 · Worker 能力自声明协议

**Harness 启示**：Agent 可以检查自身运行时插件树（创造模式）。  
**Leaf 方向**：WorkerPort 增加 `describe_capabilities() -> CapabilityManifest`，  
Scheduler 在 Run 开始时收集所有 Worker 的能力清单，存入 run metadata。  
好处：Inspector/巡检阶段可知道"某任务是由只读 Worker 完成的，结果可信度更高"。

---

#### C2 · 多会话 Fork（从历史边界派生）

**Harness 启示**：Fork 从确定的历史边界派生新 Session。  
**Leaf 方向**：`cco fork <run_id> --from-task <task_id>` 从某任务的完成点创建新 Run，复用已完成任务结果，重新执行后续任务（用于 A/B 对比不同 Worker）。  
依赖 A1（checkpoint）先落地。

---

#### C3 · 声明式 Worker 配置文件（cordis.yml 类比）

**Harness 启示**：cordis.yml 组装不同 Agent 形态。  
**Leaf 方向**：`~/.cco/profiles/` 目录存放 TOML 配置：
```toml
[profile.safe-pm]
workers = ["claude:inspect"]
capability_preset = "safe"
max_cost_usd = 0.5
```
`cco run --profile safe-pm` 覆盖默认配置。  
面向进阶用户/次受众（开发者/自动化）；主受众仍走桌面 GUI 默认。

---

## 四、UI 影响面（每个方向的桌面交互设计）

> 规则接口：`docs/product-mainpath-optimize-2026-07-20.md` 仍有效产品规则（主表面 ≤4 · 概念预算 · 壳层减法）；  
> 改动只能增量接入现有 split desk / run desk / result desk，**不**重开第三套 UX 计划。

---

### U-A1 · Run Resume — 断点恢复入口

**出现屏**：Run 台（`features/run` RunView）

**触发条件**：任务状态存在 `Failed` 且 events.jsonl 有 `checkpoint` 记录（说明有可恢复点）。

**设计方案**：
```
┌─ Run 台任务卡片（失败状态） ─────────────────────────┐
│  ✗ 任务 6/10 失败：无法访问目标文件                  │
│                                                        │
│  [重试这条]   [从这里继续]   [查看日志 ▾]             │
│       ↑            ↑                                    │
│  retry_task   resumeRun(from=checkpoint)               │
└────────────────────────────────────────────────────────┘
```

- "从这里继续"只在有 checkpoint 事件时显示（无 checkpoint = 无此按钮，只有"重新开始"）；
- 按钮层级：主 CTA = "从这里继续"（有 checkpoint 时）；次 = "重试这条"；danger = "重新开始"；
- **不**在顶部 banner 重复提示，保持壳层减法原则（规则 8）；
- 文案不出现 `run_id` / `checkpoint` 技术词（规则 7）。

---

### U-A2 · 能力预设 — 拆分台 Worker 角色标签

**出现屏**：拆分台（`features/split` SplitView）右侧步骤卡片 / 顶栏通道选区

**设计方案**：
```
┌─ 步骤卡片 ──────────────────────────────────┐
│  ● 搜集竞品资料                              │
│  🔍 只读模式  ·  claude:inspect              │
│     ↑                                        │
│  PermissionTier badge（人话，非技术枚举）    │
└──────────────────────────────────────────────┘

可选 chip 行：
  [可读写] [只读] [浏览器截图]
     ↓       ↓         ↓
  WorkspaceWrite  ReadOnly  BrowserCapture
```

- Badge 只用两种人话：**「可读写项目文件」**（WorkspaceWrite/Full）· **「只读·不改文件」**（ReadOnly）；
- 默认不展示（折叠）；仅当任务 preset ≠ 默认时显示 badge 提醒；
- 颜色语义：只读 = 蓝/中性；可读写 = 橙/注意；Full（当前默认）= 无 badge（不惊吓普通用户）；
- 悬停 tooltip 一句话说明："此步骤 Worker 将读取并修改项目文件"；
- 遵守"同一屏新概念 ≤3"（L1 规则 26）——Badge 是被动展示，不要求用户主动选择。

---

### U-A3 · 工具门控 — 审批弹层设计

**出现屏**：运行中（桌面弹层 / CLI stdin）

**触发条件**：`PolicyDecision::RequireApproval` 返回时

**桌面设计**：
```
┌─ 操作需要确认 ──────────────────────────────┐
│  Worker 请求：写入文件 config/secrets.toml   │
│  超出了当前任务的预期范围                    │
│                                              │
│     [允许一次]    [允许此任务]    [拒绝]     │
│          ↑             ↑           ↑         │
│  Allow(once)   Allow(task)     Deny          │
└──────────────────────────────────────────────┘
```

- 使用已有 `shared/confirmDialog.js`（非 window.confirm）；
- 文案：动作 + 受影响路径 + 一句话风险描述；**不**出现 `bypassPermissions` / `PolicyDecision` 技术词；
- danger 焦点在"拒绝"（与 confirmDialog 现有 danger 焦点规范一致）；
- CLI 路径：打印到 stderr，`[y/N]` 阻塞 stdin（非 TUI 路径）。

---

### U-A3bis · PermissionTier 可见性 — 安全信号设计

**出现屏**：设置页 → Worker 高级配置（默认折叠）

**设计方案**：
```
▶ 高级 · Worker 权限                      [折叠默认]
  ┌───────────────────────────────────────┐
  │  默认权限层级                          │
  │  ○ 只读（不写任何文件）                │
  │  ● 可读写项目文件  ← 推荐              │
  │  ○ 完全访问（绕过所有限制）            │
  └───────────────────────────────────────┘
  ⚠ 当前：完全访问（历史默认）
     点此了解差异 →
```

- 当前默认值（`bypassPermissions`）高亮警示，但不强迫用户修改；
- 主路径不展示此选项（规则 24 高级默认折叠）；
- 使用已有 `shared/selectUi.js` 单选增强控件；
- "点此了解差异"展开一段人话说明，不出现技术术语。

---

### U-B1 · 事件总线 — 实时进度视觉节奏

**出现屏**：Run 台任务卡片行（`features/run/logBoardCard`）

**现状**：2s setInterval 批量刷新 → 任务状态是"快照跳变"，无中间动画；

**B1 落地后 UI 变化**：
```
现在（2s 批量）：  ○ pending ... ○ pending ... ● running  [突然出现]
B1 之后（事件驱动）：○ pending → ◌ 连接中 → ● running  [流式过渡]
```

- 任务卡由批量重绘改为事件增量更新（`run_events` 订阅）；
- 状态过渡加 150ms fade-in（motion-light，符合 ui-delivery-recipes）；
- **不**加 spinner 旋转（轻量；避免注意力噪音）；
- 整体刷新率从 2s → 事件到达即更新，Run 结束感知从最长 2s 延迟降为毫秒级；
- 降级：事件通道断开时退回 polling（2s），UI 不感知、不报错。

---

### U-B2 · Headless — 开发者输出格式（DevX UX）

无桌面 UI，但 JSON schema 本身是用户体验：

```jsonc
// cco run --headless --output json
{
  "run_id": "abc123",
  "status": "completed",        // completed | failed | partial
  "summary": "完成 8/10 个任务", // 人话摘要（主受众可直接读）
  "tasks": [
    {
      "id": "t1",
      "title": "搜集竞品资料",   // 人话标题，非技术 id
      "status": "done",
      "duration_s": 42
    }
  ],
  "failed_tasks": [...],         // 空数组而非 null（方便 jq .failed_tasks[]）
  "cost_usd": 0.12,
  "exit_code": 0                 // 0=全完成；1=部分失败；2=运行时错误
}
```

- `summary` 字段人话，方便非开发用户在 CI 日志里直接看；
- `exit_code` 语义明确，CI 可直接 `if cco run --headless; then`；
- stderr 保留 `log_events` 流（可 `2>/dev/null` 丢弃）；
- 不输出 `VERDICT` / `run_id` 作为第一行（规则 23）。

---

### UI 设计约束汇总

| 规则来源 | 约束 |
|---------|------|
| 主表面 ≤4 | 所有新入口嵌入现有 split/run/result desk，不新增顶级页面 |
| 概念预算 | 主路径文案无技术词；badge/弹层一句话说清 |
| 壳层减法 | 新按钮优先次级/折叠；不在顶栏加全局状态条 |
| confirmDialog | 审批弹层复用 `shared/confirmDialog.js`；danger 焦点在拒绝 |
| selectUi | PermissionTier 选择复用 `shared/selectUi.js` |
| 主区 phase | author\|split\|run\|result 不新增 phase |
| 动效 | 事件驱动刷新加 150ms fade；无 spinner；符合 motion-light |

---

## 六、不做的方向（及理由）

| Harness 特性 | Leaf 为何不做 |
|-------------|--------------|
| Cordis 微内核/插件注册 | Leaf 用 Ports & Adapters 六边形已解决同等问题，过度工程 |
| ACP / JSON-RPC 服务端 | 超出本机轻量任务控制台定位；属于"不做多租户 SaaS" |
| 创造模式（Agent 改装自身） | 违反主受众原则；属次受众进阶，有需求再评估 |
| LSP 语义代码导航 | 不做 IDE；已有浏览器自动化覆盖可视化验收场景 |
| 遥测/指标上报 | 本机 OSS 定位；用户数据不出本机 |

---

## 五、落地优先建议

按受众价值 × 工程成本排序（基于代码实际状态修正）：

```
A1 (Run Resume 粒度)   ★★★★☆  cco resume 已有；补 checkpoint events + 桌面按钮·成本低·PM 痛点高
A2 (能力预设)          ★★★★☆  成本中·安全感知提升·配合浏览器自动化
A3 (工具门控层)        ★★★☆☆  成本中·基础设施·先做默认透传版
A3bis (PermissionTier) ★★★☆☆  现默认 bypassPermissions = Harness 最宽松·安全默认逆向·成本低
B1 (事件总线·架构A级) ★★★★☆  UI poll 2s+文件读 vs event-driven·最深架构债·成本中高
B2 (Headless 模式)     ★★★☆☆  成本低·开发者/自动化受众
B3 (压缩事件化)        ★★☆☆☆  成本低·Inspect 感知提升
C1-C3                  ★★☆☆☆  中长期·视 A/B 反馈再定
```

**建议下一轮优先 A1 + A3bis + B2**：A1 补 checkpoint events（Scheduler 小改），A3bis 补 PermissionTier 枚举（Domain 新类型，不改现有行为），B2 无头模式独立成本最低。B1 是最重的架构债，建议单独立项，不与 A 类混跑。

---

> [PROTOCOL]: 本文为方向参考，不新增实施阶段表；  
> 具体落地时在对应真源文档（`architecture-redesign-2026-07-20.md` / `cco-split-format-sqlite-2026-07-21.md` 等）更新勾选；  
> 改架构边界须先更新 L1/L2 再改代码（规则 4）。
