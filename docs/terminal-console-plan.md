# cco 监视终端 / 日志控制台改版计划

> 状态：A 路径 P0+观感修复（默认终端 transcript · stderr 折叠 · 完成条压缩）  
> 日期：2026-07-17  
> 范围：桌面监视日志（主）+ Tauri/服务层日志 API + Claude stream-json 呈现；不重写 scheduler  
> 关联：[`desktop-ux-redesign-plan.md`](./desktop-ux-redesign-plan.md)、[`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)、`web/`、`src/services.rs`、`src/runtime/provider/claude.rs`、`src/terminal/`

[PROTOCOL]: 变更时更新此头部与阶段勾选，然后检查相关 CLAUDE.md / 本目录索引。

---

## 0. 一句话

**不做假 xterm 糊原始 NDJSON。**  
默认做「结构化 Agent 日志控制台」；「原始流 / 外置终端」当二级能力。

---

## 1. 现象：为什么现在又丑又乱

| 根因 | 证据 | 用户感受 |
|------|------|----------|
| 桌面不是终端，是 `<pre>` 原文倾倒 | `web/index.html` `#cli-detail-log`；`renderDetailLog` 用 `textContent` | 灰块长文、无层次 |
| Claude print 写 **stream-json NDJSON** | `claude.rs`：`--output-format stream-json --verbose` | 满屏 `{"type":...}`，信息噪音爆炸 |
| 后端只做 **字节 tail**，不做语义裁剪 | `services.rs` `read_log_tail` | 截断从文件中间切，半行 JSON、无事件边界 |
| 轮询整包 `log_tail` 重绘 | `get_project_live` + 2s poll + 全量赋值 | 闪烁、跟丢滚动、卡 |
| stderr 粗暴拼接 | `--- stderr ---` 拼进同一 tail | 主输出与错误糊在一起 |
| 已有真·终端能力未接到桌面 | `src/terminal/` 外置 WezTerm/iTerm/`tail -f`；`get_task_logs` 几乎未用 | 监视与系统终端两套脱节 |
| Planner 日志同样是 raw pre | `#planner-log` | 规划阶段同样难看 |

**结论（本质）**：丑不是「终端组件选错」单点问题，是 **数据形态（机器 JSON 流）与展示形态（给人看的时间线）错配**。

---

## 2. 方案选型（代入最佳团队怎么选）

| 方案 | 做法 | 适合 | 不适合 cco 的原因 |
|------|------|------|-------------------|
| A. xterm.js / node-pty 真终端 | 嵌 PTY，交互 shell | 交互调试、本机 shell | Claude 主路径是 **非交互 print + NDJSON**，PTY 也救不了 JSON 乱码；重、权限复杂 |
| B. 外置系统终端 only | 一键 WezTerm/iTerm `tail -f` | 高级用户 | 离开 App，多任务监视弱 |
| **C. 结构化日志控制台（推荐）** | 解析 stream-json → 事件行/卡片；可切 raw | **编排器监视主路径** | — |
| D. C + 轻量 ANSI + 外置终端按钮 | C 为主，附 raw / 外置 follow | 完整产品 | 略多一期工程量 |

**决议：D 分两期落地；默认体验 = C。**

类比：Cursor / Claude Code / OpenHands Canvas 给人看的是 **步骤时间线**，不是原始 JSONL。  
cco 的定位是工头看板，更应如此——别把自己做成伪 Terminal.app。

---

## 3. 目标体验（产品）

```text
左：任务列表（排队/运行/完成/失败）
右：日志控制台
  顶栏：任务名 · 状态 · 字号 · 视图(可读|原始) · 复制 · 外置终端 · 停止
  主体：结构化事件流（默认可读）
  底栏：成本 · 用时 · 字节/事件数
```

**可读视图排版规则**

1. 一行一事：时间 · 类型徽章 · 摘要  
2. 类型着色：`system` 灰 / `assistant` 正文 / `tool_use` 蓝 / `tool_result` 绿或红 / `result` 强调 / `error` 红  
3. 工具调用默认折叠，点开看入参/出参截断  
4. 超长文本折叠 +「展开」；代码块等宽  
5. 自动贴底；用户上滚则暂停贴底（保留现有 stick 逻辑，做得更稳）  
6. 失败任务：顶部 **错误摘要条**（一行人话）+ 下方完整流  

**原始视图**：等宽 pre，仅 strip 无效控制符；给排障用。

---

## 4. 前端问题清单 → 改法

| # | 现状问题 | 改法 |
|---|---------|------|
| F1 | `<pre>` 直接 dump | 新组件：`LogConsole`（`#cli-detail-log` 升级为事件列表容器） |
| F2 | 无 ANSI / 无 JSON 理解 | 前端 `parseStreamJsonLines` + 可选轻量 ANSI→span（raw 模式） |
| F3 | 全量 `textContent` 重绘 | 按 `event_id`/`offset` 增量 append；切换任务才 reset |
| F4 | Planner / Exec 两套样式漂移 | 共用同一 `LogConsole`（`#planner-log` 复用） |
| F5 | 浅色 log 区对比弱、层次平 | 行 hover、类型色条、分隔；保持浅色桌面风（延续 UX 计划） |
| F6 | 无「外置终端」入口 | 详情工具栏加按钮 → 调 Tauri `open_task_terminal` |
| F7 | 字号有、视图模式无 | 增加 `可读 | 原始` 切换，localStorage 记忆 |

**不引入** 重型编辑器/Monaco；首期纯 DOM + CSS 足够。若后期事件量极大再评 virtual list。

---

## 5. 后端 / 接口问题清单 → 改法

| # | 现状问题 | 改法 |
|---|---------|------|
| B1 | `log_tail: String` 只有原文 | `TaskLive` 增可选 `log_events: Vec<LogEvent>` + 保留 `log_tail`（兼容） |
| B2 | `read_log_tail` 字节切片切断 JSON | 按 **行边界** tail；优先完整 NDJSON 行 |
| B3 | 解析只在 collect 终局 | 新增 `parse_claude_stream_line` / `build_log_events(stdout, stderr, max_events)`，复用/扩展 `parse_claude_result_json` 族 |
| B4 | live 接口塞过大 tail | 协议：`log_format: "pretty"\|"raw"`；pretty 只回 **最近 N 事件摘要**；raw 回 tail 文本 |
| B5 | `get_task_logs` 闲置 | 扩展为选中任务「加载更多 / 全量原文」；支持 `cursor`（byte offset） |
| B6 | stderr 混进 stdout 字符串 | 事件上标 `stream: stdout\|stderr\|system` |
| B7 | 桌面无 open terminal 命令 | Tauri：`open_task_terminal(run_id, task_id)` → 复用 `TerminalManager::open_follow_logs` |
| B8 | 轮询无增量 | 中期：`since_byte` / `since_event`；短期仍 2s，但 payload 变小 |

### 5.1 建议数据结构（接口契约）

```json
{
  "task_id": "inventory",
  "log_tail": "…可选 raw…",
  "log_bytes": 123456,
  "log_truncated": true,
  "events": [
    {
      "id": "e12",
      "ts": "2026-07-17T12:00:01Z",
      "kind": "tool_use",
      "stream": "stdout",
      "title": "Read",
      "summary": "src/main.rs",
      "detail": "可选长文本/JSON 字符串",
      "level": "info"
    }
  ],
  "error_summary": "acceptance failed: missing CCO_DONE"
}
```

`kind` 枚举（v1）：`meta | message | tool_use | tool_result | result | error | stderr | raw_line`  
无法识别的 NDJSON / 纯文本 → `raw_line`，保证 **永不丢行**。

### 5.2 Provider 边界

- **不改** Worker 启动主路径；仍 stream-json 落盘（机器真源）。  
- **展示层**负责变好看；scheduler / acceptance 继续读 stdout 原文。  
- fake provider 可吐少量 NDJSON 样例，方便前端单测。

---

## 6. 落地阶段（可勾选）

### P0 — 可读优先（1–2 天）

- [x] Rust：行边界 tail + `LogEvent` 解析（覆盖 assistant/tool/result/error/杂行）  
- [x] `project_live_view` / `get_task_logs` 返回 `events`  
- [x] 前端 `LogConsole` 可读视图 + 原始切换  
- [x] 错误摘要条；复制「可读文本」  
- [ ] Planner 日志复用同一组件  

**完成定义**：默认监视不再出现满屏原始 JSON；工具调用一眼可扫。

### P1 — 稳与顺（+1 天）

- [ ] 增量渲染 / 贴底手感  
- [ ] `since_byte` 或减小 live payload  
- [ ] 外置终端按钮接 `TerminalManager`  
- [ ] stderr 分色分区  
- [ ] fixtures：一段真实 stream-json 样例 + 解析单测  

### P2 — 打磨（可选）

- [ ] 虚拟列表（超长 run）  
- [ ] 事件过滤（仅工具 / 仅错误）  
- [ ] 轻量 ANSI（仅 raw）  
- [ ] 导出 HTML/MD 报告片段  

---

## 7. 明确不做（防范围膨胀）

1. 不在桌面嵌完整交互 PTY 当 v1 主路径  
2. 不把 OpenHands / xterm 全家桶引进来「看起来像终端」  
3. 不改 PlanIR / DAG 调度算法  
4. 不为好看改 Claude 为 text 输出（会丢掉结构化进度；应 **双相**：盘上 JSON、脸上事件）

---

## 8. 验收清单

- 运行中任务：可读模式见「说了什么 / 调了什么工具 / 结果如何」  
- 原始模式：可核对 NDJSON  
- 失败任务：顶栏摘要 + 可定位最后错误事件  
- 多任务切换：日志不串、滚动行为可预期  
- 无 Tauri 时（若纯静态）：降级不炸（空状态）  
- `cargo test` 覆盖 stream 行解析；手动点一次外置终端 follow  

---

## 9. 文件触点（实施地图）

```text
docs/terminal-console-plan.md     ← 本计划
src/runtime/provider/claude.rs    ← stream-json 行语义（解析可抽到 log_format 模块）
src/services.rs                   ← TaskLive / read_log_tail / events
src-tauri/src/lib.rs              ← get_task_logs 扩展、open_task_terminal
src/terminal/manager.rs           ← 桌面调用 follow logs
web/index.html / app.css / app.js ← LogConsole UI
tests/…                           ← NDJSON fixtures
```

---

## 10. 哲学自检

- **好品味**：消灭「给人类看 JSON」这个特殊情况，而不是加 20 个正则美化 if。  
- **实用**：盘上仍是 stream-json（机器相），脸上是 LogEvent（语义相）——GEB 双相同构。  
- **简单**：一个解析器 + 一个组件，Planner/Exec 共用；终端集成是按钮，不是第二套监视系统。

---

## 11. 开放确认（开工前只需答一次）

1. 默认可读视图是否接受 **非真终端**（推荐：是）？  
2. P0 是否必须含 **外置终端**，还是可放到 P1？  
3. 解析优先放 **Rust 后端**（推荐，单一真相）还是前端先 hack？

默认假设：**1=是，2=P1，3=Rust 后端**。若无异议，下一任务按 P0 开工。


---

## 12. 如何看效果（A 路径）

```bash
# 1) 打开刚打包的桌面
open dist/CCO.app

# 2) 或 CLI 假跑后看 stdout 已是 NDJSON
export CCO_STATE_ROOT=/tmp/cco-demo
export CCO_DEFAULT_PROVIDER=fake
cco run --project /path/to/proj --plan docs/plans/hello.cco.yaml --yes --provider fake
```

桌面步骤：
1. 添加任意项目文件夹  
2. Provider 选 **fake**（或 claude/codex）  
3. 选计划 → 分析拆分 → 确认开始  
4. 监视页默认 **可读**：工具调用/助手/结果分行；可切 **原始**

Codex：
- 本机已装 `codex` CLI，或 `export CCO_CODEX_BIN=/path/to/codex`
- 计划/高级选项 provider 选 `codex`


### 观感修复（2026-07-17）

- 默认视图：**终端**（密排等宽 transcript，深底）
- **结构** / **原始** 为二级
- stderr 折叠为 1 行，不再粉红卡片墙
- 完成摘要条压缩，不抢 CLI 区高度
