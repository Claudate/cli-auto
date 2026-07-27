# 费用感知 CLI 优选（Cost-aware Worker Router）

> **状态：P0–P3 ✅ · 本文件 = 本能力勾选真源**  
> 日期：2026-07-27  
> 范围：task 级 `provider` 自动优选（**不**做 HTTP model gateway）  
> 非目标：LiteLLM/Portkey 整站代理 · 静默覆盖显式 route · 训练 ML 分类器 / 外部 intent SaaS（P3 仅为本地启发式）  
> 关联：[`multi-cli-collaboration-2026-07-18.md`](./multi-cli-collaboration-2026-07-18.md) · [`architecture-redesign-2026-07-20.md`](./architecture-redesign-2026-07-20.md) · soft-fill 硬规则 L1 #13

[PROTOCOL]: 变更时更新此头部与 `src/domain/worker` L2；**禁止**平行第二套阶段表

---

## 0. 一句话

**默认自动选够用的便宜 CLI；难活和终检用强的；失败再升档；你写死的引擎不动；每笔选择能说人话。**

```text
role/tags → tier → 池内最便宜可用 → soft 填 default 任务
                ↘ 失败/卡死 → 升一档（P1）→ 仍失败走 H4 failover_order
```

---

## 1. 阶段勾选

| 阶段 | 内容 | 状态 |
|------|------|------|
| **P0** | 静态 tier 目录 + role→tier + 可用∩未熔断∩单价最低；`route_source=cost_auto`；人话 rationale | ✅ |
| **P1** | cheap/mid 失败 → 升一档重试（`cost_escalate`）；Verify=既有 acceptance/inspect | ✅ |
| **P2** | run 预算阈值降档（≥70% mid / ≥90% cheap）· 同 group/同档 wave 粘滞 · `route_source=cost_budget` | ✅ |
| **P3** | 启发式 intent（title/prompt/tags）· **默认关** · Inspect/Integrate 不降 · 无外部代理 | ✅ |

---

## 2. 硬契约

1. **只改 still-default**（空 / `default` / 等于 `plan.default_provider`）。显式非 default、tag 已改写的 **不**动。  
2. Force / `--provider` 覆盖之后 **不再** cost 改写（CLI last-write 优先）。  
3. `fake` / `sdk` **永不**自动优选或升档目标。  
4. 策略纯函数在 `domain/worker`；IO（registry / preflight）在 app/runtime。  
5. UI **不**复制策略；只渲染 `route_label` / summary 人话。

---

## 3. Tier 表（P0 默认 · 可后续配置化）

| Tier | 相对费用 | 默认池（低→高） |
|------|----------|-----------------|
| `cheap` | 低 | deepseek → qwen → gemini → kimi → codebuddy → copilot |
| `mid` | 中 | codex |
| `flagship` | 高 | claude |

| role | 默认 tier | 说明 |
|------|-----------|------|
| `scout` | cheap | 只读探路 |
| `implement` / 空 | mid | 主实现；无 role 当 implement |
| `closeout` | mid | 文档关账，不必旗舰 |
| `integrate` | flagship | 整合定一家 |
| `inspect` | flagship | 终闸定一家 |

池内无人可用 → **向上借一档**（availability escalate，仍标 `cost_auto`）。  
再无人 → 保留 soft-fill 结果，不报错。

---

## 4. 配置

```toml
# ~/.cco/config.toml [default]
cost_route_enabled = true          # P0 总开关；false = 行为与改前一致
cost_escalate_enabled = true       # P1；失败后升档
# cost_intent_enabled = false      # P3 默认关；开则 title/prompt/tags 微调档
# run_max_budget_usd = 25.0        # P2 预算阈值（有 cap 才中途降档）
```

### P3 意图规则（启发式 · 可解释）

| 信号 | 效果（role 非 inspect/integrate） |
|------|-----------------------------------|
| tags `hard`/`arch`/`critical` 或文案「架构/跨模块/…」 | 升一档（mid→flagship） |
| tags `simple`/`docs`/`typo` 或「错别字/格式化/…」 | 降一档（mid→cheap） |
| 极短 prompt（&lt;48 非空白字） | 偏简 |
| Inspect / Integrate | **永不**因意图降档 |

---

## 5. 溯源 wire（run.json）

| `route_source` | 含义 | 人话 `route_label` 形状 |
|----------------|------|-------------------------|
| `cost_auto` | P0 费用优选 | `{产品} · 费用优选` |
| `cost_escalate` | P1 失败升档 | `{产品} · 失败后升档，先前 {prev}` |
| `cost_budget` | P2 预算收紧 | `{产品} · 预算收紧，先前 {prev}` |
| 既有 | explicit / soft_fill / tag_routing / force / failover | 不变 |

### P2 预算阈值（相对 `run_max_budget_usd`）

| spend / cap | 新开 auto 任务最高档 |
|-------------|---------------------|
| &lt; 0.70 | 不限（role 默认） |
| ≥ 0.70 | Mid |
| ≥ 0.90 | Cheap |

- 无 `run_max_budget_usd` → 不夹紧、不中途降档。  
- 中途降档只动 `soft_fill` / `cost_auto`（及已 `cost_budget` 可再收）；explicit / tag / force / escalate **不动**。  
- 粘滞：同 `group` 可跟显式；无 group 时同 wave **仅**跟本 pass 的 auto 选、且 **同 tier 带**。

---

## 6. 代码落点

| 层 | 位置 |
|----|------|
| Domain | `cost_route` 目录/选型 · `cost_apply` 编排 · `cost_budget` P2 · `cost_intent` P3 |
| App | `materialize` 应用优选 · `provenance` stamp/label |
| Runtime | `patrol` 升档 · `tick.maybe_budget_downgrade_task` 开跑前降档 |
| Config | `cost_route_enabled` / `cost_escalate_enabled` / **`cost_intent_enabled`** · `run_max_budget_usd` |
| Settings UI | 卡住与重试组：三开关 + `cost_route_note`（经 `SettingsView`） |
| CLI 开跑 | `confirm_materialize` / materialize 第四返回值 → 打印费用摘要一行 |
| Runtime | `provider_unhealthy`：preflight 失败 / start 失败 → 本 run 升档与预算降档跳过 |

---

## 7. 借鉴（开源，不整仓依赖）

RouteLLM 阈值分流 · NadirClaw Route→Verify→Escalate · BitRouter role/step 表 · UncommonRoute 三档 · RelayPlane 预算降档（P2）。
