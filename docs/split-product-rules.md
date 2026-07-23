# 拆分产品规则（短真源）

> 日期：2026-07-22  
> 角色：**改拆分 / 拆分台 / confirm 行为时的短规则**（非阶段勾选表）  
> 存储与字段真源：[`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)  
> 全文与波次史：见文末 archive 指针

[PROTOCOL]: 本文**无** P0–P5 / 平行总落地勾选。残余债 **S2–S6 只认** 存储文文末。改行为先读本文 + 存储文；细节与金样回 archive。

---

## 1. 定位

| 读这个 | 不读这个当开项 |
|--------|----------------|
| 本文（行为规则） | archive 内带波次勾选的落地全文 |
| `cco-split-format-sqlite`（SoT 字段 / C1–C7 / S2–S6） | soft-sqlite 过渡文（已归档，非开项真源） |

一句话路径：

```text
计划 md → ModelSplitAgent（cco-split/v1）→ soft_accept → SQLite SoT
  → 拆分台（人话 + 可改）→ confirm_start → PlanIR → Worker
```

拆分 Agent **只拆不写业务代码**；执行 Worker **不负责拆分**。Planner ≠ Code。

---

## 2. 默认 `plan_mode=ai` 与 cco-split/v1

| 规则 | 说明 |
|------|------|
| **桌面 / CLI 默认 = `ai`** | ModelSplitAgent → 结构化 `cco-split/v1` |
| **`fast` = 高级 / 显式** | 本地 heuristic；文案禁止「推荐」 |
| 输出 schema | `cco-split/v1`：id · title · summary · body · depends_on · wave · enabled · optional · done_when · plan_ref · kind · scope_paths… |
| 主路径依赖字段 | title / body / depends / wave / enabled / optional；高级（provider/role/scope）可空不挡展示 |
| 禁拆成任务 | 非目标、PROTOCOL、修订历史、纯目录/索引、空话 |

`depends_on` **只连真先后**；禁止为凑波次串线。并行单位 = **文件/模块所有权**，不是「波次数字」。

---

## 3. soft_accept · 重叠串行

| 层 | 规则 | 失败时 |
|----|------|--------|
| **soft_accept（拆分）** | 有任务、id 不空、无环、title/body 可读 | 剪边 / 补默认；**不整图丢弃** |
| **scope 重叠** | 并行 implement 写同一路径 → 按序加 `depends_on` **串行** | 警告或自动串行，默认不否决整图 |
| **run_gate（开跑）** | ≥1 enabled；依赖指向存在 | 拦开跑，人话提示 |
| **完整才展示** | 智能拆分只有完整成功才写 `planned`/上桌；失败不展示残图、不覆盖上次成功 | `plan_failed`；有上次成功则恢复之；本地规则须用户显式选 `fast` |

严格 collab（多 provider / worktree）可在高级路径收紧；**规划 accept 默认 soften**。

---

## 4. 双受众 · 禁工人腔

同一拆分台伺候两种人（**不**做第三套「小白模式」开关）：

| 甲 · 只想办完 | 乙 · 懂一点的 PM |
|---------------|------------------|
| 短人话标题；一句话要做成什么 | 默认层与甲相同 |
| 怎样算做完；要先等谁（步骤名） | 展开可见改哪些模块 / 验收原文 |
| 顶栏：共几步 · 能否一起干 · 确认并开始 | 仍不需要读 `run.json` |

**禁止**首屏第一句：`你是执行任务 t… 的 worker` / `VERDICT` / adapter 名 / `force_serial` / `layers=N`。  
完整说明默认折叠（可展开）；高级通道默认折。首屏概念 ≤3：**步骤 · 等待 · 怎样算做完**。

---

## 5. scope + body 工单字段

每条任务应是可派给一个窗口的**工单**，不是标题刮表：

```text
【做什么】一句话结果
【改哪里】scope 文件/目录（互不抢）
【怎样算做完】可观察标准
【先等谁】无则写「无；可与 … 并行」
【不要做什么】硬契约 / 非目标
【自测】2–4 条
```

- `scope_paths[]`：仓库相对路径；纯文案任务 `[]` 并在 body 标明无代码路径。  
- `done_when`：可观察完成标准（可与「怎样算做完」同义压缩）。  
- body **禁止**以工人脚手架开头。

---

## 6. 来源条 / work_style

| 项 | 规则 |
|----|------|
| **来源常显** | 「智能拆分」vs「本地规则拆分」第一行可辨；不得藏进折叠芯片让用户误以为假拆是真拆 |
| **work_style** | 可选、可跳过的粗细/并行/话术旋钮；**不得**把主受众默认改回 `fast` |
| 优先级 | 计划结构 > 项目 work_style > 用户 work_style > 产品默认（ai + 白话） |

防卡死靠超时 / 取消 / 僵尸收尸，**不用假拆当分母**。

---

## 7. confirm 唯一开跑

1. 业务开跑 **只经** `confirm_start` / `gateway.confirmStart`（按钮可叫「执行规划」）。  
2. **禁止** UI `start_run` 旁路 Mode B。  
3. optional 须可勾选；禁止静默跳过未勾选。  
4. confirm 时：SQLite 读 CcoSplit → `run_gate` → **一次性** materialize PlanIR → Scheduler/Worker。  
5. 拆分台编辑经回写 SoT；读优先 SoT。

---

## 8. 失败 fallback 须标明来源

| 情况 | 行为 |
|------|------|
| ModelSplitAgent 失败 | fallback legacy LLM PlanIR→from_plan_ir（soften）或 heuristic |
| 日志 / 拆分台 | **写清原因与来源**（智能 / 本地规则 / fallback）；禁止静默空壳四波当成功 |
| soft_accept 仍失败 | 人话错误；不假装 planned |

---

## 9. 壳层体验（改 UI 时）

- 拆分台主 CTA 优先两键：**重新规划 · 执行规划**（能力可进高级，不删 confirm）。  
- 顶栏少噪音（阶段条/工人腔第一句）；icon + hover 说明可取。  
- 点计划/chip 可回看拆分结果；历史不静默丢。  
- 项目移除 = 从列表移除，**不删磁盘文件夹**。

---

## 10. 文末指针

**存储真源（字段 · C1–C7 · 残余债 S2–S6）**

- [`cco-split-format-sqlite-2026-07-21.md`](./cco-split-format-sqlite-2026-07-21.md)

**archive 全文（只读史 / 金样 / 已收口波次）**

| 主题 | 路径 |
|------|------|
| soft + dual-write 过渡 | [`archive/split-soft-sqlite-2026-07-21.md`](./archive/split-soft-sqlite-2026-07-21.md) |
| ModelSplitAgent 落地 | [`archive/openhands-style-split-agent-landing-2026-07-21.md`](./archive/openhands-style-split-agent-landing-2026-07-21.md) |
| 双受众拆分台 | [`archive/split-desk-dual-audience-landing-2026-07-22.md`](./archive/split-desk-dual-audience-landing-2026-07-22.md) |
| 多窗口 / scope·body 质量 | [`archive/multi-window-split-landing-2026-07-22.md`](./archive/multi-window-split-landing-2026-07-22.md) |
| 质量决策 · work_style | [`archive/split-quality-work-style-2026-07-22.md`](./archive/split-quality-work-style-2026-07-22.md) |
| 壳层减法 | [`archive/shell-chrome-simplify-2026-07-22.md`](./archive/shell-chrome-simplify-2026-07-22.md) |

S2–S6 中间摘录（非勾选真源）：`.cco-out/docs-cleanup/S2-S6-EXTRACT.md`。
