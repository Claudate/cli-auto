---
name: session-digest
description: >
  Built-in session-digest/v1 semantic compression (default every chat/agent wave — NOT an opt-in slogan).
  Use to inspect, repair, or force-refresh digest; cco desktop chat already extracts/stores/strips
  session-digest fences each turn. Also use when resuming a long thread and digest must be read first.
  Does not call cco confirm, does not spawn business workers.
---

# /session-digest — 会话语义压缩（内置）

> **铁律**：压缩是**默认能力**，不是「说了才压」。本 skill 用于检修/显式重抽/仓外 Agent 对齐。  
> **SoT**：[`docs/contracts/session-digest.md`](../../../docs/contracts/session-digest.md)  
> **工作流**：[`docs/runtime-prompts/session-digest-workflow.md`](../../../docs/runtime-prompts/session-digest-workflow.md)  
> **勾选**：[`docs/context-digest-compress-landing-2026-07-27.md`](../../../docs/context-digest-compress-landing-2026-07-27.md)

## cco 桌面（已内置）

- 每轮助手应附 ```session-digest；主机写入 `ChatSession.session_digest` 并剥 UI。  
- 下一轮自动注入压缩块；用户无需口令。

## 仓外 / 本 skill

| 场景 | 动作 |
|------|------|
| 波次结束 · 上下文将满 | **自动写** digest（默认） |
| 用户说压缩/写 digest | 强制重抽（调试） |
| 续作 / 长中断 | **先读** digest |
| 升格为记忆 | 仅稳定铁律 → MEMORY 原子条（显式） |

### 写

1. 读契约与示例。  
2. 合并上一版；`dont` 不无故变少。  
3. 只出 YAML / 或维护会话字段。  
4. 自检 goal · rejected · source。  
5. **禁止** confirm / 业务开跑。

### 读

1. 先 digest，再 `Read` 指针。  
2. 核 `dont` / `constraints`。  
3. 新决策带 rejected；新禁止只追加。

## 不做

- 把压缩做成可选开关默认关  
- gzip / 文言充上下文  
- 改 `planner/digest.rs` 职责 · 重开 guided G
