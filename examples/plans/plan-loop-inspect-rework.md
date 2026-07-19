# 计划驱动执行闭环 · 示例说明（P-loop / P2-11）

> 本文件是 **cco 启发式/规范体** 与桌面「回补并再巡检」的**说明样例**，不是可执行 YAML。  
> 真源：[`docs/plan-execute-inspect-rework-2026-07-19.md`](../../docs/plan-execute-inspect-rework-2026-07-19.md)  
> 混跑 inspect 形状：[`mixed-claude-codex-inspect.cco.yaml`](./mixed-claude-codex-inspect.cco.yaml)

## 0. 一句话

**先有可勾选的计划，再拆成可派工的工作包；落地必须对勾选负责；专门巡检对照计划查完成与遗漏；有阻塞遗漏就必须回补波。**

## 5. 实施勾选（示例真源）

| ID | 项 | 状态 |
|----|----|------|
| **L0** | 工作包含 plan_ref；巡检有对照表 + severity | 示例 |
| **L1** | require_inspect；blocking 则非成功终态；可生成 rework | 示例 |
| **L2** | 桌面显示巡检结果；一键回补；接受残留 | 示例 |

## 成功标准（示例）

| # | 指标 |
|---|------|
| **S1** | 每个必做勾选在 work-breakdown 有 plan_ref |
| **S2** | VERDICT 含勾选对照表 |
| **S3** | blocking ISSUE 存在时不得宣称为计划闭环成功 |
| **S4** | rework 后二次 inspect 能清掉对应 ISSUE |

## 非目标

- 不替代人写计划
- 无限自动重试（默认最多 2 轮回补）
- inspect 默认可大改业务

## 规范体四波（host 模板形状）

分配本类 Markdown 时，启发式 `work_order_template_from_spec` 产出：

1. **读懂目标与范围** → `.cco-out/scope/SUMMARY.md`（对齐 § 勾选）
2. **拆出可执行工作包** → `.cco-out/work-breakdown/SUMMARY.md`（**每 WP 必有 plan_ref**）
3. **按工作包落地** → `.cco-out/progress/SUMMARY.md`（`勾选 ID → 证据`）
4. **专门巡检对照计划** → `role=inspect` · `require_inspect=true` · VERDICT + 分级 ISSUES

## 巡检 ISSUES 字段（强制）

```text
- id: I-1
  severity=blocking|map|residual|out-of-scope
  plan_ref: L0 / S1 / …
  path: 文件或 n/a
  symptom: …
  fix_wp: 一句话可派工回补
```

**禁止**在存在 blocking/map 时写 `Result: PASS`。  
map（L1/L2 不同构）默认 **blocking**；回补波改文档，inspect 本波只开单。

## 回补与桌面

- host：`services::start_rework_from_run` → 新 run（implement + reinspect）
- 桌面：完成条显示「巡检 PASS/FAIL · 阻塞 N」；按钮「回补并再巡检」「接受残留」
- 最大回补轮次：2；超限 pause + 人工

## 推荐命令

```bash
# 将任意「产品方案 MD」走 Mode B 拆分（规范体四波 + require_inspect）
cco plan --project <repo> --plan examples/plans/plan-loop-inspect-rework.md
# 确认后执行；若 inspect FAIL：
# 桌面点「回补并再巡检」或 API start_rework_cmd

# 结构化混跑 + inspect 终闸（YAML）
cco parse --project <repo> --plan examples/plans/mixed-claude-codex-inspect.cco.yaml
```

## 与 multi-cli 边界

本示例**扩展** inspect 清单语义与 rework；**不**另开 Scheduler；**不**合并 multi-cli 阶段勾选。

[PROTOCOL]: 说明样例；变更时检查 examples/CLAUDE.md
