# cco 系统收尾任务：巡检 · 代码提交 Push

> 状态：**已落地**（配置总开关默认关 · 拆分后注入可选尾任务 · 确认屏默认勾选 · 设置页可配）  
> 日期：2026-07-19 · **S-PR t62 扩 open-pr**  
> 范围：Mode B 拆分结果**之后**由 host 追加的系统任务；**不**参与 AI/启发式拆解；后续可同表扩展  
> 角色：可开关的「拆完再干什么」扩展点——与业务任务图解耦  
> 关联：Mode B · P-loop inspect · 可选任务 `optional/include` · 总账 **D5 / P2-15 · P-sys-post** · **P2-7 自动开 PR（S-PR）**  
> GEB：[`/CLAUDE.md`](../../CLAUDE.md) · [`../CLAUDE.md`](../CLAUDE.md)

[PROTOCOL]: 变更时更新此头部；落地后检查 L1/L2

---

## 0. 一句话

**设置里默认关闭的两个总开关；打开后，每次拆分在业务任务末尾自动挂上「任务巡检 / 代码提交 Push」——仍是可选任务，但默认勾选，人不参与拆解。**

```text
设置 post_inspect / post_git_push（默认 false）
        ↓ 拆分 finish_plan_job
inject_system_post_tasks → proposed.json
        ↓ 确认屏
可选 · 默认勾选 · 徽章「系统」· 可取消
        ↓ confirm_start
materialize 丢掉未勾选 → 真正开跑
```

---

## 1. 产品规则

| 项 | 规则 |
|----|------|
| 谁生成 | **系统**固定 id/文案/依赖；Planner **不**产出 |
| 总开关 | `post_inspect_enabled` · `post_git_push_enabled` · **`post_open_pr_enabled`**；**默认 false** |
| 开启后 | 每次 `finish_plan_job` 注入；`optional=true` 且 **`include=true`**（默认勾选） |
| 关闭后 | 不注入；若图上有旧 sys-post-* 会剥离 |
| 确认屏 | 与普通可选同一勾选 API；徽章「系统」区分 |
| 依赖 | 巡检 ← 全部业务；Push ← **仅**巡检（若有）；**开 PR ← Push**（开 PR 时强制附带 inspect+push） |
| 提交门禁 | **先巡检通过再提交**（双层）：① 依赖边 Push←inspect；② **host** `system_push_inspect_gate` 在 spawn 前读 VERDICT，非 PASS / 有 blocking ISSUES → 任务 **Skipped** 不 spawn CLI；只开 Push/PR 也会自动附带巡检 |
| 开 PR | 本机 `gh pr create`；需已登录；**禁止** force-push / 自动 merge；主路径第一屏**不**展示（仅设置·系统收尾折叠） |
| auto-start | **业务可选**仍强制停确认屏；**仅系统收尾且全部已勾选**时可 auto-start |
| 扩展 | `system_post.rs` 增 FEATURE 即可（同模式） |

### 固定 id

| id | 标题 | role |
|----|------|------|
| `sys-post-inspect` | 任务巡检（系统）（可选） | inspect |
| `sys-post-git-push` | 代码提交 Push（系统）（可选） | integrate |
| `sys-post-open-pr` | 自动开 PR（系统）（可选） | integrate |

---

## 2. 配置

`~/.cco/config.toml` `[default]`：

```toml
post_inspect_enabled = false
post_git_push_enabled = false
post_open_pr_enabled = false
```

设置页「系统收尾（拆分后）」勾选 → `SettingsView` / `SettingsUpdate`（含 **自动开 PR**）。

---

## 3. 实现锚点

| 层 | 文件 |
|----|------|
| 注入 | [`src/plan/system_post.rs`](../../src/plan/system_post.rs) |
| 调用点 | [`src/plan/planner/job.rs`](../../src/plan/planner/job.rs) `finish_plan_job` |
| 配置 | [`src/config/mod.rs`](../../src/config/mod.rs) |
| 设置 API | [`src/services/settings.rs`](../../src/services/settings.rs) |
| 设置 UI | [`web/index.html`](../../web/index.html) · [`web/js/doctor.js`](../../web/js/doctor.js) |
| 确认屏徽章 | [`web/js/plan.js`](../../web/js/plan.js) · [`web/css/plan.css`](../../web/css/plan.css) |
| 任务上限 | `PLANNER_MAX_TASKS=20` · `MAX_TASKS=23`（为最多 3 个系统位留空：inspect/push/open-pr） |

---

## 4. 非目标

- 不改 Scheduler / confirm_start 契约  
- 不强制每次巡检（总开关关则完全无）  
- **不做** SDK 侧开 PR / Windows 专用 PR 路径 / 多 remote / force-push / 自动 merge  
- 不把系统任务交给 LLM 重新命名  
- 不进主路径第一屏（设置·系统收尾折叠）

---

## 5. 成功标准

| ID | 标准 |
|----|------|
| S1 | 默认关：拆分结果无 sys-post-* |
| S2 | 仅开巡检：末尾一项 inspect，默认勾选，依赖业务 |
| S3 | 两项都开：inspect → push 串行依赖 |
| S4 | 确认取消勾选后 confirm_start 不 spawn 该任务 |
| S5 | 设置页可改并可持久化 |
| S6 | unit：`plan::system_post` 测绿 |
| S7 | **S-PR**：仅开 open-pr → 自动附带 inspect+push；链 inspect→push→pr；默认关 |

---

## 6. 修订历史

| 时点 | 内容 |
|------|------|
| **t1 · 2026-07-19** | 用户需求：拆分底部可增巡检+push；设置默认关；开启后默认勾选；系统自带可扩展。落地 E 全量 + 测 + 文档 |
| **t2 · 2026-07-19** | auto-start：业务可选仍拦；系统收尾默认勾选不挡；确认屏 meta 区分业务/系统；`start_plan_job` 集成测 |
| **t3 · 2026-07-19** | 用户：「先巡检通过后才提交」— Push prompt 门禁 VERDICT=PASS；validate 允许 inspect→sys-post 后继 |
| **t4 · 2026-07-20** | host 硬门禁 `system_push_inspect_gate`：scheduler spawn 前 skip push；unit 覆盖 PASS/FAIL/unknown |
| **t5 · 2026-07-21 · S-PR / t62** | `post_open_pr_enabled` + `sys-post-open-pr`；开 PR 强制 inspect+push；`gh pr create` 提示词 + 安全硬规则；设置页第三勾选；MAX_TASKS=23；**不**做 SDK/Windows/merge |

[PROTOCOL]: 变更时更新此头部；落地后检查 L1/L2
