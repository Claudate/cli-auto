# docs/
> L2 | 父级: /CLAUDE.md
> 角色: **规范根**（本仓库不用 `.md/`）——工程/落地计划与规格；**不是**产品方向

产品方向（非本目录）：[`../PRODUCT.md`](../PRODUCT.md) — 给谁用 · 轻量定位 · 五步主循环；**不是**落地计划

索引分三档（**2026-07-21 · 档 B**）：真源 · 业务规则参考 · **历史已迁 [`archive/`](./archive/)**。  
**禁止**把历史 ✅ 再写成缺口；**禁止**平行第二套阶段表。

---

## 真源（改边界 / 实施勾选只认这些）

architecture-redesign-2026-07-20.md: **系统架构大改 · A0–A5 ✅ 收口**（Ports&Adapters · App 用例 · Domain · 桌面 MVVM · Worker/Split/Run/Inspect · **P2-17 t58** · **A5-5 可选不做**；**本轮实施真源**）
a5-5-workspace-crates-eval-2026-07-21.md: **A5-5** workspace/`cco-domain`/`cco-app` 评估 — **本轮不做 / 不落 crate**；门槛=A5-2+A5-4+Store DI
contracts/: A0 契约冻结（behavior-golden · run-dir · plan-job · README）

---

## 业务规则参考（改行为时读 · **不**继承阶段勾选）

product-mode-b-ai-planner.md: Mode B · confirm 唯一开跑 · optional · replan 保人工（B0–B3 / P2-1/2 主线已闭环）
multi-cli-collaboration-2026-07-18.md: 多 CLI · provider/role/scope · handoff · tags 路由（P0–P2 已落地）
plan-execute-inspect-rework-2026-07-19.md: 拆分 plan_ref · 巡检对照勾选 · 回补波（**P-loop / P2-11** 已落地）
product-mainpath-optimize-2026-07-20.md: 拆分台三栏/结果台/模板 **交互意图**（波次 1–5 UI 已闭环；勾选听架构）
gap-and-landing-plan-2026-07-18.md: 历史总账 + D5 池导航（D0–D4 已闭环 · P2-17 收口 t58；**不**新开 D 阶段）
p2-7-sdk-provider-2026-07-21.md: **P2-7 单项** 非 CLI `sdk` WorkerPort（S0 inline ✅ · S1 Messages HTTP ✅ · S2 tool loop ✅ · 默认关 · 勿整包 M5）
（根目录，非本目录成员）[`../claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md): 编排器设计（M0–M4 已落地；M5 → D5）

---

## 历史归档（[`archive/`](./archive/) · **勿当缺口 · 勿继承勾选**）

> 主线已 ✅ 子计划已迁入 `docs/archive/`（2026-07-21 档 B）。日常实施与 Agent **默认不读**；查「当初为什么」再开。索引见 [`archive/README.md`](./archive/README.md)。

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
2. **本轮实施勾选真源** = [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md)（P2-17）；其他默认参考。  
3. **禁止**平行第二套阶段表 / 回灌 D0–D4 / 把历史 ✅ 再写成缺口。  
4. 工程硬规则全文在 [`../CLAUDE.md`](../CLAUDE.md)「工程硬规则」；改规则须 GEB 同步。  
5. **档 B**：历史子计划只住 `archive/`；新链接写 `docs/archive/…` 或 `./archive/…`；禁止把已归档再移回根当「未做」。

法则: 三档清晰·真源短·参考可查·历史进 archive；**产品方向不进本清单**

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
