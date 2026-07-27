---
name: session-digest
description: >
  Compress or resume session state via session-digest/v1 YAML (semantic cache, not gzip).
  Use when the user says 压缩上下文 / 写 digest / 按 digest 续 / invokes /session-digest,
  or when a long multi-tool wave ends and hard constraints must survive the next turn.
  Does not call cco confirm, does not spawn business workers, does not inject product chat prompts.
---

# /session-digest — 会话语义压缩

> **SoT**：[`docs/contracts/session-digest.md`](../../../docs/contracts/session-digest.md)  
> **抽取提示**：[`docs/runtime-prompts/session-digest-extract.md`](../../../docs/runtime-prompts/session-digest-extract.md)  
> **工作流**：[`docs/runtime-prompts/session-digest-workflow.md`](../../../docs/runtime-prompts/session-digest-workflow.md)  
> **勾选**：[`docs/context-digest-compress-landing-2026-07-27.md`](../../../docs/context-digest-compress-landing-2026-07-27.md)

## 何时用

| 用户意图 | 动作 |
|----------|------|
| 压缩上下文 / 写 digest / 收束 | **写** |
| 按 digest 续 / 恢复同题 | **读** |
| 升格为记忆 | 仅稳定铁律 → MEMORY 原子条（显式）；禁止整份塞 `MEMORY.md` |

## 写（压缩）

1. 读契约 §3–§4 与示例 [`docs/contracts/session-digest.example.yaml`](../../../docs/contracts/session-digest.example.yaml)。  
2. 若存在上一版：读 `.cco-out/session-digest.yaml`（或用户指定路径）。  
3. 用 `session-digest-extract.md` 系统提示，结合本轮原料，**只产出 YAML**。  
4. 自检：`goal` 非空；每个 `decision` 有 **rejected**；每个 `constraint` 有 **source**；`dont` 相对上版不无故变少。  
5. 不合格 → 重抽一次；再失败 → 停并给人看缺项。  
6. 写入 **`.cco-out/session-digest.yaml`**（目录已 gitignore）。可选三行 `.cco-out/arc.md`。  
7. **禁止**调用 `cco confirm` / `confirm_start` / 业务开跑。

## 读（续作）

1. 先读 digest，再按需 `Read` `artifacts` / `source`。  
2. 行动前核 `dont` 与 `constraints`。  
3. 新决策必须带 chose+rejected+why；新禁止只追加 `dont`。  
4. 波次结束再走「写」。

## 不做

- gzip / 文言充上下文  
- 改 `src/plan/planner/digest.rs` 职责  
- 重开 guided G0–G4  
- 把 digest 当唯一真源（冲突回 source，更严优先）
