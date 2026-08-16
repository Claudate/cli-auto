# scripts/
> L2 | 父级: /CLAUDE.md

成员清单
package-app.sh: release 构建 cco + cco-desktop → dist/Leaf.app；**先 ESM dynamic import 扫 web/js**（防 main.js 导入期 SyntaxError 卡「就绪中…」）· `cd web && node build.mjs`（esbuild+terser → `dist/`，含 facade 守卫）；**防逆向双硬闸**——purge 后明文残留 `PURGE_FAIL`、产物 UI 标记 `SANITY_FAIL` 均 exit 1；X3 目视清单
smoke.sh: doctor + fake provider 最小冒烟（CCO_STATE_ROOT 隔离）
check-arch.sh: **架构硬规则门禁**（文件行数 · GIANTS 哨兵 · LEGACY_THICK state.js · **P4-1 components.css ≤200 原语守门** · gateway/invoke · domain 不依赖 tauri）；默认 warn；`STRICT=1` 可失败
check-landing-gates.sh: **落地页/营销站门禁**（example.com · 占位邮箱 · **G7 占位图** · **G8 电商/真实感商品图不得 SVG-only** · 页脚主 CTA · CTA 刷屏 · h1）；说明 `docs/runtime-prompts/landing-gates.md`；默认 FAIL 硬失败、WARN 不失败；`STRICT=1` WARN 也失败；`SKIP_G1=1`/`SKIP_G7=1`/`SKIP_G8=1` 仅显式演示
clarify-click-smoke.mjs: **澄清相静态冒烟**（opt-text 点击修复 · eventElement · 五槽 pick→brief_ready）；`node scripts/clarify-click-smoke.mjs`；不进默认 CI
chat-quiz-parse-smoke.mjs: **AI 编号题点选解析**（`**1. 题？**` 加粗标题 · hard-break · 可多选 D 项 · plain 回归）；`node scripts/chat-quiz-parse-smoke.mjs`；不进默认 CI
claim-boundary-check.mjs: **认领边界**（claim 只 draft/save_plan · 禁 confirm_start/start_run · 黄条不拦 claim · 与 assign 分轨）；W0 出货保留；不进默认 CI
ensure-v3-cta-smoke.mjs: **Ensure V3 代理冒烟**（失败卡主 CTA=回补并再巡检 · 再跑考官为 ghost）；`node scripts/ensure-v3-cta-smoke.mjs`；**不**替代 wros 人工 V1–V5  
clarify-split-visual-smoke.mjs: **澄清+拆分台静态目视契约**（三入口/认领文案 · revision_notes · risk chip · 外发提示 · 链 ensure-v3）；`node scripts/clarify-split-visual-smoke.mjs`；配合 `package-app` 扫包；不进默认 CI
provider-control-smoke.mjs: **通道下拉冒烟**（P2-17：确认台详头「默认通道」+ 每张任务卡胶囊下拉可开/可选/持久化 · 8 选项 · 步骤切换跟随 · 无页面错误）；`node scripts/provider-control-smoke.mjs`；内建 stub invoke（`__TAURI_INTERNALS__.invoke` + `window.invoke`），无需 Tauri 宿主；不进默认 CI
path-depth-wave-smoke.mjs: **path-depth 波次静态契约**（无三档英雄 · 场景芯片 · 当前理解 · 认领本波不旁路 · wave 分组/总览/串行 confirm · supersede per path）；`node scripts/path-depth-wave-smoke.mjs`；可代 W1-6 结构项；**不**替代真人桌面清单
p42-visual-smoke.mjs: **P4-2 两栏壳目视冒烟**（stub invoke 造项目 → 选项目断言 view-ring 出现/无页面错误 · view-ring 段高亮 · 搜索 1/3 · rail 折叠宽度<70px · 暗色非白 sidebar · 截图 light/dark/rail）；`node scripts/p42-visual-smoke.mjs`；需先 `cd web && node build.mjs`；不进 CI
p43-visual-smoke.mjs: **P4-3 拆分台视觉冒烟**（stub invoke 造拆分 → 断言任务卡 dsh 语言：StateDot · route pill 簇 · 默认通道 chip 跟随 provider · optional 徽标 · chevron · **卡片无 provider 下拉复活** · 底部确认 dock `执行规划` primary + hint 涂装 · live 注入状态点颜色 + runLocked 禁用 · 明暗 dock 非白 · 截图 light/dark）；`node scripts/p43-visual-smoke.mjs`；需先 `cd web && node build.mjs`；不进 CI
p44-visual-smoke.mjs: **P4-4 执行台视觉冒烟**（stub invoke 造 live DTO（t2 pending 入 wait 桶）→ 断言任务流程卡 dsh 语言：StateDot · `.is-running` 蓝追光 · 失败卡执行方式 · 自动提交状态 · 右次级列 Terminal/Diff/Read + wait/stall 琥珀条 + 日志折叠 · 详情列可折叠 · 停/续/重跑仍经 ccoRun → runApi → gateway 1:1 · 明暗截图）；`node scripts/p44-visual-smoke.mjs`；需先 `cd web && node build.mjs`；不进 CI；38/38 PASS
p45-visual-smoke.mjs: **P4-5 结果台视觉冒烟**（stub invoke 造 finished live DTO（含 inspect_loop / verification / browser_evidence）→ 断言完成/遗漏列表卡片化（StateDot · route_label 执行方式）· honest footer 巡检结论 · 验收面板 details 可折叠 + plan_items 勾选 ☑/☐ · 浏览器证据网格（截图卡 + 文本摘录）· rework 按钮文案轮次 · 明暗截图）；`node scripts/p45-visual-smoke.mjs`；需先 `cd web && node build.mjs`；不进 CI；32/32 PASS

## 硬规则

1. `check-arch.sh` 与根 `CLAUDE.md`「工程硬规则」同步；改阈值须改 L1 + 本脚本。  
2. **A5-4**：classic 业务 GIANTS 已空（S8 facade ≤200）；`state.js` 走 LEGACY_THICK 软提醒（**D9+ ~230** · shellUi/statusUi/markdown）；默认非 STRICT。  
3. `check-landing-gates.sh` 与 `docs/runtime-prompts/landing-gates.md` 同步；不扫 `docs/runtime-prompts` 自举举例。

法则: 成员完整·一行一文件·父级链接·技术词前置

[PROTOCOL]: 变更时更新此头部，然后检查 /CLAUDE.md
