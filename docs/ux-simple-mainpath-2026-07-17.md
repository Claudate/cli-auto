# cco 桌面主路径简化（易用性）

> 状态：**已落地**（合并选计划弹窗 · task-dash · CLI 再跑 · AI 事件过滤 · PATH 探测 · D1 开关）；**P2-16/S0 默认翻转**（2026-07-20：默认停拆分台）  
> 日期：2026-07-17（状态校正 2026-07-18；**D1 2026-07-18**；**S0/F0 2026-07-20**）  
> 范围：`web/` 计划区 / CLI 看板 / 环境提示；`src/runtime/provider` bin 解析；`src/doctor`  
> 关联：总账 → [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md) · Mode B 真源 → [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md) §4.1 · 主路径大改 → [`product-mainpath-optimize-2026-07-20.md`](./product-mainpath-optimize-2026-07-20.md)  
> **勿再当缺口**：下列主路径能力已闭环；残差见架构 P2-17 / 主路径 P2-16 任务表  
> **默认句（与 Mode B §4.1 同真）**：桌面默认 **停拆分台**；高级「拆分后自动开始」可选

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

## 一句话

用户主路径：**加项目 → 写/选计划 → 点「拆成步骤」→ 拆分台核对 →「确认并开始」**。AI 拆分默认**给人看**；高级可开「拆分后自动开始」。

## 主路径

```text
侧栏选项目
  → 顶部「当前计划」条（只显示选中的那一份）
  → 选择计划（弹层，不常驻列表）
     可选支路：顶栏「聊天」→ 共建计划 .md 落盘 →「拆成步骤」
  → 拆成步骤（AI 拆分 → 默认停拆分台）
  → 确认并开始 → 执行看板（可拖、可关、短日志）
```

> 聊天支路真源：[`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md)（C0–C2 已落地；不替代选文件）。  
> 聊天页体验修补（后台降噪 · fake 可信 · CTA）：[`chat-ux-focus-2026-07-19.md`](./chat-ux-focus-2026-07-19.md)（U0–U2 → 总账 P2-10；不排期则不碰）。

## 相对旧版的砍法

| 旧 | 新 |
|---|---|
| 常驻列出全部计划 + 空态文案叠在一起 | 默认只显示当前计划；列表进弹层 |
| 「分析并拆分」+ 确认页 + 开始运行 | 「拆成步骤」→ 拆分台 →「确认并开始」（高级可 auto-start） |
| 黄条永久 `claude bin not found` | 扫 `~/.local/bin` 等常见路径；可「忽略」 |
| 单列超长终端 | 分区短窗口，内部滚动，可拖可关 |

## 关键实现

- `web/index.html`：`plan-active-bar` / `plan-chooser` / `cli-board`
- `web/js/state.js`：`autoStartAfterPlan`（默认 **false**）、`#pp-auto-start`（高级拆分后自动开始）、`renderCliBoard`、`openPlanChooser`
- `web/app.css`：紧凑计划条 + 多窗口看板
- `src/runtime/provider/mod.rs`：`resolve_bin_on_disk`
- `src/doctor/mod.rs`：按默认 provider 判定整体失败

## 验证

- `node --check web/app.js`
- `cargo test --lib`（16 passed，含 D1 structured-adapter 路由测）
- 桌面需重新打包 `CCO.app` 后目视确认

## 未做 / 风险（≠ 主路径未完成）

> 主路径简化本身 **已完成**（总账 §1.3）。下列是独立 backlog / 验证项：

- 跨显示器系统级多窗口 → 总账 **P2-4 ✅ t39**（系统级「独立监视窗」可拖第二屏；非整应用多窗）
- ~~自动开跑 vs 强制确认~~ → **D1 已收口**；**S0 2026-07-20 默认翻转**：默认停拆分台；高级 `#pp-auto-start`；真源 Mode B §4.1
- ~~本机 `CCO.app` 重打包目视清单~~ → 总账 **P0-4 ✅ D3**（`scripts/package-app.sh` + 清单）

## 2026-07-18 D1 → 2026-07-20 S0 产品规则

决议（与 Mode B §4.1 同一）：

| 项 | 值 |
|----|-----|
| 桌面默认 | 拆分后 **停拆分台**（`autoStartAfterPlan: false`） |
| 高级开关 | 「拆分后自动开始」`#pp-auto-start` → 写 `PAUSE_CONFIRM_KEY=0` 后 UI 自动 `confirm_start` |
| 业务入口 | 仍只走 `confirm_start`（auto-start = UI 自动调用，非第二套 API） |
| 主 CTA | 「拆成步骤」→ 拆分台 →「确认并开始」 |

实现：`PAUSE_CONFIRM_KEY` localStorage；`=== "0"` 才 auto-start；缺省/「1」= 停台。

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

## 2026-07-18 视觉二次收敛

根因分析：
1. 作者 CSS 的 `display:flex` 覆盖 UA `[hidden]`，导致失败条/detail 幽灵面板常显
2. 完成态同时渲染 run-banner + 空失败条 + completion + CLI，信息重复
3. 历史成功仍刷环境黄条，制造「假故障」

改法：
- 全局 `[hidden]{display:none!important}`
- 合并为单一 `result-card`
- 移除 monitor 内可见 legacy detail-pane
- 完成态隐藏环境恐吓条；回填当前计划

## 2026-07-18 红框修订

1. 顶栏标题：只显示计划名，隐藏 page-sub 路径信息
2. 拆分任务区：`task-dash` KPI 卡片 + 正方形 `task-tile` 网格（数据看板风格）
3. CLI 窗口体：仅 message / tool_use / tool_result / result；丢弃 stderr/meta/raw 噪音；不回落整段 log_tail

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md


## 2026-07-18 红框动作区再收敛

确认项：Q2 合并弹窗 / Q3 折叠 task-dash / Q4 CLI 标题栏再跑 / Q5 黑区去 result 摘要

改法：
1. 「选择计划」「分配计划」合并为 `#plan-chooser` 弹窗：列表点选不关闭，底部 `btn-chooser-assign` 执行分配
2. 删除可见「换计划」；换计划能力并入弹窗重选
3. 「再跑一次」移到每个 CLI 窗口标题栏（`data-rerun`），看板右上不再放
4. 「收起运行」改为 task-dash 伸缩 icon（`btn-task-dash-toggle`），只折 KPI/任务块，不销毁 live
5. 黑区 `cli-window-body` 过滤 `kind=result`（success/$cost 由窗外徽章表达）

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

## 2026-07-18 动作区修复补丁

- 顶栏「分配计划」仅在运行中禁用（无计划也可开弹窗选计划）
- 计划列表去掉 inline onclick，只走全局委托
- 空计划列表仍刷新底部「分配计划」态
- 完成态黑区空输出不再显示误导性「暂无 AI 交互内容」
- 再跑一次前重置 closedPanels 与 task-dash 展开

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
