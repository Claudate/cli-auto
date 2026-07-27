# session-digest/v1 契约

> 状态：**C0 冻结**（2026-07-27）  
> 落地勾选真源：[`../context-digest-compress-landing-2026-07-27.md`](../context-digest-compress-landing-2026-07-27.md) §5  
> 抽取提示（**不**注入 cco 聊天/拆分 LLM）：[`../runtime-prompts/session-digest-extract.md`](../runtime-prompts/session-digest-extract.md)  
> 合格示例：[`session-digest.example.yaml`](./session-digest.example.yaml)

[PROTOCOL]: 改字段须同步本文件 · 示例 · extract 提示 · 落地计划 §3 **同提交**。digest 是缓存，不是业务开跑入口。

---

## 0. 一句话

**长会话压成带稳定 ID 的 YAML 作战图；AI 续作先读 digest，硬约束按字段执行，细节按指针回原文。**

---

## 1. 边界（防串味）

| 对象 | 是什么 | 不是 session-digest |
|------|--------|---------------------|
| **session-digest** | 一次/多日 **人·Agent 协作状态** 缓存 | — |
| **plan digest** | `src/plan/planner/digest.rs`：plan.md → greenfield/regression… | **禁止**混名、禁止改其职责 |
| **project pin/summary** | `project_pins` ≤3 · `project_last_summary` 一行 | 可 **消费** digest 摘要（C3 可选），≠ 本 schema 全量 |
| **MEMORY 原子条** | 跨会话铁律 + `MEMORY.md` 索引 | 稳定 dont/constraint **显式晋升**才写入；禁止整份 digest 糊进 MEMORY |
| **gzip / zstd / 文言** | 存档或修辞 | **禁止**当模型上下文压缩手段 |

---

## 2. 落点约定

| 用途 | 路径 |
|------|------|
| 本契约 | `docs/contracts/session-digest.md` |
| 合格实例（仓内跟踪） | `docs/contracts/session-digest.example.yaml` |
| 工作区实例（默认不提交） | `.cco-out/session-digest.yaml`（根 `.gitignore` 已忽略 `.cco-out/`） |
| 可选 lossy 时间线 | `.cco-out/arc.md`（**不得**单独充当硬约束） |

---

## 3. 字段表（v1）

```yaml
schema: session-digest/v1          # 必填 · 字面量
updated_at: <ISO-8601>             # 必填
session_ref: <string>              # 可选 · 会话/分支/计划名
goal: <string>                     # 必填 · 当前可执行目标 · 建议 ≤200 字

constraints:                       # 建议至少 1 条；可空数组但续作前应补
  - id: C1                         # 必填 · 稳定 ID（同会话不复用改义）
    text: <string>                 # 必填 · 可执行、可证伪短句 · 建议 ≤240 字
    source: <string>               # 必填 · 路径#锚 或 memory 名或「user:…」

decisions:
  - id: D1
    chose: <string>                # 必填
    rejected: <string>             # 必填 · 缺则整份不合格
    why: <string>                  # 必填
    source: <string>               # 可选

open:
  - id: O1
    q: <string>                    # 必填
    status: pending | deferred | blocked | decided
    note: <string>                 # 可选

artifacts:
  - path: <string>                 # 必填 · 仓库相对或约定绝对
    role: sot | draft | evidence | pointer

dont:
  - id: X1
    text: <string>                 # 必填
    source: <string>               # 可选
    superseded_by: <string>        # 可选 · 仅显式废止时填另一 dont/decision id

arc_one_liner: <string>            # 可选 · lossy · 不得替代上列硬字段
```

### 3.1 建议软上限（防膨胀）

| 字段 | 软上限 |
|------|--------|
| `goal` | 200 字 |
| 单条 `text` / `chose` / `rejected` / `why` | 240 字 |
| `constraints` + `dont` 合计 | 40 条 |
| `decisions` | 30 条 |
| `open` | 20 条 |
| `artifacts` | 30 条 |

超软上限：合并同义 ID 或升格 MEMORY 后从 digest 改指针，**禁止**默默删 `dont`。

---

## 4. 合格判定

| # | 检查 | 结果 |
|---|------|------|
| Q1 | 缺 `schema: session-digest/v1` 或 `updated_at` 或空 `goal` | **拒收** |
| Q2 | `constraints` / `dont` 任一条缺 `id` 或 `text` | **拒收** |
| Q3 | `constraints` 任一条缺 `source` | **拒收** |
| Q4 | `decisions[]` 任一条缺 `chose` / `rejected` / `why` | **拒收** |
| Q5 | `open[].status` 非四枚举之一 | **拒收** |
| Q6 | `artifacts[].role` 非四枚举之一 | **拒收** |
| Q7 | 仅有 `arc_one_liner`（或散文）而无 constraints/dont/decisions 结构 | **拒收** |
| Q8 | `text` 含「大致 / 可能 / 按以前那样」且无具体对象、路径或闸门 | **警告**；抽取提示应避免输出 |
| Q9 | 路径、命令、数字被意译 | **警告**；应保持字面 |

实现方可先做人读 + Agent 自检；C1 skill 应用同一清单。

---

## 5. 合并与生命周期

1. **写**：波次结束 / 用户说「压缩上下文」→ 整份替换或按 ID 合并。  
2. **合并**：同 `id` 保留更严 `text`；`dont` **只追加**；废止必须 `superseded_by`。  
3. **读**：续作先读 digest → 核 `dont`/`constraints` → 按需 `Read` `source`/`artifacts`。  
4. **冲突**：原文 source 与 digest 不一致 → 取更严约束，并在 `open[]` 记冲突。  
5. **晋升（C2）**：仅当用户或协议显式要求，将稳定铁律写成 MEMORY 原子文件；**默认不自动写**。  
6. **开跑**：digest **永不**调用 `confirm` / spawn 业务 worker。

---

## 6. 不合格反例（节选）

```yaml
# BAD — 缺 rejected；散文充 goal；无 source
schema: session-digest/v1
updated_at: "2026-07-27T12:00:00Z"
goal: 大致按以前架构来，注意别乱开跑
decisions:
  - id: D1
    chose: 默认停拆分台
    why: 比较合理
dont:
  - id: X1
    text: 别出事
```

**为何拒收**：Q1 目标不可执行；Q3/Q4 缺 source 与 rejected；dont 不可证伪。

合格对照见 [`session-digest.example.yaml`](./session-digest.example.yaml)。

---

## 7. 与 A0 其它契约的关系

| 契约 | 关系 |
|------|------|
| `behavior-golden.md` | digest 可 **引用** confirm/optional 红线原文，不得改红线 |
| `run-dir.md` / `plan-job.md` | digest 可把 run/job 路径列入 `artifacts`；**不**改变目录布局 |

---

法则: 字段胜散文 · rejected 必填 · dont 只增 · 指针回真源 · 禁止旁路 confirm
