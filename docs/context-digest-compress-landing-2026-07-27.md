# 上下文语义压缩 · Session Digest · 落地计划

> 日期：2026-07-27  
> 角色：**本能力实施勾选真源**（语义保真压缩 · 非比特压缩）  
> 由来：对话采纳「固定 schema 抽取 + 指针延迟加载」；要高压缩比、AI 再读歧义低  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)（轻量 · 不堆第二套人格 OS）  
> 架构：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（confirm 唯一开跑 · MVVM · 体积）  
> 邻接（**不**吞并勾选）：  
> · 计划文档 digest（Mode B 模式分类）：`src/plan/planner/digest.rs` — **不同对象**  
> · 项目轻记忆 pin/summary：[`archive/pilotdeck-borrow-landing-2026-07-21.md`](./archive/pilotdeck-borrow-landing-2026-07-21.md) P2-2 ✅ · `src/state/project_memory.rs`  
> · 全量引导/记忆：[`guided-plan-memory-decision-2026-07-21.md`](./guided-plan-memory-decision-2026-07-21.md) G0–G4 ☐ — **本计划不重开**  
> · 协调序：[`next-landing-sequence-2026-07-27.md`](./next-landing-sequence-2026-07-27.md) — 本能力为 **旁轨**，不替代 W0–W4  
> 状态：**C0 ✅ · C1 ✅ · C2-1/C2-2 ✅（契约内）· C2-3 可选 · C3 可选 · C4 后置**

[PROTOCOL]: **勾选只认本文件 §5**。禁止平行第二套「记忆 OS」阶段表；禁止旁路 `confirm`；禁止把 free-form 摘要当硬约束真源；禁止用 gzip/zstd/文言充当模型上下文；禁止因本计划勾 ✅ guided G 波。落地后同步 `docs/CLAUDE.md` 活跃索引；改 schema 须同步 §3 与 runtime-prompt。

---

## 0. 一句话

**把长会话压成一份带 ID 的结构化 digest（goal / constraints / decisions+rejected / open / dont / artifacts），AI 再读只信字段与指针；原文仍是真源，digest 是可执行缓存。**

```text
长 transcript / 多轮 Agent
        │
        ▼  固定 schema 抽取（LLM 或人 · 缺字段 = 失败）
session-digest.yaml（+ 可选 arc 一行时间线）
        │
        ├─ 下一轮 system/前缀只挂 digest + 指针
        └─ 需要细节 → Read 原文路径（延迟加载）
```

---

## 1. 目标 / 非目标

### 1.1 目标

| # | 目标 | 可感成功 |
|---|------|----------|
| T1 | 有一份 **机器可校验** 的 digest schema | 缺 `dont`/`rejected` 能被检查器或提示词拒收 |
| T2 | Agent/人 **会话收束** 能产出 digest，不靠自由散文 | 同一会话二次进入，先读 digest 即可续作 |
| T3 | 压缩比高且 **硬约束不丢** | 关键 `dont`/开跑闸/路径字面量仍在 |
| T4 | 与现有记忆分层 **同构不打架** | Claude `MEMORY.md` 原子条 · cco pin/summary · 本 digest 职责清晰 |

### 1.2 非目标

| 不做 | 原因 |
|------|------|
| gzip / zstd 进模型上下文 | AI 不能直接读；不解压 token 不降 |
| 文言/极度缩写当默认压缩 | 字少歧义大，工程约束最易丢 |
| 向量库当唯一上下文 | 召回「像」的，不保证「必须遵守的」 |
| Dream / 跨项目人格 / 人生 Pack | PRODUCT + guided 非目标 |
| 重开 guided G0–G4 全量 | 听 guided 文；本计划只做 **会话 digest 切片** |
| 改 `plan/planner/digest.rs` 语义 | 那是 **计划文档** 模式分类，不是会话压缩 |
| 旁路 confirm / 用 digest 自动开跑 | L1 #10 |
| JS 内复制策略 | MVVM |
| 首版强制 cco 桌面新主屏 | 概念 ≤3；digest 默认文件/高级，不抢五步主路径 |

### 1.3 与既有三层的分工

| 层 | 对象 | 真源形态 | 本计划 |
|----|------|----------|--------|
| **A. 会话 digest** | 一次/多日 Agent·人协作状态 | `session-digest` schema | **主交付** |
| **B. 原子记忆** | 跨会话稳定偏好/铁律 | Claude `memory/*.md` + `MEMORY.md` 索引 | C2 纪律对齐，不替代 |
| **C. 项目轻记忆** | 同项目「上次一行 + ≤3 pin」 | `project_last_summary` / `project_pins` | C3 **可选** 消费 digest 摘要，不扩成白盒 OS |
| **D. 计划 digest** | plan.md → greenfield/regression… | `planner/digest.rs` | **只读不改职责** |

---

## 2. 硬契约

1. **唯一业务开跑**仍是 Split 确认；digest **不得**触发 `confirm` / spawn 业务 worker。  
2. digest 是 **缓存**；冲突时以更严约束 + 原文 source 为准，并写入 `open[]`。  
3. **硬约束用原文短句**，禁止意译路径、命令、数字、闸门句。  
4. 每个 `decisions[]` 必须含 **`chose` + `rejected` + `why`**（缺 rejected = 不合格）。  
5. **`dont[]` 只追加不静默删**（归档须显式 `superseded_by`）。  
6. Presentation → App → Domain（若 C3 进 cco）；Domain 不拼策略散文。  
7. 文件软 400 / 硬 600；厚文件只抽不堆。  
8. 主路径人话第一句无 `run_id` / 裸 `VERDICT` / 引擎调试 id（产品 UI 若展示 digest）。

---

## 3. Schema 契约（C0 冻结 · 真源）

### 3.1 规范文件名与落点

| 用途 | 路径（约定） |
|------|----------------|
| Schema 说明 + 示例 | `docs/contracts/session-digest.md`（C0 新建） |
| 抽取系统提示 | `docs/runtime-prompts/session-digest-extract.md`（C0 新建） |
| 工作区实例（gitignore 或 `.cco-out/`） | `.cco-out/session-digest.yaml` 或会话目录 `digest.yaml` |
| 可选 lossy 时间线 | 同目录 `arc.md`（**不**承载硬约束） |

> 仓库**不**强制提交每会话实例；**提交的是** schema、提示、技能与本计划。

### 3.2 字段表（v1）

```yaml
schema: session-digest/v1
updated_at: ISO-8601
session_ref: optional-string   # 会话 id / 分支 / 计划文件名
goal: string                   # 当前可执行目标 · 一句

constraints:
  - id: C1                     # 稳定 ID
    text: string               # 可执行、可证伪短句
    source: string             # 路径#锚 或 memory 名

decisions:
  - id: D1
    chose: string
    rejected: string           # 必填
    why: string
    source: optional-string

open:
  - id: O1
    q: string
    status: pending | deferred | blocked | decided
    note: optional-string

artifacts:
  - path: string
    role: sot | draft | evidence | pointer

dont:
  - id: X1
    text: string
    source: optional-string
    superseded_by: optional-string  # 仅显式废止时填

# 可选 · lossy · 不得替代上列
arc_one_liner: optional-string
```

### 3.3 合格判定（机器/提示共用）

| 检查 | 失败则 |
|------|--------|
| 缺 `goal` 或空 | 拒收 |
| `constraints` / `dont` 任一条无 `id`+`text` | 拒收 |
| `decisions[]` 任一条缺 `rejected` | 拒收 |
| `text` 含「大致/可能/按以前那样」无具体对象 | 警告；C1 起抽取提示禁止输出 |
| 仅有 `arc_one_liner` 无 constraints/dont | 拒收（散文不能单飞） |

### 3.4 压缩与歧义 SLO（验收用语）

| 指标 | 目标（经验，非 CI 硬数） |
|------|--------------------------|
| 相对原料 transcript 体积 | 常压到 **5–15%** 量级（字段计数，非 gzip） |
| 再读续作 | 不读全文即可回答：目标、禁止项、已否决方案、未决 |
| 歧义事故 | 硬闸（confirm/optional/route）**零**意译丢失 |

---

## 4. 波次总览

```text
C0  契约落地     schema 文 + 抽取 prompt + 合格示例     ✅
C1  Agent 工作流  skill/协议：收束必抽 · 续作先读      ✅
C2  记忆同构      与 MEMORY 原子条分工 + 写入纪律      ✅ 主路径（C2-3 可选）
C3  cco 薄消费    （可选）summary/pin 或 DTO 挂一行    ☐  有痛再做
C4  自动化钩子    （后置）会话结束 hook / 定时重抽      ☐  不排期除非痛
```

依赖：C0 → C1 → C2；C3/C4 不阻塞 C0–C2 关账。

---

## 5. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成 · — 不做/取消

### 波次 C0 — 契约与提示

| # | 任务 | 落点 | 完成定义 | 状态 |
|---|------|------|----------|------|
| C0-1 | 写 `session-digest/v1` 契约（字段表 + 合格判定 + 与 plan digest/pin 边界） | `docs/contracts/session-digest.md` | 人/Agent 只读此文能填合法 YAML；链回本计划 | ✅ |
| C0-2 | 抽取系统提示：只输出 YAML；强制 `rejected`/`dont` 追加；冲突写 `open` | `docs/runtime-prompts/session-digest-extract.md` | 提示含 §3.3 检查清单；覆盖序写入 `runtime-prompts/README` 一行 | ✅ |
| C0-3 | 合格示例 + 不合格反例各 1 | `docs/contracts/session-digest.example.yaml`（或契约文内 fenced） | 示例含 chose/rejected/dont/source；反例点名缺 rejected | ✅ |
| C0-4 | 本计划文首状态与 `docs/CLAUDE.md` 活跃索引 | 本文 · `docs/CLAUDE.md` | 活跃参考可点到本文；**不**写入架构 A 波 | ✅ |

**C0 不做**：改 Rust/JS 业务；改 `planner/digest.rs`；SQLite 新表。

### 波次 C1 — Agent / 会话工作流

| # | 任务 | 落点 | 完成定义 | 状态 |
|---|------|------|----------|------|
| C1-1 | 工作流说明：何时抽（波次结束 / 上下文将满 / 用户说「压缩上下文」）· 续作先读 digest 再 Read 指针 | `docs/context-digest-compress-landing-2026-07-27.md` §6 或短文 `docs/runtime-prompts/session-digest-workflow.md` | 步骤 ≤7 条；默认输出路径约定清楚 | ✅ |
| C1-2 | 可选：仓库 skill 薄封装（读契约 + 跑抽取提示 + 写 `.cco-out/session-digest.yaml`） | `.claude/skills/` 下新 skill **或** 扩既有 skill 一节 | `/…` 可触发；**不**调 confirm；无 GUI 强依赖 | ✅ |
| C1-3 | `.gitignore` 若需要忽略实例 digest | 根 `.gitignore` | 实例不误提交；契约/示例仍跟踪 | ✅（已有 `.cco-out/`） |

**C1 不做**：Always-on 后台自跑；桌面新主 phase。

### 波次 C2 — 与原子记忆同构

| # | 任务 | 落点 | 完成定义 | 状态 |
|---|------|------|----------|------|
| C2-1 | 分工表写入契约：digest=会话状态缓存；MEMORY 原子条=跨会话铁律；pin=项目 ≤3 提示 | `docs/contracts/session-digest.md` | 三行对照表；禁止把整份 digest 糊进 MEMORY.md 正文 | ✅ |
| C2-2 | 晋升规则：digest 中稳定 `dont`/`constraints` **显式** 才可升格为 memory 文件 | 同契约 §晋升 | 升格须带 name/description；默认不自动写 memory | ✅ |
| C2-3 | （可选）本对话结论落一条 project memory 指针 | 用户 Claude project `memory/` | 仅当用户要跨会话记住「用 schema 压上下文」；非代码仓必须 | ✅（已有 context-digest-compress.md） |

### 波次 C3 — cco 产品薄消费（可选 · 有痛再开）

| # | 任务 | 落点 | 完成定义 | 状态 |
|---|------|------|----------|------|
| C3-1 | 评估：`compose_last_summary` 是否吸收 digest.`goal`+首条 `dont`（仍规则模板，可无 LLM） | `src/state/project_memory.rs` · 评估段写回本文 | 书面结论：做 / 不做；做则任务拆 ≤2 个 PR 量 | ☐ |
| C3-2 | 若做：Author 空态仍 **一行**；pin 仍 ≤3；**不**新主屏 | web author 空态 · app DTO | 无第二套记忆 UI；不 disable 确认 | ☐ |
| C3-3 | 若做：契约测试或单测覆盖 format 注入 | tests / unit | pin/summary 注入 chat 仍「仅上下文」 | ☐ |

**C3 禁止**：`user_profile` 富表、Dream、Guide 状态机借壳登场。

### 波次 C4 — 自动化（后置 · 默认不排期）

| # | 任务 | 状态 |
|---|------|------|
| C4-1 | 会话结束 hook / cron 重抽 | — 除非 C1 后明确疼痛 |
| C4-2 | LLMLingua 类 prompt 压缩实验 | — 研究向；不进主路径 |

---

## 6. 工作流（C1 正文草稿 · 可原样迁出）

### 6.1 压缩（写 digest）

1. 收集原料：本会话目标、已做决策、用户否决、未决、产物路径。  
2. 用 `session-digest-extract` 提示 **只出 YAML**。  
3. 跑 §3.3 合格判定；失败则补抽，禁止手写散文充数。  
4. 写入约定路径（默认 `.cco-out/session-digest.yaml`）。  
5. 可选写 `arc.md` 三行时间线（lossy）。  
6. **不要**删 `dont[]`；废止用 `superseded_by`。

### 6.2 续作（读 digest）

1. 若存在 digest → **先读 digest**，再按需 `Read` `artifacts`/`source`。  
2. 行动前核 `dont` 与 `constraints`。  
3. 新决策追加 `decisions`（含 rejected）；新禁止追加 `dont`。  
4. 波次结束回到 6.1 覆写 `updated_at`（整文件替换或按 ID 合并，合并策略：ID 稳定，text 以更严为准）。

### 6.3 用户口令（建议）

| 用户说 | Agent 做 |
|--------|----------|
| 「压缩上下文」/「写 digest」 | 6.1 |
| 「按 digest 续」/新开会话同题 | 6.2 |
| 「升格为记忆」 | 仅 C2-2 显式晋升，不整文件塞 MEMORY |

---

## 7. 体感拐点

| # | 谁会说 | 波次 |
|---|--------|------|
| K1 | 「会话收束后有一份 YAML，不是又一坨总结散文」 | C0–C1 |
| K2 | 「重开能看见否决过什么，不会重复推已拒方案」 | C1 |
| K3 | 「MEMORY 仍是短铁律；digest 是当轮作战图」 | C2 |
| K4 | （可选）「回 cco 空态仍只有一行上次，没变记忆产品」 | C3 |

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 自由摘要回潮 | 合格判定拒收；提示禁止形容词总结 |
| 与 guided/渴望文档抢叙事 | 文首非目标 + next-landing 旁轨声明 |
| 与 plan digest 命名混淆 | 契约文必须写「session-digest vs plan digest」对照 |
| digest 当唯一真源 | PROTOCOL：冲突回 source；open 记冲突 |
| C3 滑向白盒 OS | pin≤3 · 一行空态 · 禁止新表除非评估通过 |
| 体积膨胀 | 实例 gitignore；字段软上限可在契约加（goal≤200 字等）C0 可写建议 cap |

---

## 9. 文档与索引义务

| 动作 | 何时 |
|------|------|
| 更新 `docs/CLAUDE.md` 活跃业务参考一行 | C0-4 |
| `runtime-prompts/README` 覆盖序 +1 | C0-2 |
| **不**改 architecture 勾选 | 全程 |
| **不**把 guided G 勾成 ✅ | 全程 |
| next-landing 仅可加「旁轨指针」一句 | 可选；勾选仍只认本文 |

---

## 10. 建议派工序（给人 / Agent）

```text
今日可做：C0-1 → C0-2 → C0-3 → C0-4（纯文档，可一次 PR）
紧接：    C1-1 → C1-2（skill）→ C1-3
同构：    C2-1 → C2-2（C2-3 仅用户要跨工具记忆时）
暂停线：  C3/C4 等真实疼痛（重复冷启动丢约束、或 pin 一行不够）
```

**本计划文件本身在 C0 前已存在**；C0-4 负责索引与文首状态从「定稿」改为「C0 ✅」等实勾。

---

## 11. 文首状态机（改状态只改这里 + 上表）

| 日期 | 状态 |
|------|------|
| 2026-07-27 | 定稿发布 · C0–C2 ☐ · C3 可选 · C4 后置 |
| 2026-07-27 | **C0–C2 主路径 ✅**（契约+示例+extract+workflow+skill+索引）；C3/C4 仍后置 |

---

法则: schema 胜散文 · rejected 必填 · dont 只增 · 指针回真源 · 旁路 confirm 永不许 · 不吞并 guided/plan-digest

[PROTOCOL]: 勾选变更只改 §5 与 §11；schema 变更改 §3 + contracts + extract 提示三处同提交。
