# cco 计划驱动执行闭环（拆分 · 落地 · 巡检 · 回补）

> **状态：已落地 · 阶段勾选勿继承**（P-loop / P2-11 主线已闭环；Ensure 关账见独立活跃文）  
> 阶段勾选史：**L0–L2 已落地**（2026-07-19；**P-loop / P2-11**；**不**回灌 D0–D4 / P-chat / multi-cli 已勾项）——**勿**当未做工作再开。  
> 日期：2026-07-19  
> 范围：Mode B 规划拆分质量 · 工作包完成定义 · **专门巡检**对照计划勾选 · 遗漏分级 · **回补波**强制闭环 · host 门禁与启发式模板  
> 角色：执行方法论与产品/host 真源——把「有计划 → 能拆清 → 做完可验 → 漏了必补」钉成可派工契约；**不**替代 Mode B `confirm_start`；**不**重做聊天 UX；**不**扩 C3 流式  
> **Q3 修订（2026-07-24 Ensure）**：inspect **仍**不改业务 L1/源码凑 PASS；台账/地图有界关账改由 Ensure E2（`role=closeout` / `sys-closeout`）承担——见 [`inspect-ensure-close-loop-2026-07-24.md`](./inspect-ensure-close-loop-2026-07-24.md)（**不**复活本文件阶段勾选）。  
> 关联真源：
> - Mode B → [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)（分配后 Planner → 确认 → 执行）
> - 多 CLI / 检验员 → [`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md)（role/scope/inspect VERDICT · handoff · REWORK_HOOK；本计划**补全「对照计划勾选 + 回补闭环」**，**不**另开第二套 Scheduler）
> - Ensure 关账闭环 → [`inspect-ensure-close-loop-2026-07-24.md`](./inspect-ensure-close-loop-2026-07-24.md)（**本问题活跃勾选**）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md)（本计划 → **P-loop / P2-11 已落地**；**勿**回灌 D0–D4）
> - 反例触发 → [`chat-utf8-fence-panic-2026-07-19.md`](./archive/chat-utf8-fence-panic-2026-07-19.md)（四波 PASS 仍有 I-1 文档滞后 / I-2 验收降级 / inspect 只开单不修）  
> GEB 入口：[`/CLAUDE.md`](../CLAUDE.md)（L1）· [`./CLAUDE.md`](./CLAUDE.md)（L2 docs）

> **定稿（t1）**：本前言 + §0–§11 冻结角色、问题、契约、阶段与非目标。  
> **落地（t2）**：§5 L0–L2 全 ✅；启发式 work-order + host ISSUES 分级门禁 + rework TaskIR + 桌面一键回补/接受残留。  
> 实施勾选史 = **§5**（L0–L2）；**禁止**第二份「执行闭环总览」；**禁止**把本计划写成 multi-cli 未完成或 P-chat 回灌；**阶段勾选勿继承**。  
> 与总账边界：本闭环 → **P-loop / P2-11**；与 multi-cli 残差**分列**（可引用其 inspect/handoff，不合并勾选）。

[PROTOCOL]: 变更时更新此头部；**阶段勾选勿继承**；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**先有可勾选的计划，再拆成可派工的工作包；落地必须对勾选负责；专门巡检对照计划查完成与遗漏；有阻塞遗漏就必须回补波，直到巡检 PASS 或显式豁免。**

```text
【计划】可验收勾选真源（§ / 阶段 / 成功标准）
    ↓
【拆分】工作包 = 动词 + 路径 + 不做 + 完成标志（对齐计划勾选 ID）
    ↓
【落地】按包实现；进度表回写「计划勾选 ID → 证据」
    ↓
【巡检】专用 inspect：对照计划清单逐项 PASS/FAIL/SKIP；写 VERDICT + ISSUES
    ↓
【回补】阻塞项 → rework 波（实现者）→ 再巡检；非阻塞须显式豁免或进进度表
```

---

## 1. 为什么现在不够（问题真源）

> **定稿（t1）**：2026-07-19 用户反馈 + P-chat-utf8 四波 run（scope / work-breakdown / progress / inspect）对照。

### 1.1 用户要的闭环

| 步 | 用户语言 | 必须成立 |
|----|----------|----------|
| 1 | 解决问题，给计划 | 有独立计划文档，勾选真源清晰 |
| 2 | 按计划拆分清晰 | 工作包可派工，不拆成目录名/非目标章节 |
| 3 | 专门巡检 | 独立角色/波次，**对照计划**查是否做完、是否遗漏 |
| 4 | 有遗漏要补充 | inspect 不是终点；阻塞遗漏 → 回补实现 → 再巡检 |

### 1.2 现状断点（P-chat-utf8 反例）

| # | 断点 | 证据 | 后果 |
|---|------|------|------|
| **B1** | 启发式四波固定模板 | `heuristic.rs` `work_order_template_from_spec`：读范围 → 拆包 → 落地 → 检验 | 拆分与**具体计划 §5 勾选**弱绑定 |
| **B2** | 落地波「遵守非目标」易缩验收 | progress 把 F1 改成 `f1_verify` 同入口，GUI 变 optional | 计划 S5/V4 被静默降级仍可勾 ✅ |
| **B3** | 巡检「默认不改业务 / 不改文档」 | inspect prompt + multi-cli 默认 inspect 只写 `.cco-out/inspect/**` | **I-1 类文档滞后**只开 ISSUES、永不回写 L1/L2 |
| **B4** | VERDICT=PASS + residual 并存 | `ISSUES.md` I-1/I-2/I-3 全 non-blocking | 用户感知「还有一堆问题」；run 已 Done |
| **B5** | REWORK_HOOK 不自动成波 | handoff 有 hook 文案；无「阻塞 ISSUES → 强制 rework 任务」 | 遗漏停留在报告里 |
| **B6** | 多文件状态不同步 | scope 仍 F1 ☐、计划 §4 F1 ✅、L1 仍「待验」 | 假残留 / 地图不同构 |

### 1.3 与已有能力边界

| 已有（勿当本计划从零发明） | 本计划补什么 |
|---------------------------|--------------|
| Mode B：计划 → Planner → confirm_start → Scheduler | **拆分与验收对照计划勾选**的方法论 + host/prompt 约束 |
| multi-cli：`role=inspect` · VERDICT/ISSUES · `enforce_inspect_verdict` · handoff | **计划清单巡检表**、**遗漏分级**、**回补波契约**、**地图类遗漏可写** |
| heuristic 规范体四波 | 升级为「对齐计划 § 勾选」的工作序，而非泛化四步 |
| P-chat-utf8 热修本身 | **反例**；本计划**不**重开 fence 修复 |

---

## 2. 产品目标与用户心智

### 2.1 三句话

1. **计划是唯一勾选真源**——拆分和巡检都对着它的阶段/成功标准，不另造第二清单。  
2. **拆分必须可派工**——每个包：改哪、不改哪、完成标志、对应计划哪几条。  
3. **巡检专门做，漏了就补**——PASS 表示阻塞项清零（或已豁免）；不是「实现者自称做完」。

### 2.2 修后主路径（执行闭环）

```text
① 选定/写好计划文档（勾选真源 = 计划 § 阶段 / 成功标准 / 验证清单）
② 分配计划 → Planner 产出任务图（或规范体 work-order）
③ 确认屏可见：波次 + 每任务绑定的「计划勾选 ID」
④ 实现波执行；progress 回写 勾选 ID → 证据路径/命令
⑤ 巡检波：逐项对照计划勾选 → VERDICT + ISSUES（分级）
⑥ 若有阻塞遗漏：自动或一键生成回补波 → 实现 → 再巡检
⑦ 仅当 巡检 PASS（阻塞=0）或用户显式「接受残留」→ run 可宣告闭环
```

### 2.3 成功时用户不应再感到

| 修前 | 修后 |
|------|------|
| 「四波全绿但 ISSUES 一堆」 | 阻塞项必须回补或明示豁免；PASS 含义单一 |
| 「复检只开单不修」 | 回补波是实现者任务；巡检可二次验 |
| 「F1 被改成假验」 | 降级须写进 ISSUES 且默认 **阻塞**（除非计划允许） |
| 「L1 还写待验、计划已勾完」 | 地图类遗漏有专门回补或巡检可写允许路径 |

---

## 3. 契约规格（冻结）

> **冻结（t1）**：下列为执行闭环 **唯一产品真源**；实现走 §5 L0–L2。  
> 与 multi-cli：复用 VERDICT/ISSUES 路径与 `enforce_inspect_verdict`；**扩展语义**见 §3.3–§3.5。

### 3.1 计划文档（输入契约）

可被本闭环消费的计划，**至少**具备：

| 要素 | 要求 |
|------|------|
| **勾选真源** | 明确「实施勾选 = §X」（如 §4/§5 阶段表） |
| **成功标准** | 可判定的 S* / V*（命令、产物路径、禁止项） |
| **边界** | 非目标 / 勿做表（巡检用其判越界与伪缺口） |
| **阶段 ID** | 稳定 ID（F0/F1、U0、C0…）便于工作包回溯 |

**禁止**：只有散文无勾选表还声称「已闭环」（热修类须先补勾选表，见 P-chat-utf8 形态）。

### 3.2 拆分（工作包契约）

每个工作包 **必须** 含：

```text
WP-id · 标题（动词开头）
· 对应计划勾选：§x 条目 / S* / V*（可多对一）
· 改哪些路径（或「只读验证 + 重编」）
· 不做哪些（引用计划非目标）
· 完成标志（可观察：测试命令、文件存在、勾选回写）
· 验收可否降级：默认否；若可，写清「等价条件」与「降级后是否阻塞巡检」
```

| 硬规则 | 说明 |
|--------|------|
| **R-split-1** | 禁止把「Board / 非目标 / 修订历史 / PROTOCOL」当工作包 |
| **R-split-2** | 每个必做计划勾选 ≥1 个 WP 覆盖；可选勾选可标 `optional` |
| **R-split-3** | 「验收/重编/GEB 指针」若计划要求，必须是独立 WP 或落地 WP 的完成标志，**禁止**静默省略 |
| **R-split-4** | 输出落盘：`.cco-out/work-breakdown/SUMMARY.md`（或 PlanIR 任务 prompt 内嵌同等字段） |

### 3.3 落地（实现波契约）

| 规则 | 冻结 |
|------|------|
| **R-impl-1** | 完成一项必须在 `.cco-out/progress/SUMMARY.md`（或 handoff fragment）写：`勾选 ID → 证据` |
| **R-impl-2** | **禁止**把计划成功标准改写成更弱定义而不记 ISSUES（例如 S5「桌面发中文」→ 仅 unit，须标 **降级** 且默认阻塞） |
| **R-impl-3** | 范围外需求不实现；写入 progress「拒做 + 归属计划/非目标」 |
| **R-impl-4** | 实现波 **可以** 改业务代码与计划允许的文档路径；以 WP 路径表为准 |

### 3.4 专门巡检（inspect 契约）

巡检是 **独立波/独立任务**（`role: inspect` 或等价），**不是**实现波顺手自测。

#### 3.4.1 必做对照表

巡检必须产出 **计划勾选对照表**（可嵌在 `VERDICT.md` 或 `CHECKLIST.md`）：

| 列 | 含义 |
|----|------|
| 计划勾选 ID | 如 F0、S1、V4 |
| 状态 | `PASS` \| `FAIL` \| `SKIP` \| `DEGRADED` |
| 证据 | 命令输出摘要 / 路径 / mtime |
| 备注 | 降级理由、豁免引用 |

#### 3.4.2 VERDICT 语义（收紧）

| VERDICT | 条件 |
|---------|------|
| **PASS** | 所有 **必做** 勾选为 PASS 或 **已登记豁免**；无未处理 **blocking** ISSUES |
| **FAIL** | 任一必做勾选 FAIL，或存在未处理 blocking ISSUE |
| **未知** | 无 VERDICT 文件 → host 按现网 `enforce_inspect_verdict` / Unknown 策略（建议 L1 起：Unknown 视同 FAIL 当 `require_inspect`） |

**禁止**：存在 blocking ISSUE 仍写 PASS。  
**允许**：non-blocking residual 在 PASS 下附录，但须列表且 **不得** 伪装成「没问题」。

#### 3.4.3 遗漏分级（ISSUES）

| 级别 | 定义 | 默认处置 |
|------|------|----------|
| **blocking** | 计划必做未完成；验收被静默降级；回归红；越界改了非目标 | **必须** 回补波或用户显式「接受残留」 |
| **map** | L1/L2/总账/计划状态指针与 § 勾选不同构 | **默认 blocking**（GEB 法则）；回补可只改文档 |
| **residual** | 可选 F2、不排期项、计划允许的后续 | 不阻塞 PASS；写入 ISSUES 附录 |
| **out-of-scope** | 用户抱怨但不在本计划 | 记 out-of-scope；**不**当本 run FAIL |

每条 ISSUE 稳定字段：

```text
- id: I-*
- severity: blocking | map | residual | out-of-scope
- plan_ref: § / S* / V*
- path: 文件或 n/a
- symptom: …
- fix_wp: 建议回补工作包一句话（供 rework 直接派工）
```

#### 3.4.4 巡检写权限（相对 multi-cli 的增量）

| 路径类 | 默认 |
|--------|------|
| 业务源码 | **只读**（与 multi-cli inspect 一致） |
| `.cco-out/inspect/**` | 读写 |
| **map 回补例外** | L1/L2 一行指针、总账 §2 状态一行、本计划 § 勾选——**仅当 ISSUE.severity=map 且 fix 仅文档** 时，允许 **回补波（实现者）** 修改；inspect 本波仍默认不改，避免「检验员兼施工」 |

> 决议默认：**inspect 只开单；回补波修**（含 map）。若单人 run 想合并，见 §8 Q3。

### 3.5 回补波（rework 契约）

```text
巡检 FAIL 或 PASS 但存在未豁免 blocking
  → host 或桌面提示「需回补」
  → 生成 rework 任务（可 1 包多 ISSUE 或 1 ISSUE 1 包）
  → 实现者按 fix_wp 修改
  → progress 追加
  → 再次巡检（可缩短为「只验 ISSUE 列表」）
  → 直到 PASS 或用户「接受残留」
```

| 规则 | 冻结 |
|------|------|
| **R-rework-1** | blocking 未清不得把 run 标为「计划闭环成功」 |
| **R-rework-2** | rework prompt **必须**粘贴 ISSUES 原文 + plan_ref，禁止空话「再检查一下」 |
| **R-rework-3** | 回补后 progress 必须勾掉对应 plan_ref |
| **R-rework-4** | 最大回补轮次：默认 **2**（可配）；超限 → pause + 人工 |

### 3.6 Host / Planner 落点（技术）

| 层 | 行为 |
|----|------|
| **heuristic** `work_order_template_from_spec` | 拆包 prompt 强制「对应计划勾选 ID」；巡检 prompt 强制「逐项勾选表 + 分级 ISSUES」；**去掉**「有 residual 也可 PASS」的歧义 |
| **llm Planner** | system 增加闭环五步与 R-split / R-inspect 摘要 |
| **PlanIR** | 规范体默认 `require_inspect: true`（L1）；validate 已有 inspect 终闸 |
| **Scheduler** | 维持 `enforce_inspect_verdict`；L1+：blocking 未清时 run 不得 `Succeeded`（或 Done+需回补标记） |
| **handoff** | ISSUES 行带 `severity=`；REWORK_HOOK 含可派工 fix_wp |
| **桌面** | 确认屏/完成屏展示「巡检 PASS/FAIL · 阻塞 N」；FAIL 提供「生成回补并再跑」 |

### 3.7 产物目录（约定）

```text
.cco-out/
  scope/SUMMARY.md           # 目标/范围/验收（对齐计划）
  work-breakdown/SUMMARY.md  # WP 表（含 plan_ref）
  progress/SUMMARY.md        # 勾选 ID → 证据
  inspect/
    VERDICT.md               # PASS|FAIL + 勾选对照表
    ISSUES.md                # 分级遗漏
    CHECKLIST.md             # 可选；可与 VERDICT 合并
  rework/                    # 可选；回补轮次记录
    ROUND-1.md
```

---

## 4. 与现网 multi-cli / Mode B 的关系

```text
【本计划】执行方法论 + 计划对照巡检 + 回补闭环
【multi-cli】多 provider · scope · inspect 工具默认 · VERDICT 门禁
【Mode B】谁触发规划/确认/开跑
```

- **不**新建第二 Scheduler。  
- **不**取消 multi-cli 的「inspect 默认不改业务」。  
- **扩展**：巡检清单 = 计划勾选；遗漏分级；回补波；map 类由 rework 写 GEB。  
- 启发式四波 **升级**为本闭环的默认「规范体」形状，而非无关四步作文。

---

## 5. 阶段切分与勾选

> **实施勾选真源 = 本 §**。总账 **P2-11 / P-loop** 已出池并落地（t2）。

### 5.0 总览

| 阶段 | 目标 | 状态 | 主要触点 |
|------|------|------|----------|
| **L0** | 契约文档 + 启发式/巡检 prompt 升级（无强制 host） | ✅ | `docs/*` · `heuristic.rs` work-order 文案 · 示例计划 |
| **L1** | host：require_inspect 默认 · VERDICT/ISSUES 分级消费 · 阻塞则非成功终态 · rework 任务生成 | ✅ | `plan/` · `scheduler` · `handoff` · 桌面文案 |
| **L2** | 桌面一键回补再跑 · 确认屏展示勾选覆盖 · 轮次上限 UX | ✅ | `web/` · `services` |

### L0 — 契约与模板 ✅

- [x] 本文件定稿（t1）+ L1/L2/总账指针  
- [x] `work_order_template_from_spec`：拆包强制 plan_ref；落地强制证据；巡检强制勾选表 + severity  
- [x] 巡检 prompt：**禁止**在存在 blocking 时写 PASS  
- [x] 示例：`examples/plans/plan-loop-inspect-rework.md`  
- [x] 对照 P-chat-utf8：I-1 map/blocking 语义写入 ISSUES 分级与单元测

### L1 — Host 门禁与回补 ✅

- [x] 规范体 work-order 尾波 `role=inspect` + `require_inspect=true`  
- [x] 解析 ISSUES severity；blocking/map>0 时 PASS 也 Failed（不得 Completed 闭环成功）  
- [x] REWORK_HOOK → `build_rework_plan` / `start_rework_from_run`  
- [x] map 类 rework 路径白名单（CLAUDE.md · docs/** · gap/本计划）  
- [x] `cargo test`：VERDICT FAIL / blocking ISSUES / residual PASS / rework 依赖图  
- [x] 与 multi-cli P2-3 **兼容**（`enforce_inspect_verdict` 扩展 Unknown/blocking）

### L2 — 桌面闭环 UX ✅

- [x] 完成 UI：`inspect_loop` 条显示 VERDICT / 阻塞数 / 回补轮次  
- [x] 「回补并再巡检」→ `start_rework_cmd`  
- [x] 「接受残留」→ `accept_residual_cmd` → handoff `ACCEPTED_RESIDUAL`  
- [x] host 轮次上限 `REWORK_MAX_ROUNDS=2`；超限拒绝并提示人工  

### 5.1 边界

| 勿做 | 归属 |
|------|------|
| 聊天 UX / fence panic 重开 | chat-ux-focus / P-chat-utf8 |
| 自动 merge/PR | multi-cli 非目标 |
| inspect 默认可大改业务 | multi-cli N6 |
| 全仓库任意 `&str` 审计 | 另项 |
| 回灌 D0–D4 / 把 P-chat 勾回 ☐ | **禁止** |

---

## 6. 非目标

| # | 非目标 | 说明 |
|---|--------|------|
| **N1** | 取代人写计划 | 仍要人/chat 产出计划文档 |
| **N2** | 无限自动重试 | 默认最多 2 轮回补 |
| **N3** | 通用 Agent 操作系统 | 只服务 cco Mode B 执行闭环 |
| **N4** | 取消 soft-fallback / fake | 与聊天策略无关 |
| **N5** | 检验员兼全部施工 | 默认 rework 分离 |
| **N6** | 一次解决所有产品 backlog | 只保证「按当前计划」闭环 |

---

## 7. 成功标准

| # | 指标 | 验收 |
|---|------|------|
| **S1** | 拆分可追溯 | 每个必做计划勾选能在 work-breakdown 找到 plan_ref |
| **S2** | 巡检对照计划 | VERDICT 含勾选对照表；非空泛「看起来完成了」 |
| **S3** | 阻塞必回补 | blocking ISSUE 存在时 run 不得宣称为计划闭环成功 |
| **S4** | 回补可再验 | rework 后二次 inspect 能清掉对应 ISSUE |
| **S5** | 地图同构 | map 类 ISSUE 清零或显式豁免；L1/L2 与计划 § 状态一致 |
| **S6** | 反例收敛 | 用 P-chat-utf8 类 run：I-1 不得以 residual 方式在 PASS 下静默 |

---

## 8. 默认决议

| Q | 问题 | 默认 |
|---|------|------|
| **Q1** | 验收降级（GUI→harness）默认级别？ | **blocking**，除非计划正文写明允许等价 |
| **Q2** | map（GEB 指针）默认级别？ | **blocking** |
| **Q3** | inspect 能否直接改 L1/L2？ | **否**；rework 波改 |
| **Q4** | 规范体是否默认 require_inspect？ | **L1 起是** |
| **Q5** | 回补最大轮次？ | **2** |
| **Q6** | 与 multi-cli 关系？ | **扩展**；勾选分列 P2-11，不把 multi-cli 勾回未完成 |
| **Q7** | 用户接受残留？ | 须显式动作；写入 open_risks |

---

## 9. 验证清单

| # | 步骤 | 期望 |
|---|------|------|
| V1 | 规范体计划 → 拆包 | work-breakdown 每 WP 有 plan_ref |
| V2 | 落地故意不做 map 指针 | 巡检 FAIL 或 blocking I-map |
| V3 | 回补只改 L1/L2 一行 | 二次巡检 PASS |
| V4 | 静默把 S5 改成 unit-only | 巡检标 DEGRADED/blocking |
| V5 | residual 可选 F2 未做 | 可 PASS 且 ISSUES 附录 residual |
| V6 | `cargo test` 相关门禁 | L1 后绿 |

---

## 10. 文档与 GEB

落地或定稿时同步：

| 层 | 动作 |
|----|------|
| 本文件 | 状态 / §5 勾选 / §11 修订 |
| [`docs/CLAUDE.md`](./CLAUDE.md) | 成员一行 |
| [`/CLAUDE.md`](../CLAUDE.md) | config 一行 |
| [`gap-and-landing-plan`](./gap-and-landing-plan-2026-07-18.md) | 关联真源 · §2 **P-loop** · D5 **P2-11** · §9 追加 |
| [`multi-cli-collaboration`](./multi-cli-collaboration-2026-07-18.md) | 关联本计划（扩展巡检/回补；不合并阶段勾选） |
| [`product-mode-b-ai-planner`](./product-mode-b-ai-planner.md) | 可选：主流程注「执行闭环见本计划」 |
| **禁止** | 回灌 D0–D4；把 multi-cli 已落地 P 勾回 ☐ |

---

## 11. 修订历史

| 时点 | 内容 |
|------|------|
| 2026-07-19 | **t1 定稿**：用户要求「计划→清晰拆分→专门巡检→遗漏回补」；冻结 §0–§10；反例 P-chat-utf8 B1–B6；阶段 L0–L2；总账 **P-loop / P2-11**；未实施 |
| 2026-07-19 | **t2 落地 L0–L2**：启发式 work-order plan_ref/severity；`parse_issues_text`+blocking 门禁；`build_rework_plan`/`start_rework_from_run`；桌面巡检条+回补/接受残留；示例 `examples/plans/plan-loop-inspect-rework.md`；GEB 同步 |

**规则**：既有行语义禁止改写；后续变更另起行追加。

---

## 附录 A · 巡检对照表示例

```markdown
# VERDICT

**Result: FAIL**

| plan_ref | status | evidence |
|----------|--------|----------|
| F0 | PASS | cargo test services::chat 15 passed |
| F1 | DEGRADED | f1_verify only; no GUI | 
| S5 | FAIL | plan requires desktop send |

## Blocking
- I-1 severity=map plan_ref=§8 GEB path=CLAUDE.md …
- I-2 severity=blocking plan_ref=S5/V4 …
```

---

## 附录 B · 工作包表示例（摘录）

```markdown
## WP4 · 验收桌面中文主路径
- plan_ref: F1, S5, V4, V5
- paths: scripts/package-app.sh · dist/（生成）· 可选 web 只读
- 不做: 改 fence 逻辑；UX 注意力
- 完成标志:
  - [ ] dist/CCO.app mtime > chat.rs
  - [ ] GUI 或计划允许的等价：…（若用 harness，须在 ISSUES 记 DEGRADED 且 plan 允许）
- 降级: 默认不允许
```

---

## 附录 C · 与 P-chat-utf8 四波对照（设计检验）

| 当时 | 本计划下应如何 |
|------|----------------|
| t4 PASS + I-1 residual | I-1 = **map/blocking** → 不得闭环成功 |
| F1 仅 f1_verify | **DEGRADED**；若计划未允许 → FAIL → rework 或改计划 |
| inspect 不改 L1 | 正确；应 **rework 波** 改指针 |
| 无回补波 | L1 host 生成 rework 或桌面一键 |
| 范围外 UX | out-of-scope，不进 blocking |
