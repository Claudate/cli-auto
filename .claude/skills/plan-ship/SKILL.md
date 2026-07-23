---
name: plan-ship
description: >
  Architect planning in cco monorepo chat. Stack frozen for desktop (Rust/Tauri/web).
  Thick rules live in docs/runtime-prompts. Pairs with ship-ui.
---

# plan-ship（cco）

## 真源

- 厚：`docs/runtime-prompts/chat-plan-writing.md`（受众、栈表、计划结构、资产门禁）  
- 营销结构：`examples/marketing-landing-reference/SPEC.md`  
- 门禁：`docs/runtime-prompts/landing-gates.md`  
- 产品：`PRODUCT.md`；架构：`docs/architecture-redesign-2026-07-20.md`

## 栈冻结（cco 本体）

Rust CLI + Tauri 2 + `web/`；不重写为 Next。UI 增强走 `ship-ui`。

## 出海个人站

独立静态/Astro 小项目或子目录；**不要**塞进拆分台当官网生成器（除非单独立项）。  
计划须含建议技术 + 真实资产或「仅演示」。

## 聊天产出（短）

```markdown
## 目标（人话）
## In / Out
## 建议技术
## 主路径 / 挂载相位
## 成功标准（含门禁/走查）
## W0 第一刀
```

然后实现或交拆分；不写空架构长文。
