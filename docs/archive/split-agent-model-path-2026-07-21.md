# 专用拆分 Agent（走模型）· 任务参数 · 放哪

> 日期：2026-07-21  
> 产品：拆分 **继续走模型**（不是本地 heuristic 主路径）；用 **提示词** 对计划做顺序/并发/是否执行等；输出 **带固定参数的任务**；代码靠参数识别。  
> 配套：僵尸 planning 收尸见 `job.rs` `try_reap_zombie_planning`（本轮已落地）。  
> 存储终态：[`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)

---

## 1. 开源信息与可借鉴点（只借思路，不换栈）

| 来源 | 模式 | 对 cco 的借鉴 | 不借鉴 |
|------|------|---------------|--------|
| [OpenHands Planning Mode](https://www.openhands.dev/blog/openhands-product-update---march-2026) · [Plan Mode 文档](https://docs.openhands.dev/overview/plan-mode) | 专用 **Planning Agent** → 结构化 `PLAN.md` → 人确认 → Code Mode | **独立拆分 Agent**；plan 与 execute 分离 | 整站 agent IDE |
| [Planner subagent 结构计划](https://arxiv.org/html/2603.05344v1) | 探索 → 分析 → 固定章节 plan 文件 | Prompt 固定章节/字段；输出可验收 | 换运行时 |
| [LangGraph plan-and-execute](https://www.langchain.com/blog/planning-agents) | Planner 节点出多步计划 → Executor 执行 | Planner 与 Worker **分节点** | 引入 LangGraph |
| [Deep Agents](https://www.langchain.com/deep-agents) | 规划工具 + 子 agent 并行 | 拆分 agent 可 spawn 隔离、有进度 | 整包依赖 |
| [CrewAI hierarchical](https://docs.crewai.com/) | Manager 拆任务再委派 | 角色：拆分 vs 实现 | 多 agent 戏剧 UI |
| [AutoGen task decomposition](https://microsoft.github.io/autogen/) | Planner 拆 3–5+ 子任务 | 规模门禁；子任务可验收 | 群聊 |
| Structured outputs / tool_use（OpenAI 等） | JSON Schema 强制字段 | **强制 cco 任务参数 schema**，少废话 | 绑定单一云厂商 |
| agent-workflow / DAG 引擎 | LLM 出图 + 拓扑并行 | depends_on + can_parallel | 换 TS 运行时 |

**共同结论**：专业拆分 = **专用 Planner Agent + 结构化输出（schema）+ 人闸 + 执行层另算**；不是「让干活的 Claude 顺便拆一下」。

---

## 2. 推荐架构（在 cco 里怎么放）

```text
┌─────────────────────────────────────────────────────────┐
│ Presentation (web / CLI)                                │
│  「拆成步骤」→ app::split::start_job                      │
└───────────────────────┬─────────────────────────────────┘
                        ▼
┌─────────────────────────────────────────────────────────┐
│ Application · split                                     │
│  start_job / get_job / confirm（唯一开跑）                │
└───────────────────────┬─────────────────────────────────┘
                        ▼
┌─────────────────────────────────────────────────────────┐
│ Split Agent Port（新）                                    │
│  trait SplitAgentPort { split(plan_md) → CcoSplitDoc }  │
│  实现：                                                  │
│   · ModelSplitAgent  ← 主路径：提示词 + 结构化输出        │
│   · 可选 HeuristicSplitAgent 仅兜底/测试                  │
└───────────┬─────────────────────────────┬───────────────┘
            │ 模型调用                     │ 落库
            ▼                             ▼
┌──────────────────────┐    ┌─────────────────────────────┐
│ Provider（轻量优先）  │    │ state/sqlite · CcoSplit SoT  │
│ Messages/HTTP 或 CLI │    │ tasks 带固定参数              │
│ timeout / 心跳 / 收尸 │    └─────────────────────────────┘
└──────────────────────┘
            │ confirm
            ▼
┌──────────────────────┐
│ materialize → PlanIR │ → Scheduler → Worker（执行 AI）
└──────────────────────┘
```

### 目录落点（建议）

| 路径 | 职责 |
|------|------|
| `src/domain/split/` 或扩 `domain/plan/` | **CcoSplitTask 参数类型**（纯）：id/title/body/depends/optional/enabled/kind/done_when/plan_ref… |
| `src/ports/split_agent.rs` | `SplitAgentPort` trait |
| `src/plan/split_agent/` **或** `src/runtime/provider/split_agent/` | **ModelSplitAgent**：拼提示词、调模型、解析 schema、soften |
| `src/plan/planner/job.rs` | 编排：start → agent → 写 SQLite → planned；**收尸** |
| `src/app/split.rs` | 用例不变：start/confirm |
| `web/features/split` | 按参数渲染：顺序/波次/是否执行/说明 |

**不要**把拆分逻辑堆进 `WorkerPort` 执行 worker；拆分与执行是两种 agent。

---

## 3. 模型拆分：提示词 + 固定任务参数

### 3.1 输出 schema（代码识别靠这些字段）

```json
{
  "schema": "cco-split/v1",
  "title": "短名",
  "max_parallel": 2,
  "tasks": [
    {
      "id": "t1",
      "title": "中文短标题",
      "summary": "一句话",
      "body": "给执行 AI 的完整说明…",
      "depends_on": [],
      "optional": false,
      "enabled": true,
      "kind": "do",
      "done_when": "怎样算做完",
      "plan_ref": "§A1 / 文档锚点",
      "can_parallel": true
    }
  ]
}
```

| 参数 | 软件用途 |
|------|----------|
| `depends_on` | **顺序** / 波次拓扑 |
| `can_parallel` + `max_parallel` | **并发**展示与调度上限 |
| `optional` + `enabled` | **是否执行**（勾选） |
| `kind` | do / check / system → 徽章与路由 |
| `body` | 确认后塞进 worker prompt |
| `done_when` / `plan_ref` | 拆分台与巡检对照 |

**校验**：结构软（有任务、无环、id 合法）；**不要**用 scope 重叠整图否决（已 soften / 或拆分 schema 根本不强制 collab scope）。

### 3.2 提示词角色（专业拆分 Agent）

```text
你是 cco 的「计划拆分 Agent」。
输入：用户 Markdown 计划。
输出：仅一个 JSON，schema=cco-split/v1。
目标：把计划变成可并行执行的步骤清单，供人确认后由其他 AI worker 执行。

硬规则：
- 一步 = 一个可完成结果；title 像待办不是章节名
- depends_on 只表示真实先后；无依赖则 []，可并行
- max_parallel 是同时路数上限，不要为凑波次乱加边
- optional 步骤 enabled 默认 false
- 禁止把目录/修订历史/非目标拆成任务
- 不要写执行代码，只写步骤说明 body
```

实现时：system + 计划正文；**强制 JSON**（tool/schema 优先于「整包 Claude Code 当 planner」）。

### 3.3 调用形态（性能）

| 方式 | 建议 |
|------|------|
| **首选** | 轻量 Messages/HTTP + structured output（P2-7 SDK 方向） |
| 次选 | Claude CLI print，但 **专用短 prompt、低 max_turns、超时收尸** |
| 禁止当默认 | 用「干活用」满配 agent 会话拆计划（慢、难解析） |

---

## 4. 僵尸 planning（本轮代码）

**问题**：进程没了，`status` 仍是 `planning`，UI 一直转。

**已做**（`src/plan/planner/job.rs`）：

- `try_reap_zombie_planning`：  
  - meta.json **pid 已死** 且创建超过 45s → `plan_failed`  
  - 创建超过 **12 分钟**硬超时 → `plan_failed`  
  - 长时间无状态更新 / 日志过旧 → `plan_failed`  
- `get_plan_job` / `latest_plan_job_for_project` 时自动 reap  
- LLM 心跳每 ~4s **更新 `job.updated_at`**，活着的不误杀  

**未做（可后续）**：supersede 时 **kill** 旧 planner pid。

---

## 5. 实施勾选（模型拆分 Agent）

| ID | 任务 | 状态 |
|----|------|------|
| **Z1** | 僵尸 reap + 心跳 | ✅ 本轮 |
| **A1** | domain：`CcoSplit` 类型 + schema | ☐ |
| **A2** | `SplitAgentPort` + `ModelSplitAgent`（提示词 + 解析） | ☐ |
| **A3** | job 走 Agent → SQLite SoT | ☐ |
| **A4** | confirm materialize | ☐ |
| **A5** | 桌面按参数展示 | ☐ |

---

## 6. 修订

| 日期 | 说明 |
|------|------|
| 2026-07-21 | 开源对照 + 放哪；Z1 僵尸收尸；坚持模型拆分 + 固定任务参数 |
