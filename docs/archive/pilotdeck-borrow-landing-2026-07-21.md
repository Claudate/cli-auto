# PilotDeck 借鉴 · 落地实施计划

> 日期：2026-07-21  
> 角色：**横切体验/契约落地真源**（派工 / 勾选 / PR 边界）  
> 对照来源：对话分析 [OpenBMB/PilotDeck](https://github.com/OpenBMB/PilotDeck)（Agent OS：白盒记忆 · 智能路由 · Always-on）  
> 产品方向：[`../PRODUCT.md`](../PRODUCT.md)（轻量任务控制台 · 五步 · 非开发主受众）  
> 架构边界（**不重开 A0–A5**）：[`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)  
> 业务规则参考：[`plan-execute-inspect-rework-2026-07-19.md`](./plan-execute-inspect-rework-2026-07-19.md) · [`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md)  
> 记忆/引导设计（**④ 接轨 · 不整包实施**）：[`guided-plan-memory-decision-2026-07-21.md`](./guided-plan-memory-decision-2026-07-21.md) · [`subjective-desire-decision-concept.md`](./subjective-desire-decision-concept.md)  
> 契约：[`contracts/run-dir.md`](./contracts/run-dir.md) · [`contracts/plan-job.md`](./contracts/plan-job.md)  
> 范围：`src/report` · `src/state` · `src/domain` · `src/services/live` · `src/app` · `web/js/features/result|run` · 可选 SQLite；**不**内嵌 AgentLoop · **不** Always-on 自跑 · **不**旁路 `confirm`  
> 状态：**波次 P0–P2 实施勾选真源**（§3：P0–P2 任务表；**P2-3 ✅** 与 guided 互链 · 全量 Guide 不在本计划）

[PROTOCOL]: **勾选只认本文件 §3 任务表**。禁止平行第二套阶段表；禁止把 PilotDeck 功能清单整包对标；禁止 AGPL 代码搬运（只借形状）。落地后回写 `docs/CLAUDE.md` 索引与相关 L2。改 run.json / report 形状须同步 `contracts/run-dir.md`。

---

## 0. 目标 / 非目标

### 0.1 一句话目标

在**不改产品定位、不旁路 Mode B** 的前提下，把 PilotDeck 里与 cco 同向的四块能力收成可交付切片：

```text
① 结果台/report：人话用时+费用 + 对照计划验收
② 失败卡：执行方式来源（谁跑的 / 为何 / 下一步）
③ 计划验收节可感知 + 无巡检也有占位 report
④ 项目级轻记忆（Brief 摘要 / pin）— 薄版，不做 Dream
```

### 0.2 与 PilotDeck 的边界（只借形状）

| 借 | 不借 |
|----|------|
| Plan/Report 章节纪律 · Verification · fallback 报告 | AgentLoop / 自研 tool runtime |
| 任务级成本叙事 · 主强子弱**角色**（非自动 Judge） | TokenSaver 自动换模作默认 |
| `resolvedFrom` 式 route 可观测 | 静默路由盖显式 provider |
| 完成须可验证 / 工件落盘 | Always-on 无人确认自跑 |
| WorkSpace 级 pin/摘要 | 白盒记忆全链路 + Dream Mode + 一键回滚 UI |
| — | 飞书/企微一等公民 · MCP 作主叙事 · AGPL 源码 |

### 0.3 非目标

| 不做 | 原因 |
|------|------|
| 重开 A0–A5 / workspace crates | 架构已收口 |
| 新建业务开跑入口 / UI `start_run` | L1 硬规则 #10 |
| JS 内算 soft-fill / inspect 门禁 / 费用策略 | MVVM；策略在 Rust |
| Dream Mode / 跨项目人格向量 / 人生 OS | PRODUCT 轻量 + guided 非目标 |
| 默认硬挡「空验收」confirm | 少打断；默认结构强制、内容软提示 |
| 费用当结果台第一句 | 第一句 = 对照计划结论 |
| 主路径堆 provider/role/scope 三词 | 概念 ≤3；默认只「执行方式」一句 |
| 抄 PilotDeck TypeScript 实现 | AGPL-3.0 |

### 0.4 硬契约（落地时不可破）

1. **唯一业务开跑**：`split::confirm` / `confirm_start`。回补只经 `start_rework`。  
2. soft-fill **不得**静默覆盖任务已显式 route（Force 须显式语义）。  
3. Presentation → App → Domain；Domain 不拼路径 / 不 spawn provider。  
4. 结果台 / 失败卡 **只渲染 App DTO**；人话第一句无 `run_id` / 裸 `VERDICT` / 引擎调试 id。  
5. `report` = 终态快照；`handoff` = 事中账本；占位 report **不**伪造 PASS。  
6. 文件软 400 / 硬 600；禁止往 classic facade / `state.js` 堆业务。  
7. 改 `TaskState` / run-dir 布局 → 先改 [`contracts/run-dir.md`](./contracts/run-dir.md) + 兼容 default。

---

## 1. 现状盘点（地形 · 2026-07-21）

| 能力 | 已有 | 缺口 |
|------|------|------|
| 费用 | `TaskState.cost_usd` · `planner_cost.json` · live `planner_cost_usd`/`exec_cost_usd` · report Budget · Run 面板 $ | **结果台不消费**；无 cost 时常被当成「没有」而非「未汇总」 |
| 时长 | `started_at`/`finished_at` · Result 计划行已有 elapsed | 与费用未同屏人话账单 |
| 巡检人话 | `inspect_loop` DTO · `inspectCopy.js` · rework/accept CTA | **未回指**计划 `## 验收` 清单 |
| report | `write_reports` → report.md/json | 标题 `# cco report · {run_id}`；无「对照计划」节；缺 VERDICT 时无 fallback 模板 |
| 验收节 | `structure_plan_markdown` 补 `## 验收`；TaskIR `acceptance` / CcoSplit `done_when` | 常为 stub `- [ ] …`；确认前无黄条；`成功标准` 别名未统一 |
| route | soft/force `RouteFillReport`；`failover_used` | **不持久化** explicit/soft_fill/tag/failover 来源 |
| 记忆 | SQLite split/job；chat 文件会话 | 无 project pin / last_summary |

---

## 2. 波次总览

```text
波次 P0  结果台账单 + report 人话/fallback     ~1–2 人日   ← 最快可感知
波次 P1  route 来源 + 验收 stub 黄条           ~2–3 人日
波次 P2  验收清单对照 + 轻记忆 pin/summary     ~2–4 人日   （④ 可独立延期）
```

依赖：P0 无阻塞可先做；P1 动 run.json 字段须契约同步；P2-记忆可与 P2-对照并行（不同树），但均依赖 P0 结果台壳稳定。

### 2.1 体感拐点（做完用户会说）

| # | 用户会说的话 | 波次 |
|---|--------------|------|
| **T1** | 「做完一看就知道对照计划过没过、大概花了多久/多少钱」 | P0 |
| **T2** | 「没开巡检也有一页结果说明，不是空白」 | P0 |
| **T3** | 「这步挂了，知道是不是我选的执行方式」 | P1 |
| **T4** | 「验收还是空的，确认前有人提醒，但不拦我」 | P1 |
| **T5** | 「再进项目有一行上次卡在哪」 | P2 |

---

## 3. 任务表（实施勾选真源）

> 状态：☐ 待做 · ░ 进行中 · ✅ 完成  
> 每任务：落点 · 步骤 · 完成定义 · 自测 · 依赖

### 波次 P0 — 结果台账单 + report 人话/fallback

#### P0-1 · 结果台消费 live 费用与用时 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`web/js/features/result/ResultView.js`](../web/js/features/result/ResultView.js) · 可选抽 `resultSummary.js` 纯展示 helper；**不**改 classic facade 堆逻辑 |
| **步骤** | 1. 计划行 `planLine` 在已有步数/完成/用时后追加费用一句。 2. 口径：`exec`+`planner` 皆有 →「约 $x（规划 $a · 执行 $b）」；仅一侧 → 标明哪侧；皆无 →「费用未汇总」。 3. **禁止** `$0.00` 假装免费。 4. 第一句仍是完成比/对照，费用不作 heading。 |
| **完成定义** | 终态结果台可见人话费用或「未汇总」；无 cost 的 fake 跑不误导为 $0 |
| **自测** | 桌面：fake 一跑 → 见「费用未汇总」或仅规划费；有 cost 的录包/fixture 若有则见数字。`node` 单测 helper 可选 |
| **依赖** | 无（live 字段已有） |

#### P0-2 · report.md 标题人话 + 元数据下沉 ☐

| 项 | 内容 |
|----|------|
| **落点** | [`src/report/mod.rs`](../src/report/mod.rs) |
| **步骤** | 1. 标题改为 `# 本轮结果 · 《计划短名》`（从 `plan_path` 取文件名）。 2. `run_id` / adapter / 绝对路径改到文末 `## 备注` 或 metadata 列表，**不进 H1**。 3. Budget 保留；前加一行人话总述（status + 完成任务数，若可算）。 4. `report.json` 可增 `headline` 字段，旧字段兼容。 |
| **完成定义** | 打开 `report.md` 第一行无人话以外的 run_id；CLI `print_report` 同步 |
| **自测** | `cargo test` 若有 report 测则扩；否则手跑一局看 `runs/*/report.md` |
| **依赖** | 无 |

#### P0-3 · 无巡检 / 缺 VERDICT 的占位 report（fallback） ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/report/mod.rs`](../src/report/mod.rs)（可拆 `fallback.rs` 若超软上限）· 单测 |
| **步骤** | 1. `write_reports` **始终**写出完整 md 骨架：摘要 · **对照计划** · 步骤结果 · 花费与用时 · 后续 · 备注。 2. 无 handoff / 无 VERDICT / `require_inspect` 未满足 → `## 对照计划` 写人话占位（「本轮未产出巡检结论」/「未开启对照计划巡检」），**Notes 记 fallback 原因**；**绝不**写 PASS。 3. 有 inspect → 填入 verdict 人话 + issue 摘要（读 handoff 已有字段，report 适配器可调已有 inspect 视图，避免再 parse 原文若已有 DTO）。 4. 与 PilotDeck `buildFallbackReport` **同构思路、自写实现**。 |
| **完成定义** | 关巡检跑完仍有可读 `report.md`；开巡检且 FAIL 时对照节有遗漏信息 |
| **自测** | 单测：构造 RunState 无 handoff → md 含「对照计划」+ Notes；有 mock 巡检数据 → 非占位 |
| **依赖** | P0-2 标题可同 PR |

#### P0-4 · 结果台与 report 叙事对齐（薄） ✅

| 项 | 内容 |
|----|------|
| **落点** | ResultView · 可选 live 增 `result_cost_note` 若不想在 JS 拼格式（**优先 Rust 拼好人话**进 live 或专用 query） |
| **步骤** | 1. 若 P0-1 在 JS 拼费用变复杂 → 把 `cost_line` / `duration_line` 下沉 `services/live` 或 app 查询。 2. 诚实脚注 `honestInspectCopy` 保持；与 report「对照计划」用语一致（「巡检对照计划：…」）。 |
| **完成定义** | UI 与 report.md 同轮结论不矛盾 |
| **自测** | 目视一局 |
| **依赖** | P0-1 · P0-3 |
| **落地** | 费用句仍 `resultSummary`（未下沉 Rust）；`inspectCopy` 导出 `PLAN_COMPARE_COPY` / `planCompareKind`，headline 与 `src/report/fallback.rs` 同词（通过 / 有遗漏 / 未产出 / 未开启） |

**P0 出门门禁**

- [x] 非开发话术：结果台第一屏无 run_id 标题（`ResultView` →「本轮结果」）  
- [x] fake 跑：有结果页 + report 有对照占位（P0-1 费用 · P0-3 fallback 骨架 · 单测）  
- [x] 不碰 confirm / soft-fill / Scheduler 策略（P0 仅 report + result UI）

---

### 波次 P1 — route 来源 + 验收 stub 提示

#### P1-1 · 契约：`TaskState` route 溯源字段 ☐

| 项 | 内容 |
|----|------|
| **落点** | [`docs/contracts/run-dir.md`](./contracts/run-dir.md) · [`src/state/mod.rs`](../src/state/mod.rs) |
| **步骤** | 1. 文档先增可选字段：`route_source`（`explicit` \| `soft_fill` \| `tag_routing` \| `force` \| `failover`）· `route_previous`（failover 时）· 可选 `route_note`。 2. serde default 空 → **旧 run 可读**。 3. `TaskState::pending` 初始化不写死错误来源。 |
| **完成定义** | 契约与代码字段一致；旧 `run.json` load 不炸 |
| **自测** | 反序列化缺字段 fixture |
| **依赖** | 无（本波第一步） |

#### P1-2 · confirm / tag / failover 写入 provenance ☑

| 项 | 内容 |
|----|------|
| **落点** | [`src/domain/worker/route.rs`](../src/domain/worker/route.rs)（报告可扩）· [`src/app/split.rs`](../src/app/split.rs) confirm 路径 · tag routing 调用点 · scheduler failover 点 |
| **步骤** | 1. soft-fill 改写的 task → `soft_fill`；kept explicit → `explicit`。 2. force → `force`。 3. tag routing 改写 → `tag_routing`（若在 soft 之后，以后写为准并记录）。 4. failover 换 provider → `failover` + `route_previous`。 5. **禁止**在 domain 写路径；状态写入在 app/runtime 组装 RunState 时。 |
| **完成定义** | 新 run 的 `run.json` 任务可见 `route_source`；mixed 计划 kept 为 explicit |
| **自测** | domain/app 单测 soft/force 路径；可选集成 |
| **依赖** | P1-1 |

#### P1-3 · live DTO + 失败/未完成人话 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/services/live.rs`](../src/services/live.rs) `TaskLiveView` · [`web/js/features/result/ResultView.js`](../web/js/features/result/ResultView.js) · run 失败卡（`logBoardCard` / RunView miss） |
| **步骤** | 1. live 下发 `route_source` + **App 拼好的** `route_label`（如「Codex · 你在拆分台指定的」/「默认填充」/「故障切换前为 Claude」）。 2. 结果台 miss 行、执行失败卡展示一句；主路径不露 raw enum。 3. 旧 run 无字段 → 仅「执行方式：{产品标签}」。 |
| **完成定义** | 人为改 provider 的失败任务文案含「指定」语义；soft-fill 默认可见「默认」类语义 |
| **自测** | 目视 + 可选 DTO 快照测 |
| **依赖** | P1-2 |
| **落地** | `app::run::compose_route_label` 拼人话；`TaskLiveView.route_source`+`route_label`；ResultView miss / logBoardCard fail / RunView 失败条消费 `route_label` |

#### P1-4 · 计划验收 stub 检测 + 确认黄条 ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/domain/chat/normalize.rs`](../src/domain/chat/normalize.rs) 或 `domain/plan` 纯函数 · split 确认前 DTO · project/split UI 黄条 |
| **步骤** | 1. 纯函数：`acceptance_quality(md) -> filled \| stub \| missing`（stub = 仅有占位 `- [ ] …` / 「请补充」类）。 2. `## 成功标准` 视作验收节已存在（structure 别名）。 3. plan job / confirm 视图增 `acceptance_is_stub: bool` + 人话一句。 4. UI 黄条：**不** disable 确认；CTA 可变为「仍要开始（验收未写清）」类次强调。 |
| **完成定义** | 空壳验收计划在确认前可见提示；写满验收无提示 |
| **自测** | domain 单测 stub/filled；UI 目视 |
| **依赖** | 无（可与 P1-1 并行） |

**P1 出门门禁**

- [x] soft-fill 单测仍保证不盖 explicit（P1-2）  
- [x] 旧 run 兼容（route_* optional；无字段 → 仅产品标签）  
- [x] 主路径失败卡概念 ≤「步骤状态 + 执行方式 + 原因」（P1-3）  

---

### 波次 P2 — 对照清单 + 轻记忆

#### P2-1 · 计划验收清单 vs 巡检并排（Verification 完整） ✅

| 项 | 内容 |
|----|------|
| **落点** | domain 抽清单纯函数 · app/live `verification` DTO · ResultView 副栏或可折叠「原计划要验收」 · report `## 对照计划` 增强 |
| **步骤** | 1. 从 plan md parse checklist 行（只结构，不 LLM）。 2. 有 inspect → 巡检为准，清单作副栏。 3. 无 inspect → 显示「计划写了 N 条验收，本轮未自动对照」。 4. 任务级 `acceptance`/`done_when` 可选列入。 |
| **完成定义** | 结果台能看见「计划要什么」与「巡检说什么」的对照关系 |
| **自测** | parse 单测 + 目视 |
| **依赖** | P0-3 · P1-4 更佳 |

#### P2-2 · 项目轻记忆：last_summary + pin（薄） ✅

| 项 | 内容 |
|----|------|
| **落点** | [`src/state/sqlite.rs`](../src/state/sqlite.rs) 新表 · app 薄 CRUD · author 空态一行 · 设置/项目高级 pin 管理（≤3 pin） |
| **步骤** | 1. 表：`project_last_summary(project_id, text, updated_at)` · `project_pins(project_id, key, value, pinned_at)`。 2. 结果台「完成并回写」/ accept residual：**规则模板**写 summary（可先无 LLM）。 3. Author 空态：有则「上次：… · 沿用 / 忽略」。 4. pin 注入 chat/planner prompt **仅作上下文**，不改 route、不 auto-confirm。 5. **不做** Dream、回滚时间线、跨项目人格。 |
| **完成定义** | 同项目二进可见上次一行；pin ≤3 可增删 |
| **自测** | sqlite 读写测；目视空态 |
| **依赖** | 无强依赖 P0；建议 P0 后做以免结果台 CTA 不稳 |

#### P2-3 · guided / Brief 接轨（文档勾选 · 可选代码） ✅

| 项 | 内容 |
|----|------|
| **落点** | 回写 [`guided-plan-memory-decision-2026-07-21.md`](./guided-plan-memory-decision-2026-07-21.md) §3.1 / §5.6.1 / §6 / §8 与本文件互链；**全量 Guide 状态机不在本计划必做** |
| **步骤** | 1. 标明 P2-2 表（`project_last_summary` + `project_pins`）可被 Guide Brief / 预判条复用。 2. Guide 全量仍听 guided §8 **G0–G4** 排期，**不**在本文件开第二套波次。 |
| **完成定义** | 两文档互链无双轨勾选冲突 |
| **自测** | 文档审阅 |
| **依赖** | P2-2 表形状稳定 |

**交付摘要（2026-07-22）**

| 文档 | 互链点 | 勾选边界 |
|------|--------|----------|
| 本文件 §3 P2-2/P2-3 | ✅ 轻记忆薄切片 + 接轨 | P2 出门门禁见下 |
| guided §5.6.1 · §3.1 | P2-2 表 = Guide Brief 可复用地基 | **G0–G4 仍全部 ☐**（薄表 ≠ Guide 状态机 ship） |
| guided §8 序言 | 排期主权在 guided；禁止 pilotdeck 第二套 G 波次 | 无双轨 ✅ 冲突 |

**P2 出门门禁**

- [x] 记忆失败 best-effort，不挡结束本轮（`try_writeback_from_run` / accept residual；sqlite 失败仅 warn）  
- [x] pin 不出现在主 CTA 第一屏超过 1 行（主路径仅 Author 空态「上次：…」一行；pin CRUD 在设置页高级区）  
- [x] 无 Dream / 无自动开跑（pin/summary 仅上下文；不改 route、不 auto-confirm）  


---

## 4. 目标 DTO / 落盘形状（约定）

### 4.1 结果摘要（live 或 result query · 示意）

```text
result_summary:
  headline: string          # 巡检/完成人话，无 VERDICT 裸词
  duration_human: string
  cost:
    kind: known | partial | unknown
    total_usd?: number
    planner_usd?: number
    exec_usd?: number
    note: string            # 「费用未汇总」等
  verification:
    source: inspect | plan_only | none
    items?: [{ text, status: pass|fail|unknown|skipped }]
    blocking_count?: number
    residual_count?: number
  # run_id 仅 advanced / 备注
```

P0 可只实现 `cost` + 既有 `inspect_loop`，不一次上齐 `items`。

### 4.2 Task route（run.json）

```text
tasks.{id}.route_source?: "explicit"|"soft_fill"|"tag_routing"|"force"|"failover"
tasks.{id}.route_previous?: string
tasks.{id}.route_note?: string
```

### 4.3 report.md 骨架（终态）

```markdown
# 本轮结果 · 《计划短名》

## 摘要
…

## 对照计划
…  <!-- 巡检结论或 fallback 占位；禁止伪造 PASS -->

## 步骤结果
…

## 花费与用时
…

## 后续
…

## 备注
- run_id: …
- fallback: …  <!-- 若有 -->
```

### 4.4 轻记忆表（P2 · 已落地 · Guide 可复用）

```text
project_last_summary(project_id PK, text, updated_at)
project_pins(project_id, key, value, pinned_at)  -- 每项目 pin 数硬顶 3
```

> **Guide Brief 接轨**（P2-3）：上述两表即 guided 文档 §5.6.1 所指「可被预判条 / synthesize 写回复用」的地基。  
> 全量 `user_profile` / `guide_*` / 富 `project_memory` **不在本计划实现**；排期与勾选只认 [`guided-plan-memory-decision-2026-07-21.md`](./guided-plan-memory-decision-2026-07-21.md) §8 G0–G4。

---

## 5. PR 切片建议

| PR | 内容 | 风险 |
|----|------|------|
| **PR-A** | P0-2 + P0-3 report 人话 + fallback + 测 | 低；纯 adapter |
| **PR-B** | P0-1 (+ P0-4) 结果台费用/用时 | 低；UI |
| **PR-C** | P1-1 + P1-2 契约与写入 | 中；run.json |
| **PR-D** | P1-3 UI 失败卡 | 低 |
| **PR-E** | P1-4 验收 stub | 低 |
| **PR-F** | P2-1 对照清单 | 中 |
| **PR-G** | P2-2 记忆表 + 空态 | 中；sqlite |

禁止：单 PR 混 Always-on 想象 + 改 Scheduler；禁止 PR 内引入 AGPL 文件。

---

## 6. 测试与门禁

| 级别 | 要求 |
|------|------|
| 单测 | report fallback；acceptance_quality；route_source soft/force；sqlite memory CRUD（P2） |
| 契约 | 改 TaskState → 更新 `contracts/run-dir.md`；behavior-golden 若锁 report 标题则改金样 |
| 架构 | `scripts/check-arch.sh`；不增 domain→UI 依赖 |
| 目视（非开发脚本） | ① 结果台第一句 ② 无巡检有报告 ③ 失败见执行方式来源 ④ 空验收黄条 ⑤（P2）二进有上次 |
| 回归 | Mode B confirm · optional · rework · soft-fill 不盖 explicit |

---

## 7. 明确延后 / 永不做（本计划）

| 项 | 归类 |
|----|------|
| TokenSaver Judge 默认开 | 永不默认；若实验须高级关 + 显式 |
| Always-on 发现并自动执行 | 永不（与 confirm 冲突）；最多「建议草稿进 author」另项 |
| Dream Mode / 记忆回滚时间线 | 永不进默认；研究另文 |
| 全量 Guide 多角色对抗状态机 | 听 guided 文档，不在本勾选强绑 |
| MCP / IM 通道一等公民 | 非本计划 |
| 硬挡空验收 confirm | 默认不做 |

---

## 8. 回写清单（某波 ✅ 后）

- [x] 本文件 §3 勾选（P0–P2 含 P2-3）  
- [x] [`docs/CLAUDE.md`](./CLAUDE.md) 索引状态行  
- [x] [`contracts/run-dir.md`](./contracts/run-dir.md)（P1 route_* · 既有）  
- [x] [`src/report/CLAUDE.md`](../src/report/CLAUDE.md) / [`web/CLAUDE.md`](../web/CLAUDE.md) 成员一行（若增文件 · 既有波次回写）  
- [x] guided 文档互链（**P2-3 ✅** · 见 guided §5.6.1 / §8 序言）  
- [ ] 可选：根 [`CLAUDE.md`](../CLAUDE.md) 仅当硬规则变更（本计划预期不改硬规则）

---

## 9. 启动指令（给人 / Agent）

```text
真源：docs/pilotdeck-borrow-landing-2026-07-21.md
先做：波次 P0（PR-A report fallback → PR-B 结果台费用）
禁止：旁路 confirm · 抄 PilotDeck 源码 · Dream Mode · 重开 A0–A5
验收：§6 目视 + 单测；主路径第一句无人话以外的 run_id/VERDICT
```

---

## 10. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-07-21 | 初版：P0–P2 任务表 · 四块借鉴 · 非目标与硬契约 · PR 切片 |
| 2026-07-22 | **P2-3 ✅**：与 guided §3.1/§5.6.1/§6/§8 互链；标明 P2-2 表可被 Guide Brief 复用；Guide 全量仍听 guided G0–G4（无双轨）；P2 出门门禁勾完 |

[PROTOCOL]: 改波次/完成定义 → 只改本文件 §3 并更新 §10；实施代码 PR 描述链到本文件任务 ID（如 `P0-3`）。
