# cco 聊天页注意力收敛（Chat UX Focus）

> 状态：**U0–U2 已落地**（终检：后台单入口 + soft-fallback 无 fence + 计划卡 CTA + env_note；**不阻塞** D0–D4 / P-chat C0–C2 已闭环项）  
> 日期：2026-07-19  
> 范围：桌面 `#page-chat` 信息架构与噪声降级 · 假模板/CLI 故障呈现 · 就绪条 CTA 层级 · 后台 Mode B 态在聊天页的展示  
> 角色：主路径**体验修补**子计划——在 **P-chat C0–C2 已落地** 之上消灭「一页三态叠放」；**不**替代选文件分配；**不**扩 C3（流式/多会话/方案 B/diff）；**不**改 Mode B `confirm_start` 契约  
> 关联真源：
> - 聊天共建 → [`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md)（C0–C2 ✅ · C3→D5/P2-9；本计划**不**改其 §1.4 / §3.3 方案 A / §6 非目标）
> - 稳定性热修 → [`chat-utf8-fence-panic-2026-07-19.md`](./chat-utf8-fence-panic-2026-07-19.md)（fence UTF-8 · **P-chat-utf8**；与本计划**分列**，**不**并入 U0–U2）
> - 主路径 → [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md)（三步主路径；聊天为可选支路）
> - Mode B → [`product-mode-b-ai-planner.md`](../product-mode-b-ai-planner.md)（分配后 phase；本计划只约束**离开 workspace 时**如何提示）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md)（未完善唯一总账；本计划 → **D5/P2-10**；**勿**回灌 D0–D4 / 把 P-chat 勾回 ☐）
> GEB 入口：[`/CLAUDE.md`](../../CLAUDE.md)（L1）· [`./CLAUDE.md`](../CLAUDE.md)（L2 docs）

> **定稿（t1）**：本前言 + §0–§11 冻结角色、问题、规格、阶段与非目标。  
> 实施勾选真源 = **§5**（U0–U2）；**禁止**第二份「聊天 UX 总览」；**禁止**把本计划写成「P-chat 未完成」。  
> 与总账边界：本修补 → **D5/P2-10**；C3 仍 = **P2-9**；二者**分列**，勿合并。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**聊天页只服务「写好一份计划文档」；后台确认/运行、环境故障、假模板不得抢主路径；满意 → 保存 → 分配，视觉顺序与规格一致。**

```text
【本页】澄清 → 草稿 → 保存 .md → 分配（方案 A）
【旁路】后台 Mode B 态 = 可关次要条；CLI 故障 = 环境条 + Doctor；fake ≠ 可分配草稿
```

---

## 1. 现状问题（对照截图 2026-07-19）

> **定稿（t1）**：下列为 2026-07-19 桌面「共建计划」页目视 + 工作树核对后的问题真源；变更须先改代码再回写本表。

### 1.1 现象：一页叠三态

| 层 | 截图表现 | 规格意图（chat-plan-builder） |
|----|----------|------------------------------|
| **A. 本页任务** | 用户说「你好」；助手吐计划草稿；绿条「草稿已就绪」 | 多轮澄清 → 草稿 → 保存 → 分配 |
| **B. 后台 Mode B** | 顶栏副标题「待确认」· 顶栏「返回确认」· 底栏黑条「multi-cli…待确认」 | 离开 workspace 时**可回**，不抢主任务 |
| **C. 环境故障** | 气泡内嵌「Claude CLI 暂不可用…empty assistant reply…」+ 本地模板全文 | soft fallback 保可用性；**不**冒充真实 AI 计划 |

用户无法回答：「我该继续聊、该保存假计划、还是该回确认 multi-cli？」

### 1.2 根因映射（代码锚点）

| # | 根因 | 证据 | 后果 |
|---|------|------|------|
| **R1** | 后台态多入口同屏 | `plan.js` `updateBgPlanBanner`（body 底栏 + **primary** 钮）· `renderPlanPicker` 顶栏 `#btn-monitor-plan`「返回确认」· `state.js` chat 副标题再写一遍「待确认」 | 同一事件说 3 次；primary 色抢聊天主 CTA |
| **R2** | 故障写进 assistant 消息 | `chat.rs` soft-fallback：`（本机 Claude CLI 暂不可用…）` + `fake_chat_reply` 拼进 `reply` 再 `push` 为 assistant | 历史污染；无法当系统条处理；会话重载仍像「AI 说的」 |
| **R3** | fake 仍走完整 plan fence | `fake_chat_reply` 固定输出 ` ```plan `；`extract_plan_fence` → `draft_plan.markdown`；`renderChatReadyBar` 见 markdown 即亮绿条 | 「你好」→ 四步协作大纲 +「可保存可分配」；假数据当真交付 |
| **R4** | 就绪条 CTA 与规格反序 | 未保存：`#btn-chat-assign` 已显但 disabled；`#btn-chat-save` 为 ghost；composer「发送」为 primary | 视线落在发送/灰分配，不在「保存」 |
| **R5** | 标题/说明四次重复 | 顶栏「共建计划」+ 副标题 + 页内 h2 + 页内 muted | 无优先级；夹后台文案更乱 |
| **R6** | 计划卡 = 整段 pre dump | `chatFormatBody` → `.chat-plan-pre` 全文；卡片**无**「采用并保存」 | 不像可操作产物（规格 §3.2 要求预览 + 动作） |

### 1.3 与已落地能力的边界

| 已完成（**勿**当本计划缺口） | 本计划**只**修 |
|------------------------------|----------------|
| P-chat C0–C2：`#page-chat` · `chat_*` · 落盘 · 方案 A `assignFromChat` | 噪声、假可信、CTA 层级、后台条形态 |
| Mode B phase / confirm_start / auto-start | **不**改业务入口；只改「离开 workspace 时」UI 提示 |
| C3 流式/多会话/方案 B/diff（P2-9） | **不**做；仍不排期则不碰 |

---

## 2. 产品目标与用户心智

### 2.1 三句话（修后）

1. 没有计划？去**聊天**说清楚要干什么。  
2. AI 帮我写成计划；**不行就告诉我环境坏了**，别塞假大纲。  
3. **保存**后才能**分配**；后台有别的计划在跑/待确认时，一条次要提示够了。

### 2.2 修后主路径（chat 支路，不变语义）

```text
① 侧栏选项目
② 打开聊天
③ 多轮澄清（本页唯一主任务）
④ 真实 AI 给出计划草稿卡片
⑤ 「保存为计划」→ 落盘 → 就绪条切到「已保存 + 分配计划」
⑥ 分配 → 方案 A chooser → Mode B（与现网同源）
```

旁路（允许存在，**不得**压 ③–⑤）：

```text
· 他计划 planning/confirm/running → 单条可关 banner（ghost 回看）
· CLI 不可用 → 环境条 +「环境检查」；消息区不进完整假 plan
· CCO_CHAT_FAKE=1 联调 → 明示 mock；默认不点亮「可分配」
```

### 2.3 成功时用户不应再感到

| 修前 | 修后 |
|------|------|
| 「为什么底栏逼我去确认别的计划？」 | 可关一条灰字提示，不挡输入 |
| 「这计划是真的还是模板？」 | 故障/ mock 有独立系统态，不进「AI」气泡当正文 |
| 「保存和分配哪个是下一步？」 | 未保存只 primary「保存」；保存后 primary「分配」 |
| 「标题说了四遍还夹返回确认」 | 顶栏短标题；页内一句步骤 |

---

## 3. 界面规格

> **冻结（t1）**：下列为聊天页注意力收敛的 **UI 唯一真源**；实现走 §5 U0–U2。  
> **不**改 chat-plan-builder §3.3 方案 A 跳转语义；**不**改顶栏「分配计划」在 workspace 的 primary 角色。

### 3.1 信息架构（chat 页）

```text
顶栏（page=chat · 已选项目）:
  [选择计划] ghost · （可选）[监控] ghost 仅有可监视活动时
  预算 chip：运行/确认态可显；纯写计划时可藏（U2）
  ✗ 不显示「聊天」自指按钮（已在 chat）
  ✗ 不把「返回确认」做成顶栏主动作区唯一亮点

主区:
  页头：与 AI 共建计划 · {项目名}   （删重复副标题堆叠）
  一句步骤：先写好计划文档 → 保存 → 分配
  消息流
  [系统环境条] 仅 CLI/fake 故障时
  [就绪条] 仅有真实草稿/已保存时
  输入 + 发送
  底提示一句（与就绪条不重复）

浮层/固定:
  #bg-plan-banner：单条 · ghost 钮 · 可关 · 不挡 composer 焦点区
```

### 3.2 后台 Mode B 提示（R1）

| 规则 | 冻结 |
|------|------|
| **唯一常驻** | `#bg-plan-banner` **或** 顶栏 `#btn-monitor-plan` 二选一为主；**禁止**副标题 + 顶栏 + 底栏三连喊同一句 |
| 推荐默认 | **保留**顶栏 ghost「返回确认/查看监控」；底栏 banner **可关**（`localStorage` 记关断，同 session 或 至 phase 变更再显） |
| 按钮样式 | banner 内按钮 = **ghost/secondary**，**禁止** `btn primary`（避免抢就绪条/发送） |
| 文案 | 必须带**计划名**；跨项目态须可理解（「项目 X · 计划 Y 待确认」） |
| 副标题 | chat 页 `#page-sub` **不再**追加「待确认，可点返回确认」（顶栏钮已够） |
| 位置 | banner 不得遮挡 `#chat-input` 与就绪条；优先顶栏下细条或主区底、输入**上方**的次要条 |

### 3.3 环境故障与 fake（R2 · R3）

| 场景 | UI | 消息流 | `draft_plan` / 就绪条 |
|------|-----|--------|----------------------|
| CLI 失败 / empty reply（生产 soft-fallback） | **系统环境条**（非 assistant）：摘要 +「环境检查」+ 可选「重试」 | assistant **不**内嵌 stderr 长串；可一句「暂时无法联系本机 Claude CLI」 | **不**因 fake 模板自动写入可保存 markdown；`fake=true` 时 **不**点亮「计划草稿已就绪」可分配路径 |
| `CCO_CHAT_FAKE=1` / provider=fake（联调） | 页头或条上 **Mock** 标记 | 可用模板（明示模拟） | 联调可保存（可选）；**默认** `assign` 仍要求用户知悉 mock，或仅 `save` 不鼓励分配——**U0 默认：fake 草稿可保存、分配前 toast 强提示** |
| 真实 AI 成功 + fence | 无环境条 | 卡片展示 | 未保存：就绪条「草稿未保存」+ primary **保存**；保存后：path + primary **分配** |

后端契约增量（见 §4）：

```text
ChatSendResponse.fake 已有
+ 建议：env_note?: string   // 给人读的短故障，前端进系统条，不进 assistant body
或：assistant reply 不含 diagnostic 括号长文；diagnostic 仅在 fake 时由前端 toast/条展示
```

### 3.4 就绪条与 CTA（R4）

| 状态 | 就绪条 | 保存 | 分配 | 发送 |
|------|--------|------|------|------|
| 无 draft markdown | 隐藏 | — | — | primary（常态对话） |
| 有 markdown · 未保存 · **非 fake 阻断** | 显：「计划草稿已就绪（尚未保存）」 | **primary**「保存为计划」 | **隐藏**（勿 disabled 占位） | secondary/ghost 或保持 primary 皆可；**保存更抢眼** |
| 已保存 `chatDraftPlan` | 显：`已保存：{path}` | ghost「重新保存」 | **primary**「分配计划」 | 常态 |
| fake 且 U0 策略 | 显但标「本地模板 · 非真实 AI」 | 可保存 | 保存后分配时 **toast 二次提示** | — |
| `chatBusy` | 按钮 disabled | disabled | disabled | 「思考中…」 |

对齐 chat-plan-builder §3.3：**仅** `chatDraftPlan` 有路径时启用分配——本计划把「未保存就露出灰分配」改为 **隐藏**，减少「为什么不能点」摩擦。

### 3.5 计划卡片（R6）

| 项 | v1（本计划） |
|----|----------------|
| 展示 | 标题（md 首个 `#`）· 最多 4 条任务大纲 · 「展开全文」 |
| 动作 | 卡片内 **「采用并保存」** → 同 `saveChatPlan` |
| 禁止 | 卡片上「开始运行 / 分配并开跑」 |

### 3.6 文案收敛（R5）

| 位置 | 保留 | 删除/合并 |
|------|------|-----------|
| `#page-title` | 「共建计划」 | — |
| `#page-sub` | `与 AI 写计划 · {项目}` | **删**「待确认/返回确认」后缀 |
| 页内 h2 | 可与 title 二选一；若留 h2 则 sub 极短 | 禁止 title+h2+muted 三句同义 |
| 页内 muted | 一句：`先保存计划文档，再分配进入拆分` | 与 composer-hint 不重复「满意后…」两套 |
| composer-hint | 仅输入辅助（Enter 发送等） | 不重复就绪条状态句 |

### 3.7 边界

| 勿做 | 归属 |
|------|------|
| 改方案 A → 方案 B 默认 | chat-plan-builder C3 / P2-9 |
| 聊天直调 `confirm_start` | **禁止** Mode B |
| 取消 soft-fallback 导致无 CLI 时页死 | 保留可用性；只改**呈现**与**是否当真草稿** |
| 重做整个 desktop 壳 | desktop-ux 0–4 已落地 |
| 把 C3 流式塞进本计划 | P2-9 |

---

## 4. 技术设计

> **冻结（t1）**：下列为实现锚点；**U0 可只改 web/**；U1 可动 `src/services/chat.rs` 回复组装；**禁止**新 Scheduler / 新 page 枚举。

### 4.1 前端改动面

| 文件 | 改动 |
|------|------|
| `web/js/plan.js` | `updateBgPlanBanner`：按钮改 ghost；可选 dismiss；文案带项目/计划名；避免与顶栏重复时隐藏其一 |
| `web/js/state.js` | `showPage("chat")` 副标题去掉 phase 待确认拼接 |
| `web/js/plan.js` `renderPlanPicker` | chat 页隐藏 `#btn-open-chat`；监控钮保持 ghost |
| `web/js/chat.js` | `renderChatReadyBar` CTA 表 §3.4；`resp.fake` → 环境条 + 不点亮可分配就绪；`chatFormatBody` 计划卡折叠 + 采用保存；系统条 DOM |
| `web/css/chat.css` · `plan.css` | 环境条 · banner 不挡 composer · 卡片折叠 · 就绪条 primary 切换 |
| `web/index.html` | 可选：`#chat-env-bar` 静态槽；页头文案收敛 |

### 4.2 后端改动面（U1）

| 项 | 行为 |
|----|------|
| `chat_send` soft-fallback | `reply` = **短**人话或空 + 可选固定引导；**diagnostic 长串**不进 `messages[].content`（可 `tracing::warn` + 可选 `env_note` 字段） |
| `fake_chat_reply` | 保留联调；生产 soft-fallback **可**改为不含 ` ```plan ` 的短说明，避免误提取 draft（推荐 U1） |
| `ChatSendResponse` | 保持 `fake: bool`；可选 `env_note: Option<String>`（有则前端只画系统条） |
| 兼容 | 旧会话 JSON 无 `env_note` 仍可读；历史里已污染的 assistant 诊断文案不强制迁移 |

### 4.3 状态字段（增量）

| 字段 | 含义 |
|------|------|
| `state.chatFake` / 每条消息侧车 | 最近一轮是否 fake（驱动环境条） |
| `state.bgBannerDismissed` | 用户关掉 banner；phase/run_id 变化时复位 |
| 现有 `chatDraftPlan` · `draft_plan` | 语义不变；**写入策略**随 fake 收紧 |

### 4.4 分配跳转

**不变**：`assignFromChat` → `selectPlan` → `showPage("workspace")` → `openPlanChooser(true)` → 用户点分配 → `analyzePlanFromPicker`。

仅增加：若 `fake` 来源草稿，toast 明示后再开 chooser（U0）。

### 4.5 测试 / 验证

| 层 | 命令/动作 |
|----|-----------|
| 语法 | `node --check web/js/chat.js` · `plan.js` · `state.js` |
| 单测 | `cargo test --lib services::chat`（fallback 不把 diagnostic 当 plan fence 或 fake 无 fence 时无 draft——按 U1 决议） |
| 目视 | 打包桌面：① 无 CLI 发「你好」② 有后台 confirm 时进聊天 ③ 真草稿保存→分配 |

---

## 5. 阶段切分与勾选

> **冻结（t1）**：实施勾选真源 = 本 §；总账 **P2-10** 出池后开工。  
> **U0 → U1 → U2**；U0 可独立 ship 体验主痛。

### 5.0 总览

| 阶段 | 目标 | 状态 | 主要触点 |
|------|------|------|----------|
| **U0** | 注意力与 CTA（纯前端可完） | ✅ | `web/js/*` · `web/css/*` · `index.html` |
| **U1** | 故障/fake 可信度（后端回复组装） | ✅ | `src/services/chat.rs` + 前端系统条 |
| **U2** | 文案/卡片/顶栏抛光 | ✅ | 卡片折叠 · 标题收敛 · 计划卡 CTA |

### U0 — 注意力与 CTA ✅

- [x] 后台态：副标题不再复读「待确认」；banner 钮改 ghost；可关  
- [x] 顶栏 chat 页隐藏自指「聊天」  
- [x] 就绪条：未保存 **隐藏**分配、**primary** 保存；已保存 primary 分配（演进：CTA 迁计划卡脚，sticky 就绪条默认隐藏）  
- [x] `resp.fake`：toast 保留 + 环境条/模板标注；分配前强提示  
- [x] `node --check` 相关 js 通过  

### U1 — 故障呈现与 draft 策略 ✅

- [x] soft-fallback 的 diagnostic **不**写入 assistant 正文（或极短）  
- [x] 生产 fallback **默认不**产出可提取的 ` ```plan `（或提取后前端拒绝当就绪）  
- [x] 可选 `env_note` + `#chat-env-bar`（环境检查 CTA）  
- [x] `cargo test --lib` chat 相关绿  

### U2 — 抛光 ✅

- [x] 计划卡：标题 + ≤4 大纲 + 展开 +「采用并保存」/「执行此计划」  
- [x] 页头/hint 文案单源  
- [x] 纯聊天态弱化预算 chip（可选；顶栏与后台互斥）  
- [x] 目视清单 §9 全绿  

### 5.1 边界

| 勿做 | 归属 |
|------|------|
| 流式 / 多会话 / 方案 B / diff | **P2-9 / C3** |
| 回灌 P-chat C0–C2 为未完成 | **禁止** |
| 实现 multi-cli 协作方案 | multi-cli 计划 / D5 |
| 在本阶段改 `confirm_start` / Scheduler | **禁止** |

---

## 6. 非目标

| # | 非目标 | 说明 |
|---|--------|------|
| **N1** | 重做聊天产品形态 | 仍是「散文 md → 分配」；不是 Chat IDE |
| **N2** | 去掉 soft-fallback | 无 CLI 时桌面须可打开；只改呈现与是否当真草稿 |
| **N3** | 方案 B 一键分配默认 | 仍方案 A（chat-plan-builder §8 Q2） |
| **N4** | 跨项目全局聊天会话 | 仍 per-project `.cco/chat/` |
| **N5** | 修复「用户只说你好也应产出深计划」的模型质量 | 真 CLI 提示词打磨可另项；本计划管 **假模板与噪声** |
| **N6** | TUI/CLI 聊天页 | 仍仅桌面 |

---

## 7. 成功标准

| # | 指标 | 验收 |
|---|------|------|
| **S1** | 一页一主任务 | chat 页目视：后台提示 ≤1 处有效入口；无三连「返回确认」 |
| **S2** | 故障不可信草稿 | 无 CLI 时发消息：无「完整协作大纲」冒充 AI；无 stderr 长文进气泡 |
| **S3** | CTA 顺序 | 未保存只强调保存；保存后只强调分配 |
| **S4** | 方案 A 不回归 | 真保存后分配仍进 chooser，不直跑 worker |
| **S5** | 回归绿 | `node --check` + `cargo test --lib`（chat）通过 |

---

## 8. 默认决议

| Q | 问题 | 默认 | 备注 |
|---|------|------|------|
| **Q1** | banner vs 顶栏监控谁留？ | **顶栏 ghost 保留**；banner 可关且按钮 ghost | 减 primary 争抢 |
| **Q2** | fake 能否保存？ | **能**（联调/演示） | 分配前 toast；生产 fallback U1 尽量不产 fence |
| **Q3** | 生产 fallback 是否完全禁止 draft？ | **U1 是**（无 fence 或前端忽略） | 避免「你好→四步计划」 |
| **Q4** | 是否新增 `env_note` API 字段？ | **U1 优选**；U0 可仅前端 `resp.fake` | 有字段更干净 |
| **Q5** | 与 P2-9 关系 | **分列** P2-10 | 本计划不是 C3 |

---

## 9. 验证清单

| # | 步骤 | 期望 |
|---|------|------|
| V1 | 有后台 confirm · 进聊天 | 仅一处「返回确认/查看」；composer 可用；可关 banner |
| V2 | 断 CLI · 发「你好」 | 系统/环境提示；无完整假计划就绪可分配 |
| V3 | `CCO_CHAT_FAKE=1` · 发消息 | 明示 mock；保存/分配有提示 |
| V4 | 真 CLI · 产出 fence · 保存 · 分配 | 方案 A chooser；顶栏/就绪 CTA 正确 |
| V5 | 切项目 | 会话与 banner dismiss 不串（按现有 per-project 规则） |
| V6 | `node --check` + `cargo test --lib` | 绿 |

---

## 10. 文档与 GEB

落地或定稿时同步：

| 文件 | 动作 |
|------|------|
| [`docs/CLAUDE.md`](../CLAUDE.md) | 成员清单 + 本文件一行 |
| [`/CLAUDE.md`](../../CLAUDE.md) | config 指针 + 一句状态 |
| [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md) | §2.1 **P2-10** · §4 D5 池行；**勿**改 D0–D4 勾选 |
| [`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md) | 头部或 §3 末 **追加**指针「体验修补见 chat-ux-focus」；**禁止**改写已冻 t 行语义 |
| [`ux-simple-mainpath-2026-07-17.md`](./ux-simple-mainpath-2026-07-17.md) | 可选一句：聊天支路体验 → 本文件 |

---

## 11. 修订历史

| 时点 | 内容 |
|------|------|
| **t1 · 2026-07-19** | 初稿定稿：§0–§11；问题来自桌面共建计划截图分析；阶段 U0–U2；总账 **P2-10**；与 P-chat C0–C2 / P2-9 边界钉死 |
| **t2 · 2026-07-20** | 只读终检：U0–U2 代码已齐（banner ghost/可关、chat 自指隐藏、计划卡 CTA、`env_note` soft-fallback 无 fence、`chatFormatPlanCard`）；勾满 §5；总账 t29 |

---

## 附录 A — 修前/修后线框（示意）

**修前（问题态）**

```text
顶栏: 共建计划 | 与AI写计划·项目·待确认… | [聊天][返回确认][预算][刷新]
主区: 与AI共建计划 + 又一句说明
      气泡(故障全文+假计划)
      绿条 草稿就绪 [保存 ghost][分配 disabled primary]
      输入 [发送 primary]
底:  黑条 「multi-cli」待确认 [返回确认 primary]
```

**修后（目标态）**

```text
顶栏: 共建计划 | 与AI写计划·项目 | [选择计划][返回确认 ghost?][刷新]
主区: 一句步骤
      [环境条：CLI 不可用 → 环境检查]   // 仅故障
      气泡（真人话；假计划不进或折叠+Mock）
      就绪: 未保存 → [保存 primary]
            已保存 → 已保存 path [重新保存][分配 primary]
      输入 [发送]
     （可选细条：他计划待确认 · 关闭）
```

---

[PROTOCOL]: 变更时更新此头部与 §5 勾选；落地后回写 §10 GEB 与总账 P2-10 状态
