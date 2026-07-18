# cco 未完善项总览与落地计划

> 状态：草案（分析完成，待确认优先级后执行）  
> 日期：2026-07-18  
> 范围：产品缺口 · 文档 GEB · 代码质量 · 验证发布  
> 关联真源：
> - [`../claude-cli-orchestrator-plan.md`](../claude-cli-orchestrator-plan.md)（编排器设计）
> - [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)（产品主路径 B）
> - [`desktop-ux-redesign-plan.md`](./desktop-ux-redesign-plan.md)（桌面壳 UX）
> - [`terminal-console-plan.md`](./terminal-console-plan.md)（监视日志）
> - [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（主路径简化）

[PROTOCOL]: 变更时更新此头部与阶段勾选，然后检查 /CLAUDE.md 与 docs/CLAUDE.md

---

## 0. 一句话

**内核（M0–M4）已可用；桌面主路径已能「选计划→分配→跑」；缺口集中在：B 模式收尾、监视体验 P1、文档同构、发布验证、超大文件拆分。**

---

## 1. 项目认知摘要

### 1.1 这是什么

`cco`（CLI Orchestrator）= 本机 **任务控制台**：

```text
计划文档 → PlanIR（适配器/Planner）→ DAG Scheduler → WorkerProvider(claude/codex/fake)
                ↓
         state/report/CLI/TUI/Tauri 桌面壳
```

技术栈：`Rust + Tokio + Clap + ratatui + Tauri 2 + 原生 web(HTML/CSS/JS)`。

### 1.2 模块地图（现实）

| 路径 | 职责 | 体量风险 |
|------|------|----------|
| `src/plan/` | 适配器 + PlanIR + **Planner（Mode B）** | `planner.rs` ~1124 行，超标 |
| `src/runtime/` | scheduler / provider / worktree / acceptance / log_events | `claude.rs` ~937、`log_events` ~748 |
| `src/services.rs` | CLI 与桌面共用服务层 | ~880 行，超标 |
| `src/cli/` | clap 命令面 | `mod.rs` ~882 行，超标 |
| `src/tui/` | 多页 TUI | 可接受 |
| `src/terminal/` | 外置终端 / session | CLI/TUI 已用；**桌面未接** |
| `src-tauri/` | Tauri commands | 薄壳 |
| `web/` | 桌面 UI 状态机 | `app.js`/`app.css` 各 2k+ 行，严重超标 |
| `docs/` | 产品/UX 计划 | **无 L2 索引** |
| `tests/` | 集成/金样 | 覆盖调度主路径，缺桌面 E2E |

### 1.3 已完成（不要再当缺口）

| 层 | 状态 |
|----|------|
| M0–M4 编排内核 | ✅ doctor/run/resume/status/stop/report/logs/term/tui |
| Providers | ✅ claude / codex / fake |
| Plan 适配 | ✅ cco-plan/v1 · serial-prompts · raw-single |
| 桌面壳 UX 0–4 | ✅ 浅色主从、项目内开跑、大日志区 |
| 主路径简化 | ✅ 合并选计划弹窗、task-dash、CLI 再跑、AI 事件过滤 |
| Mode B0/B1 主线 | ✅ phase 状态机、plan job、LLM+heuristic、confirm_start、波次/waiting_on |
| 终端日志 A 路径 P0 | ✅ `log_events` + 可读/原始/终端 transcript 观感 |

---

## 2. 现象层：还有什么没完善

### 2.1 产品功能缺口（按优先级）

#### P0 — 必须闭环（否则产品叙事不完整）

| ID | 缺口 | 证据 | 建议动作 |
|----|------|------|----------|
| P0-1 | CLI `run` 默认仍直接 `load_plan`，未默认走规划 | Mode B 表：`CLI run 默认先规划 ☐` | `cco run` 对齐桌面：非结构化 → plan job；结构化可 `--skip-plan` |
| P0-2 | 结构化计划「跳过规划」入口缺失 | B3 ☐；UX 与 B 文档冲突：桌面 `autoStartAfterPlan` 跳过人工确认 | 高级选项：`skip_plan`；确认屏开关可配置 |
| P0-3 | 设计真源未同步 Mode B | B3 ☐；`claude-cli-orchestrator-plan.md` 仍写「设计稿/下一步建仓」 | 改状态为「已落地 M0–M4 + 桌面 + B 主线」，补 B 流程图 |
| P0-4 | 桌面 App 重打包验证未闭环 | `ux-simple-mainpath` 未做 / 风险 | `cargo build -p cco-desktop --release` + `scripts/package-app.sh` + 目视主路径 |

#### P1 — 体验与边界（影响「敢用」）

| ID | 缺口 | 证据 | 建议动作 |
|----|------|------|----------|
| P1-1 | 终端控制台 P1：增量渲染 / 减 payload | terminal-console P1 ☐ | live 协议 `since` 或事件增量；前端 append |
| P1-2 | 桌面「外置终端」按钮未接 | terminal 计划 F6/B7；`open_task_terminal` 未见桌面绑定 | Tauri cmd + 详情工具栏按钮 → `TerminalManager::open_follow_logs` |
| P1-3 | Planner 日志未复用 LogConsole | terminal P0 未勾「Planner 复用」 | `#planner-log` 走同一事件渲染 |
| P1-4 | B3 上限：任务数 / prompt 长度 / 超时 | B3 ☐ | 常量 + validate + UI 提示 |
| P1-5 | 规划预算 vs worker 预算分离展示 | B3 ☐ | plan job 成本字段 + 顶栏分栏 |
| P1-6 | 黄金用例矩阵 | B3 ☐ | 散文 md / 半结构化 / 已是 v1 三套 E2E |
| P1-7 | 自动开跑与「必须确认」产品冲突 | 主路径简化 vs Mode B 硬规则 | **产品决议**：默认 auto-start 或默认 confirm；二选一写进真源 |

#### P2 — 增强 / backlog（可延后）

| ID | 缺口 | 来源 |
|----|------|------|
| P2-1 | 确认屏删任务 / 改依赖 | Mode B2 可选 ☐ |
| P2-2 | replan 保留人工修改 | Mode B2 可选 ☐ |
| P2-3 | 虚拟列表 / 事件过滤 / ANSI / 导出报告 | terminal P2 |
| P2-4 | 跨显示器系统级多窗口 | ux-simple 未做 |
| P2-5 | TUI 内嵌真 PTY 网格 | M3 未勾 |
| P2-6 | Claude Code skill `/cco-run` | M4 可选 |
| P2-7 | M5：SDK provider / Mermaid 导出 / 自动开 PR / Windows launcher | orchestrator M5 |
| P2-8 | Codex 已有 provider，M5 文案仍写「第二 provider」— 文档过期 | 文档债 |

### 2.2 文档 / GEB 缺口（协议债务）

> 启动清单要求读 `.md/`：**仓库无 `.md` 目录**；规范分散在根 `CLAUDE.md`、`docs/*`、`web/CLAUDE.md`。

| 层 | 现状 | 缺口 |
|----|------|------|
| L1 `/CLAUDE.md` | ✅ 存在但偏薄 | 未列本文件；未反映 Mode B / log_events / codex |
| L2 模块地图 | 仅 `web/CLAUDE.md` | **缺** `src/`、`src-tauri/`、`docs/`、`tests/`、`scripts/`、`examples/` 及全部子模块 |
| L3 文件契约 | 仅少数文件 | 缺 L3：**约 30 个** `src/**/*.rs` + `src-tauri` + `web/index.html` + `web/app.css` |
| 计划文档索引 | 无 | `docs/CLAUDE.md` 缺失 → 计划漂移难发现 |
| 设计真源时效 | 过期段落 | M0「下一步建仓」、M5「第二 provider」与代码不符 |
| 协议目录 | 无 `.md/` | 若要坚持 AGENTS 规则，需 **播种 `.md/` 或明确以 `docs/` 为规范根** |

### 2.3 代码质量 / 架构味道

| 味道 | 位置 | 本质 |
|------|------|------|
| 文件过大（>800 行铁律） | `planner.rs` `claude.rs` `services.rs` `cli/mod.rs` `web/app.js` `web/app.css` | 职责未切开；改一处易碎 |
| 双主路径心智 | 桌面 auto-start vs B 强制 confirm | 产品规则未单一真相 |
| 监视两套入口 | 桌面 LogConsole vs `src/terminal` 外置 | 外置未接到桌面，能力闲置 |
| 文档滞后 | 多份 plan 状态勾选不齐 | GEB 回环未成为默认动作 |
| 验证缺口 | 桌面无自动化；依赖人工 App | 回归靠眼睛 |

---

## 3. 本质层：根因

1. **产品主路径有过两次叠加决议**（UX 壳 → Mode B 确认 → 再简化为 auto-start），规则未在一个真源文件里「消灭特殊情况」。  
2. **桌面迭代快于文档回环**：`web/` 日更，L2/L3 与 orchestrator 真源未同步。  
3. **能力纵向切通，横向未收口**：CLI/TUI 的 terminal、budget、plan job 在桌面侧未全部接线。  
4. **单体文件堆功能**：Planner / Claude provider / services / 前端状态机各自长成「局部上帝对象」。

哲学判断：  
**不是「功能太少」，是「完成定义不唯一」——代码能跑，系统尚未自证完成。**

---

## 4. 落地计划（可勾选）

### 阶段 D0 — 文档操作系统（0.5–1 天）

**目标**：地图与地形同构；任何人/Agent 进入仓库不迷路。

- [ ] **决议**：规范根用 `docs/`（推荐）或新建 `.md/` 镜像；写入 L1  
- [ ] 更新 `/CLAUDE.md`：技术栈、Mode B、log_events、codex、本计划链接  
- [ ] 新建 `docs/CLAUDE.md`（L2）：本目录成员清单 + 状态一句话  
- [ ] 新建 `src/CLAUDE.md` + 关键子模块 L2（`plan/` `runtime/` `cli/` `terminal/` `tui/`）  
- [ ] 新建 `src-tauri/CLAUDE.md`、`tests/CLAUDE.md`、`scripts/CLAUDE.md`、`examples/CLAUDE.md`  
- [ ] 核心文件补 L3（先：`services.rs` `planner.rs` `scheduler.rs` `cli/mod.rs` `src-tauri/lib.rs` `web/index.html` `web/app.css`）  
- [ ] 修订 `claude-cli-orchestrator-plan.md` 状态段：M0–M4 已完成、桌面与 B 主线、M5 backlog  
- [ ] 各 plan 勾选框与代码对齐一次（terminal P0 实勾、Mode B 实勾）

**验收**：打开 L1 → 能点到每个模块职责；随机抽 5 个核心文件有 L3。

### 阶段 D1 — 产品规则收口（0.5 天，先决议再写码）

**必须先回答的 3 个问题（阻塞实现）：**

1. 桌面默认：**分配后自动开跑**，还是 **强制确认屏**？  
2. CLI `run` 是否默认走 plan job（与桌面一致）？  
3. 结构化 `cco-plan/v1` 是否默认 skip-plan？

建议默认（可否决）：

| 项 | 建议 |
|----|------|
| 桌面 | 默认 auto-start；高级开关「规划后暂停确认」 |
| CLI run | 默认：可 parse 的结构化直接 exec；散文/未知 → plan 后需 `--yes` 等同确认或打印 DAG 再确认 |
| 结构化 | 显式/自动 skip-plan |

- [ ] 把决议写回 Mode B + UX 真源（消灭双文档冲突）  
- [ ] 实现 P0-1 / P0-2 与决议一致的行为  
- [ ] P0-3 真源同步  

### 阶段 D2 — 监视与桌面接线（1–2 天）

- [ ] P1-2 外置终端按钮  
- [ ] P1-3 Planner 共用 LogConsole  
- [ ] P1-1 增量/减负（至少行边界 tail + 前端少重绘）  
- [ ] stream-json fixture 单测补全  

**验收**：跑 fake/claude 时可读视图干净；一键外置 tail；planner 阶段不糊成 raw 墙。

### 阶段 D3 — 边界与金样（1 天）

- [ ] P1-4 上限常量  
- [ ] P1-5 预算分栏（可先 CLI report + 桌面顶栏简版）  
- [ ] P1-6 三套黄金用例 + `cargo test`  
- [ ] P0-4 重打包 `CCO.app` 主路径目视清单打勾  

### 阶段 D4 — 结构减肥（按需，可并行）

**原则：不为拆而拆；只在下一次改该文件时顺手切开。**

| 文件 | 建议切分 |
|------|----------|
| `src/plan/planner.rs` | `planner/job.rs` · `planner/llm.rs` · `planner/heuristic.rs` · `planner/view.rs` |
| `src/services.rs` | `services/projects.rs` · `services/runs.rs` · `services/live.rs` · `services/settings.rs` |
| `src/cli/mod.rs` | 按子命令文件 |
| `src/runtime/provider/claude.rs` | `spawn` / `poll_bg` / `parse_result` |
| `web/app.js` | `state` · `plan` · `monitor` · `log` · `doctor` 模块（或 IIFE 分段） |
| `web/app.css` | tokens / layout / plan / monitor / log |

### 阶段 D5 — Backlog 池（不排期则不碰）

- 确认屏编辑依赖 / replan 策略  
- 虚拟列表、事件过滤、导出  
- TUI 真 PTY、skill、自动 PR、Windows launcher  
- 跨屏系统多窗口  

---

## 5. 推荐执行顺序（代入最佳团队）

若代入 **Stripe 式产品工程 + Linux 式好品味**：

```text
D0 文档同构（半天，降低所有后续返工）
  → D1 产品规则三问决议 + 实现对齐（先消灭双路径）
    → D2 桌面监视接线（用户每天看见的面）
      → D3 金样与打包验证（敢说 ship）
        → D4 大文件拆分（只在热改路径上做）
          → D5 backlog 按用户真实疼痛挑选
```

**不推荐**：同时开确认屏编辑器 + 虚拟列表 + 全量 GEB L3 灌水。那是复杂度表演，不是完成。

### 任务量与 Agent 策略（对应 AGENTS §8）

| 阶段 | Token/复杂度 | 建议 |
|------|--------------|------|
| D0 | 中，机械 | 可本会话完成；或 worker 按目录并行写 L2 |
| D1 | 低码量高决策 | **必须用户决议**，不可 Agent 自作主张 |
| D2–D3 | 中高 | 本会话分任务；每任务独立 commit |
| D4 | 高回归风险 | 单文件纵向切片 + 测试绿灯；忌大爆炸 PR |
| D5 | 不定 | 单独立项 |

---

## 6. 成功标准（本计划自身）

| 指标 | 目标 |
|------|------|
| 未完善项可检索 | 本文 §2 为唯一总账；子计划只保留细节 |
| 文档同构 | L1 + docs/src/web L2 存在；核心 L3 ≥ 7 文件 |
| 产品双路径 | D1 后仅一套默认主路径描述 |
| 可发布 | D3 后 `CCO.app` 主路径清单全绿 |
| Git 留痕 | 每阶段至少 1 个本地 commit |

---

## 7. 非目标

- 本计划不重写 Scheduler  
- 不引入云端多租户  
- 不把桌面改成 IDE  
- 不为第三方依赖目录灌 GEB 文档  

---

## 8. 开放确认（执行前只需答一次）

请确认以下默认假设（回「按默认」或逐条改）：

1. **规范根**：以 `docs/` + 根 `CLAUDE.md` 为 GEB 真源，**不**新建平行 `.md/` 目录（避免双份）。  
2. **桌面默认**：保持 **分配后自动开跑**；高级区加「规划后确认」开关。  
3. **下一执行阶段**：先做 **D0 文档同构**，再进入 D1 决议实现。  
4. **D4 大拆分**：暂缓，直到有功能改动碰到超标文件。  
5. **本文件**：作为未完善总账；子计划文件只更新状态勾选，不再另开第三份总览。

---

## 9. 修订历史

| 日期 | 说明 |
|------|------|
| 2026-07-18 | 初稿：全库侦察后汇总产品/文档/质量缺口与 D0–D5 落地顺序 |
