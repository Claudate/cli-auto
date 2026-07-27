# session-digest 工作流（读 / 写）

> **不注入**产品 LLM。  
> 契约：[`../contracts/session-digest.md`](../contracts/session-digest.md)  
> 抽取：[`session-digest-extract.md`](./session-digest-extract.md)  
> 勾选：[`../context-digest-compress-landing-2026-07-27.md`](../context-digest-compress-landing-2026-07-27.md) C1

---

## 何时写（压缩）

- 波次 / 计划阶段结束  
- 上下文将满或多工具长会话  
- 用户说：「压缩上下文」「写 digest」「收束状态」

步骤：

1. 收集目标、决策（含否决）、约束、禁止、未决、产物路径。  
2. 用 `session-digest-extract` 只出 YAML。  
3. 按契约 §4 合格判定；失败则重抽。  
4. 写入 `.cco-out/session-digest.yaml`（已 gitignore）。  
5. 可选三行 `arc.md`（lossy，不承载硬约束）。  
6. `dont[]` 只增；废止用 `superseded_by`。

## 何时读（续作）

- 新开会话同题 / 用户说「按 digest 续」  
- 长中断后恢复

步骤：

1. 若存在 `.cco-out/session-digest.yaml`（或用户指定路径）→ **先读**。  
2. 行动前核 `dont` 与 `constraints`。  
3. 需要细节再 `Read` `source` / `artifacts`。  
4. 新决策追加（含 rejected）；新禁止追加 dont。  
5. 波次结束回到「何时写」。

## 口令

| 用户说 | 做 |
|--------|-----|
| 压缩上下文 / 写 digest | 写 |
| 按 digest 续 | 读 |
| 升格为记忆 | 仅显式把稳定铁律写成 MEMORY 原子条；不整文件塞索引 |

## 禁止

- 用 digest 调用 confirm / 开跑  
- 自由散文冒充 digest  
- 与 plan `digest.rs` 模式字段混写
