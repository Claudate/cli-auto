# session-digest 工作流（内置 · 默认每轮）

> **产品铁律**：压缩是**聊天自带能力**，不是口令开关。用户**不必**说「压缩上下文」才会压。  
> 契约：[`../contracts/session-digest.md`](../contracts/session-digest.md)  
> 抽取形状：[`session-digest-extract.md`](./session-digest-extract.md)（Agent 手写/重抽仍可用）  
> 勾选：[`../context-digest-compress-landing-2026-07-27.md`](../context-digest-compress-landing-2026-07-27.md)

---

## 主机行为（cco 桌面聊天 · 已接线）

每轮 `chat_send`：

1. **读**：若 `session.session_digest` 有值 → 注入下一轮 system 前缀（在 pin/summary 之后）。  
2. **写**：助手回复末尾的 ```session-digest 由主机抽取 → 浅检合格则写入 `session.session_digest`。  
3. **显**：从落库的 assistant `content` / UI `reply` **剥掉** digest 围栏，主路径只留人话（```plan 保留）。  
4. **史**：再拼历史时对消息再 strip 一次，避免旧回声占 token。

用户**零操作**；口令「压缩上下文」仅作调试/强制重抽，**不是**功能开关。

## Claude Code / 仓外 Agent

- 默认：长波次结束或上下文将满时**自动**维护 `.cco-out/session-digest.yaml` 或会话等价物。  
- `/session-digest`：显式读写/检修，不是「打开压缩」。  
- 续作：有 digest 则**先读**再按指针展开。

## 合并纪律

1. `dont[]` 只增；废止用 `superseded_by`。  
2. `decisions` 必含 `rejected`。  
3. 冲突取更严，记入 `open[]`。  
4. **禁止**用 digest 调 confirm / 开跑。

## 禁止

- 把压缩做成高级设置默认关  
- 自由散文冒充 digest  
- 与 plan `digest.rs` 模式字段混写
