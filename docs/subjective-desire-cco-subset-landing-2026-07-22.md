# 主观渴望 · 对本仓有用子集 · 落地计划

> 日期：2026-07-22  
> 角色：**实施勾选真源**（从构思抽出、只服务 cco 任务主路径）  
> 构思真源：[`archive/subjective-desire-decision-concept.md`](./archive/subjective-desire-decision-concept.md)（**不排期** · 本文才是可拆可跑）  
> 工程邻文：[`guided-plan-memory-decision-2026-07-21.md`](./guided-plan-memory-decision-2026-07-21.md)（全量 Guide G0–G4 仍 ☐ · **不**在本计划重开）  
> 轻记忆地基：[`archive/pilotdeck-borrow-landing-2026-07-21.md`](./archive/pilotdeck-borrow-landing-2026-07-21.md) P2-2 pin/summary ✅（**已 archive** · 薄表可复用）  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)（cco = 任务控制台 · 主受众 PM/出海）  
> 架构：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（confirm 唯一开跑 · MVVM）  
> **澄清相子集指针**（vibe-check 轻量落地 · 非本文件串味）：[`clarify-phase-vibe-check-subset.md`](./clarify-phase-vibe-check-subset.md) · 实施 [`chat-20260725-0402.md`](./chat-20260725-0402.md)  
> 协调序：[`next-landing-sequence-2026-07-27.md`](./next-landing-sequence-2026-07-27.md) **W1**  
> 状态：**D0 ✅ · D1/D2 主路径由澄清相 t\* 吸收 ✅ · 残余仅 D1-2 空态 pin（可选）· 禁止新开 D3**

[PROTOCOL]: **勾选只认本文件 §3**。禁止平行第二套「人生 OS」阶段表；禁止旁路 `confirm`；禁止把 Brief 冻结写成业务开跑；禁止 Worker `role` 与「内心诉求角色」混词。全量渴望仪器 / 医学包 / 向量人格 = **永不在本计划**。

---

## 0. 从构思收成什么

构思文档写的是「本机镜子 · 接近主观渴望」。**cco 仓库只接它的近场子集**（构思 §5.7–5.8）：

```text
主叙事 A：cco 仍是任务控制台
只做：写计划前想清楚（目标/非目标/会失去什么/验收）
可选：Brief 中间页 → 用户认领后再落 plan.md
不做：人生 Pack · 内心多方剧场 · 主持人评分 · 改主叙事为渴望工具
```

### 0.1 一句话目标

让 PM/出海用户在 **① 生成计划** 时少写空心计划：聊天/落盘前能看见「目标 · 非目标 · 会失去什么 · 怎样算做完」，且 **Brief 认领 ≠ 确认开跑**。

### 0.2 非目标

| 不做 | 原因 |
|------|------|
| 人生渴望主叙事 / 双 App 人格 | 概念预算；PRODUCT 否决 D、慎 B |
| 内心多方 Worker 化 | 污染 role/混跑路由 |
| Brief 一键开跑 | 破坏 confirm 唯一闸 |
| 医疗/法律/投顾话术 | 构思硬边界 |
| 云端画像 | local-first |
| 全量 Guide G0–G4 | 听 guided 文档，本计划只做子集 |

### 0.3 硬契约

1. 唯一业务开跑：`split::confirm`。  
2. Brief「我认领」只写 plan / 草稿，**不**调 confirm。  
3. Presentation → App → Domain；JS 不写策略。  
4. 主路径第一句人话：无 run_id / VERDICT / 引擎名。  
5. 文件软 400 / 硬 600。

---

## 0.4 W1 对账表（2026-07-27 · 相对澄清相）

| 本计划项 | 状态 | 吸收 / 证据 |
|----------|------|-------------|
| **D0-1** 模板五节 | ✅ | `web/js/features/templates/catalog.js`：`req-outline` + 出海模板含 **目标 / 非目标 / 会失去什么 / 验收 / 风险**；无「渴望/判决」词 |
| **D0-2** 黄条不拦 | ✅ | 澄清相 `chatClarify.detectHollowGaps` + 拆分台 `splitFillMeta` acceptance 黄条；**不** disable 确认/认领；`claim-boundary-check` 断言 hollow 不闸 claim |
| **D0-3** 文档边界 | ✅ | 本文 + guided 互链 + `docs/CLAUDE.md` 索引；**无**双轨勾选 |
| **D1-1** 缺槽追问 | ✅ **由澄清 t1–t3 吸收** | 五槽 + ≤5 题 A/B/C +「你定/直接出计划」；`domain/chat/clarify.rs` · `chat-20260725-0402` |
| **D1-2** Author 空态 pin | ☐ **残余可选** | pilotdeck 表可复用；本序 W4 有痛再补；**不**阻塞主路径 |
| **D1-3** 文案护栏 | ✅ **由澄清吸收** | CTA「认领并写成计划」· 开跑仍「执行规划」· 无人生背书；边界文钉死 |
| **D2-1** Brief 结构 | ✅ **由澄清 t4 吸收** | Brief 分组：问题/给谁/做不做/得失/未决/验收/V1 · 证据轻标签 |
| **D2-2** 可选入口 | ✅ **由澄清三入口吸收** | 「想清楚再说 / 从想法到计划 / 已想清直接写」 |
| **D2-3** 认领后 summary | ☐ **残余可选** | best-effort；有痛再补；**禁止**当开跑 |

**禁止**：因本文旧 ☐ 重做澄清相；**禁止**新开 D3。

---

## 1. 体感拐点

| # | 用户会说 | 波次 | 2026-07-27 |
|---|----------|------|------------|
| **T1** | 「新计划模板里就有：非目标、会失去什么、验收」 | D0 | ✅ |
| **T2** | 「聊着聊着缺验收时，会先问我一句，不急着出整篇计划」 | D1 | ✅ 澄清 |
| **T3** | 「想看清取舍时，有一页得/失/未决；点认领才进计划，不会直接开跑」 | D2 | ✅ 澄清 |
| **T4** | 「再进项目，空态还能看见上次约束一行」 | D1-2 | ☐ 可选 |

---

## 2. 波次总览

```text
D0  计划章节契约（模板 + 结构提示）     ✅
D1  聊天缺槽追问 + 项目短记忆复用       ✅ 主路径（澄清）· D1-2 可选残余
D2  Brief 中间态（可选入口 · 认领→plan） ✅（澄清）· D2-3 可选残余
```

依赖：澄清相 t1–t5 已实现；本文件只对账，不平行第二实现。

---

## 3. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成 · **吸收** = 由澄清相落地，勿重做

### 波次 D0 — 计划章节契约

#### D0-1 · 计划模板补「非目标 / 会失去什么 / 验收」 ✅

| 项 | 内容 |
|----|------|
| **落点** | `web/js/features/templates/catalog.js` |
| **完成定义** | 新建「通用需求大纲」可见五节；出海模板对齐；无「渴望/判决」词 |
| **证据** | 2026-07-27 W1：五节写入 `req-outline` + `overseas-landing` |

#### D0-2 · 结构提示：缺验收/非目标时黄条（不拦） ✅

| 项 | 内容 |
|----|------|
| **落点** | `domain` acceptance_quality · `chatClarify` hollow · `splitFillMeta` |
| **完成定义** | 空心计划可见提醒；**不** disable 确认/认领 |
| **证据** | 澄清 t5 + claim-boundary-check；拆分台 P1-4 黄条 |

#### D0-3 · 文档：子集边界回写构思与 guided ✅

| 项 | 内容 |
|----|------|
| **落点** | 本文件 · guided · `docs/CLAUDE.md` |
| **完成定义** | 索引可点；无双轨勾选；头注链澄清相 |

---

### 波次 D1 — 聊天缺槽 + 短记忆

#### D1-1 · 聊天：信息不足时先追问再 fence ✅ **吸收 · 澄清 t1–t3**

见 [`chat-20260725-0402.md`](./chat-20260725-0402.md) · [`clarify-phase-vibe-check-subset.md`](./clarify-phase-vibe-check-subset.md)。

#### D1-2 · Author 空态消费 project_last_summary / pin ☐ **残余可选**

| 项 | 内容 |
|----|------|
| **落点** | Author 空态 · pilotdeck P2-2 表 |
| **完成定义** | 同项目二进可见上次一行（若已有写回） |
| **本序** | 无痛不做；W4 发现再立 |

#### D1-3 · 文案护栏：confirm / Brief 用词分离 ✅ **吸收 · 澄清**

认领 ≠ 开跑；主路径无人生背书。

---

### 波次 D2 — Brief 中间态

#### D2-1 · Brief 数据结构 ✅ **吸收 · 澄清 t4**

#### D2-2 · UI：可选「先理清再写计划」入口 ✅ **吸收 · 三入口**

#### D2-3 · 结果/记忆：认领后可选回写 last_summary ☐ **残余可选**

与 D1-2 同档；有痛再补。

---

## 4. 出门门禁

### D0

- [x] 新计划五节齐；黄条不拦 confirm  
- [x] 文档互链无双轨  

### D1

- [x] 模糊输入会追问或可跳过（澄清）  
- [ ] 空态记忆一行（D1-2 可选）  
- [x] 主路径无人生背书词  

### D2

- [x] Brief 认领 ≠ 开跑（claim-boundary + 边界文）  
- [x] 跳过 Brief / plan-only 路径存在（三入口）  

---

## 5. 明确永不做（本计划）

| 项 | 归类 |
|----|------|
| 改 cco 主叙事为渴望仪器 | 否决（构思 D） |
| 内心多方 / 人生 Pack 默认开 | 姊妹仓或远期；非本子集 |
| 主持人自动打分默认开 | 不做 |
| 医学/法律/投顾结论 | 永不 |
| Always-on 自跑 / 旁路 confirm | 永不 |
| 新开 D3 或平行「渴望全量」阶段表 | 永不 |

---

## 6. 启动指令

```text
真源：docs/subjective-desire-cco-subset-landing-2026-07-22.md
现状：D0–D2 主路径 ✅（模板 + 澄清吸收）；残余仅 D1-2 / D2-3 可选
禁止：旁路 confirm · 人生 Pack · 与 guided G 波次双轨勾选 · 重做澄清相
验收：§0.4 对账表；构思 §5.8 优先级 1–4 有着落，5–6 仍排除
```

---

## 7. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-07-22 | 初版：从 subjective-desire 构思 §5.8 抽出 D0–D2 可拆任务表 |
| 2026-07-22 | **docs-cleanup C3**：构思真源 / pilotdeck 链改 `archive/…`；D0–D2 ☐ 保留 |
| 2026-07-25 | 头注加澄清相子集指针 → `clarify-phase-vibe-check-subset.md`（不串味、不另起勾选） |
| 2026-07-27 | **W1 对账**：§0.4 表；D0 ✅ 模板五节+黄条；D1/D2 主路径标「澄清吸收」；残余仅 D1-2/D2-3 可选；**禁止 D3** |
