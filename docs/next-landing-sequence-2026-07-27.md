# cco 下一轮落地序（缺口收口 · 2026-07-27）

> **角色：协调序 / 排期真源**——只定「先做什么、后做什么、不碰什么」。  
> **不是**第二套阶段勾选表。各问题的 ☐/✅ **仍只认**下列既有真源：  
>
> | 主题 | 勾选真源（唯一） |
> |------|------------------|
> | 澄清相 | [`chat-20260725-0402.md`](./chat-20260725-0402.md) · 边界 [`clarify-phase-vibe-check-subset.md`](./clarify-phase-vibe-check-subset.md) |
> | 巡检关账 Ensure | [`inspect-ensure-close-loop-2026-07-24.md`](./inspect-ensure-close-loop-2026-07-24.md) §5–§6 |
> | 拆分 SQLite 残余 | [`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md) §5 S2–S6 |
> | 主观渴望子集 | [`subjective-desire-cco-subset-landing-2026-07-22.md`](./subjective-desire-cco-subset-landing-2026-07-22.md) §3 |
> | 会话语义压缩（**旁轨**） | [`context-digest-compress-landing-2026-07-27.md`](./context-digest-compress-landing-2026-07-27.md) §5 · **不**插入 W0–W4 主序 |
> | 产品方向 | [`../PRODUCT.md`](../PRODUCT.md)（**不**当勾选） |
> | 架构 | [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（A0–A5 ✅ · **不**重开） |
>
> 历史总账 [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md) D5 池：**不排期则不碰**；本序出池项须写明对应池 ID 或显式用户疼痛。  
> 状态：**W0 ✅（commit+冒烟+打包嵌入+`clarify-split-visual-smoke`；手指 30s 可选体验）· W1 ✅ · W2 自动化代理 §6.1 ✅ / 真人 V1–V5 ☐ · W3 S3/S4 ✅ · S2/S5/S6 不做 · W4 无新痛** · **旁轨** session-digest **C0–C2 ✅**（C3/C4 后置）· 勾选在各真源；本文只改本状态行与 §9。

[PROTOCOL]: 禁止在本文复制各真源的任务级 ☐ 表；禁止平行「N0–N4 实现勾选」替代上表；禁止旁路 `confirm_start`；禁止把 Claude Code 源码仓能力搬进 cco；落地后同步 `docs/CLAUDE.md` 活跃索引。

---

## 0. 一句话

**先把已写好的澄清相交到用户手上并目视关账，再用人话项目验巡检关账闭环，再按疼痛点拆 SQLite 可选债；主观渴望 D0–D2 大半已被澄清相吸收，只做对账与模板补洞，不重开。**

```text
W0  澄清相出货（commit + GUI 目视）     ← 工作树已有产物 · 最高优先级
W1  渴望子集对账（D0 模板/黄条补洞）   ← 与澄清重叠 · 薄
W2  Ensure 人工 V1–V5（真实项目）      ← 代码已绿 · 缺现场铁律
W3  拆分 S3/S4 核销 + S2 按痛          ← 可选债 · 先审计再写
W4  非开发主路径打磨（仅疼痛）         ← 无新阶段表 · 随 W0–W2 发现补
```

---

## 1. 现状快照（2026-07-27 · 对照代码/文档）

| 缺口（口语） | 事实 | 还缺什么 |
|--------------|------|----------|
| **澄清相** | t1–t6 ✅ · 冒烟+claim 边界绿 · **打包嵌入+visual-smoke 关 residual** | 可选：打开 `dist/CCO.app` 亲手点三入口（体验，非开项） |
| **巡检关账** | E0–E6 ✅（金样 + package）；`auto_closeout`/`auto_rework` 已接线 | **wros 类真实计划 V1–V5 人工** |
| **拆分 SQLite** | C1–C7 ✅ SoT；S2–S6 文末 ☐ | **S4 可能已默认 false → 核销**；S3 与 C6 部分重叠 → 审计；S2 可选；S5/S6 中长期 |
| **主观渴望 D0–D2** | 文内仍 ☐；澄清相已覆盖：三入口、缺槽追问、Brief 认领≠开跑、黄条不拦 | **对账勾选 + 计划模板五节（D0-1）若仍缺则补**；D1/D2 主路径以澄清为准 |
| **非开发主路径** | UX 波次 / 拆分台双受众 / 壳层减法 **已 archive ✅** | **无独立大波**；只跟澄清 GUI、结果台人话、失败卡 CTA 一起打磨 |
| **Claude Code 源码仓** | 研究向反编译 | **本序明确不做** |

工作树已改未提交（澄清相关，不完全列表）：

- 新：`src/domain/chat/clarify.rs` · `web/js/features/chat/chatClarify.js` · `scripts/clarify-click-smoke.mjs`
- 改：`services/chat/*` · `web/js/features/chat/*` · `docs/chat-20260725-0402.md` 等

---

## 2. 硬边界（本序全程）

1. **唯一业务开跑**：Split 确认（`confirm_start` / `SplitUseCase.confirm`）。聊天 / Brief 认领 **禁止** spawn 业务 worker。  
2. **不**新建上帝 `*Manager`；策略在 domain；JS 只发意图 + 渲染 DTO。  
3. **不**继承 gap D0–D4 / 架构 A 波勾选；**不**把 archive ✅ 写成缺口。  
4. **不** vendor / 移植 Claude Code 反编译源码。  
5. **不**开 guided G0–G4 全量、人生 Pack、第二 Planner、A5-5 crate。  
6. 文件软 400 / 硬 600；厚文件只抽不堆。  
7. 主路径文案：无 `run_id` / `VERDICT` / 引擎名作第一句。

---

## 3. 波次（只定序 · 完成定义写在这里 · 勾选回真源）

### W0 · 澄清相出货（P0 · 预计 0.5–1 人日）

**用户疼痛**：功能在分支/工作树里，主路径用户摸不到；inspect 仍记 GUI residual。

| # | 动作 | 落点 | 完成定义 |
|---|------|------|----------|
| W0-1 | 自测绿：`cargo test` 相关 chat/clarify + 既有 lib 金样 | CI 本地 | clarify / plan_writing / chat 相关测绿 |
| W0-2 | 可选：跑 `scripts/clarify-click-smoke.mjs`（若依赖桌面则记环境） | scripts | 有通过证据或注明「需 GUI 环境跳过」 |
| W0-3 | **commit** 澄清相（含 docs closeout 指针）；**不**顺手塞无关 diff | git | 一次或两次清晰 commit；工作树澄清文件落地 |
| W0-4 | 桌面 **30 秒目视**（真源成功标准 #10 residual） | 打包 App 或 `package-app` 后 | 三入口各一条：选入口 → 答/跳 → Brief → **认领并写成计划** → 可见草稿；**确认无开跑** |
| W0-5 | 回写 residual：CLOSEOUT / 计划成功标准 #10 / 文首状态 | `chat-20260725-0402.md` · `.cco-out` 若再跑 | residual 关或明确「仅某平台未测」 |

**不做**：扩展 vibe-check 全量、改 Scheduler、第二套 Brief 文件格式大战。

**勾选回写**：只改 [`chat-20260725-0402.md`](./chat-20260725-0402.md) 成功标准与文首；边界文若状态句过时顺手一句。

---

### W1 · 主观渴望子集对账（P1 · 预计 0.5–1 人日）

**问题**：`subjective-desire-…` 仍标 D0–D2 ☐，与已落地澄清相**叙事重复**，Agent 会当缺口重做。

| # | 动作 | 落点 | 完成定义 |
|---|------|------|----------|
| W1-1 | **对账表**：D1 追问 / D2 Brief / 认领≠confirm → 标「由澄清相 t\* 吸收」+ 链到 chat 计划与边界文 | `subjective-desire-cco-subset-landing-2026-07-22.md` §3 | 不再把已实现能力标 ☐ |
| W1-2 | **D0-1 模板五节**（若 catalog 仍缺）：`目标 / 非目标 / 会失去什么 / 验收 / 风险` | `web/js/features/templates/catalog.js` · 可选 `examples/` | 新建「需求大纲」类可见五节；无「渴望/判决」词 |
| W1-3 | **D0-2 黄条**：确认澄清黄条与计划结构提示不双条抢戏；缺则 domain 纯函数 + 一处 UI | domain + 拆分/作者前 DTO | 空心计划一句提醒；不 disable 确认 |
| W1-4 | 文首状态：D0–D2 哪些 ✅ / 哪些吸收 / 残余仅列出 | 同 subjective 文 | 状态行可读；**禁止**新开 D3 |

**不做**：guided G 波、人生 Pack、Brief 一键开跑。

**勾选回写**：只认 [`subjective-desire-cco-subset-landing-2026-07-22.md`](./subjective-desire-cco-subset-landing-2026-07-22.md)。

---

### W2 · Ensure 人工铁律（P0 并列 · 预计 0.5–1.5 人日 · 可与 W0 后紧接）

**用户疼痛**：末尾 inspect 曾「功能绿、台账红、再跑考官空转」；自动化已绿，**现场未封口**。

| # | 动作 | 完成定义（= 真源 §6 V1–V5） |
|---|------|------------------------------|
| W2-1 | 选 **同一类** 计划（门禁 + 台账成功标准；可用历史 wros 类或精简夹具项目） | 有可复述的 plan.md |
| W2-2 | **V1** 无人值守：implement → closeout → inspect **PASS** | run Done；无红框空转 |
| W2-3 | **V2** 故意跳过 closeout 写入 | 自动 rework 一轮后 PASS，或 B 类已清且人话可懂 |
| W2-4 | **V3** 失败卡 UI | 主 CTA = 回补路径，不是「再跑一次考官」 |
| W2-5 | **V4** 故意留业务缺口 | `docs_only` 停人或 A 回补后有业务 diff（按当前 config 默认） |
| W2-6 | **V5** 无证据勾台账 | closeout 纪律禁止；inspect 仍可 FAIL |
| W2-7 | 证据：run_id · 截图或 `ISSUES`/`VERDICT` 路径 · 回写 §5 E6 / §6 | 真源文 V1–V5 勾上 |

**挂载提醒（已实现，实测时核对）**：

- `inject_closeout_task` · `app/run/ensure_loop` · `tests/ensure_close_loop.rs`
- config：`auto_closeout` / `auto_rework` / `auto_rework_docs_only`

**不做**：放宽 E2 写业务源码；重开 P-loop 阶段表；改 Mode B 入口。

**勾选回写**：只认 [`inspect-ensure-close-loop-2026-07-24.md`](./inspect-ensure-close-loop-2026-07-24.md)。

---

### W3 · 拆分 SQLite 残余（P2 · 按痛 · 预计 0.5–2 人日可拆）

**原则**：先 **审计核销**，再写代码。C1–C7 已是 SoT。

| ID | 建议 | 动作 |
|----|------|------|
| **S4** | **先核销** | 对照 `Config.planner_critic_enabled` 默认 `false` + fast/heuristic 永不跑 LLM critic；若产品语义已满足 → 在 cco-split §5 **勾 ✅ 并写证据一行**；未满足再改配置/设置暴露 |
| **S3** | **审计后补洞** | C6 已 kill planning pid / supersede；查是否「心跳 + status 写回 SQLite」仍缺。缺什么补什么；已满足则勾 ✅ |
| **S2** | **有痛再做** | 桌面/API 列表走 SQLite（失败回落 JSON）。立项门槛：拆分台列表明显慢或扫盘出错 |
| **S5** | **中长期 · 本序默认不做** | 规划两段式 / 轻量 API |
| **S6** | **中长期 · 本序默认不做** | runs/task_state 进同一 SQLite |

**不做**：PlanIR dual-write 回潮；在 archive soft 文再开第二份 S 表。

**勾选回写**：只认 [`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md) §5。

---

### W4 · 非开发主路径打磨（P3 · 仅疼痛驱动）

**不是**新 UX 大改计划（主路径大改波次 1–5 与 nondev landing **已 ✅ archive**）。

本序只允许三类补丁（发现于 W0/W2 再立）：

| 类 | 例 | 约束 |
|----|----|------|
| 文案 | 澄清/Brief/失败卡第一句仍像引擎 | 改 copy 与 DTO 字段，不堆设置 |
| 误导 CTA | 与 V3 重叠的结果台按钮 | 跟 Ensure 真源，不另开 UI 阶段 |
| 空态 | Author 空态一行上次约束（主观 D1-2 / pin） | 复用 pilotdeck 已有表；无则小补丁 |

**不做**：IDE 化、日志默认主焦点、概念 >3、第二套拆分台。

---

## 4. 推荐日历（单人或「实现 + 巡检」两人）

```text
Day 0–1   W0-1…W0-5   澄清测绿 → commit → 目视 → 关 residual
Day 1     W1-1…W1-4   对账文档 + 模板五节（若缺）
Day 1–2   W2-1…W2-7   真实/夹具项目跑 V1–V5（可与文档日交错）
Day 2–3   W3          S4/S3 核销；S2 仅当列表痛
（按需）  W4          只修 W0/W2 日志里的人话/CTA
```

并行建议：

- **实现 Agent**：W0 → W1 → W3  
- **验收人 / 第二会话**：W2（避免实现自己既当考官又改环）

---

## 5. 每波出门清单（共用）

1. 改代码前打开**该问题唯一勾选文**，确认未把 ✅ 当 ☐。  
2. 测：相关 `cargo test` + 触及契约金样；UI 波必目视一句。  
3. 文档：真源 ☐→✅；`docs/CLAUDE.md` 活跃句若状态变了改半行。  
4. commit：主题清晰；澄清 / Ensure / split **尽量分 commit**。  
5. **推送**仅在用户说「推送」时（远端 Claudate/cli-auto 记忆）。

---

## 6. 明确不做（防范围漂移）

| 不做 | 原因 |
|------|------|
| 复刻 Claude Code QueryEngine / 工具运行时 | 产品是编排器不是第二 agent IDE |
| guided G0–G4 全量 | 后置；与澄清子集划界 |
| A5-5 workspace crates | 评估结论本轮不做 |
| D5 池其它项（真 PTY、确认屏大编辑器…） | 不排期则不碰 |
| 巡检无界改业务凑 PASS | Ensure E2 白名单硬约束 |
| 聊天 confirm / 自动开跑 | L1 业务硬契约 |
| 平行「总阶段 N0–N4 实现表」 | 本文只协调序 |

---

## 7. 成功时用户会说

| # | 体感 | 对应 |
|---|------|------|
| 1 | 「模糊想法先问几句，认领后才出计划，而且不会自己开跑」 | W0 |
| 2 | 「新计划模板里就有非目标、会失去什么、验收」 | W1 |
| 3 | 「跑完最后一步会自己收台账；红了主按钮是回补，不是傻重考」 | W2 |
| 4 | 「拆分台不假转圈；列表/状态不靠扫盘碰运气」（若做了 S） | W3 |
| 5 | 「整页仍像任务控制台，不像 IDE」 | W4 + PRODUCT |

---

## 8. 与「Claude Code 源码仓」的边界（再钉一次）

- 允许：对照 **本机 CLI** 行为修 `stream-json` / `agent_id` 脆点（属 provider 热修，**不进本序主波**）。  
- 禁止：把反编译 `src/` 当依赖或设计蓝图扩 W0–W4。  
- 情报类（KAIROS 等）最多进 D5 池观察，**本序零实现**。

---

## 9. 修订史

| 日期 | 变更 |
|------|------|
| 2026-07-27 | 初版：W0 澄清出货 · W1 渴望对账 · W2 Ensure 人工 · W3 S 核销 · W4 疼痛打磨；勾选回既有真源 |
| 2026-07-27 | **执行**：W0 测绿+冒烟+关逻辑 residual（GUI 目视仍 open）· W1 对账+模板五节 · W3 S3/S4 核销 · W2 人工 V1–V5 仍 ☐ |
| 2026-07-27 | **W2 代理加厚**：`ensure-v3-cta-smoke` + inspect §6.1 证据表；再跑 ensure/closeout/classify 绿；**仍不关**真人 V1–V5 / 澄清 GUI 目视 |
| 2026-07-27 | **W0 residual 关**：`package-app` + 包内扫码 + `clarify-split-visual-smoke` 12/12；手指 30s 改可选体验。**W2**：ensure 金样修 Scheduler 字段后再绿；§6 真人 V1–V5 仍 ☐ |
