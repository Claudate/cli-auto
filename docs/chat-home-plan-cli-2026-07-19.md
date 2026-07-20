# cco 聊天主窗 · 计划可改 · CLI 不卡死（主路径再收敛）

> 状态：**方案已定稿 · H0–H4 已落地**（终检 t9：S1–S8 全 PASS；**不阻塞** D0–D4 / P-chat C0–C2 / P-loop L0–L2 已闭环项）  
> 日期：2026-07-19  
> 范围：桌面默认入口与路由 · 聊天侧计划列表/全文预览 · **未执行计划可改** · 已执行计划标识 · CLI 卡死可见化与阈值 · 执行前默认/任务级 CLI · **重试耗尽后换 CLI** · 运行时长在状态旁  
> 角色：主路径**体验与稳定性**子计划——把「聊天写计划 → 改到满意 → 分配执行 → 卡住能醒」钉成一条心智；**不**另开第二套 Scheduler；**不**回灌 P-chat / P-loop / multi-cli 已勾项  
> 关联真源：
> - 主路径 → [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（三步已落地；本计划把默认入口从「工作区选计划」收敛为「有跑进执行、无跑进聊天」）
> - 聊天共建 → [`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md)（C0–C2 ✅；本计划扩「主窗 + 右侧计划轨 + 全文弹窗 + 未跑可改」）
> - 聊天注意力 → [`chat-ux-focus-2026-07-19.md`](./chat-ux-focus-2026-07-19.md)（U0–U2 · P2-10；**可并实施**噪声/CTA，**不**替代本计划路由/计划轨）
> - 执行闭环 → [`plan-execute-inspect-rework-2026-07-19.md`](./plan-execute-inspect-rework-2026-07-19.md)（对照计划巡检 · **P-loop ✅**；**≠** 进程卡死巡检）
> - 多 CLI 协作 → [`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md)（声明/越界/检验员 · **未实施**；本计划只做 **默认 CLI + 卡死换家** 最小可用，**不**全量 role/scope）
> - Mode B → [`product-mode-b-ai-planner.md`](./product-mode-b-ai-planner.md)（`confirm_start` 仍是唯一业务 worker 入口）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](./gap-and-landing-plan-2026-07-18.md)（本计划 → **D5 / P2-12 · P-chat-home**；**勿**回灌 D0–D4）
> GEB 入口：[`/CLAUDE.md`](../CLAUDE.md)（L1）· [`./CLAUDE.md`](./CLAUDE.md)（L2 docs）

> **定稿（t1）**：本前言 + §0–§10 冻结角色、问题、目标、规格、阶段与非目标。  
> 实施勾选真源 = **§5**（H0–H4）；**禁止**第二份「聊天主窗总览」；**禁止**把本计划写成 P-chat / P-loop 未完成。  
> 与总账边界：本增强 → **D5/P2-12**；与 **P2-9**（C3 流式）、**P2-10**（注意力）、**multi-cli P0–P2** **分列**。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**没在跑就进聊天主窗把计划聊清楚、改到满意；一点执行就进任务面板；列表能看出「跑过没」；CLI 卡死要能看见、能重试、重试尽了能换另一家 CLI。**

```text
【入口】有活动 run → 执行面板；无活动 run → 聊天主窗
【聊天】写/改计划 · 右侧计划轨 · 点开全文弹窗 · 未执行可改 · 保存后再分配
【执行】分配前选默认 CLI（可改）· 看板显示状态+时长 · 卡死巡检+同家重试 · 耗尽→换 CLI 再试
```

---

## 1. 用户需求拆解（真源）

> 2026-07-19 用户口述整理；实现以本表为准，不得偷换成「只修文案」。

| # | 用户语言 | 产品语义 |
|---|----------|----------|
| **U1** | 聊天应该是主窗口 | 选中项目后：无活动 run → 默认 `page=chat`（不是 welcome 文案 + 硬进 workspace） |
| **U2** | 聊天确定计划后，没执行的计划可以继续编辑 | 已保存但**从未成功开跑**（或 run 未达终态成功）的 `.md` 可在 App 内改 markdown 再保存；**不是**只能 `open_path` 用系统编辑器 |
| **U3** | 后面是执行 | 仍走「分配 → Mode B 拆分 → confirm_start」；**不**聊天里直接 spawn worker |
| **U4** | 进来后有执行任务就进任务执行面板，没有就进聊天 | 项目 `selectProject` / 冷启动恢复：`hasActiveRun()` → workspace+running；否则 → chat |
| **U5** | 选计划时已执行的不要显示或有明显标识 | 计划 chooser + 聊天右侧轨：已成功跑过的计划 **badge「已执行」**；默认 **折叠/滤掉**（开关可显历史） |
| **U6** | CLI 跑半天不动，没有巡查重启 | 内核已有 stall 巡检，但默认 600s、UI 弱感知、**不换 provider**；要：**可见** + 可配阈值 + 耗尽后 **换 CLI** |
| **U7** | 确认计划后右侧显示计划列表；点击弹窗全文 | 聊天页右栏 `plan-rail`；点击 → modal 渲染完整 markdown（只读 +「采用编辑」） |
| **U8** | CLI 运行时间显示到运行状态右侧 | 看板标题行：`[状态 badge] · 时长` 同排（不仅 meta 行 `task_id · elapsed`） |
| **U9** | 多 CLI：执行前默认 CLI，用户可改；某 CLI 卡住重试几次没用就切换 | 分配前/确认前可选 default provider；任务级可改（最小：确认屏或任务条）；stall/fail 重试 `retry_max` 次仍失败 → **自动换另一可用 provider 再试 1 轮**（可关） |

---

## 2. 现状对照（2026-07-19 代码）

### 2.1 已有（勿当从零发明）

| 能力 | 锚点 | 说明 |
|------|------|------|
| 聊天共建落盘 → 分配 | `web/js/chat.js` `saveChatPlan` / `assignFromChat` · `services/chat.rs` | C0–C2 ✅；分配后 `showPage("workspace")` + chooser |
| 设置默认 provider / 重试 / 卡死秒数 | `settings` · `web/js/doctor.js` · `config.default.{default_provider,retry_max,stall_secs}` | 默认 **retry_max=2 · stall_secs=600** |
| Scheduler 卡死巡检 + 同 provider 重试 | `src/runtime/scheduler.rs` `patrol_stall` / `finish_or_retry` | 日志字节无增长 → stop → Pending 再起 |
| 分配弹窗 Provider 选择 | `#pp-provider` in `#plan-chooser` | 跑级默认；**Mode B `apply_worker_defaults` 会把全部任务刷成同一 provider**（桌面尚无混部保真） |
| CLI 看板 elapsed | `web/js/log.js` `formatElapsed` → `.cli-window-meta` | 在 meta，不在 status 右侧；**无 stall 倒计时** |
| 项目进工作区 | `plan.js` `selectProject` → 恒 `showPage("workspace")` | 与 U1/U4 冲突 |
| 拆分结果编辑 | `openEditPlan` → 确认屏改 title/prompt | **无**任务级 provider 下拉；**≠** 散文计划 `.md` 未跑前编辑 |
| 计划列表 | `renderPlanChooser` 路径+标题 | **无**已执行 badge / 过滤 |
| Stall 测 | `tests/retry_and_stall.rs` · `CCO_FAKE_HANG` | 同 provider 重试；**无**换家用例 |
| 多 CLI 混部/handoff 切片 | multi-cli 文档部分已落地 | **仍缺**确认屏改引擎（multi-cli P2-6）与 **stall→换 CLI**；本计划只取默认 CLI + failover |

### 2.2 断点（本计划要消）

| # | 断点 | 后果 |
|---|------|------|
| **B1** | 入口永远 workspace / welcome，聊天是支路按钮 | 「主窗口」心智不成立 |
| **B2** | 保存后预览 = `open_path` 系统编辑器；无 App 内改稿 | 和想要的不一致时改不动或改完不同步 |
| **B3** | 无「已执行」元数据参与列表 | 已跑计划与草稿混排，易误再分配 |
| **B4** | stall 默认 10 分钟、只看 **stdout 字节增长**、UI 几乎不喊 | 用户以为「死了」；有输出心跳但业务卡死时也检不出 |
| **B5** | `finish_or_retry` **永不换** `task.provider`；桌面 Mode B 还 **全量刷** provider | codex 卡死重试仍 codex；混部计划一分配就被抹平 |
| **B6** | 聊天无右侧计划轨 / 无全文 modal | 确认后找不到「我的计划们」 |
| **B7** | 时长不在状态 badge 旁 | 扫一眼不知道跑了多久 |

### 2.3 与相近计划边界

| 计划 | 关系 |
|------|------|
| **P2-10 chat-ux-focus** | 同页降噪/CTA；**可同 PR 顺手**，勾选仍分列 |
| **P2-9 C3** | 流式/多会话/diff — **本计划不做** |
| **P-loop** | 对照**计划勾选**的 inspect/rework — **不是**进程 hang |
| **multi-cli 全文** | role/scope/handoff 检验员 — 本计划只取 **默认 CLI + failover** 切片 |
| **Mode B P2-1** | 确认屏删任务/改依赖 — 可选叠加，**非**本计划主交付 |

---

## 3. 产品目标与修后主路径

### 3.1 三句心智

1. **先聊清楚**——默认进聊天；计划不对就在列表里打开改，满意再分配。  
2. **有活先进去看**——只要有任务在跑/待确认拆分，进执行（工作区）；别让用户在聊天里猜。  
3. **卡住要自己醒**——看得见倒计时/重试；同家重试尽了换另一家 CLI，别干等。

### 3.2 修后主路径

```text
① 选项目
② 路由：
     · 活动 run（running/starting/…）     → workspace 执行面板
     · 有 planJob 待确认/规划中（可选）   → workspace 对应 phase
     · 否则                               → chat 主窗
③ 聊天多轮 → 草稿卡片 → 保存 .md
④ 右侧计划轨可见；点击 → 全文弹窗；未执行 →「编辑」→ 保存覆盖
⑤ 分配计划（就绪条 / 轨上 CTA）→ chooser：选 CLI 默认 + 并发 + 是否暂停确认
⑥ Mode B 拆分 →（可选确认屏改任务级 provider）→ confirm_start
⑦ 看板：状态 badge 右侧显示运行时长；stall 条提示「无日志 Xs / 阈值 Ys · 第 n 次」
⑧ stall/fail → 同 provider 重试至 retry_max → 仍失败且 failover 开 → 切换备用 CLI 再试
⑨ 完成 → 该计划记「已执行」；列表 badge / 默认隐藏
```

### 3.3 成功时用户不应再感到

| 修前 | 修后 |
|------|------|
| 「为什么一进项目就是空工作区？」 | 无跑默认聊天 |
| 「保存后改不了只能系统编辑器」 | App 内编辑未跑计划 |
| 「列表里分不清跑没跑过」 | badge + 默认滤历史 |
| 「CLI 半天不动是不是挂了」 | 状态旁时长 + stall 提示 + 自动重试/换家 |
| 「Codex 卡死只能手动再选 Claude 重开」 | 耗尽后自动换可用 provider（可关） |

---

## 4. 界面与契约规格（冻结）

### 4.1 入口路由（H0）

| 条件（按优先级） | 页面 | phase |
|------------------|------|-------|
| `hasActiveRun()` | `workspace` | `running` |
| `planJob` 为 planning / planned（暂停确认） | `workspace` | `planning` / `confirm` |
| 其它（含 done / 无 live） | **`chat`** | 不强制改 phase 展示 |

- `selectProject` **禁止**无条件 `showPage("workspace")`。  
- 欢迎页：无项目仍 welcome；有项目未选中仍侧栏选项目。  
- 顶栏「监控/返回执行」仅在有活动 run 或待确认时显眼。

### 4.2 聊天主窗 + 右侧计划轨（H1）

```text
#page-chat
  ┌─ 主列 ─────────────────┬─ 右轨 plan-rail ──┐
  │ 消息流                 │ 本项目计划列表     │
  │ 环境条 / 就绪条        │ · 草稿 / 未执行    │
  │ 输入                   │ · 已执行（可折叠） │
  └────────────────────────┴───────────────────┘
  modal#plan-full-view：标题 + markdown 渲染 + [关闭] [编辑] [分配]
```

| 规则 | 冻结 |
|------|------|
| 列表数据 | `list_plans` + 元数据：`last_run_status` / `last_run_at` / `ever_completed`（见 §4.5） |
| 点击项 | 打开全文 modal（`preview` 读文件内容，**不**默认 `open_path`） |
| 编辑 | 仅 `!ever_completed` 或用户强制「另存为新计划」；编辑器 = textarea/简单 md，保存走现有 `chat_save_plan` 或新 `save_plan_md` |
| 已执行 | 默认 **分组折叠**「历史」；开关「显示已执行」；badge 文案 **已执行** / **失败过** / **未执行** |
| 分配 | 未保存改动禁止分配；保存后 CTA 与就绪条一致 |

### 4.3 未执行计划可改（H1/H2）

| 状态 | 可编辑正文 | 可分配 |
|------|------------|--------|
| 仅内存 draft | ✅ | ❌ 须先保存 |
| 已落盘且从未 completed run | ✅ | ✅ |
| 已有 completed run 绑定该 plan_path | ❌ 直接改原文件（防历史漂移）；✅ 「另存副本再改」 | 副本可分配 |
| 拆分后任务图 | 走现有确认屏 / `openEditPlan`（**不**在本计划重做） | — |

### 4.4 运行时长与 stall 可见性（H3）

| 位置 | 规格 |
|------|------|
| CLI 窗口标题行 | `badge(status)` **右侧** 紧跟 `· {elapsed}`（运行中每 poll 轻量刷新，不重建 chrome） |
| meta 行 | 可保留 task_id / cost / provider / attempt；**时长以标题行为准**避免双份抢视线时可缩 meta |
| stall UI | 当 `last_retry_reason=stall` 或 live 暴露 `stall_idle_secs`：黄条「日志 Ns 无增长 · 阈值 Ms · 将重试/已换 CLI」 |
| 设置 | 保留 `stall_secs` / `retry_max`；文案改人话：「多久没新日志算卡死」「同 CLI 最多再试几次」 |
| 默认值建议 | **stall_secs: 180**（3 分钟，可设）；**retry_max: 2** 不变；文档说明旧默认 600 偏钝 |

> 默认阈值变更须写进设置说明与 release note；用户已改过 config 的不覆盖。

### 4.5 已执行标识（数据）

最小实现（推荐）：

```text
list_plans 扩展（或并行 list_plan_meta）：
  path, title,
  last_run_id?, last_run_status?, last_run_finished_at?,
  ever_completed: bool
```

数据源：扫 `~/.cco/runs/*/run.json` 的 `plan_path` + `status`（可缓存 per project）。  
**禁止**仅靠文件 mtime 猜「已执行」。

### 4.6 多 CLI：默认 + 可改 + 卡死换家（H4）

#### 4.6.1 执行前

| 层 | UI | 写入 |
|----|-----|------|
| 全局默认 | 设置 `default_provider` | `config.default` |
| 本 run 默认 | chooser `#pp-provider`（已有） | plan job / start_run `provider` |
| 任务级 | 确认屏每任务下拉（最小 H4）；无确认屏时全员用 run 默认 | `TaskIR.provider` |

#### 4.6.2 Failover 策略（冻结）

```text
attempt 使用 task.provider（或 default）
  → stall | fail | timeout
  → attempt <= retry_max ？ 同 provider 再起
  → 否则若 failover_enabled 且存在备用 provider 且 preflight 通过：
        task.provider = fallback
        attempt 计数可重置 1 次「换家额度」（fallback_extra_attempts，默认 1）
  → 仍失败 → 任务 Failed（现网语义）
```

| 项 | 默认 |
|----|------|
| `failover_enabled` | **true**（设置可关） |
| 备用顺序 | 当前为 codex → 试 claude；当前为 claude → 试 codex；fake 不参与生产 failover |
| 用户手动 stop | **不** failover、**不**重试 |
| 与 multi-cli 全文 | 不引入 role/scope；事件记 `task_retry` + `provider_switched` |

#### 4.6.3 与「多 CLI 协作」文档关系

- **本计划交付**：人在跑前选 CLI + 机器在卡死后换 CLI。  
- **不交付**：scope 越界、handoff 全文契约、inspect 终闸角色（仍归 multi-cli / P-loop）。

---

## 5. 阶段切分与勾选（实施真源）

### H0 — 入口路由（聊天主窗 / 有跑进执行）

- [x] `selectProject` / 冷启动恢复按 §4.1 路由  
- [x] 顶栏监控入口与 banner 与 chat 默认共存（不三连喊：对齐 P2-10 时可合并）  
- [x] 测：无 run 进 chat；造 fake 活动 run 进 workspace（`node scripts/h0-entry-route-check.mjs` + `node --check`）  


### H1 — 聊天右轨 + 全文弹窗 + 未执行可编辑

- [x] `#plan-rail` 列表（分组/历史折叠交 H2；无 meta 时全部当未执行）  
- [x] `plan-full-view` modal：完整 markdown（`read_plan_md`，不默认 `open_path`）  
- [x] 未执行：App 内编辑 + 保存覆盖；已执行：另存副本  
- [x] 分配仍走方案 A / chooser（不改 `confirm_start` 契约）；未保存改动禁止分配  

### H2 — 已执行标识与过滤

- [x] plan meta（ever_completed / last_run_*）服务端或 services 聚合 — `list_plan_meta` / Tauri `get_plan_meta`  
- [x] chooser 与 plan-rail 共用 badge +「显示已执行」开关  
- [x] 默认隐藏已成功执行计划  

### H3 — 时长在状态右 + stall 可见 + 阈值

- [x] CLI 标题行 badge 右 `elapsed`  
- [x] live/UI 暴露 stall 闲置与阈值；重试原因可见（`stall_idle_secs` / `stall_threshold_secs` + `stallStripText` / meta `last_retry_reason`）  
- [x] 默认 `stall_secs` 建议 180（配置迁移说明；serde 默认，不覆盖用户显式值）  
- [x] 设置文案人话化（retry_max / stall_secs + H4 failover 开关/只读顺序）  

### H4 — 执行前 CLI 可改 + 重试尽切换 CLI

- [x] 确认屏（或任务条）任务级 provider 最小编辑（`#confirm-task-provider` + `update_plan_task`；Mode B soft-fill 保用户改过的引擎）  
- [x] Scheduler：`finish_or_retry` 增加 provider failover 分支 + 事件  
- [x] 设置：`failover_enabled` + 备用顺序只读说明  
- [x] 测：fake hang → 同家重试 → 换家；手动 stop 不换  

### 建议落地序

```text
H0（路由，体感最大）
 → H1（右轨/全文/可改，主路径闭环）
 → H3（时长+stall 可见，先止血「以为挂了」）
 → H2（已执行标识，依赖 meta）
 → H4（failover，内核+设置）
```

P2-10 U0–U2 可在 H0/H1 同迭代顺手，**勾选分列**。

---

## 6. 非目标

| ID | 不做 | 原因 |
|----|------|------|
| N1 | 聊天内直接跑 worker / 跳过 Mode B | 破坏 confirm_start 唯一入口 |
| N2 | 流式 token / 多会话 tabs（C3） | P2-9 |
| N3 | multi-cli 全文 role/scope/handoff UI | 另册；本计划只 failover 切片 |
| N4 | 已执行计划原地改历史 md 当「同一交付」 | 历史漂移；用另存副本 |
| N5 | 跨项目全局聊天 | 仍按项目会话 |
| N6 | 回灌 D0–D4 / 把 P-chat·P-loop 勾回 ☐ | 总账纪律 |

---

## 7. 成功标准

| ID | 标准 | 验证 |
|----|------|------|
| S1 | 无活动 run 时选项目 → 落在 chat | 目视 + 路由单测/脚本 |
| S2 | 有活动 run 时选项目 → workspace 看板 | 同上 |
| S3 | 未执行计划可在 App 内改并保存后再分配 | 桌面路径 |
| S4 | 列表/轨上已执行有 badge；默认不抢未执行 | 造 completed run 后 list |
| S5 | 状态 badge 右侧可见 live elapsed | 看板目视 |
| S6 | stall 在阈值内触发重试且 UI 可见原因 | `CCO_FAKE_HANG` + 短 stall_secs |
| S7 | 重试尽后换备用 CLI 再试（可关） | unit + fake 双 provider |
| S8 | `cargo test` / `node --check web/js/*` 绿 | CI 本地 |

---

## 8. 风险与默认决策

| Q | 议题 | 默认 |
|---|------|------|
| Q1 | 待确认 planJob 是否抢过 chat？ | **是**进 workspace（用户已分配，优先确认/跑） |
| Q2 | 已执行过滤默认藏还是 badge？ | **藏 + 可展开**；badge 在展开后仍显示 |
| Q3 | stall 默认 180 是否太敏？ | 先 180；设置可调；日志仍在涨则不触发 |
| Q4 | failover 是否改 task 图持久化？ | **run 态覆盖**写入 run.json/task state；原 plan 文件可选不改 |
| Q5 | 与 P2-10 是否合并一个 PR？ | 允许同迭代；**文档勾选分列** |
| Q6 | Mode B 是否停止「全量刷 provider」？ | **H4 起**：job 默认只填「仍为 default/空」的任务；用户在确认屏改过的 provider **保留**（对齐 CLI soft `--provider`，告别桌面 hard wipe） |
| Q7 | stall 是否升级为「无 tool 事件」？ | **本计划不做**；仍 stdout 指纹；另立热项时再开 |

---

## 9. 关键文件地图（实施导航）

| 区域 | 文件 |
|------|------|
| 路由 | [`web/js/plan.js`](../web/js/plan.js) `selectProject` · [`web/js/state.js`](../web/js/state.js) `showPage` |
| 聊天 UI | [`web/js/chat.js`](../web/js/chat.js) · [`web/index.html`](../web/index.html) · `web/css/chat.css` |
| 计划列表 | [`web/js/plan.js`](../web/js/plan.js) `renderPlanChooser` · services `list_plans` |
| 看板时长 | [`web/js/log.js`](../web/js/log.js) · `web/css/monitor.css` |
| 设置 | [`web/js/doctor.js`](../web/js/doctor.js) · [`src/services/settings.rs`](../src/services/settings.rs) · [`src/config/mod.rs`](../src/config/mod.rs) |
| Stall / 重试 / failover | [`src/runtime/scheduler.rs`](../src/runtime/scheduler.rs) · [`src/state/mod.rs`](../src/state/mod.rs) · live view |
| 跑次元数据 | [`src/services/runs.rs`](../src/services/runs.rs) / 新 plan_meta 聚合 |

---

## 10. 修订历史

| 时点 | 内容 |
|------|------|
| **t1 · 2026-07-19** | 初稿定稿：对照用户九条需求 + 工作树锚点；冻结 H0–H4、非目标、成功标准；总账 ID **P2-12 / P-chat-home** |
| **t9 · 2026-07-19** | 只读终检：S1–S8 全 PASS；勾满 H3 stall 可见 + H4 确认屏任务级 provider；头状态 → **H0–H4 已落地** |

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`
