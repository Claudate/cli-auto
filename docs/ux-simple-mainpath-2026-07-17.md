# cco 桌面主路径简化（易用性）

> 状态：已落地前端 + PATH 探测  
> 日期：2026-07-17  
> 范围：`web/` 计划区 / CLI 看板 / 环境提示；`src/runtime/provider` bin 解析；`src/doctor`

[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

## 一句话

用户只做三步：**加项目 → 选一份计划 → 点「分配计划」**。其余（AI 拆分、开跑、监视）自动完成。

## 主路径

```text
侧栏选项目
  → 顶部「当前计划」条（只显示选中的那一份）
  → 选择计划（弹层，不常驻列表）
  → 分配计划（AI 拆分 → 自动开始）
  → CLI 窗口看板（可拖、可关、短日志）
```

## 相对旧版的砍法

| 旧 | 新 |
|---|---|
| 常驻列出全部计划 + 空态文案叠在一起 | 默认只显示当前计划；列表进弹层 |
| 「分析并拆分」+ 确认页 + 开始运行 | 「分配计划」一键：拆完自动开跑 |
| 黄条永久 `claude bin not found` | 扫 `~/.local/bin` 等常见路径；可「忽略」 |
| 单列超长终端 | 分区短窗口，内部滚动，可拖可关 |

## 关键实现

- `web/index.html`：`plan-active-bar` / `plan-chooser` / `cli-board`
- `web/app.js`：`autoStartAfterPlan`、`renderCliBoard`、`openPlanChooser`
- `web/app.css`：紧凑计划条 + 多窗口看板
- `src/runtime/provider/mod.rs`：`resolve_bin_on_disk`
- `src/doctor/mod.rs`：按默认 provider 判定整体失败

## 验证

- `node --check web/app.js`
- `cargo test --lib`（13 passed）
- 桌面需重新打包 `CCO.app` 后目视确认

## 未做 / 风险

- 未实现跨显示器的系统级多窗口（仍是应用内面板）
- 自动开跑跳过了波次人工确认；高级用户可后续加开关
- 未在本机完成 `CCO.app` 重打包验证（需 `cargo build -p cco-desktop --release` + package 脚本）


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
