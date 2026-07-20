# docs/
> L2 | 父级: /CLAUDE.md
> 角色: **规范根**（本仓库不用 `.md/`）

成员清单
gap-and-landing-plan-2026-07-18.md: 未完善唯一总账（§1.3/§2.1/§2.3/§3/§5 已冻结；**§6 成功标准 t18 全绿**；**§7 非目标 t19 已冻**；**§8 开放确认 t20 按默认已冻**；**§9 修订历史 t21 已闭环**；D0–D4 闭环；**D5 池 t15**；**§5 序 t16**；**§5.4 Agent 策略 t17** D0→D5 不排期则不碰）
desktop-ux-redesign-plan.md: 桌面壳 UX 阶段 0–4 已实施（勿再当缺口）
product-mode-b-ai-planner.md: 产品主路径 B；B0–B3 主线已闭环（D1/D3）；可选编辑 **P2-1/P2-2 已落地**（删任务/改依赖 · replan 保人工）
terminal-console-plan.md: 结构化日志控制台；A 路径 P0 + P1 已闭环（D2）；**P2 已闭环**（过滤/ANSI/导出 MD · 虚拟列表 t34）
ux-simple-mainpath-2026-07-17.md: 三步主路径已落地；跨屏多窗口 → D5/P2-4
chat-plan-builder-2026-07-18.md: 聊天共建计划 → 落盘 .md →「分配计划」进 Mode B（**已落地** C0–C2 ✅ · 五指标全绿 · **§9 验证清单 t11 七绿** · **§10 t12 文档/GEB 齐** · **§11 t13 闭环**；C3 t32–t34 全闭环 → 总账 **P2-9 ✅**）
chat-ux-focus-2026-07-19.md: 聊天页注意力收敛（后台态降噪 · fake/故障可信 · CTA 序 · 卡片抛光 · **U0–U2 已落地** → 总账 **D5/P2-10 · P-chat-ux ✅**；**不**回灌 P-chat C0–C2）
chat-utf8-fence-panic-2026-07-19.md: 聊天 plan fence UTF-8 panic 热修（`extract_plan_fence`/历史截断 CJK 安全 · **F0+F1 已闭环** 15 测绿 · 桌面重编+f1_verify · F2 可选不排期；**P-chat-utf8**；**不**并入 P2-10 / 不回灌 P-chat）
plan-execute-inspect-rework-2026-07-19.md: 计划驱动执行闭环（拆分 plan_ref · 专门巡检对照勾选 · 遗漏分级 · 回补波 · **L0–L2 已落地** · 总账 **P-loop / P2-11**；扩 multi-cli inspect，不另开 Scheduler）
multi-cli-collaboration-2026-07-18.md: 多 CLI 协作（Claude+Codex 并跑 · 声明/越界/检验员/handoff · **P0–P1 全绿 · P2-1/2/3/4/5/6 已落地 t33** tags 路由 + planner provider/role/scope）
chat-home-plan-cli-2026-07-19.md: 聊天主窗 · 未执行可改 · 已执行标识 · 入口按 run 路由 · stall 可见 · 重试尽换 CLI · **H0–H4 已落地** → 总账 **D5/P2-12 · P-chat-home ✅**
ux-plan-mgmt-attach-ttl-2026-07-19.md: 计划管理默认藏右栏 · 标题列表/单击双击 · CLI/本地规范化 · 附图 · 会话 2 天清理 · **主线已落地** G0–G6/G0b/G4 → 总账 **D5/P2-13 · P-plan-mgmt**
plan-mgmt-to-exec-flow-2026-07-19.md: 计划管理→执行任务操作流收敛（单列表/单 CTA/入口不越权/拆完回跳 · **E0–E4 已落地** → 总账 **D5/P2-14 · P-plan-exec-flow**；不回灌 P2-12/P2-13；桌面重打包目视）
system-post-tasks-2026-07-19.md: 系统收尾任务（巡检 · git push · 设置总开关默认关 · 拆分后可选默认勾选 · **已落地** → **D5/P2-15 · P-sys-post**）
（关联 skill，非 docs 成员）`../.claude/skills/cco-run/`：Claude Code `/cco-run` 薄封装 · 总账 **P2-6 ✅ t37**

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
