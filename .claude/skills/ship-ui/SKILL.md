---
name: ship-ui
description: >
  Ship UI for cco desktop web shell (web/). Implements against web/CLAUDE.md and
  docs/runtime-prompts. No methodology dumps. Use when changing desks, chat/split/run/result chrome, CSS.
---

# ship-ui（cco 桌面）

## 真源

- 产品：`PRODUCT.md`
- 前端：`web/CLAUDE.md`
- **交付/营销纪律**：`docs/runtime-prompts/*.md`
- **落地页结构**：`examples/marketing-landing-reference/SPEC.md`
- **门禁**：`./scripts/check-landing-gates.sh web`（或站点根）

## 硬约束

1. MVVM；IPC 只经 `web/js/shared/gateway.js`  
2. phase：`author | split | run | result`  
3. 开跑只经 Split `confirmStart`  
4. 新功能 `web/js/features/*`；不堆 facade/`state.js`  
5. 人话；图标 `shared/icons.js`（Lucide）  
6. 体积软 400 / 硬 600  

## 交付

- 主 CTA 唯一；空错载  
- 改完说明点击路径  
- 若动营销类静态页：跑 landing-gates  

全局姿态见用户 skill `ship-ui`（薄）；**厚规则只改 docs/runtime-prompts**。
