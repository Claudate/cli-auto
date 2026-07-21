# cco 聊天 plan fence UTF-8 panic 热修

> 状态：**F0 代码已落地**（`cargo test --lib services::chat` 15 绿）· **F1 桌面包重编+中文主路径已验** · **F2 可选防御不排期则不碰**  
> 日期：2026-07-19  
> 范围：`src/services/chat.rs` 的 `extract_plan_fence` + 历史截断 · 回归单测 · 桌面 `chat_send` 中文路径  
> 角色：P-chat **已闭环能力上的 P0 稳定性热修**——消灭中文路径下 `chat_send` join panic；**不**开新分配入口；**不**做 UX 注意力收敛；**不**做右侧计划栏  
> 关联真源：
> - 聊天共建 → [`chat-plan-builder-2026-07-18.md`](./chat-plan-builder-2026-07-18.md)（C0–C2 ✅；本热修**不**改其方案 A / C3）
> - 注意力收敛 → [`chat-ux-focus-2026-07-19.md`](./chat-ux-focus-2026-07-19.md)（U0–U2 · **分列**；本热修**不**替代 P2-10）
> - 总账 → [`gap-and-landing-plan-2026-07-18.md`](../gap-and-landing-plan-2026-07-18.md)（登记 **P-chat-utf8**；**勿**回灌 D0–D4 / 把 P-chat 勾回 ☐）
> - 执行闭环（方法论反例触发）→ [`plan-execute-inspect-rework-2026-07-19.md`](../plan-execute-inspect-rework-2026-07-19.md)（四波 residual 类问题归 **P-loop**；本热修**不**重开）
> - 同类范式 → `src/runtime/log_events.rs` `floor_char_boundary` / `truncate`（live 日志 CJK 安全；chat 应对齐）  
> GEB 入口：[`/CLAUDE.md`](../../CLAUDE.md)（L1）· [`./CLAUDE.md`](../CLAUDE.md)（L2 docs）

> **定稿（t1）**：本前言 + §0–§11 冻结角色、根因、规格、阶段与非目标。  
> 实施勾选真源 = **§5**（F0–F2）；**禁止**并入 chat-ux-focus 当「体验项」；**禁止**写成「P-chat 未完成」。  
> 与总账边界：本热修 = **P-chat 残差稳定性**（单开 **P-chat-utf8**）；**不**占 D5/P2-9·P2-10。

[PROTOCOL]: 变更时更新此头部与阶段勾选；落地后检查 `docs/CLAUDE.md` 与 `/CLAUDE.md`

---

## 0. 一句话

**中文聊天发送不得因解析 ``` 代码块而 panic；`&str` 切片必须落在 UTF-8 char boundary。**

```text
【修前】reply 含 plain ``` + 中文「若/和」→ extract_plan_fence 字节硬切 → chat_send join panic
【修后】fence 按 ASCII 语言标签 / 字符边界推进；历史截断按 chars；单测锁死回归
```

---

## 1. 现象与根因

> **定稿（t1）**：2026-07-19 桌面「共建计划」截图 + 本地 `rustc` 最小复现 + 工作树核对。

### 1.1 用户可见现象

系统气泡（可重复两次）：

```text
发送失败：chat_send join error: task NNN panicked with message
"start byte index 3 is not a char boundary; it is inside '若' (bytes 2..5 of string)"
"start byte index 3 is not a char boundary; it is inside '和' (bytes 2..5 of string)"
```

| 层 | 行为 |
|----|------|
| 前端 | `invoke("chat_send_cmd")` 失败 → 系统气泡 |
| Tauri | `spawn_blocking` worker **panic** → `JoinError` 格式化为 `chat_send join error: …`（[`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs) `chat_send_cmd`） |
| 服务 | `chat_send` 内 `extract_plan_fence(&reply)` 字节切片 panic |
| 半成功 | 用户消息 early save 已落盘；assistant 未写入 → 只见「我」气泡 |

### 1.2 根因（代码锚点）

| # | 根因 | 证据 | 后果 |
|---|------|------|------|
| **R1** | `extract_plan_fence` 对非 `plan` fence 使用 `after[3..]` **固定 3 字节**前进 | 修前 `src/services/chat.rs`；panic 文案 `start byte index 3` | 中文 3 字节/字 → 切在「若/和」中间 |
| **R2** | `build_user_prompt` 用 `&m.content[..4000]` 字节截断 | 同文件历史循环 | 长中文历史第二轮+ 可同类 panic |
| **R3** | 仓库已有 CJK 安全范式未复用到 chat | `log_events::floor_char_boundary` / `chars().take` | 同类 bug 修过 live 日志，chat 漏网 |

### 1.3 最小复现（已本地确认）

下列输入在**修前**必 panic，**修后**不得 panic：

```text
```\n若无异议
```\n和xx
好的\n```\n若需调整
text ``` 若xxx
```é若
```

` ```plan\n# t\n``` ` 修前/修后均正常（走 plan 分支）。

### 1.4 与 UX / 产品反馈的边界

截图同时出现「返回确认」噪声、右侧计划栏诉求等——**归属其它计划**：

| 诉求 | 归属 |
|------|------|
| 后台三连「返回确认」、CTA、fake 可信 | [`chat-ux-focus`](./chat-ux-focus-2026-07-19.md) → D5/P2-10 |
| 右侧计划列表 / 落盘确认 / 列表分配 | **未立项**产品 IA；**非**本热修 |
| 本热修 | **仅** R1–R3 稳定性 |

---

## 2. 目标与成功定义

### 2.1 目标

1. 任意含 CJK / emoji 的 plain ``` 或 ```plan 混排，**不**触发 `chat_send` panic。  
2. 历史 prompt 截断 **不**切半字。  
3. 回归单测覆盖截图同款字「若」「和」。  
4. **不**改变方案 A 分配语义、Mode B、`confirm_start`。

### 2.2 成功标准

| # | 指标 | 验收 |
|---|------|------|
| **S1** | 无 join panic | 中文 + plain ``` + plan fence 路径：`extract_plan_fence` / `chat_send`（fake）不 panic |
| **S2** | plan 语义保留 | last ```plan wins；CJK body 可提取 |
| **S3** | 截断安全 | `truncate_chars` 对 CJK 不产生 U+FFFD / 不 panic |
| **S4** | 单测绿 | `cargo test --lib services::chat` 全绿（含新增用例） |
| **S5** | 桌面主路径 | 重编包后共建计划连发中文含代码块：无「char boundary」系统错误（**F1**） |

---

## 3. 技术规格

> **冻结（t1）**：实现真源 = 本 § + §5 勾选。

### 3.1 `extract_plan_fence`

```text
找到 ```（ASCII，idx 与 idx+3 恒为 char boundary）
  after = search[idx+3..]
  tag_len = ASCII [A-Za-z0-9_+-]* 的 UTF-8 字节长（按 char 累加）
  if tag eq_ignore_ascii_case "plan":
    body = after[tag_len..] 去前导空白
    取到下一 ``` → best = body；search 前进
  else:
    若有闭合 ``` → search 跳到其后
    否则 search = after（禁止 after[3..]）
last plan wins
```

### 3.2 `truncate_chars`

```text
chars().count() <= max → 原串
否则 chars().take(max) + "…"
禁止 &str[..byte_budget]
```

### 3.3 触点

| 文件 | 改动 |
|------|------|
| [`src/services/chat.rs`](../../src/services/chat.rs) | `fence_lang_tag_len` · 重写 `extract_plan_fence` · `truncate_chars` · 历史循环调用 · 单测 |
| [`src/services/CLAUDE.md`](../../src/services/CLAUDE.md) | 成员行注 CJK 安全 |
| 可选 F2 | `src-tauri` `chat_send_cmd` `catch_unwind` → 业务错误串（**不替代** F0） |

### 3.4 禁止

| 禁止 | 原因 |
|------|------|
| 固定字节 `after[N..]` 扫 fence | 本 bug 类 |
| 把热修并进 U0–U2 勾选 | 边界混淆 |
| 改 Mode B / 方案 A / 就绪条产品语义 | 超范围 |
| 右侧计划栏 / 落盘确认弹窗 | 未立项 |

---

## 4. 阶段切分与勾选

> **实施勾选真源 = 本 §**。

### 4.0 总览

| 阶段 | 目标 | 状态 | 触点 |
|------|------|------|------|
| **F0** | 根因修复 + 单测 | ✅ | `src/services/chat.rs` · `services/CLAUDE.md` |
| **F1** | 桌面包重编 + 目视 | ✅ | `scripts/package-app.sh` · `chat_send` 中文主路径（含 fake/soft） |
| **F2** | Tauri join 防御（可选） | ☐ 不排期则不碰 | `chat_send_cmd` catch_unwind |

### F0 — 代码与单测 ✅

- [x] `extract_plan_fence` 去掉 `after[3..]`；ASCII tag + 跳闭合 fence  
- [x] `build_user_prompt` → `truncate_chars(..., 4000)`  
- [x] 单测：`cjk_after_plain_fence_no_panic` · `plan_after_cjk_plain_fence` · `plan_with_cjk_body` · `skips_markdown…` · `truncate_chars_cjk_safe`  
- [x] `cargo test --lib services::chat` → **15 passed**（2026-07-19）  
- [x] L3 note：`extract_plan_fence / history truncate 必须 char-boundary 安全`

### F1 — 桌面验证 ✅

- [x] 重编桌面包（`scripts/package-app.sh` → `dist/CCO.app`；二进制 mtime 晚于 F0 `chat.rs`；含 soft-fallback / fake 模板文案）  
- [x] 与桌面同入口 `chat_send`：中文「若/和」+ plain ``` + plan 混排不 panic（`.cco-out/inspect/f1_verify` → `F1_VERIFY_OK`）  
- [x] 期望达成：无 `char boundary` panic；fake 有 assistant+draft；soft-fallback 短人话无 plan fence  
- [x] `CCO_CHAT_FAKE=1`：plan fence 可提取、draft_plan 非空（V5）  

### F2 — 可选防御 ☐

- [ ] `chat_send_cmd` 对 panic 转 `Err(String)`（用户见短错误，非整 task join 文案）  
- [ ] **不**替代 F0；不排期则不碰

### 4.1 边界

| 勿做 | 归属 |
|------|------|
| U0 三连返回确认 / CTA | chat-ux-focus / P2-10 |
| C3 流式/多会话 | P2-9 |
| 右侧计划 IA | 未立项 |
| 回灌 P-chat C0–C2 为未完成 | **禁止** |

---

## 5. 非目标

| # | 非目标 | 说明 |
|---|--------|------|
| **N1** | 聊天 UX 重设计 | 见 chat-ux-focus |
| **N2** | 取消 soft-fallback / fake | 可用性策略不变 |
| **N3** | 全仓库扫一切 `&str[n..]` | 仅 chat 发送主路径 + 已知 4000 截断；其它模块另项 |
| **N4** | TUI/CLI 聊天 | 桌面 chat 服务层共用修复即覆盖 CLI 调 `chat_send` 若有 |

---

## 6. 验证清单

| # | 步骤 | 期望 | 阶段 |
|---|------|------|------|
| V1 | `cargo test --lib services::chat` | 全绿，含 CJK 用例 | F0 ✅ |
| V2 | 单测输入 ` ```\n若…` | 不 panic、无 plan | F0 ✅ |
| V3 | 单测 plain ``` 后接 ```plan | 提取 plan body | F0 ✅ |
| V4 | 桌面发中文含代码块 | 无 join panic | F1 ✅（重编包 + `chat_send` 中文主路径验） |
| V5 | fake 发送仍可保存 fence | 联调路径不回退 | F1 ✅ |

---

## 7. 默认决议

| Q | 问题 | 默认 |
|---|------|------|
| **Q1** | 是否 F2 catch_unwind 必做？ | **否**；F0 根因优先 |
| **Q2** | 是否进 D5 池？ | **否**；P-chat 残差热修，单开 **P-chat-utf8** |
| **Q3** | 是否改 plan fence 语言白名单？ | **仅** `plan` 提取；其它 fence 跳过（与修前意图一致） |
| **Q4** | markdown fence 是否当 plan？ | **否**（与修前 skip 一致） |

---

## 8. 文档与 GEB

落地同步：

| 层 | 动作 |
|----|------|
| 本文件 | 状态 / §4 勾选 / §10 修订 |
| [`docs/CLAUDE.md`](../CLAUDE.md) | 成员一行 |
| [`/CLAUDE.md`](../../CLAUDE.md) | config 指针一行 |
| [`gap-and-landing-plan`](../gap-and-landing-plan-2026-07-18.md) | 关联真源 · §2 P-chat-utf8 · §9 追加行 |
| [`src/services/CLAUDE.md`](../../src/services/CLAUDE.md) | chat.rs 注 CJK |
| **禁止** | 把 P-chat 勾回 ☐；与 P2-10 合并 |

---

## 9. 风险

| 风险 | 缓解 |
|------|------|
| 旧桌面包仍跑旧 lib | F1 强制重编；文案提示用户 |
| 跳过 plain fence 方式改变扫描位置 | 单测 lock last-plan-wins 与 CJK 混排 |
| 误把 UX 项塞进本计划 | §1.4 / §4.1 / N1 硬边界 |

---

## 10. 修订历史

| 时点 | 内容 |
|------|------|
| 2026-07-19 | **F1 验收**：`package-app.sh` 重编 `dist/CCO.app`；`cargo test --lib services::chat` 15 绿；`.cco-out/inspect/f1_verify` 中文+plain ```+plan / 长历史 / soft-fallback 全过（`F1_VERIFY_OK`）；F2 仍不排期 |
| 2026-07-19 | **t1 定稿 + F0 落地**：根因 R1–R3；规格 §3；F0 代码+15 测绿；F1/F2 待；GEB 指针 |
| 2026-07-19 | 触发：桌面截图系统黄气泡 `start byte index 3` + 「若/和」；用户要求独立计划文档 |

**规则**：既有行语义禁止改写；后续变更另起行追加。

---

## 11. 闭环条件

| 条件 | 状态 |
|------|------|
| F0 代码 + 单测 | ✅ |
| F1 桌面包重编 + 中文主路径 | ✅ |
| L1/L2/总账指针 | 随 t1 同步 |
| F2 | 可选，不阻塞闭环叙事 |

**F0 可单独宣告「库层热修已落地」**；**F1 已重编桌面包并验 `chat_send` 中文主路径**——用户换新包后不应再见到该 `char boundary` join 错误。
