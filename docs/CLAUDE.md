# docs/
> L2 | 父级: /CLAUDE.md
> 角色: **规范根**（本仓库不用 `.md/`）——工程/落地计划与规格；**不是**产品方向

产品方向（非本目录）：[`../PRODUCT.md`](../PRODUCT.md) — 给谁用 · 轻量定位 · 五步主循环；**不是**落地计划

索引分三档（**2026-07-22 · 档 B 再归档**）：真源 · 业务规则参考 · **历史已迁 [`archive/`](./archive/)**。  
**禁止**把历史 ✅ 再写成缺口；**禁止**平行第二套阶段表。

---

## 真源（改边界 / 实施勾选只认这些）

architecture-redesign-2026-07-20.md: **系统架构大改 · A0–A5 ✅ 收口**（Ports&Adapters · App 用例 · Domain · 桌面 MVVM · Worker/Split/Run/Inspect · **P2-17 t58** · **A5-5 可选不做**；**本轮架构/实施真源**）
a5-5-workspace-crates-eval-2026-07-21.md: **A5-5** workspace/`cco-domain`/`cco-app` 评估 — **本轮不做 / 不落 crate**；门槛=A5-2+A5-4+Store DI
contracts/: A0 契约冻结（behavior-golden · run-dir · plan-job · README）
runtime-prompts/: **软件内底层提示真源**（Markdown 加载 · 聊天/拆分/规划器 · **ui-delivery-recipes** 效果配方 · layout/color/type/**copy**/motion · **backend-architecture** · **landing-gates**；覆盖序见目录 README）
split-product-rules.md: **拆分产品规则短真源**（改拆分/拆分台/confirm 行为；**无**平行阶段表；全文波次史见 archive）
cco-split-format-sqlite-2026-07-21.md: **cco 独立拆分格式 + SQLite SoT**（顺序/并发/是否执行 + 完整任务字段；**S3/S4 ✅ 核销** · S2/S5/S6 ☐ 可选/中长期 · 文末唯一勾选）
browser-automation-cco.md: **浏览器自动化契约**（网页验收/抓取回填/冒烟 · Kitewright 默认 MCP · tags `browser` · **W0/W1/W2 文档+risk ✅** · W3 结果台 ☐ · **本能力唯一勾选**）

---

## 业务规则参考（改行为时读 · **不**继承阶段勾选）

### 还在做（活跃落地 · 可有 ☐）

cost-aware-cli-router-2026-07-27.md: **费用感知 CLI 优选**（**P0/P1 ✅** · P2 预算降档/粘滞 ☐ · P3 intent ☐；role→tier→最便宜可用 · 失败升档 · 显式 route 不动）
next-landing-sequence-2026-07-27.md: **下一轮落地序（协调）**（**W0/W1/W3 ✅** · W2 自动化代理 ✅ / **真人 V1–V5 ☐** · W0 GUI 目视 residual · W4 无新痛；**不**替代各题勾选真源）
inspect-ensure-close-loop-2026-07-24.md: **巡检关账闭环 Ensure**（**E0–E6 ✅** · §6.1 自动化代理 ✅ · **wros 真人 V1–V5 ☐** · closeout · 有界关账 · 自动 rework · UI 反误导；**本问题唯一勾选落点**）
clarify-phase-vibe-check-subset.md: **澄清相 · 能力边界真源**（vibe-check 轻量子集 · 三入口/Brief 认领≠开跑 · 非 vibe-check/guided 全量 · ⊆ PRODUCT ①）
chat-20260725-0402.md: **澄清相实施计划**（t1–t6 ✅ · inspect PASS · W0 冒烟绿 · **桌面 GUI 30s 目视 residual** · 成功标准见文末；证据 CLOSEOUT · VERDICT · clarify/claim smoke）
subjective-desire-cco-subset-landing-2026-07-22.md: **主观渴望 · 对本仓有用子集落地**（**D0 ✅ · D1/D2 主路径由澄清吸收 ✅** · 残余 D1-2/D2-3 可选 · 无人生 Pack · **禁止 D3**；W1 对账 §0.4）
guided-plan-memory-decision-2026-07-21.md: **引导/记忆/对抗工程草稿**（G0–G4 ☐ 全量未 ship；§5.6.1 可复用 pilotdeck 薄表；文首后置；**不**开第二套波次）

### C4 + 活跃交互 / 总账

product-mode-b-ai-planner.md: Mode B · confirm 唯一开跑 · optional · replan 保人工（B0–B3 / P2-1/2 主线已闭环）
multi-cli-collaboration-2026-07-18.md: 多 CLI · provider/role/scope · handoff · tags 路由（P0–P2 已落地）
plan-execute-inspect-rework-2026-07-19.md: 拆分 plan_ref · 巡检对照勾选 · 回补波（**P-loop / P2-11** 已落地）
product-mainpath-optimize-2026-07-20.md: 拆分台三栏/结果台/模板 **交互意图**（波次 1–5 UI 已闭环；勾选听架构；体验全文已 archive）
gap-and-landing-plan-2026-07-18.md: 历史总账 + D5 池导航（D0–D4 已闭环 · P2-17 收口 t58；**不**新开 D 阶段）

---

## 历史归档（[`archive/`](./archive/) · **勿当缺口 · 勿继承勾选**）

> 主线已 ✅ 子计划已迁入 `docs/archive/`（2026-07-21 档 B + **2026-07-22 再归档** + **2026-07-24** human-status）。日常实施与 Agent **默认不读**；查「当初为什么」再开。索引见 [`archive/README.md`](./archive/README.md)。

### 2026-07-24 再归档

archive/human-status-verify-dual-landing-2026-07-24.md — 人话状态 + 可执行验收双层 H0–H3 ✅（done_when≠shell · StatusOneLiner · merge_check；**规则已进** split-product-rules / cco-split 字段表）

### 2026-07-22 再归档（B 表）

archive/claude-cli-orchestrator-plan.md — 编排器设计 M0–M4（根迁入；**工程设计现行真源 = architecture**）  
archive/ux-nondev-mainpath-2026-07-21.md — 非开发主路径诊断+原则  
archive/ux-nondev-landing-2026-07-21.md — 非开发体验落地 A–D ✅  
archive/shell-chrome-simplify-2026-07-22.md — 壳层减法 A–D ✅  
archive/split-desk-dual-audience-landing-2026-07-22.md — 拆分台双受众 S0–S3 ✅  
archive/multi-window-split-landing-2026-07-22.md — 多窗口可并发 W1–W4 ✅  
archive/split-quality-work-style-2026-07-22.md — 拆分质量/习惯 Q0–Q6 ✅  
archive/openhands-style-split-agent-landing-2026-07-21.md — OpenHands 拆分 Agent P0–P5 ✅  
archive/split-agent-model-path-2026-07-21.md — 专用拆分 Agent 路径（被 openhands 吸收）  
archive/pilotdeck-borrow-landing-2026-07-21.md — PilotDeck 借鉴 P0–P2 ✅  
archive/p2-7-sdk-provider-2026-07-21.md — P2-7 `sdk` WorkerPort S0–S2 ✅  
archive/split-soft-sqlite-2026-07-21.md — 软接受+SQLite **过渡**（**S2–S6 开项只认 cco-split 文末**）  
archive/subjective-desire-decision-concept.md — 主观渴望构思全文（无排期；子集落地留根）

### 2026-07-21 档 B（既有）

archive/desktop-ux-redesign-plan.md — 桌面壳 UX 0–4  
archive/ux-simple-mainpath-2026-07-17.md — 三步主路径；默认停拆分（S0）后经 P2-16  
archive/terminal-console-plan.md — 监视日志 A/P0–P2  
archive/chat-plan-builder-2026-07-18.md — 聊天共建 C0–C3 / P2-9  
archive/chat-ux-focus-2026-07-19.md — 注意力 U0–U2 / P2-10  
archive/chat-utf8-fence-panic-2026-07-19.md — fence UTF-8 热修 F0+F1  
archive/chat-home-plan-cli-2026-07-19.md — 聊天主窗 H0–H4 / P2-12  
archive/ux-plan-mgmt-attach-ttl-2026-07-19.md — 计划管理 G0–G6 / P2-13  
archive/plan-mgmt-to-exec-flow-2026-07-19.md — 计划→执行 E0–E4 / P2-14  
archive/system-post-tasks-2026-07-19.md — 系统收尾 / P2-15  
（关联 skill，非 docs 成员）`../.claude/skills/cco-run/`：`/cco-run` · P2-6 ✅

---

## 硬规则（继承 L1）

1. **产品方向**不进本清单正文（见 [`../PRODUCT.md`](../PRODUCT.md)）。  
2. **本轮架构/实施勾选真源** = [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（P2-17）；拆分行为短规则 = [`split-product-rules.md`](./split-product-rules.md)；存储/S2–S6 = [`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)。其他默认参考。  
3. **禁止**平行第二套阶段表 / 回灌 D0–D4 / 把历史 ✅ 再写成缺口。  
4. 工程硬规则全文在 [`../CLAUDE.md`](../CLAUDE.md)「工程硬规则」；改规则须 GEB 同步。  
5. **档 B**：历史子计划只住 `archive/`；新链接写 `docs/archive/…` 或 `./archive/…`；禁止把已归档再移回根当「未做」。

法则: 三档清晰·真源短·参考可查·历史进 archive；**产品方向不进本清单**

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
